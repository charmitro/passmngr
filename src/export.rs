//! Export vault entries to various formats
//!
//! Supports:
//! - Firefox CSV (url,username,password) - most compatible
//! - JSON (preserves all metadata)
//! - Extended CSV (all fields)

use crate::model::Vault;
use anyhow::Result;
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    /// Firefox/Chrome compatible CSV (url,username,password)
    Firefox,
    /// JSON format with all metadata
    Json,
    /// Extended CSV with all fields
    CsvExtended,
}

impl ExportFormat {
    pub fn parse_format(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "firefox" | "ff" | "chrome" => Some(Self::Firefox),
            "json" => Some(Self::Json),
            "csv" | "extended" => Some(Self::CsvExtended),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Firefox => "firefox",
            Self::Json => "json",
            Self::CsvExtended => "csv-extended",
        }
    }
}

/// Export vault to Firefox-compatible CSV format
///
/// Format: url,username,password
/// This is the simplest format that Firefox can import directly
fn export_firefox_csv(vault: &Vault) -> Result<String> {
    let mut output = String::from("url,username,password\n");

    for entry in &vault.entries {
        let url = entry.url.as_deref().unwrap_or("");
        let username = csv_escape(&entry.username);
        let password = csv_escape(&entry.password);

        output.push_str(&format!("\"{}\",{},{}\n", url, username, password));
    }

    Ok(output)
}

/// Export vault to JSON format (preserves all metadata)
fn export_json(vault: &Vault) -> Result<String> {
    let json = serde_json::to_string_pretty(vault)?;
    Ok(json)
}

/// Export vault to extended CSV format (all fields)
///
/// Format: name,username,password,url,notes,tags
fn export_csv_extended(vault: &Vault) -> Result<String> {
    let mut output = String::from("name,username,password,url,notes,tags\n");

    for entry in &vault.entries {
        let name = csv_escape(&entry.name);
        let username = csv_escape(&entry.username);
        let password = csv_escape(&entry.password);
        let url = csv_escape(entry.url.as_deref().unwrap_or(""));
        let notes = csv_escape(entry.notes.as_deref().unwrap_or(""));
        let tags = csv_escape(&entry.tags.join(","));

        output.push_str(&format!(
            "{},{},{},{},{},{}\n",
            name, username, password, url, notes, tags
        ));
    }

    Ok(output)
}

/// Escape a string for CSV format
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        format!("\"{}\"", s)
    }
}

/// Export vault to file
pub fn export_to_file(vault: &Vault, path: &Path, format: ExportFormat) -> Result<()> {
    let content = match format {
        ExportFormat::Firefox => export_firefox_csv(vault)?,
        ExportFormat::Json => export_json(vault)?,
        ExportFormat::CsvExtended => export_csv_extended(vault)?,
    };

    // Write to file
    let mut file = File::create(path)?;
    file.write_all(content.as_bytes())?;

    // Set secure permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, permissions)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Entry;

    #[test]
    fn test_firefox_export() {
        let mut vault = Vault::new();
        vault.add_entry(Entry::new(
            "GitHub".to_string(),
            "user@example.com".to_string(),
            "pass123".to_string(),
            Some("https://github.com".to_string()),
            None,
            vec![],
        ));

        let result = export_firefox_csv(&vault).unwrap();
        assert!(result.contains("url,username,password"));
        assert!(result.contains("https://github.com"));
        assert!(result.contains("user@example.com"));
        assert!(result.contains("pass123"));
    }

    #[test]
    fn test_csv_escape() {
        assert_eq!(csv_escape("simple"), "\"simple\"");
        assert_eq!(csv_escape("has,comma"), "\"has,comma\"");
        assert_eq!(csv_escape("has\"quote"), "\"has\"\"quote\"");
    }

    #[test]
    fn test_extended_csv_export() {
        let mut vault = Vault::new();
        vault.add_entry(Entry::new(
            "GitHub".to_string(),
            "user@example.com".to_string(),
            "pass123".to_string(),
            Some("https://github.com".to_string()),
            Some("My notes".to_string()),
            vec!["work".to_string(), "dev".to_string()],
        ));

        let result = export_csv_extended(&vault).unwrap();
        assert!(result.contains("name,username,password,url,notes,tags"));
        assert!(result.contains("GitHub"));
        assert!(result.contains("My notes"));
        assert!(result.contains("work,dev"));
    }
}
