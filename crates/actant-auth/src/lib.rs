//! actant-auth — authentication and session minting.
//!
//! Phase 6 surface. Three things:
//!
//! 1. **Verify** an inbound JWT signed by a configured HS256 secret (used by
//!    service accounts and dev sessions). OIDC providers are Phase 6.5.
//! 2. **Mint** an opaque session token that maps back to an actor via the
//!    `session_token` table (created here, not in the canonical schema —
//!    Phase 6 add).
//! 3. **Resolve** the (actor, workspace, roles) tuple for an authenticated
//!    request.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod oidc;
pub use oidc::{DiscoveryDoc, HttpFetcher, Jwk, JwkSet, OidcResolver};

use actant_core::{ActantError, ActorId, WorkspaceId};
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// One verified principal.
#[derive(Debug, Clone)]
pub struct Principal {
    /// Workspace this principal authenticates against.
    pub workspace_id: WorkspaceId,
    /// Actor this principal acts as.
    pub actor_id: ActorId,
    /// Roles granted (often a single string like "admin").
    pub roles: Vec<String>,
    /// Expiry in unix seconds.
    pub expires_at: i64,
}

/// JWT claims we accept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (the actor id).
    pub sub: String,
    /// Issuer (workspace id).
    pub iss: String,
    /// Roles.
    #[serde(default)]
    pub roles: Vec<String>,
    /// Issued-at.
    pub iat: i64,
    /// Expires-at.
    pub exp: i64,
}

/// Sign + verify HS256 JWTs. Phase 6 only — production OIDC providers
/// arrive in 6.5 (Auth0 / Clerk / WorkOS).
pub fn sign(claims: &Claims, secret: &[u8]) -> Result<String, ActantError> {
    let header = serde_json::json!({"alg":"HS256","typ":"JWT"}).to_string();
    let header_b64 = b64(&header);
    let claims_b64 = b64(&serde_json::to_string(claims)?);
    let signing_input = format!("{header_b64}.{claims_b64}");
    let mut mac =
        HmacSha256::new_from_slice(secret).map_err(|e| ActantError::Internal(e.to_string()))?;
    mac.update(signing_input.as_bytes());
    let sig_bytes = mac.finalize().into_bytes();
    let sig_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig_bytes);
    Ok(format!("{signing_input}.{sig_b64}"))
}

/// Verify a signed JWT and parse its claims.
pub fn verify(token: &str, secret: &[u8]) -> Result<Claims, ActantError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(ActantError::InvalidInput("malformed jwt".into()));
    }
    let signing_input = format!("{}.{}", parts[0], parts[1]);
    let sig = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|e| ActantError::InvalidInput(e.to_string()))?;
    let mut mac =
        HmacSha256::new_from_slice(secret).map_err(|e| ActantError::Internal(e.to_string()))?;
    mac.update(signing_input.as_bytes());
    mac.verify_slice(&sig)
        .map_err(|_| ActantError::PermissionDenied("invalid signature".into()))?;
    let claims_json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| ActantError::InvalidInput(e.to_string()))?;
    let claims: Claims = serde_json::from_slice(&claims_json)?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    if now > claims.exp {
        return Err(ActantError::PermissionDenied("token expired".into()));
    }
    Ok(claims)
}

/// Turn verified claims into a Principal.
pub fn principal_from_claims(c: &Claims) -> Principal {
    Principal {
        workspace_id: WorkspaceId::from_string(c.iss.clone()),
        actor_id: ActorId::from_string(c.sub.clone()),
        roles: c.roles.clone(),
        expires_at: c.exp,
    }
}

fn b64(s: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(s.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> i64 {
        time::OffsetDateTime::now_utc().unix_timestamp()
    }

    #[test]
    fn round_trip_signing() {
        let claims = Claims {
            sub: "act_alice".into(),
            iss: "ws_team_a".into(),
            roles: vec!["admin".into()],
            iat: now(),
            exp: now() + 3600,
        };
        let token = sign(&claims, b"shared-secret").unwrap();
        let parsed = verify(&token, b"shared-secret").unwrap();
        assert_eq!(parsed.sub, "act_alice");
        assert_eq!(parsed.roles, vec!["admin".to_string()]);
    }

    #[test]
    fn wrong_secret_rejected() {
        let claims = Claims {
            sub: "x".into(),
            iss: "y".into(),
            roles: vec![],
            iat: now(),
            exp: now() + 60,
        };
        let token = sign(&claims, b"right").unwrap();
        assert!(verify(&token, b"wrong").is_err());
    }

    #[test]
    fn expired_rejected() {
        let claims = Claims {
            sub: "x".into(),
            iss: "y".into(),
            roles: vec![],
            iat: now() - 100,
            exp: now() - 50,
        };
        let token = sign(&claims, b"k").unwrap();
        assert!(verify(&token, b"k").is_err());
    }

    #[test]
    fn principal_extracts_workspace_and_actor() {
        let c = Claims {
            sub: "act_z".into(),
            iss: "ws_x".into(),
            roles: vec!["viewer".into()],
            iat: 0,
            exp: 0,
        };
        let p = principal_from_claims(&c);
        assert_eq!(p.actor_id.as_str(), "act_z");
        assert_eq!(p.workspace_id.as_str(), "ws_x");
        assert_eq!(p.roles, vec!["viewer".to_string()]);
    }
}
