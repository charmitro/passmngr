//! Cryptographic operations for vault encryption
//!
//! ## Algorithm Choices
//!
//! **Argon2id** for key derivation:
//! - Memory-hard: Resists GPU/ASIC attacks by requiring significant RAM
//! - Hybrid mode: Combines data-dependent and data-independent memory access
//! - Parameters: 3 iterations, 64 MiB memory, 4 threads
//! - Time cost: ~100ms on modern hardware (intentional security vs usability trade-off)
//!
//! **ChaCha20-Poly1305** for authenticated encryption:
//! - Stream cipher: Fast, constant-time (no timing attacks)
//! - AEAD: Authentication prevents tampering
//! - Well-analyzed: IETF RFC 8439 standard
//! - No known practical attacks
//!
//! ## Security Properties
//!
//! - 256-bit keys (post-quantum security margin)
//! - Random salt per vault (prevents rainbow tables)
//! - Random nonce per encryption (prevents replay)
//! - Authenticated encryption (detects tampering)
//! - Memory-hard KDF (resists brute-force)
//!
//! ## Threat Model
//!
//! Protected against:
//! - Offline brute-force attacks (Argon2id makes this expensive)
//! - Tampering (Poly1305 MAC detects modifications)
//! - Cryptanalysis (well-studied algorithms)
//!
//! Not protected against:
//! - Active keyloggers while unlocked
//! - Root/admin access to running process
//! - Weak master passwords (user responsibility)
//! - Side-channel attacks (not relevant for local CLI tool)

use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2, ParamsBuilder, Version,
};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Size of encryption key in bytes (256 bits)
const KEY_SIZE: usize = 32;

/// Size of nonce in bytes (96 bits for ChaCha20-Poly1305)
const NONCE_SIZE: usize = 12;

/// Size of salt in bytes (128 bits)
const SALT_SIZE: usize = 16;

/// KDF parameters stored with the vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    pub algorithm: String,
    pub salt: Vec<u8>,
    pub time_cost: u32,
    pub memory_cost: u32,
    pub parallelism: u32,
}

impl KdfParams {
    /// Create new KDF parameters with recommended settings
    pub fn new() -> Result<Self> {
        let mut salt = vec![0u8; SALT_SIZE];
        OsRng.fill_bytes(&mut salt);

        Ok(Self {
            algorithm: "argon2id".to_string(),
            salt,
            // Time cost: 3 iterations (minimum recommended for interactive use)
            time_cost: 3,
            // Memory cost: 64 MiB (balances security vs. usability)
            // Higher values = more secure but slower unlock time
            memory_cost: 65536, // 64 MiB (in KiB units)
            // Parallelism: 4 threads (utilizes modern multi-core CPUs)
            parallelism: 4,
        })
    }
}

impl Default for KdfParams {
    fn default() -> Self {
        Self::new().expect("Failed to generate KDF parameters")
    }
}

/// Cipher parameters stored with the vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CipherParams {
    pub algorithm: String,
    pub nonce: Vec<u8>,
}

impl CipherParams {
    /// Create new cipher parameters with random nonce
    pub fn new() -> Self {
        let mut nonce = vec![0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce);

        Self {
            algorithm: "chacha20poly1305".to_string(),
            nonce,
        }
    }
}

impl Default for CipherParams {
    fn default() -> Self {
        Self::new()
    }
}

/// Encryption key derived from master password
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct EncryptionKey {
    key: [u8; KEY_SIZE],
}

impl EncryptionKey {
    /// Derive encryption key from password using Argon2id
    pub fn derive(password: &str, params: &KdfParams) -> Result<Self> {
        // Build Argon2 parameters
        let argon2_params = ParamsBuilder::new()
            .m_cost(params.memory_cost)
            .t_cost(params.time_cost)
            .p_cost(params.parallelism)
            .output_len(KEY_SIZE)
            .build()
            .map_err(|e| anyhow!("Failed to build Argon2 parameters: {}", e))?;

        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, argon2_params);

