#![allow(dead_code)]

use anyhow::Result;
use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration as StdDuration, Instant},
};
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;

/// Representation of a user in the system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    pub id: u32,
    pub username: String,
    pub display_name: String,
    pub admin: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub must_change_password: bool,
}

/// Persistent authentication configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub passphrase_hash: String,
    pub jwt_secret: String,
    pub users: Vec<User>,
    pub created_at: i64,
}

impl AuthConfig {
    /// Get next user id.
    pub fn next_id(&self) -> u32 {
        self.users.iter().map(|u| u.id).max().unwrap_or(0) + 1
    }

    /// Add a user ensuring unique username (case-insensitive).
    pub fn add_user(&mut self, user: User) -> Result<()> {
        if self
            .users
            .iter()
            .any(|u| u.username.eq_ignore_ascii_case(&user.username))
        {
            anyhow::bail!("duplicate_user");
        }
        self.users.push(user);
        Ok(())
    }

    /// Check if username has admin role.
    pub fn is_admin(&self, username: &str) -> bool {
        self.users
            .iter()
            .any(|u| u.username.eq_ignore_ascii_case(username) && u.admin)
    }
}

/// Hash a passphrase using argon2id.
pub fn hash_passphrase(pass: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(pass.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!(e))?
        .to_string();
    Ok(hash)
}

/// Verify a passphrase against an encoded hash.
pub fn verify_passphrase(pass: &str, hash: &str) -> bool {
    if let Ok(parsed) = PasswordHash::new(hash) {
        Argon2::default()
            .verify_password(pass.as_bytes(), &parsed)
            .is_ok()
    } else {
        false
    }
}

/// Claims stored within issued JWTs.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

/// Issue a JWT for a given subject valid for the provided duration.
pub fn issue_jwt(secret: &[u8], sub: &str, valid_for: Duration) -> Result<String> {
    let exp = (OffsetDateTime::now_utc() + valid_for).unix_timestamp() as usize;
    let claims = Claims {
        sub: sub.into(),
        exp,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )?;
    Ok(token)
}

/// Verify a JWT and return its claims if valid.
pub fn verify_jwt(secret: &[u8], token: &str) -> Result<Claims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let data = decode::<Claims>(token, &DecodingKey::from_secret(secret), &validation)?;
    if data.claims.exp < OffsetDateTime::now_utc().unix_timestamp() as usize {
        anyhow::bail!("expired");
    }
    Ok(data.claims)
}

/// Determine if a token should be refreshed given a threshold duration.
pub fn needs_refresh(claims: &Claims, within: Duration) -> bool {
    let expire = OffsetDateTime::from_unix_timestamp(claims.exp as i64).unwrap();
    expire - OffsetDateTime::now_utc() < within
}

/// Simple in-memory login rate limiter.
#[derive(Clone)]
pub struct LoginRateLimiter {
    inner: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    max: usize,
    window: StdDuration,
}

impl LoginRateLimiter {
    pub fn new(max: usize, window: StdDuration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            max,
            window,
        }
    }

    /// Returns true if the attempt is allowed, false if rate limited.
    pub async fn check(&self, key: &str) -> bool {
        let mut guard = self.inner.lock().await;
        let now = Instant::now();
        let entry = guard.entry(key.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < self.window);
        if entry.len() >= self.max {
            return false;
        }
        entry.push(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;

    #[test]
    fn hash_and_verify() {
        let hash = hash_passphrase("secret").unwrap();
        assert!(verify_passphrase("secret", &hash));
        assert!(!verify_passphrase("bad", &hash));
    }

    #[test]
    fn jwt_issue_and_verify() {
        let secret = b"secret";
        let token = issue_jwt(secret, "user", Duration::seconds(60)).unwrap();
        let claims = verify_jwt(secret, &token).unwrap();
        assert_eq!(claims.sub, "user");
    }

    #[test]
    fn jwt_expiry() {
        let secret = b"secret";
        let token = issue_jwt(secret, "user", Duration::seconds(-10)).unwrap();
        // Validation should fail because exp is in the past
        let res = verify_jwt(secret, &token);
        assert!(res.is_err());
    }

    #[test]
    fn refresh_logic() {
        let now = OffsetDateTime::now_utc();
        let claims = Claims {
            sub: "a".into(),
            exp: (now + Duration::minutes(5)).unix_timestamp() as usize,
        };
        assert!(needs_refresh(&claims, Duration::hours(1)));
        assert!(!needs_refresh(&claims, Duration::minutes(1)));
    }

    #[tokio::test]
    async fn rate_limiter_blocks() {
        let limiter = LoginRateLimiter::new(2, StdDuration::from_secs(60));
        assert!(limiter.check("u").await);
        assert!(limiter.check("u").await);
        assert!(!limiter.check("u").await);
    }

    #[test]
    fn unique_username_case_insensitive() {
        let mut cfg = AuthConfig {
            passphrase_hash: String::new(),
            jwt_secret: String::new(),
            users: Vec::new(),
            created_at: 0,
        };
        cfg.add_user(User {
            id: 1,
            username: "Alice".into(),
            display_name: "Alice".into(),
            admin: false,
            disabled: false,
            avatar_url: None,
            must_change_password: false,
        })
        .unwrap();
        assert!(cfg
            .add_user(User {
                id: 2,
                username: "alice".into(),
                display_name: "Another".into(),
                admin: false,
                disabled: false,
                avatar_url: None,
                must_change_password: false,
            })
            .is_err());
    }

    #[test]
    fn admin_role_check() {
        let cfg = AuthConfig {
            passphrase_hash: String::new(),
            jwt_secret: String::new(),
            users: vec![User {
                id: 1,
                username: "admin".into(),
                display_name: "Admin".into(),
                admin: true,
                disabled: false,
                avatar_url: None,
                must_change_password: false,
            }],
            created_at: 0,
        };
        assert!(cfg.is_admin("admin"));
        assert!(!cfg.is_admin("user"));
    }
}
