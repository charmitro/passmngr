# passmngr

A fast, minimal TUI password manager with vim-like keybindings.

## Features

- **Fast**: Zero perceptible latency, instant search
- **Secure**: Argon2id + ChaCha20-Poly1305 encryption
- **Local-only**: No cloud, no telemetry
- **Keyboard-first**: Vim-like interface

## Install

```bash
cargo build --release
./target/release/passmngr
```

Vault stored at: `~/.local/share/passmngr/vault.enc`

## Keys

- `j/k` - Navigate
- `/` - Search
- `n` - New entry
- `e` - Edit
- `d` - Delete
- `y` - Copy password
- `:w` - Save
- `:q` - Quit

## Import/Export

```bash
passmngr import ~/passwords.csv
passmngr export firefox ~/backup.csv
```

Formats: `firefox`, `json`, `csv`

## License

MIT License - see [LICENSE](LICENSE) file for details.
