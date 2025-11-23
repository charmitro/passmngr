//! Application state and logic

use crate::model::{Entry, Vault};
use crate::storage::VaultFile;
use anyhow::Result;
use ratatui::widgets::ListState;
use std::path::PathBuf;
use std::time::Instant;
use uuid::Uuid;

/// Application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Search,
    Command,
    Detail,
    Locked,
}

impl Mode {
    pub fn as_str(&self) -> &str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Search => "SEARCH",
            Mode::Command => "COMMAND",
            Mode::Detail => "DETAIL",
            Mode::Locked => "LOCKED",
        }
    }
}

/// Form fields for entry creation/editing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    Name,
    Username,
    Password,
    Url,
    Notes,
    Tags,
}

impl FormField {
    pub fn as_str(&self) -> &str {
        match self {
            FormField::Name => "Name",
            FormField::Username => "Username",
            FormField::Password => "Password",
            FormField::Url => "URL",
            FormField::Notes => "Notes",
            FormField::Tags => "Tags",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            FormField::Name => FormField::Username,
            FormField::Username => FormField::Password,
            FormField::Password => FormField::Url,
            FormField::Url => FormField::Notes,
            FormField::Notes => FormField::Tags,
            FormField::Tags => FormField::Name,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            FormField::Name => FormField::Tags,
            FormField::Username => FormField::Name,
            FormField::Password => FormField::Username,
            FormField::Url => FormField::Password,
            FormField::Notes => FormField::Url,
            FormField::Tags => FormField::Notes,
        }
    }
}

/// Form data for entry creation/editing
#[derive(Debug, Clone, Default)]
pub struct FormData {
    pub name: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,
    pub tags: String,
    pub editing_id: Option<Uuid>,
}

/// Application state
pub struct App {
    pub vault: Vault,
    pub vault_path: PathBuf,
    pub password: String,
    pub mode: Mode,
    pub selected: usize,
    pub search_query: String,
    pub command_buffer: String,
    pub command_completions: Vec<String>,
    pub completion_index: usize,
    pub status_message: Option<String>,
    pub should_quit: bool,
    pub dirty: bool,
    pub filtered_entries: Vec<Uuid>,
    pub form_data: FormData,
    pub focused_field: FormField,
    pub pending_save: bool,
    pub list_state: ListState,
    pub show_password: bool,
    // Auto-lock fields
    pub last_activity: Instant,
    pub unlock_input: String,
}

