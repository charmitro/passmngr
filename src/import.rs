//! Import entries from various formats
//!
//! Supports:
//! - Firefox CSV (full export with 8+ columns)
//! - Firefox/Chrome simple CSV (url,username,password)
//! - JSON (our native format)
//! - Generic CSV with flexible header detection

use crate::model::{Entry, Vault};
use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ImportedEntry {
    pub name: String,
    pub username: String,
    pub password: String,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
}

impl ImportedEntry {
    pub fn to_entry(self) -> Entry {
        Entry::new(
            self.name,
            self.username,
            self.password,
            self.url,
            self.notes,
            self.tags,
        )
    }
}

#[derive(Debug)]
pub struct ImportPreview {
    pub total_entries: usize,
    pub new_entries: usize,
    pub duplicates: Vec<DuplicateInfo>,
    pub entries: Vec<ImportedEntry>,
}

#[derive(Debug, Clone)]
pub struct DuplicateInfo {
    pub imported_name: String,
    pub imported_username: String,
    pub existing_id: Uuid,
    pub existing_name: String,
}

/// Detect file format and import
pub fn import_from_file(path: &Path, vault: &Vault) -> Result<ImportPreview> {
    let contents = fs::read_to_string(path)?;

    // Try to detect format
    if contents.trim_start().starts_with('{') || contents.trim_start().starts_with('[') {
        // JSON format
        import_json(&contents, vault)
    } else {
        // CSV format
        import_csv(&contents, vault)
    }
}

/// Import from JSON format
fn import_json(contents: &str, vault: &Vault) -> Result<ImportPreview> {
    let imported_vault: Vault = serde_json::from_str(contents)?;

    let mut entries = Vec::new();
    let mut duplicates = Vec::new();

    for entry in imported_vault.entries {
        let imported = ImportedEntry {
            name: entry.name.clone(),
            username: entry.username.clone(),
            password: entry.password.clone(),
            url: entry.url.clone(),
            notes: entry.notes.clone(),
            tags: entry.tags.clone(),
        };

        // Check for duplicates
        if let Some(dup) = find_duplicate(vault, &imported) {
            duplicates.push(dup);
        }

        entries.push(imported);
    }

    Ok(ImportPreview {
        total_entries: entries.len(),
        new_entries: entries.len() - duplicates.len(),
        duplicates,
        entries,
    })
}

/// Import from CSV format (auto-detect variant)
fn import_csv(contents: &str, vault: &Vault) -> Result<ImportPreview> {
    let mut lines = contents.lines();
    let header = lines.next().ok_or_else(|| anyhow!("Empty CSV file"))?;

    let format = detect_csv_format(header)?;

    let mut entries = Vec::new();
    let mut duplicates = Vec::new();

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }

        let imported = parse_csv_line(line, &format)?;

        // Check for duplicates
        if let Some(dup) = find_duplicate(vault, &imported) {
            duplicates.push(dup);
        }

        entries.push(imported);
    }

    Ok(ImportPreview {
        total_entries: entries.len(),
        new_entries: entries.len() - duplicates.len(),
        duplicates,
        entries,
    })
}

/// CSV format variants
#[derive(Debug)]
enum CsvFormat {
    /// Firefox full export: url,username,password,httpRealm,formActionOrigin,guid,timeCreated,timeLastUsed,timePasswordChanged
    FirefoxFull {
        url_idx: usize,
        username_idx: usize,
        password_idx: usize,
    },
    /// Firefox/Chrome simple: url,username,password
    FirefoxSimple {
        url_idx: usize,
        username_idx: usize,
        password_idx: usize,
    },
    /// Extended: name,username,password,url,notes,tags
    Extended {
        name_idx: Option<usize>,
        username_idx: usize,
        password_idx: usize,
        url_idx: Option<usize>,
        notes_idx: Option<usize>,
        tags_idx: Option<usize>,
    },
}

/// Detect CSV format from header line
fn detect_csv_format(header: &str) -> Result<CsvFormat> {
    let headers = parse_csv_headers(header);

    // Check for Firefox full format
    if headers.iter().any(|h| h == "httprealm") || headers.iter().any(|h| h == "formactionorigin") {
        return Ok(CsvFormat::FirefoxFull {
            url_idx: find_header_idx(&headers, &["url"])?,
            username_idx: find_header_idx(&headers, &["username"])?,
            password_idx: find_header_idx(&headers, &["password"])?,
        });
    }

    // Check for Firefox simple format (url,username,password)
    if headers.len() == 3
        && headers.iter().any(|h| h == "url")
        && headers.iter().any(|h| h == "username")
        && headers.iter().any(|h| h == "password")
    {
        return Ok(CsvFormat::FirefoxSimple {
            url_idx: find_header_idx(&headers, &["url"])?,
            username_idx: find_header_idx(&headers, &["username"])?,
            password_idx: find_header_idx(&headers, &["password"])?,
        });
    }

    // Extended format (flexible)
    Ok(CsvFormat::Extended {
        name_idx: find_header_idx(&headers, &["name", "title", "site"]).ok(),
        username_idx: find_header_idx(&headers, &["username", "login", "email", "user"])?,
        password_idx: find_header_idx(&headers, &["password", "pass", "pwd"])?,
        url_idx: find_header_idx(&headers, &["url", "website", "site"]).ok(),
        notes_idx: find_header_idx(&headers, &["notes", "note", "comment", "comments"]).ok(),
        tags_idx: find_header_idx(&headers, &["tags", "tag", "labels", "categories"]).ok(),
    })
}

