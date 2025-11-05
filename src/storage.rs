//! Storage layer for encrypted vault persistence

use crate::crypto::{CipherParams, EncryptionKey, KdfParams};
use crate::model::Vault;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Encrypted vault file format
#[derive(Debug, Serialize, Deserialize)]
pub struct VaultFile {
    pub version: u32,
    pub kdf: KdfParams,
    pub cipher: CipherParams,
    pub ciphertext: Vec<u8>,
}

impl VaultFile {
    /// Get default vault file path
    pub fn default_path() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
        let data_dir = home.join(".local").join("share").join("passmngr");
        Ok(data_dir.join("vault.enc"))
    }

    /// Ensure the vault directory exists
    pub fn ensure_dir(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    /// Load and decrypt vault from file
    pub fn load(path: &Path, password: &str) -> Result<Vault> {
        // Read encrypted file
        let contents = fs::read(path)?;
        let vault_file: VaultFile = serde_json::from_slice(&contents)?;

        // Verify version
        if vault_file.version != 1 {
            return Err(anyhow!("Unsupported vault version: {}", vault_file.version));
        }

        // Derive key from password
        let key = EncryptionKey::derive(password, &vault_file.kdf)?;

        // Decrypt vault data
        let plaintext = key.decrypt(&vault_file.ciphertext, &vault_file.cipher)?;

        // Deserialize vault
        let vault: Vault = serde_json::from_slice(&plaintext)?;

        Ok(vault)
    }

    /// Encrypt and save vault to file
    pub fn save(path: &Path, vault: &Vault, password: &str) -> Result<()> {
        // Serialize vault to JSON
        let plaintext = serde_json::to_vec(vault)?;

        // Generate new crypto parameters
        let kdf_params = KdfParams::new()?;
        let cipher_params = CipherParams::new();

        // Derive key and encrypt
        let key = EncryptionKey::derive(password, &kdf_params)?;
        let ciphertext = key.encrypt(&plaintext, &cipher_params)?;

        // Create vault file structure
        let vault_file = VaultFile {
            version: 1,
            kdf: kdf_params,
            cipher: cipher_params,
            ciphertext,
        };

        // Ensure directory exists
        Self::ensure_dir(path)?;

        // Write to temp file first, then rename (atomic operation)
        let temp_path = path.with_extension("tmp");
        let contents = serde_json::to_vec_pretty(&vault_file)?;
        fs::write(&temp_path, contents)?;
        fs::rename(&temp_path, path)?;

        Ok(())
    }

    /// Check if vault file exists
    pub fn exists(path: &Path) -> bool {
        path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Entry;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let vault_path = temp_dir.path().join("test_vault.enc");

        let mut vault = Vault::new();
        vault.add_entry(Entry::new(
            "Test Entry".to_string(),
            "user@example.com".to_string(),
            "password123".to_string(),
            Some("https://example.com".to_string()),
            Some("Test notes".to_string()),
            vec!["test".to_string()],
        ));

        let password = "test_master_password";

        // Save vault
        VaultFile::save(&vault_path, &vault, password).unwrap();
        assert!(VaultFile::exists(&vault_path));

        // Load vault
        let loaded_vault = VaultFile::load(&vault_path, password).unwrap();
        assert_eq!(loaded_vault.entries.len(), 1);
        assert_eq!(loaded_vault.entries[0].name, "Test Entry");
    }

    #[test]
    fn test_wrong_password() {
        let temp_dir = TempDir::new().unwrap();
        let vault_path = temp_dir.path().join("test_vault.enc");

        let vault = Vault::new();
        VaultFile::save(&vault_path, &vault, "correct_password").unwrap();

        let result = VaultFile::load(&vault_path, "wrong_password");
        assert!(result.is_err());
    }
}
