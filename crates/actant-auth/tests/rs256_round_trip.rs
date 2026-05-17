//! Real RS256 round trip: generate an RSA keypair, sign a JWT with
//! `jsonwebtoken`, expose the public key as a JWK over a stub fetcher,
//! verify through `actant_auth::oidc::verify_rs256`.

use std::collections::HashMap;

use actant_auth::oidc::{self, HttpFetcher, OidcResolver};
use actant_auth::Claims;
use actant_core::ActantError;
use async_trait::async_trait;
use base64::Engine;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};

struct StubFetcher {
    map: HashMap<String, String>,
}

#[async_trait]
impl HttpFetcher for StubFetcher {
    async fn get(&self, url: &str) -> Result<String, ActantError> {
        self.map
            .get(url)
            .cloned()
            .ok_or_else(|| ActantError::NotFound(format!("no stub for {url}")))
    }
}

fn b64u(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn jwk_from_public(pk: &RsaPublicKey, kid: &str) -> serde_json::Value {
    serde_json::json!({
        "kid": kid,
        "alg": "RS256",
        "kty": "RSA",
        "n": b64u(&pk.n().to_bytes_be()),
        "e": b64u(&pk.e().to_bytes_be()),
    })
}

#[tokio::test]
async fn rs256_real_round_trip() {
    // 1. Generate an RSA keypair.
    let mut rng = rand::thread_rng();
    let private = RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let public = RsaPublicKey::from(&private);

    // 2. Sign a JWT with `kid=k1`.
    let pem = private
        .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
        .unwrap()
        .to_string();
    let encoding_key = EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap();
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("k1".to_string());
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let claims = Claims {
        sub: "act_alice".into(),
        iss: "https://issuer.example.com".into(),
        roles: vec!["admin".into()],
        iat: now,
        exp: now + 600,
    };
    let token = jsonwebtoken::encode(&header, &claims, &encoding_key).unwrap();

    // 3. Stub the OIDC discovery + JWKS responses.
    let jwks = serde_json::json!({"keys": [jwk_from_public(&public, "k1")]});
    let discovery = serde_json::json!({
        "issuer": "https://issuer.example.com",
        "jwks_uri": "https://issuer.example.com/.well-known/jwks.json"
    });
    let mut map = HashMap::new();
    map.insert(
        "https://issuer.example.com/.well-known/openid-configuration".into(),
        discovery.to_string(),
    );
    map.insert(
        "https://issuer.example.com/.well-known/jwks.json".into(),
        jwks.to_string(),
    );
    let fetcher = StubFetcher { map };

    // 4. Verify via the actant-auth path.
    let resolver = OidcResolver::new();
    let parsed = oidc::verify_rs256(&resolver, "https://issuer.example.com", &token, &fetcher)
        .await
        .unwrap();
    assert_eq!(parsed.sub, "act_alice");
    assert_eq!(parsed.iss, "https://issuer.example.com");
    assert_eq!(parsed.roles, vec!["admin".to_string()]);
}

#[tokio::test]
async fn rs256_rejects_tampered_token() {
    let mut rng = rand::thread_rng();
    let private = RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let public = RsaPublicKey::from(&private);
    let pem = private
        .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
        .unwrap()
        .to_string();
    let encoding_key = EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap();
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("k1".to_string());
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let claims = Claims {
        sub: "act_alice".into(),
        iss: "https://issuer.example.com".into(),
        roles: vec![],
        iat: now,
        exp: now + 600,
    };
    let token = jsonwebtoken::encode(&header, &claims, &encoding_key).unwrap();
    // Flip a byte in the signature.
    let mut tampered = token.clone();
    let last = tampered.len() - 1;
    let ch = tampered.as_bytes()[last];
    let replacement = if ch == b'a' { b'b' } else { b'a' };
    unsafe {
        tampered.as_bytes_mut()[last] = replacement;
    }

    let jwks = serde_json::json!({"keys": [jwk_from_public(&public, "k1")]});
    let mut map = HashMap::new();
    map.insert(
        "https://issuer.example.com/.well-known/openid-configuration".into(),
        serde_json::json!({
            "issuer": "https://issuer.example.com",
            "jwks_uri": "https://issuer.example.com/.well-known/jwks.json"
        })
        .to_string(),
    );
    map.insert(
        "https://issuer.example.com/.well-known/jwks.json".into(),
        jwks.to_string(),
    );
    let fetcher = StubFetcher { map };

    let resolver = OidcResolver::new();
    let res =
        oidc::verify_rs256(&resolver, "https://issuer.example.com", &tampered, &fetcher).await;
    assert!(matches!(res, Err(ActantError::PermissionDenied(_))));
}