/// Parse CSV headers
fn parse_csv_headers(header: &str) -> Vec<String> {
    header
        .split(',')
        .map(|h| h.trim().trim_matches('"').to_lowercase())
        .collect()
}

/// Find header index by trying multiple variations
fn find_header_idx(headers: &[String], variations: &[&str]) -> Result<usize> {
    for var in variations {
        if let Some(idx) = headers.iter().position(|h| h == var) {
            return Ok(idx);
        }
    }
    Err(anyhow!("Required header not found: {:?}", variations))
}

/// Parse a single CSV line based on format
fn parse_csv_line(line: &str, format: &CsvFormat) -> Result<ImportedEntry> {
    let fields = parse_csv_fields(line);

    match format {
        CsvFormat::FirefoxFull {
            url_idx,
            username_idx,
            password_idx,
        }
        | CsvFormat::FirefoxSimple {
            url_idx,
            username_idx,
            password_idx,
        } => {
            let url = fields.get(*url_idx).map(|s| s.to_string());
            let name = generate_name_from_url(url.as_deref().unwrap_or(""));

            Ok(ImportedEntry {
                name,
                username: fields.get(*username_idx).cloned().unwrap_or_default(),
                password: fields.get(*password_idx).cloned().unwrap_or_default(),
                url,
                notes: None,
                tags: Vec::new(),
            })
        }
        CsvFormat::Extended {
            name_idx,
            username_idx,
            password_idx,
            url_idx,
            notes_idx,
            tags_idx,
        } => {
            let url = url_idx.and_then(|idx| fields.get(idx).map(|s| s.to_string()));
            let name = if let Some(idx) = name_idx {
                fields
                    .get(*idx)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| generate_name_from_url(url.as_deref().unwrap_or("")))
            } else {
                generate_name_from_url(url.as_deref().unwrap_or(""))
            };

            let tags = if let Some(idx) = tags_idx {
                fields
                    .get(*idx)
                    .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            Ok(ImportedEntry {
                name,
                username: fields.get(*username_idx).cloned().unwrap_or_default(),
                password: fields.get(*password_idx).cloned().unwrap_or_default(),
                url,
                notes: notes_idx.and_then(|idx| fields.get(idx).map(|s| s.to_string())),
                tags,
            })
        }
    }
}

/// Parse CSV fields (handles quoted fields)
fn parse_csv_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current_field = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if in_quotes && chars.peek() == Some(&'"') {
                    // Escaped quote
                    current_field.push('"');
                    chars.next();
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ',' if !in_quotes => {
                fields.push(current_field.trim().to_string());
                current_field.clear();
            }
            _ => current_field.push(ch),
        }
    }
    fields.push(current_field.trim().to_string());

    fields
}

/// Generate entry name from URL
fn generate_name_from_url(url: &str) -> String {
    if url.is_empty() {
        return "Imported Entry".to_string();
    }

    // Extract domain from URL
    let url_lower = url.to_lowercase();
    let domain = if let Some(start) = url_lower.find("://") {
        &url_lower[start + 3..]
    } else {
        &url_lower
    };

    let domain = domain.split('/').next().unwrap_or(domain);
    let domain = domain.split(':').next().unwrap_or(domain);

    // Remove www. prefix
    let domain = domain.strip_prefix("www.").unwrap_or(domain);

    // Capitalize first letter
    let mut chars = domain.chars();
    match chars.next() {
        None => "Imported Entry".to_string(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Find duplicate entry in vault
fn find_duplicate(vault: &Vault, imported: &ImportedEntry) -> Option<DuplicateInfo> {
    for entry in &vault.entries {
        // Consider it a duplicate if username and URL match
        let urls_match = match (&entry.url, &imported.url) {
            (Some(a), Some(b)) => a.to_lowercase() == b.to_lowercase(),
            (None, None) => true,
            _ => false,
        };

        if entry.username.to_lowercase() == imported.username.to_lowercase() && urls_match {
            return Some(DuplicateInfo {
                imported_name: imported.name.clone(),
                imported_username: imported.username.clone(),
                existing_id: entry.id,
                existing_name: entry.name.clone(),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_name_from_url() {
        assert_eq!(generate_name_from_url("https://github.com"), "Github.com");
        assert_eq!(
            generate_name_from_url("https://www.google.com"),
            "Google.com"
        );
        assert_eq!(
            generate_name_from_url("http://example.org/path"),
            "Example.org"
        );
        assert_eq!(generate_name_from_url(""), "Imported Entry");
    }

    #[test]
    fn test_parse_csv_fields() {
        let fields = parse_csv_fields("\"value1\",\"value2\",\"value3\"");
        assert_eq!(fields, vec!["value1", "value2", "value3"]);

        let fields = parse_csv_fields("\"has,comma\",\"has\"\"quote\",normal");
        assert_eq!(fields, vec!["has,comma", "has\"quote", "normal"]);
    }

    #[test]
    fn test_detect_firefox_simple() {
        let header = "url,username,password";
        let format = detect_csv_format(header).unwrap();
        matches!(format, CsvFormat::FirefoxSimple { .. });
    }
}
