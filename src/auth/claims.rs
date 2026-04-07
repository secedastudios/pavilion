use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use surrealdb::types::RecordId;

const TOKEN_EXPIRY_SECS: u64 = 24 * 60 * 60; // 24 hours

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub name: String,
    pub roles: Vec<String>,
    pub exp: u64,
    pub iat: u64,
}

impl Claims {
    pub fn person_id(&self) -> RecordId {
        RecordId::new("person", self.sub.as_str())
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

pub fn issue_token(
    person_key: &str,
    name: &str,
    roles: &[String],
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs();

    let claims = Claims {
        sub: person_key.to_string(),
        name: name.to_string(),
        roles: roles.to_vec(),
        exp: now + TOKEN_EXPIRY_SECS,
        iat: now,
    };

    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
}

pub fn verify_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}