        // Derive key
        let salt_string = SaltString::encode_b64(&params.salt)
            .map_err(|e| anyhow!("Failed to encode salt: {}", e))?;

        let hash = argon2
            .hash_password(password.as_bytes(), &salt_string)
            .map_err(|e| anyhow!("Failed to hash password: {}", e))?;

        let hash_output = hash.hash.ok_or_else(|| anyhow!("No hash output"))?;

        let hash_bytes = hash_output.as_bytes();

        if hash_bytes.len() != KEY_SIZE {
            return Err(anyhow!("Invalid key size: {}", hash_bytes.len()));
        }

        let mut key = [0u8; KEY_SIZE];
        key.copy_from_slice(hash_bytes);

        Ok(Self { key })
    }

    /// Encrypt data using ChaCha20-Poly1305
    pub fn encrypt(&self, plaintext: &[u8], cipher_params: &CipherParams) -> Result<Vec<u8>> {
        if cipher_params.nonce.len() != NONCE_SIZE {
            return Err(anyhow!("Invalid nonce size"));
        }

        let cipher = ChaCha20Poly1305::new_from_slice(&self.key)
            .map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

        let nonce = Nonce::try_from(cipher_params.nonce.as_slice())
            .map_err(|_| anyhow!("Invalid nonce"))?;

        cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow!("Encryption failed: {}", e))
    }

    /// Decrypt data using ChaCha20-Poly1305
    pub fn decrypt(&self, ciphertext: &[u8], cipher_params: &CipherParams) -> Result<Vec<u8>> {
        if cipher_params.nonce.len() != NONCE_SIZE {
            return Err(anyhow!("Invalid nonce size"));
        }

        let cipher = ChaCha20Poly1305::new_from_slice(&self.key)
            .map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

        let nonce = Nonce::try_from(cipher_params.nonce.as_slice())
            .map_err(|_| anyhow!("Invalid nonce"))?;

        cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|e| anyhow!("Decryption failed: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_derivation() {
        let params = KdfParams::new().unwrap();
        let key = EncryptionKey::derive("test_password", &params);
        assert!(key.is_ok());
    }

    #[test]
    fn test_encryption_decryption() {
        let params = KdfParams::new().unwrap();
        let key = EncryptionKey::derive("test_password", &params).unwrap();

        let plaintext = b"Hello, World!";
        let cipher_params = CipherParams::new();

        let ciphertext = key.encrypt(plaintext, &cipher_params).unwrap();
        assert_ne!(&ciphertext[..], plaintext);

        let decrypted = key.decrypt(&ciphertext, &cipher_params).unwrap();
        assert_eq!(&decrypted[..], plaintext);
    }

    #[test]
    fn test_wrong_password() {
        let params = KdfParams::new().unwrap();
        let key1 = EncryptionKey::derive("password1", &params).unwrap();
        let key2 = EncryptionKey::derive("password2", &params).unwrap();

        let plaintext = b"Secret data";
        let cipher_params = CipherParams::new();

        let ciphertext = key1.encrypt(plaintext, &cipher_params).unwrap();

        // Attempting to decrypt with wrong password should fail
        let result = key2.decrypt(&ciphertext, &cipher_params);
        assert!(result.is_err());
    }

    #[test]
    fn test_tampered_ciphertext() {
        let params = KdfParams::new().unwrap();
        let key = EncryptionKey::derive("test_password", &params).unwrap();

        let plaintext = b"Sensitive data";
        let cipher_params = CipherParams::new();

        let mut ciphertext = key.encrypt(plaintext, &cipher_params).unwrap();

        // Tamper with ciphertext
        if let Some(byte) = ciphertext.get_mut(0) {
            *byte ^= 0xFF;
        }

        // Decryption should fail due to authentication tag
        let result = key.decrypt(&ciphertext, &cipher_params);
        assert!(result.is_err());
    }
}
