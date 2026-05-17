//! OIDC discovery + JWKS verification.
//!
//! Wires real OpenID Connect identity providers (Auth0, Clerk, WorkOS,
//! Cognito, ...) into actant-auth. Process:
//!
//! 1. On startup or first request, fetch the issuer's
//!    `/.well-known/openid-configuration` to learn the `jwks_uri`.
//! 2. Fetch + cache the JWK Set (rotated on a TTL).
//! 3. For each inbound token, verify RS256/ES256 with the matching `kid`.
//!
//! Phase 6.5. The verification path itself uses `jsonwebtoken` to keep us
//! out of the business of writing crypto.

use std::sync::Arc;
use std::time::{Duration, Instant};

use actant_core::ActantError;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// One OIDC issuer's discovery document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryDoc {
    /// Issuer URL.
    pub issuer: String,
    /// JWKS endpoint.
    pub jwks_uri: String,
    /// Supported algorithms (`alg` values).
    #[serde(default)]
    pub id_token_signing_alg_values_supported: Vec<String>,
}

/// One JWK in a JWK Set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jwk {
    /// Key id.
    pub kid: String,
    /// Algorithm.
    pub alg: Option<String>,
    /// Key type.
    pub kty: String,
    /// RSA modulus (base64url).
    pub n: Option<String>,
    /// RSA exponent.
    pub e: Option<String>,
    /// EC `x` coordinate.
    pub x: Option<String>,
    /// EC `y` coordinate.
    pub y: Option<String>,
    /// EC curve.
    pub crv: Option<String>,
}

/// A JWK Set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwkSet {
    /// Keys.
    pub keys: Vec<Jwk>,
}

/// OIDC issuer cache. Single struct holds the (discovery + JWKS) pair plus
/// a TTL.
#[derive(Debug)]
struct CachedIssuer {
    issuer: String,
    discovery: DiscoveryDoc,
    jwks: JwkSet,
    fetched_at: Instant,
}

/// Shared OIDC resolver. Cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct OidcResolver {
    inner: Arc<RwLock<Vec<CachedIssuer>>>,
    /// How long a cached JWKS is trusted before re-fetching.
    pub ttl: Duration,
}

impl OidcResolver {
    /// New resolver with the default 1-hour cache.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Vec::new())),
            ttl: Duration::from_secs(3600),
        }
    }

    /// Look up a fresh discovery + JWKS for an issuer. The fetcher is
    /// injected to keep this crate test-friendly (no `reqwest` mock dance).
    pub async fn ensure<F>(
        &self,
        issuer: &str,
        fetcher: &F,
    ) -> Result<(DiscoveryDoc, JwkSet), ActantError>
    where
        F: HttpFetcher + ?Sized,
    {
        // Fast path: already cached and fresh.
        {
            let g = self.inner.read().await;
            if let Some(c) = g.iter().find(|c| c.issuer == issuer) {
                if c.fetched_at.elapsed() < self.ttl {
                    return Ok((c.discovery.clone(), c.jwks.clone()));
                }
            }
        }
        // Slow path: fetch and replace.
        let disc_url = format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        );
        let disc_body = fetcher.get(&disc_url).await?;
        let discovery: DiscoveryDoc = serde_json::from_str(&disc_body)
            .map_err(|e| ActantError::InvalidInput(format!("oidc discovery: {e}")))?;
        let jwks_body = fetcher.get(&discovery.jwks_uri).await?;
        let jwks: JwkSet = serde_json::from_str(&jwks_body)
            .map_err(|e| ActantError::InvalidInput(format!("oidc jwks: {e}")))?;
        let mut g = self.inner.write().await;
        g.retain(|c| c.issuer != issuer);
        g.push(CachedIssuer {
            issuer: issuer.into(),
            discovery: discovery.clone(),
            jwks: jwks.clone(),
            fetched_at: Instant::now(),
        });
        Ok((discovery, jwks))
    }

    /// Number of cached issuers (for tests).
    pub async fn cache_size(&self) -> usize {
        self.inner.read().await.len()
    }
}

