#![allow(dead_code)]

use anyhow::Result;
use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

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
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
