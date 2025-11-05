mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use passmngr::{
    app::{App, Mode},
    export::{export_to_file, ExportFormat},
    import::import_from_file,
    model::Vault,
    storage::VaultFile,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "passmngr")]
#[command(about = "A fast, minimal TUI password manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Export passwords to a file (⚠️ PLAINTEXT!)
    Export {
        /// Format: firefox, json, or csv
        #[arg(value_name = "FORMAT")]
        format: String,

        /// Output file path
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
    /// Import passwords from a file
    Import {
        /// Input file path
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// Skip duplicate entries
        #[arg(short, long)]
        skip_duplicates: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle CLI commands
    if let Some(command) = cli.command {
        return handle_cli_command(command);
    }

    // No command - run TUI
    run_tui()
}

fn handle_cli_command(command: Commands) -> Result<()> {
    let vault_path = VaultFile::default_path()?;

    match command {
        Commands::Export { format, path } => {
            // Load vault
            let password = prompt_password("Enter master password: ")?;
            let vault = VaultFile::load(&vault_path, &password)?;

            // Parse format
            let export_format = ExportFormat::parse_format(&format)
                .ok_or_else(|| anyhow::anyhow!("Invalid format. Use: firefox, json, or csv"))?;

            // Export
            export_to_file(&vault, &path, export_format)?;

            println!(
                "✓ Exported {} entries to {}",
                vault.entries.len(),
                path.display()
            );
            println!("⚠️  WARNING: File contains PLAINTEXT passwords!");
            println!("   Delete it after use: rm {}", path.display());

            Ok(())
        }
        Commands::Import {
            path,
            skip_duplicates,
        } => {
            // Load vault
            let password = prompt_password("Enter master password: ")?;
            let mut vault = VaultFile::load(&vault_path, &password)?;

            // Preview import
            let preview = import_from_file(&path, &vault)?;

            println!("Import Preview:");
            println!("  Total entries: {}", preview.total_entries);
            println!("  New entries: {}", preview.new_entries);
            println!("  Duplicates: {}", preview.duplicates.len());

            if !preview.duplicates.is_empty() {
                println!("\nDuplicates found:");
                for dup in &preview.duplicates {
                    println!(
                        "  - {} ({}) matches existing '{}'",
                        dup.imported_name, dup.imported_username, dup.existing_name
                    );
                }

                if !skip_duplicates {
                    println!("\nUse --skip-duplicates to skip them, or press Enter to import all (may create duplicates)");
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                }
            }

            // Import entries
            let mut imported_count = 0;
            for imported_entry in preview.entries {
                // Check if it's a duplicate
                let is_duplicate = preview.duplicates.iter().any(|d| {
                    d.imported_name == imported_entry.name
                        && d.imported_username == imported_entry.username
                });

                if is_duplicate && skip_duplicates {
                    continue;
                }

                vault.add_entry(imported_entry.to_entry());
                imported_count += 1;
            }

            // Save vault
            VaultFile::save(&vault_path, &vault, &password)?;

            println!("✓ Imported {} entries", imported_count);
            if skip_duplicates && !preview.duplicates.is_empty() {
                println!("  Skipped {} duplicates", preview.duplicates.len());
            }

            Ok(())
        }
    }
}

fn run_tui() -> Result<()> {
    // Get vault path
    let vault_path = VaultFile::default_path()?;

    // Check if vault exists
    let (vault, password) = if VaultFile::exists(&vault_path) {
        // Prompt for password and load vault
        let password = prompt_password("Enter master password: ")?;
        match VaultFile::load(&vault_path, &password) {
            Ok(vault) => (vault, password),
            Err(e) => {
                eprintln!("Failed to unlock vault: {}", e);
                eprintln!("Incorrect password or corrupted vault.");
                std::process::exit(1);
            }
        }
    } else {
        // Create new vault
        println!(
            "No vault found. Creating new vault at: {}",
            vault_path.display()
        );
        let password = prompt_new_password()?;
        let vault = Vault::new();

        // Save the empty vault
        VaultFile::save(&vault_path, &vault, &password)?;
        println!("Vault created successfully!");

        (vault, password)
    };

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new(vault_path, password, vault);

    // Run app
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

/// Main application loop
fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        // Handle pending save operation after drawing
        // This ensures "Saving..." status is visible before the blocking operation
        if app.pending_save {
            app.save()?;
        }

        if app.should_quit {
            break;
        }

        if event::poll(std::time::Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                handle_key_event(app, key)?;
            }
        }

        // Clear status message after a short delay
        // (In a real implementation, you'd want a timer for this)
    }

    Ok(())
}

