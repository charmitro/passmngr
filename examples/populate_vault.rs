//! Example program to populate the vault with sample data for testing

use passmngr::{
    model::{Entry, Vault},
    storage::VaultFile,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let vault_path = VaultFile::default_path()?;

    // Create test vault
    let mut vault = Vault::new();

    // Add sample entries
    vault.add_entry(Entry::new(
        "GitHub Personal".to_string(),
        "user@example.com".to_string(),
        "correct-horse-battery-staple".to_string(),
        Some("https://github.com".to_string()),
        Some("Personal GitHub account for open source projects".to_string()),
        vec!["work".to_string(), "dev".to_string()],
    ));

    vault.add_entry(Entry::new(
        "GitLab Work".to_string(),
        "john.doe@company.com".to_string(),
        "super-secure-password-123".to_string(),
        Some("https://gitlab.company.com".to_string()),
        Some("Company GitLab instance".to_string()),
        vec!["work".to_string()],
    ));

    vault.add_entry(Entry::new(
        "AWS Console".to_string(),
        "admin@company.com".to_string(),
        "aws-super-secret-2024".to_string(),
        Some("https://console.aws.amazon.com".to_string()),
        None,
        vec!["work".to_string(), "cloud".to_string()],
    ));

    vault.add_entry(Entry::new(
        "Email Personal".to_string(),
        "user@gmail.com".to_string(),
        "gmail-password-456".to_string(),
        Some("https://gmail.com".to_string()),
        Some("Personal email account".to_string()),
        vec!["personal".to_string()],
    ));

    vault.add_entry(Entry::new(
        "Database Production".to_string(),
        "dbadmin".to_string(),
        "postgres-prod-pass-789".to_string(),
        None,
        Some("Production PostgreSQL database credentials".to_string()),
        vec!["work".to_string(), "database".to_string()],
    ));

    // Save with test password
    let password = "testpassword";
    VaultFile::save(&vault_path, &vault, password)?;

    println!("Vault populated successfully at: {}", vault_path.display());
    println!("Password: {}", password);
    println!("Added {} entries", vault.entries.len());

    Ok(())
}
