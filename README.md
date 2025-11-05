# passmngr

A fast, minimal TUI password manager with vim-like keybindings.

## Features

- **Fast & Instant**: Zero perceptible latency, instant search as you type
- **Secure**: Argon2id key derivation + ChaCha20-Poly1305 encryption
- **Local-only**: No cloud sync, complete user control
- **Keyboard-first**: Vim-like interface, all actions via keyboard
- **Minimal**: ~10 dependencies, single-person comprehensible
- **Auditable**: Simple, documented code and file format

## Building

```bash
cargo build --release
```

## Running

### TUI Mode (Interactive)
```bash
cargo run --release
# or after building:
./target/release/passmngr
```

On first run, you'll be prompted to create a master password. The vault is stored at:
- `~/.local/share/passmngr/vault.enc` (Linux/macOS)

### CLI Mode (Scriptable)
```bash
# Export passwords (⚠️ plaintext!)
./target/release/passmngr export firefox ~/backup.csv
./target/release/passmngr export json ~/backup.json

# Import passwords
./target/release/passmngr import ~/passwords.csv
./target/release/passmngr import ~/passwords.csv --skip-duplicates

# Get help
./target/release/passmngr --help
./target/release/passmngr export --help
./target/release/passmngr import --help
```

## Testing

Populate the vault with sample data:

```bash
cargo run --example populate_vault
```

This creates a vault with password: `testpassword`

Then run the application:

```bash
cargo run
```

Enter the password: `testpassword`

## Key Bindings

### Normal Mode
- `j/k` - Move down/up
- `g/G` - Jump to top/bottom
- `/` - Enter search mode
- `n` - Create new entry
- `e` - Edit selected entry
- `d` - Delete selected entry
- `Enter` - View entry details
- `y` - Copy password to clipboard
- `Y` - Copy username to clipboard
- `:` - Enter command mode
- `Esc` - Clear search

### Search Mode
- Type to search (instant results)
- `Enter` - Keep filter active, allow navigation
- `Esc` - Clear filter, show all entries
- `Backspace` - Delete character

### Insert Mode (Create/Edit Entry)
- Type to enter text in current field
- `Tab` - Next field
- `Shift+Tab` - Previous field
- `Ctrl+S` - Save entry
- `Esc` - Cancel without saving

### Detail Mode
- `q` or `Esc` - Return to list
- `e` - Edit this entry
- `y` - Copy password
- `Y` - Copy username

### Command Mode
- `Tab` - Autocomplete command (cycles through matches)
- `:w` - Save vault
- `:q` - Quit (warns if unsaved)
- `:wq` or `:x` - Save and quit
- `:q!` - Force quit without saving
- `:export <format> <path>` - Export passwords (⚠️ plaintext!)
  - Formats: `firefox` (most compatible), `json` (full backup), `csv` (all fields)
  - Example: `:export firefox ~/backup.csv`
  - Press `Tab` after `:export ` to cycle through format options

## Export/Import

### Exporting Passwords

⚠️  **WARNING**: Exported files contain PLAINTEXT passwords! Delete them after use.

**CLI Method (Recommended for Scripts):**
```bash
# Export formats: firefox, json, csv
passmngr export firefox ~/backup.csv      # Firefox/Chrome compatible
passmngr export json ~/backup.json        # Full backup with metadata
passmngr export csv ~/backup.csv          # Extended CSV with all fields
```

**TUI Method:**
```bash
# In the TUI, press : to enter command mode, then:
:export firefox ~/backup.csv      # Firefox/Chrome compatible
:export json ~/backup.json        # Full backup with metadata
:export csv ~/backup.csv          # Extended CSV with all fields
```

**Export Formats:**
- **firefox**: Simple 3-column CSV (url,username,password) - imports directly into Firefox/Chrome
- **json**: Full vault backup with notes, tags, timestamps - use for backups
- **csv**: Extended CSV with all fields - compatible with most password managers

**Security:**
- Files are automatically set to mode 600 (owner read/write only)
- Export shows prominent warning
- Delete export files immediately after use

### Importing Passwords

**CLI Method (Recommended):**
```bash
# Import with preview
passmngr import ~/passwords.csv

# Skip duplicates automatically
passmngr import ~/passwords.csv --skip-duplicates
```

**Supported Import Formats:**
- Firefox full CSV export (8+ columns with metadata)
- Firefox/Chrome simple CSV (url,username,password)
- JSON backups from passmngr
- Generic CSV with flexible header detection

**Duplicate Handling:**
- Detects duplicates by matching username + URL
- Shows preview before importing
- Option to skip duplicates with --skip-duplicates flag

## Architecture

```
TUI Layer (ratatui)
    ↓
Application Core
    ↓
Crypto Layer (Argon2id + ChaCha20-Poly1305)
    ↓
Storage Layer (Encrypted JSON)
```

## Security

- **Key Derivation**: Argon2id (3 iterations, 64 MiB memory, 4 threads)
- **Encryption**: ChaCha20-Poly1305 (authenticated encryption)
- **File Format**: Encrypted JSON with versioning
- **No telemetry**: Zero data collection

## File Format

The vault file (`vault.enc`) is a JSON file containing:
- Version number
- KDF parameters (algorithm, salt, costs)
- Cipher parameters (algorithm, nonce)
- Encrypted vault data (entries as JSON when decrypted)

## License

MIT License - see [LICENSE](LICENSE) file for details.
