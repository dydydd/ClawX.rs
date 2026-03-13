//! Device identity management using Ed25519 signing
//!
//! Compatible with Electron version's device identity format.

use anyhow::Result;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use base64::{Engine as _, engine::general_purpose::{URL_SAFE_NO_PAD, STANDARD}};

/// Device identity containing a unique device ID and Ed25519 signing key
#[derive(Debug)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub public_key_pem: String,
    pub private_key_pem: String,
    signing_key: SigningKey,
}

#[derive(Debug, Serialize, Deserialize)]
struct DeviceIdentityFile {
    version: i32,
    device_id: String,
    public_key_pem: String,
    private_key_pem: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at_ms: Option<i64>,
}

/// Compute SHA256 fingerprint of the raw public key bytes
fn fingerprint_public_key(public_key_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(public_key_bytes);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Convert Ed25519 public key to PEM format
fn public_key_to_pem(verifying_key: &VerifyingKey) -> String {
    // SPKI prefix for Ed25519
    const ED25519_SPKI_PREFIX: &[u8] = &[
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00
    ];

    let mut spki = Vec::with_capacity(ED25519_SPKI_PREFIX.len() + 32);
    spki.extend_from_slice(ED25519_SPKI_PREFIX);
    spki.extend_from_slice(verifying_key.as_bytes());

    let b64 = STANDARD.encode(&spki);
    let mut pem = String::from("-----BEGIN PUBLIC KEY-----\n");

    // Split into 64-char lines
    for (i, c) in b64.chars().enumerate() {
        if i > 0 && i % 64 == 0 {
            pem.push('\n');
        }
        pem.push(c);
    }
    pem.push_str("\n-----END PUBLIC KEY-----\n");

    pem
}

/// Convert Ed25519 private key to PEM format
fn private_key_to_pem(signing_key: &SigningKey) -> String {
    // PKCS#8 prefix for Ed25519 private key
    const ED25519_PKCS8_PREFIX: &[u8] = &[
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70,
        0x04, 0x22, 0x04, 0x20
    ];

    let mut pkcs8 = Vec::with_capacity(ED25519_PKCS8_PREFIX.len() + 32);
    pkcs8.extend_from_slice(ED25519_PKCS8_PREFIX);
    pkcs8.extend_from_slice(signing_key.as_bytes());

    let b64 = STANDARD.encode(&pkcs8);
    let mut pem = String::from("-----BEGIN PRIVATE KEY-----\n");

    for (i, c) in b64.chars().enumerate() {
        if i > 0 && i % 64 == 0 {
            pem.push('\n');
        }
        pem.push(c);
    }
    pem.push_str("\n-----END PRIVATE KEY-----\n");

    pem
}

/// Parse PEM to get raw public key bytes
fn parse_pem_public_key(pem: &str) -> Result<Vec<u8>> {
    let pem = pem
        .replace("-----BEGIN PUBLIC KEY-----", "")
        .replace("-----END PUBLIC KEY-----", "")
        .replace('\n', "")
        .replace('\r', "");

    let der = STANDARD.decode(&pem)?;

    // Skip SPKI prefix (12 bytes) to get raw 32-byte Ed25519 public key
    if der.len() < 44 {
        anyhow::bail!("Invalid public key length");
    }

    Ok(der[12..44].to_vec())
}

/// Parse PEM to get signing key
fn parse_pem_private_key(pem: &str) -> Result<SigningKey> {
    let pem = pem
        .replace("-----BEGIN PRIVATE KEY-----", "")
        .replace("-----END PRIVATE KEY-----", "")
        .replace('\n', "")
        .replace('\r', "");

    let der = STANDARD.decode(&pem)?;

    // Skip PKCS#8 prefix (16 bytes) to get raw 32-byte Ed25519 private key
    if der.len() < 48 {
        anyhow::bail!("Invalid private key length");
    }

    let key_bytes: [u8; 32] = der[16..48]
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid private key length"))?;

    Ok(SigningKey::from_bytes(&key_bytes))
}

impl DeviceIdentity {
    /// Load device identity from disk, or create a new one if it doesn't exist
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            Self::load(path)
        } else {
            Self::create_new(path)
        }
    }

    /// Load device identity from file (Electron-compatible format)
    fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let file: DeviceIdentityFile = serde_json::from_str(&content)?;

        let signing_key = parse_pem_private_key(&file.private_key_pem)?;
        let verifying_key = signing_key.verifying_key();

        // Verify device ID matches public key
        let public_key_bytes = parse_pem_public_key(&file.public_key_pem)?;
        let derived_id = fingerprint_public_key(&public_key_bytes);

        let device_id = if derived_id != file.device_id {
            // Update device ID if it doesn't match
            tracing::info!("Updating device ID to match public key fingerprint");
            derived_id
        } else {
            file.device_id
        };

        Ok(Self {
            device_id,
            public_key_pem: file.public_key_pem,
            private_key_pem: file.private_key_pem,
            signing_key,
        })
    }

    /// Create a new device identity and save it to disk
    fn create_new(path: &Path) -> Result<Self> {
        let mut rng = OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key = signing_key.verifying_key();

        let public_key_pem = public_key_to_pem(&verifying_key);
        let private_key_pem = private_key_to_pem(&signing_key);
        let device_id = fingerprint_public_key(verifying_key.as_bytes());

        let identity = Self {
            device_id: device_id.clone(),
            public_key_pem: public_key_pem.clone(),
            private_key_pem: private_key_pem.clone(),
            signing_key,
        };

        // Save to disk (Electron-compatible format)
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = DeviceIdentityFile {
            version: 1,
            device_id,
            public_key_pem,
            private_key_pem,
            created_at_ms: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64
            ),
        };

        let content = serde_json::to_string_pretty(&file)?;
        std::fs::write(path, content)?;

        tracing::info!("Created new device identity: {}", identity.device_id);

        Ok(identity)
    }

    /// Sign a payload and return base64url-encoded signature
    pub fn sign_payload(&self, payload: &str) -> String {
        use ed25519_dalek::Signer;
        let signature = self.signing_key.sign(payload.as_bytes());
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    }

    /// Get the verifying key (public key)
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Get raw public key bytes
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }
}

/// Get the default device identity file path (same as Electron)
pub fn get_device_identity_path() -> std::path::PathBuf {
    let home = dirs::home_dir().expect("Failed to get home directory");
    home.join(".openclaw").join("device-identity.json")
}