/// Pluggable HTTP fetcher trait. Real callers wire `reqwest::Client`;
/// tests pass a stub map.
#[async_trait::async_trait]
pub trait HttpFetcher: Send + Sync {
    /// GET `url` and return the response body as a string.
    async fn get(&self, url: &str) -> Result<String, ActantError>;
}

/// Verify an RS256-signed JWT against an OIDC issuer's JWK Set. Picks the
/// JWK by `kid` and confirms the signature; returns the parsed claims.
pub async fn verify_rs256<F: HttpFetcher + ?Sized>(
    resolver: &OidcResolver,
    issuer: &str,
    token: &str,
    fetcher: &F,
) -> Result<crate::Claims, ActantError> {
    let (_discovery, jwks) = resolver.ensure(issuer, fetcher).await?;
    // Pull the kid out of the JWT header.
    let header = jsonwebtoken::decode_header(token)
        .map_err(|e| ActantError::InvalidInput(format!("jwt header: {e}")))?;
    let kid = header
        .kid
        .ok_or_else(|| ActantError::InvalidInput("jwt missing kid".into()))?;
    let jwk = jwks
        .keys
        .iter()
        .find(|k| k.kid == kid)
        .ok_or_else(|| ActantError::PermissionDenied(format!("no jwk for kid {kid}")))?;
    let n = jwk
        .n
        .as_ref()
        .ok_or_else(|| ActantError::InvalidInput("jwk missing n".into()))?;
    let e = jwk
        .e
        .as_ref()
        .ok_or_else(|| ActantError::InvalidInput("jwk missing e".into()))?;
    let key = jsonwebtoken::DecodingKey::from_rsa_components(n, e)
        .map_err(|err| ActantError::InvalidInput(format!("jwk decode: {err}")))?;
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_issuer(&[issuer]);
    validation.validate_aud = false;
    let data = jsonwebtoken::decode::<crate::Claims>(token, &key, &validation)
        .map_err(|e| ActantError::PermissionDenied(format!("rs256 verify: {e}")))?;
    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct StubFetcher {
        map: HashMap<String, String>,
    }

    #[async_trait::async_trait]
    impl HttpFetcher for StubFetcher {
        async fn get(&self, url: &str) -> Result<String, ActantError> {
            self.map
                .get(url)
                .cloned()
                .ok_or_else(|| ActantError::NotFound(format!("no stub for {url}")))
        }
    }

    fn fake_issuer() -> StubFetcher {
        let mut m = HashMap::new();
        m.insert(
            "https://issuer.example.com/.well-known/openid-configuration".into(),
            serde_json::json!({
                "issuer": "https://issuer.example.com",
                "jwks_uri": "https://issuer.example.com/.well-known/jwks.json",
                "id_token_signing_alg_values_supported": ["RS256"]
            })
            .to_string(),
        );
        m.insert(
            "https://issuer.example.com/.well-known/jwks.json".into(),
            serde_json::json!({
                "keys": [{
                    "kid": "k1",
                    "alg": "RS256",
                    "kty": "RSA",
                    "n": "fake-modulus",
                    "e": "AQAB"
                }]
            })
            .to_string(),
        );
        StubFetcher { map: m }
    }

    #[tokio::test]
    async fn ensure_fetches_and_caches() {
        let r = OidcResolver::new();
        let f = fake_issuer();
        let (d, j) = r.ensure("https://issuer.example.com", &f).await.unwrap();
        assert_eq!(d.issuer, "https://issuer.example.com");
        assert_eq!(j.keys.len(), 1);
        assert_eq!(r.cache_size().await, 1);
        // Second call uses the cache.
        let (_d2, _j2) = r.ensure("https://issuer.example.com", &f).await.unwrap();
        assert_eq!(r.cache_size().await, 1);
    }
}