impl App {
    /// Create new application with loaded vault
    pub fn new(vault_path: PathBuf, password: String, vault: Vault) -> Self {
        let filtered_entries = vault.entries.iter().map(|e| e.id).collect();
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            vault,
            vault_path,
            password,
            mode: Mode::Normal,
            selected: 0,
            search_query: String::new(),
            command_buffer: String::new(),
            command_completions: Vec::new(),
            completion_index: 0,
            status_message: None,
            should_quit: false,
            dirty: false,
            filtered_entries,
            form_data: FormData::default(),
            focused_field: FormField::Name,
            pending_save: false,
            list_state,
            show_password: false,
            last_activity: Instant::now(),
            unlock_input: String::new(),
        }
    }

    /// Generate a secure password for the current field
    pub fn generate_password(&mut self) {
        if self.mode == Mode::Insert && self.focused_field == FormField::Password {
            let password = crate::crypto::generate_secure_password(20);
            self.form_data.password = password;
            self.set_status("Generated high-entropy password".to_string());
            self.show_password = true; // Show it so user knows
        }
    }

    /// Lock the vault (clear data from memory)
    pub fn lock(&mut self) {
        if self.mode == Mode::Locked {
            return;
        }

        // Save if dirty before locking?
        // Safety decision: Do not auto-save on lock. If the user didn't save,
        // we might be saving bad state or they might not want to persist.
        // However, losing data is bad.
        // Man-Rated decision: If dirty, we cannot safely lock without potential data loss.
        // But leaving it unlocked is a security violation.
        // Protocol: We lock. Data in memory is wiped. Unsaved changes are LOST.
        // This enforces the "save often" discipline.

        // Clear sensitive data
        self.vault.entries.clear();
        self.filtered_entries.clear();
        self.search_query.clear();
        self.form_data = FormData::default();
        let _ = self.clear_clipboard(); // Ignore error, best effort

        // Zeroize master password from memory
        // Note: String doesn't guarantee zeroization on drop, but we overwrite it here.
        // For true security, we'd use `secrecy` crate, but this is a good baseline.
        self.password = String::new();

        self.mode = Mode::Locked;
        self.unlock_input.clear();
        self.set_status("Vault Locked due to inactivity".to_string());
    }

    /// Attempt to unlock the vault
    pub fn unlock(&mut self) -> Result<()> {
        // Attempt to load vault with provided password
        // This verifies the password via authentication tag (ChaCha20-Poly1305)
        match VaultFile::load(&self.vault_path, &self.unlock_input) {
            Ok(vault) => {
                self.vault = vault;
                self.password = self.unlock_input.clone(); // Restore password

                // Restore state
                self.filtered_entries = self.vault.entries.iter().map(|e| e.id).collect();
                self.update_search();
                self.mode = Mode::Normal;
                self.unlock_input.clear();
                self.last_activity = Instant::now();
                self.set_status("Vault Unlocked".to_string());
                Ok(())
            }
            Err(_) => {
                self.set_status("Incorrect password or vault error".to_string());
                self.unlock_input.clear();
                Err(anyhow::anyhow!("Unlock failed"))
            }
        }
    }

    /// Update activity timestamp
    pub fn touch_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Get currently selected entry
    pub fn get_selected_entry(&self) -> Option<&Entry> {
        self.filtered_entries
            .get(self.selected)
            .and_then(|id| self.vault.get_entry(id))
    }

    /// Get currently selected entry ID
    pub fn get_selected_id(&self) -> Option<Uuid> {
        self.filtered_entries.get(self.selected).copied()
    }

    /// Update filtered entries based on search query
    pub fn update_search(&mut self) {
        self.filtered_entries = self
            .vault
            .search(&self.search_query)
            .into_iter()
            .map(|e| e.id)
            .collect();

        // Clamp selected index to valid range
        let len = self.filtered_entries.len();
        let non_empty = (len > 0) as usize;
        self.selected = self.selected.min(len.saturating_sub(1)) * non_empty;

        // Update list state selection
        self.list_state
            .select([None, Some(self.selected)][(len > 0) as usize]);
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        let can_move = (self.selected > 0) as usize;
        self.selected -= can_move;
        self.list_state.select(Some(self.selected));
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        let can_move = (self.selected + 1 < self.filtered_entries.len()) as usize;
        self.selected += can_move;
        self.list_state.select(Some(self.selected));
    }

    /// Jump to top
    pub fn jump_to_top(&mut self) {
        self.selected = 0;
        self.list_state.select(Some(self.selected));
    }

    /// Jump to bottom
    pub fn jump_to_bottom(&mut self) {
        if !self.filtered_entries.is_empty() {
            self.selected = self.filtered_entries.len() - 1;
            self.list_state.select(Some(self.selected));
        }
    }

    /// Add a new entry to the vault
    pub fn add_entry(&mut self, entry: Entry) {
        self.vault.add_entry(entry);
        self.dirty = true;
        self.update_search();
    }

    /// Delete currently selected entry
    pub fn delete_selected(&mut self) -> Option<Entry> {
        if let Some(id) = self.get_selected_id() {
            let entry = self.vault.remove_entry(&id);
            self.dirty = true;
            self.update_search();
            entry
        } else {
            None
        }
    }

    /// Request save operation (sets pending_save flag and shows immediate feedback)
    pub fn request_save(&mut self) {
        self.pending_save = true;
        self.set_status("Saving...".to_string());
    }

    /// Save vault to disk
    ///
    /// Note: This operation blocks for ~100-150ms due to Argon2id key derivation,
    /// which is intentionally memory-hard for security. The delay is unavoidable
    /// and necessary to protect against brute-force attacks.
    ///
    /// This should be called from the main loop after a draw() to ensure the
    /// "Saving..." status is visible before the blocking operation.
    pub fn save(&mut self) -> Result<()> {
        VaultFile::save(&self.vault_path, &self.vault, &self.password)?;
        self.dirty = false;
        self.pending_save = false;
        self.set_status("Vault saved".to_string());
        Ok(())
    }

    /// Set status message
    pub fn set_status(&mut self, message: String) {
        self.status_message = Some(message);
    }

    /// Clear status message
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    /// Enter search mode
    pub fn enter_search_mode(&mut self) {
        self.mode = Mode::Search;
        self.search_query.clear();
        self.update_search();
    }

    /// Exit search mode
    pub fn exit_search_mode(&mut self) {
        self.mode = Mode::Normal;
        self.search_query.clear();
        self.update_search();
    }

    /// Enter command mode
    pub fn enter_command_mode(&mut self) {
        self.mode = Mode::Command;
        self.command_buffer.clear();
        self.command_completions.clear();
        self.completion_index = 0;
    }

    /// Get list of available commands
    fn available_commands() -> Vec<&'static str> {
        vec![
            "q",
            "quit",
            "q!",
            "quit!",
            "w",
            "write",
            "wq",
            "x",
            "export firefox ",
            "export json ",
            "export csv ",
        ]
    }

    /// Autocomplete current command buffer
    pub fn autocomplete_command(&mut self) {
        let prefix = self.command_buffer.trim();

        // If we're cycling through existing completions
        if !self.command_completions.is_empty() {
            self.completion_index = (self.completion_index + 1) % self.command_completions.len();
            self.command_buffer = self.command_completions[self.completion_index].clone();
            return;
        }

        // Find all matching commands
        let matches: Vec<String> = Self::available_commands()
            .into_iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .map(|s| s.to_string())
            .collect();

        if matches.is_empty() {
            return;
        }

        if matches.len() == 1 {
            // Single match - complete it
            self.command_buffer = matches[0].clone();
            self.command_completions.clear();
        } else {
            // Multiple matches - cycle through them
            self.command_completions = matches;
            self.completion_index = 0;
            self.command_buffer = self.command_completions[0].clone();
        }
    }

    /// Reset command completion state (call when user types)
    pub fn reset_completion(&mut self) {
        self.command_completions.clear();
        self.completion_index = 0;
    }

    /// Execute command
    pub fn execute_command(&mut self) -> Result<()> {
        let cmd = self.command_buffer.trim().to_string();

        // Handle export command separately
        if cmd.starts_with("export ") {
            return self.handle_export_command(&cmd);
        }

        match cmd.as_str() {
            "q" | "quit" => {
                if self.dirty {
                    self.set_status(
                        "Unsaved changes! Use :q! to force quit or :wq to save and quit"
                            .to_string(),
                    );
                } else {
                    self.should_quit = true;
                }
            }
            "q!" | "quit!" => {
                self.should_quit = true;
            }
            "w" | "write" => {
                self.request_save();
            }
            "wq" | "x" => {
                self.request_save();
                self.should_quit = true;
            }
            _ => {
                self.set_status(format!("Unknown command: {}", cmd));
            }
        }

        self.mode = Mode::Normal;
        self.command_buffer.clear();
        Ok(())
    }

    /// Handle export command
    /// Format: export <format> <path>
    /// Example: export firefox ~/backup.csv
    fn handle_export_command(&mut self, cmd: &str) -> Result<()> {
        use crate::export::{export_to_file, ExportFormat};
        use std::path::PathBuf;

        let parts: Vec<&str> = cmd.split_whitespace().collect();

        if parts.len() != 3 {
            self.set_status(
                "Usage: export <format> <path> (formats: firefox, json, csv)".to_string(),
            );
            self.mode = Mode::Normal;
            self.command_buffer.clear();
            return Ok(());
        }

        let format_str = parts[1];
        let path_str = parts[2];

        let format = match ExportFormat::parse_format(format_str) {
            Some(f) => f,
            None => {
                self.set_status("Invalid format. Use: firefox, json, or csv".to_string());
                self.mode = Mode::Normal;
                self.command_buffer.clear();
                return Ok(());
            }
        };

        // Expand ~ to home directory
        let path = if path_str.starts_with("~") {
            let home = dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
            PathBuf::from(path_str.replacen("~", &home.to_string_lossy(), 1))
        } else {
            PathBuf::from(path_str)
        };

        // Export vault
        match export_to_file(&self.vault, &path, format) {
            Ok(_) => {
                self.set_status(format!(
                    "⚠ EXPORTED {} PLAINTEXT PASSWORDS to {} - DELETE AFTER USE!",
                    self.vault.entries.len(),
                    path.display()
                ));
            }
            Err(e) => {
                self.set_status(format!("Export failed: {}", e));
            }
        }

        self.mode = Mode::Normal;
        self.command_buffer.clear();
        Ok(())
    }

    /// Copy password to clipboard
    pub fn copy_password_to_clipboard(&mut self) -> Result<()> {
        if let Some(entry) = self.get_selected_entry() {
            let mut clipboard = arboard::Clipboard::new()?;
            clipboard.set_text(&entry.password)?;
            self.set_status(format!("Password copied for '{}'", entry.name));
            Ok(())
        } else {
            self.set_status("No entry selected".to_string());
            Ok(())
        }
    }

    /// Copy username to clipboard
    pub fn copy_username_to_clipboard(&mut self) -> Result<()> {
        if let Some(entry) = self.get_selected_entry() {
            let mut clipboard = arboard::Clipboard::new()?;
            clipboard.set_text(&entry.username)?;
            self.set_status(format!("Username copied for '{}'", entry.name));
            Ok(())
        } else {
            self.set_status("No entry selected".to_string());
            Ok(())
        }
    }

    /// Clear clipboard content (security feature)
    pub fn clear_clipboard(&mut self) -> Result<()> {
        let mut clipboard = arboard::Clipboard::new()?;
        clipboard.set_text("")?;
        Ok(())
    }

    /// Enter insert mode for creating a new entry
    pub fn enter_insert_mode(&mut self) {
        self.mode = Mode::Insert;
        self.form_data = FormData::default();
        self.focused_field = FormField::Name;
    }

    /// Enter edit mode for editing the selected entry
    pub fn enter_edit_mode(&mut self) {
        let form_data = self.get_selected_entry().map(|entry| FormData {
            name: entry.name.clone(),
            username: entry.username.clone(),
            password: entry.password.clone(),
            url: entry.url.clone().unwrap_or_default(),
            notes: entry.notes.clone().unwrap_or_default(),
            tags: entry.tags.join(", "),
            editing_id: Some(entry.id),
        });

        if let Some(data) = form_data {
            self.mode = Mode::Insert;
            self.form_data = data;
            self.focused_field = FormField::Name;
        } else {
            self.set_status("No entry selected".to_string());
        }
    }

    /// Get the current field value
    pub fn get_field_value(&self, field: FormField) -> &str {
        match field {
            FormField::Name => &self.form_data.name,
            FormField::Username => &self.form_data.username,
            FormField::Password => &self.form_data.password,
            FormField::Url => &self.form_data.url,
            FormField::Notes => &self.form_data.notes,
            FormField::Tags => &self.form_data.tags,
        }
    }

    /// Get mutable reference to current field value
    pub fn get_field_value_mut(&mut self, field: FormField) -> &mut String {
        match field {
            FormField::Name => &mut self.form_data.name,
            FormField::Username => &mut self.form_data.username,
            FormField::Password => &mut self.form_data.password,
            FormField::Url => &mut self.form_data.url,
            FormField::Notes => &mut self.form_data.notes,
            FormField::Tags => &mut self.form_data.tags,
        }
    }

    /// Save the form data as a new or updated entry
    pub fn save_form(&mut self) {
        // Validate required fields
        if self.form_data.name.trim().is_empty() {
            self.set_status("Name is required".to_string());
            return;
        }

        // Parse tags
        let tags: Vec<String> = self
            .form_data
            .tags
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if let Some(id) = self.form_data.editing_id {
            // Update existing entry
            if let Some(entry) = self.vault.get_entry_mut(&id) {
                entry.name = self.form_data.name.clone();
                entry.username = self.form_data.username.clone();
                entry.password = self.form_data.password.clone();
                entry.url = if self.form_data.url.is_empty() {
                    None
                } else {
                    Some(self.form_data.url.clone())
                };
                entry.notes = if self.form_data.notes.is_empty() {
                    None
                } else {
                    Some(self.form_data.notes.clone())
                };
                entry.tags = tags;
                entry.touch();

                let entry_name = entry.name.clone();
                self.dirty = true;
                self.set_status(format!("Updated entry '{}'", entry_name));
            }
        } else {
            // Create new entry
            let entry = Entry::new(
                self.form_data.name.clone(),
                self.form_data.username.clone(),
                self.form_data.password.clone(),
                if self.form_data.url.is_empty() {
                    None
                } else {
                    Some(self.form_data.url.clone())
                },
                if self.form_data.notes.is_empty() {
                    None
                } else {
                    Some(self.form_data.notes.clone())
                },
                tags,
            );

            self.set_status(format!("Created entry '{}'", entry.name));
            self.add_entry(entry);
        }

        self.mode = Mode::Normal;
        self.show_password = false;
        self.update_search();
    }

    /// Cancel form editing
    pub fn cancel_form(&mut self) {
        self.mode = Mode::Normal;
        self.form_data = FormData::default();
        self.show_password = false; // Reset visibility when canceling
    }

    /// Toggle password visibility
    pub fn toggle_password_visibility(&mut self) {
        self.show_password = !self.show_password;
    }
}
