//! Data model for password entries and vault structure

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single password entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: Uuid,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub name: String,
    pub username: String,
    pub password: String,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
}

impl Entry {
    /// Create a new entry with generated ID and timestamps
    pub fn new(
        name: String,
        username: String,
        password: String,
        url: Option<String>,
        notes: Option<String>,
        tags: Vec<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            created: now,
            modified: now,
            name,
            username,
            password,
            url,
            notes,
            tags,
        }
    }

    /// Update the modified timestamp
    pub fn touch(&mut self) {
        self.modified = Utc::now();
    }

    /// Check if entry matches search query (case-insensitive)
    pub fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();

        let name_match = self.name.to_lowercase().contains(&query);
        let username_match = self.username.to_lowercase().contains(&query);
        let url_match = self
            .url
            .as_ref()
            .map(|u| u.to_lowercase().contains(&query))
            .unwrap_or(false);
        let notes_match = self
            .notes
            .as_ref()
            .map(|n| n.to_lowercase().contains(&query))
            .unwrap_or(false);
        let tags_match = self.tags.iter().any(|t| t.to_lowercase().contains(&query));

        name_match | username_match | url_match | notes_match | tags_match
    }
}

/// The vault containing all password entries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vault {
    pub version: u32,
    pub entries: Vec<Entry>,
}

impl Vault {
    /// Create a new empty vault
    pub fn new() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }

    /// Add a new entry to the vault
    pub fn add_entry(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    /// Remove an entry by ID
    pub fn remove_entry(&mut self, id: &Uuid) -> Option<Entry> {
        if let Some(pos) = self.entries.iter().position(|e| &e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    /// Get a mutable reference to an entry by ID
    pub fn get_entry_mut(&mut self, id: &Uuid) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| &e.id == id)
    }

    /// Get an immutable reference to an entry by ID
    pub fn get_entry(&self, id: &Uuid) -> Option<&Entry> {
        self.entries.iter().find(|e| &e.id == id)
    }

    /// Search entries by query
    pub fn search(&self, query: &str) -> Vec<&Entry> {
        if query.is_empty() {
            self.entries.iter().collect()
        } else {
            self.entries.iter().filter(|e| e.matches(query)).collect()
        }
    }
}

impl Default for Vault {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_matches() {
        let entry = Entry::new(
            "GitHub".to_string(),
            "user@example.com".to_string(),
            "password123".to_string(),
            Some("https://github.com".to_string()),
            Some("Personal account".to_string()),
            vec!["work".to_string(), "dev".to_string()],
        );

        assert!(entry.matches("github"));
        assert!(entry.matches("user"));
        assert!(entry.matches("example"));
        assert!(entry.matches("work"));
        assert!(entry.matches("personal"));
        assert!(!entry.matches("gitlab"));
    }

    #[test]
    fn test_vault_operations() {
        let mut vault = Vault::new();
        assert_eq!(vault.entries.len(), 0);

        let entry = Entry::new(
            "Test".to_string(),
            "user".to_string(),
            "pass".to_string(),
            None,
            None,
            vec![],
        );
        let id = entry.id;

        vault.add_entry(entry);
        assert_eq!(vault.entries.len(), 1);

        assert!(vault.get_entry(&id).is_some());

        vault.remove_entry(&id);
        assert_eq!(vault.entries.len(), 0);
    }

    #[test]
    fn test_search() {
        let mut vault = Vault::new();

        vault.add_entry(Entry::new(
            "GitHub".to_string(),
            "user1".to_string(),
            "pass1".to_string(),
            None,
            None,
            vec![],
        ));

        vault.add_entry(Entry::new(
            "GitLab".to_string(),
            "user2".to_string(),
            "pass2".to_string(),
            None,
            None,
            vec![],
        ));

        let results = vault.search("github");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "GitHub");

        let results = vault.search("git");
        assert_eq!(results.len(), 2);

        let results = vault.search("");
        assert_eq!(results.len(), 2);
    }
}