/// Handle keyboard input based on current mode
fn handle_key_event(app: &mut App, key: event::KeyEvent) -> Result<()> {
    app.clear_status();

    match app.mode {
        Mode::Normal => handle_normal_mode(app, key)?,
        Mode::Search => handle_search_mode(app, key)?,
        Mode::Command => handle_command_mode(app, key)?,
        Mode::Detail => handle_detail_mode(app, key)?,
        Mode::Insert => handle_insert_mode(app, key)?,
    }

    Ok(())
}

/// Handle keys in Normal mode
fn handle_normal_mode(app: &mut App, key: event::KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('q') => {
            app.enter_command_mode();
            app.command_buffer.push('q');
        }
        KeyCode::Char(':') => {
            app.enter_command_mode();
        }
        KeyCode::Char('j') => app.move_down(),
        KeyCode::Char('k') => app.move_up(),
        KeyCode::Char('g') => app.jump_to_top(),
        KeyCode::Char('G') => app.jump_to_bottom(),
        KeyCode::Char('/') => app.enter_search_mode(),
        KeyCode::Char('n') => app.enter_insert_mode(),
        KeyCode::Char('e') => app.enter_edit_mode(),
        KeyCode::Char('d') => {
            if let Some(entry) = app.delete_selected() {
                app.set_status(format!("Deleted entry '{}'", entry.name));
            }
        }
        KeyCode::Char('y') => app.copy_password_to_clipboard()?,
        KeyCode::Char('Y') => app.copy_username_to_clipboard()?,
        KeyCode::Enter => {
            app.mode = Mode::Detail;
        }
        KeyCode::Esc => {
            app.search_query.clear();
            app.update_search();
        }
        _ => {}
    }

    Ok(())
}

/// Handle keys in Search mode
fn handle_search_mode(app: &mut App, key: event::KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.update_search();
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.update_search();
        }
        KeyCode::Enter => {
            // Keep filter, exit search mode
            app.mode = Mode::Normal;
        }
        KeyCode::Esc => {
            // Clear filter, exit search mode
            app.exit_search_mode();
        }
        _ => {}
    }

    Ok(())
}

/// Handle keys in Command mode
fn handle_command_mode(app: &mut App, key: event::KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char(c) => {
            app.command_buffer.push(c);
            app.reset_completion();
        }
        KeyCode::Backspace => {
            app.command_buffer.pop();
            app.reset_completion();
        }
        KeyCode::Tab => {
            app.autocomplete_command();
        }
        KeyCode::Enter => {
            app.execute_command()?;
        }
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.command_buffer.clear();
        }
        _ => {}
    }

    Ok(())
}

/// Handle keys in Detail mode
fn handle_detail_mode(app: &mut App, key: event::KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.mode = Mode::Normal;
        }
        KeyCode::Char('e') => {
            app.enter_edit_mode();
        }
        KeyCode::Char('y') => {
            app.copy_password_to_clipboard()?;
        }
        KeyCode::Char('Y') => {
            app.copy_username_to_clipboard()?;
        }
        _ => {}
    }

    Ok(())
}

/// Handle keys in Insert mode (for creating/editing entries)
fn handle_insert_mode(app: &mut App, key: event::KeyEvent) -> Result<()> {
    use crossterm::event::KeyModifiers;

    match key.code {
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.save_form();
        }
        KeyCode::Esc => {
            app.cancel_form();
        }
        KeyCode::Tab => {
            app.focused_field = app.focused_field.next();
        }
        KeyCode::BackTab => {
            app.focused_field = app.focused_field.prev();
        }
        KeyCode::Char(c) => {
            let field_value = app.get_field_value_mut(app.focused_field);
            field_value.push(c);
        }
        KeyCode::Backspace => {
            let field_value = app.get_field_value_mut(app.focused_field);
            field_value.pop();
        }
        _ => {}
    }

    Ok(())
}

/// Prompt for password (without echo)
fn prompt_password(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::Write::flush(&mut io::stdout())?;
    let password = rpassword::read_password()?;
    Ok(password)
}

/// Prompt for new password with confirmation
fn prompt_new_password() -> Result<String> {
    loop {
        let password1 = prompt_password("Enter new master password: ")?;
        if password1.len() < 8 {
            eprintln!("Password must be at least 8 characters long.");
            continue;
        }

        let password2 = prompt_password("Confirm master password: ")?;
        if password1 == password2 {
            return Ok(password1);
        } else {
            eprintln!("Passwords do not match. Try again.");
        }
    }
}
