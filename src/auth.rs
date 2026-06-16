use anyhow::{anyhow, Context, Result};
use base64::Engine as _;
use ed25519_dalek::{Signer, SigningKey};

pub const ENV_KEY_ID: &str = "POLYMARKET_US_KEY_ID";
pub const ENV_SECRET_KEY: &str = "POLYMARKET_US_SECRET_KEY";

pub const HEADER_ACCESS_KEY: &str = "X-PM-Access-Key";
pub const HEADER_TIMESTAMP: &str = "X-PM-Timestamp";
pub const HEADER_SIGNATURE: &str = "X-PM-Signature";

#[derive(Clone)]
pub struct UsAuth {
    key_id: String,
    signing_key: SigningKey,
}

impl UsAuth {
    pub fn from_env() -> Result<Self> {
        let key_id = std::env::var(ENV_KEY_ID)
            .with_context(|| format!("{ENV_KEY_ID} not set"))?;
        let secret_b64 = std::env::var(ENV_SECRET_KEY)
            .with_context(|| format!("{ENV_SECRET_KEY} not set"))?;
        Self::from_parts(key_id, &secret_b64)
    }

    pub fn from_parts(key_id: String, secret_b64: &str) -> Result<Self> {
        let secret = base64::engine::general_purpose::STANDARD
            .decode(secret_b64.trim())
            .context("POLYMARKET_US_SECRET_KEY is not valid Base64")?;

        let signing_key = match secret.len() {
            64 => {
                let seed: [u8; 32] = secret[..32].try_into().expect("first 32 bytes");
                SigningKey::from_bytes(&seed)
            }
            32 => {
                let seed: [u8; 32] = secret.as_slice().try_into().expect("len checked == 32");
                SigningKey::from_bytes(&seed)
            }
            n => {
                return Err(anyhow!(
                    "POLYMARKET_US_SECRET_KEY must decode to 64 bytes (keypair) or 32 bytes (seed), got {n}"
                ))
            }
        };

        Ok(Self { key_id, signing_key })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    fn signing_payload(timestamp_ms: i64, method: &str, path: &str) -> String {
        format!("{}{}{}", timestamp_ms, method.to_uppercase(), path)
    }

    pub fn sign(&self, method: &str, path: &str) -> (i64, String) {
        let ts = chrono::Utc::now().timestamp_millis();
        let payload = Self::signing_payload(ts, method, path);
        let sig_bytes = self.signing_key.sign(payload.as_bytes()).to_bytes();
        let signature = base64::engine::general_purpose::STANDARD.encode(sig_bytes);
        (ts, signature)
    }

    pub fn signed_headers(&self, method: &str, path: &str) -> [(&'static str, String); 3] {
        let (ts, sig) = self.sign(method, path);
        [
            (HEADER_ACCESS_KEY, self.key_id.clone()),
            (HEADER_TIMESTAMP, ts.to_string()),
            (HEADER_SIGNATURE, sig),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Verifier, VerifyingKey};

    const SAMPLE_SECRET: &str =
        "lxcsopNhvp+FyZMtVPnHPeHAGihFMPEZcUg6TrJX6kCfwSEXu8v8vmyi3wJbMFUs3a9Fe7mkyRIwfZZkd/5kPg==";

    #[test]
    fn loads_64_byte_keypair_and_signs_verifiably() {
        let auth = UsAuth::from_parts("483074f3-key".into(), SAMPLE_SECRET).unwrap();

        let (ts, sig_b64) = auth.sign("GET", "/v1/account/balance");
        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(sig_b64)
            .unwrap();
        let sig = ed25519_dalek::Signature::from_slice(&sig_bytes).unwrap();

        let raw = base64::engine::general_purpose::STANDARD
            .decode(SAMPLE_SECRET)
            .unwrap();
        let pub_bytes: [u8; 32] = raw[32..64].try_into().unwrap();
        let vk = VerifyingKey::from_bytes(&pub_bytes).unwrap();
        let payload = format!("{}GET/v1/account/balance", ts);
        assert!(vk.verify(payload.as_bytes(), &sig).is_ok());
    }
}

