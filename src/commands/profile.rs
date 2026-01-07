//! Profile command implementation
//!
//! Manages vqx connection profiles that map to the underlying CLI's
//! connection options.
//!
//! Based on: CLI Reference Guide PDF
//! - "Profile" section: Profile file format and options
//! - "Command Line Options" section: -s, -b, -u, -p, -t, -n, -trust

use crate::cli::{
    OutputFormat, ProfileCommands, ProfileDefaultArgs, ProfileDeleteArgs, ProfileExportArgs,
    ProfileImportArgs, ProfileInitArgs, ProfileSetArgs, ProfileShowArgs,
};
use crate::error::{Result, VqxError};
use crate::profile::{
    Profile, ProfileManager, ProfileStore, DEFAULT_PROFILE_NAME, DEFAULT_VANTIQ_URL,
};
use console::style;
use dialoguer::{Confirm, Input, Password, Select};
use std::fs;
use tracing::info;

/// Run profile subcommand
pub async fn run(cmd: &ProfileCommands, output_format: OutputFormat) -> Result<()> {
    match cmd {
        ProfileCommands::List => list(output_format).await,
        ProfileCommands::Show(args) => show(args, output_format).await,
        ProfileCommands::Set(args) => set(args).await,
        ProfileCommands::Delete(args) => delete(args).await,
        ProfileCommands::Default(args) => set_default(args).await,
        ProfileCommands::Import(args) => import(args).await,
        ProfileCommands::Export(args) => export(args).await,
        ProfileCommands::Init(args) => init(args).await,
    }
}

/// List all profiles
async fn list(output_format: OutputFormat) -> Result<()> {
    let manager = ProfileManager::new()?;
    let store = manager.store();
    let names = store.list_names();
    let default_name = &store.default_profile;

    match output_format {
        OutputFormat::Json => {
            let json = serde_json::json!({
                "default": default_name,
                "profiles": names,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Csv => {
            println!("name,is_default");
            for name in &names {
                println!("{},{}", name, name == default_name);
            }
        }
        OutputFormat::Text => {
            println!();
            println!("{}", style("Configured Profiles").bold().cyan());
            println!("{}", style("─".repeat(40)).dim());

            if names.is_empty() {
                println!("{}", style("No profiles configured.").dim());
                println!();
                println!(
                    "Run '{}' to create your first profile.",
                    style("vqx profile init").green()
                );
            } else {
                for name in &names {
                    let marker = if name == default_name {
                        style(" (default)").green()
                    } else {
                        style("").dim()
                    };
                    println!("  • {}{}", style(name).bold(), marker);
                }
            }
            println!();
        }
    }

    Ok(())
}

/// Show profile details
async fn show(args: &ProfileShowArgs, output_format: OutputFormat) -> Result<()> {
    let manager = ProfileManager::new()?;
    let profile = manager.store().get(&args.name)?;

    // Mask secrets unless explicitly requested
    let display_profile = if args.show_secrets {
        profile.clone()
    } else {
        profile.masked()
    };

    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&display_profile)?);
        }
        OutputFormat::Csv => {
            println!("field,value");
            println!("url,{}", display_profile.url);
            if let Some(ref u) = display_profile.username {
                println!("username,{}", u);
            }
            if let Some(ref p) = display_profile.password {
                println!("password,{}", p);
            }
            if let Some(ref t) = display_profile.token {
                println!("token,{}", t);
            }
            if let Some(ref n) = display_profile.namespace {
                println!("namespace,{}", n);
            }
            println!("trust_ssl,{}", display_profile.trust_ssl);
        }
        OutputFormat::Text => {
            println!();
            println!(
                "{} {}",
                style("Profile:").bold().cyan(),
                style(&args.name).bold()
            );
            println!("{}", style("─".repeat(40)).dim());
            println!("  URL:        {}", display_profile.url);

            if let Some(ref u) = display_profile.username {
                println!("  Username:   {}", u);
            }
            if let Some(ref p) = display_profile.password {
                println!("  Password:   {}", p);
            }
            if let Some(ref t) = display_profile.token {
                println!("  Token:      {}", t);
            }
            if let Some(ref n) = display_profile.namespace {
                println!("  Namespace:  {}", n);
            }
            println!("  Trust SSL:  {}", display_profile.trust_ssl);

            if let Some(ref desc) = display_profile.description {
                println!("  Note:       {}", desc);
            }

            println!();
            println!(
                "{}",
                style("Auth type: ").dim().to_string() + profile.auth_type()
            );

            // Show PDF reference for auth type
            match profile.auth_type() {
                "access token" => {
                    println!(
                        "{}",
                        style(
                            "  (PDF: 'public clouds and any server using keycloak access require use of the token option')"
                        )
                        .dim()
                    );
                }
                "username/password" => {
                    println!(
                        "{}",
                        style("  (PDF: 'username/password can only be used for Edge servers')")
                            .dim()
                    );
                }
                _ => {}
            }
            println!();
        }
    }

    Ok(())
}

/// Create or update a profile
async fn set(args: &ProfileSetArgs) -> Result<()> {
    let mut manager = ProfileManager::new()?;

    // Get existing profile or create new one
    let mut profile = manager.store().get(&args.name).cloned().unwrap_or_default();

    // Update fields if provided
    if let Some(ref url) = args.url {
        profile.url = url.clone();
    }
    if let Some(ref username) = args.username {
        profile.username = Some(username.clone());
    }
    if let Some(ref password) = args.password {
        if args.secure {
            // Store in secure storage
            manager.set_secret(&args.name, "password", password)?;
            profile.use_secure_storage = true;
            profile.password = None; // Don't store in file
        } else {
            profile.password = Some(password.clone());
        }
    }
    if let Some(ref token) = args.token {
        if args.secure {
            manager.set_secret(&args.name, "token", token)?;
            profile.use_secure_storage = true;
            profile.token = None;
        } else {
            profile.token = Some(token.clone());
        }
    }
    if let Some(ref namespace) = args.namespace {
        profile.namespace = Some(namespace.clone());
    }
    if args.trust_ssl {
        profile.trust_ssl = true;
    }
    if let Some(ref desc) = args.description {
        profile.description = Some(desc.clone());
    }

    // Validate
    profile.validate()?;

    // Save
    manager.store_mut().set(&args.name, profile);
    manager.save()?;

    println!(
        "{} Profile '{}' saved.",
        style("✓").green(),
        style(&args.name).bold()
    );

    Ok(())
}

/// Delete a profile
async fn delete(args: &ProfileDeleteArgs) -> Result<()> {
    let mut manager = ProfileManager::new()?;

    // Check if profile exists
    if !manager.store().exists(&args.name) {
        return Err(VqxError::ProfileNotFound {
            name: args.name.clone(),
        });
    }

    // Confirm deletion
    if !args.force {
        let confirmed = Confirm::new()
            .with_prompt(format!("Delete profile '{}'?", args.name))
            .default(false)
            .interact()
            .map_err(|e| VqxError::Other(e.to_string()))?;

        if !confirmed {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Delete secrets from secure storage
    manager.delete_secret(&args.name, "password")?;
    manager.delete_secret(&args.name, "token")?;

    // Delete from store
    manager.store_mut().remove(&args.name);
    manager.save()?;

    println!(
        "{} Profile '{}' deleted.",
        style("✓").green(),
        style(&args.name).bold()
    );

    Ok(())
}

/// Set default profile
async fn set_default(args: &ProfileDefaultArgs) -> Result<()> {
    let mut manager = ProfileManager::new()?;

    manager.store_mut().set_default(&args.name)?;
    manager.save()?;

    println!(
        "{} Default profile set to '{}'.",
        style("✓").green(),
        style(&args.name).bold()
    );

    Ok(())
}

/// Import profiles from file
async fn import(args: &ProfileImportArgs) -> Result<()> {
    let content = fs::read_to_string(&args.file).map_err(|_| VqxError::FileReadFailed {
        path: args.file.display().to_string(),
    })?;

    let imported_store = ProfileStore::from_toml(&content)?;
    let mut manager = ProfileManager::new()?;

    let mut count = 0;
    for (name, profile) in imported_store.profiles {
        if manager.store().exists(&name) && !args.overwrite {
            println!(
                "{} Skipping '{}' (already exists, use --overwrite to replace)",
                style("⚠").yellow(),
                name
            );
            continue;
        }
        manager.store_mut().set(&name, profile);
        count += 1;
    }

    manager.save()?;

    println!(
        "{} Imported {} profile(s) from '{}'.",
        style("✓").green(),
        count,
        args.file.display()
    );

    Ok(())
}

/// Export profiles to file
async fn export(args: &ProfileExportArgs) -> Result<()> {
    let manager = ProfileManager::new()?;
    let store = manager.store();

    // Mask secrets unless explicitly included
    let export_store = if args.include_secrets {
        store.clone()
    } else {
        let mut masked_store = ProfileStore::new();
        masked_store.default_profile = store.default_profile.clone();
        for (name, profile) in &store.profiles {
            masked_store.set(name.clone(), profile.masked());
        }
        masked_store
    };

    let content = export_store.to_toml()?;
    fs::write(&args.file, content).map_err(|_| VqxError::FileWriteFailed {
        path: args.file.display().to_string(),
    })?;

    println!(
        "{} Exported profiles to '{}'.",
        style("✓").green(),
        args.file.display()
    );

    if !args.include_secrets {
        println!(
            "{}",
            style("Note: Secrets were masked. Use --include-secrets to export credentials.").dim()
        );
    }

    Ok(())
}

/// Interactive profile creation
async fn init(args: &ProfileInitArgs) -> Result<()> {
    println!();
    println!("{}", style("vqx Profile Setup").bold().cyan());
    println!("{}", style("─".repeat(40)).dim());
    println!();
    println!("This wizard will help you create a connection profile.");
    println!(
        "{}",
        style("PDF Reference: 'Profile' and 'Command Line Options' sections").dim()
    );
    println!();

    // Profile name
    let default_name = args
        .name
        .clone()
        .unwrap_or_else(|| DEFAULT_PROFILE_NAME.to_string());

    let name: String = Input::new()
        .with_prompt("Profile name")
        .default(default_name)
        .interact_text()
        .map_err(|e| VqxError::Other(e.to_string()))?;

    // Server URL
    let url: String = Input::new()
        .with_prompt("Vantiq server URL")
        .default(DEFAULT_VANTIQ_URL.to_string())
        .interact_text()
        .map_err(|e| VqxError::Other(e.to_string()))?;

    // Authentication type
    println!();
    println!("{}", style("Authentication").bold());
    println!(
        "{}",
        style(
            "PDF Note: 'public clouds and any server using keycloak access require use of the token option'"
        )
        .dim()
    );
    println!(
        "{}",
        style("         'username/password can only be used for Edge servers'").dim()
    );
    println!();

    let auth_options = vec![
        "Access Token (recommended for public clouds)",
        "Username/Password (Edge servers only)",
    ];

    let auth_choice = Select::new()
        .with_prompt("Authentication method")
        .items(&auth_options)
        .default(0)
        .interact()
        .map_err(|e| VqxError::Other(e.to_string()))?;

    let mut profile = Profile::new(&url);

    match auth_choice {
        0 => {
            // Token authentication
            let token: String = Password::new()
                .with_prompt("Access token (long-lived token from Vantiq)")
                .interact()
                .map_err(|e| VqxError::Other(e.to_string()))?;
            profile.token = Some(token);
        }
        1 => {
            // Username/password
            let username: String = Input::new()
                .with_prompt("Username")
                .interact_text()
                .map_err(|e| VqxError::Other(e.to_string()))?;

            let password: String = Password::new()
                .with_prompt("Password")
                .interact()
                .map_err(|e| VqxError::Other(e.to_string()))?;

            profile.username = Some(username);
            profile.password = Some(password);

            // Namespace option (only for username/password)
            println!();
            println!(
                "{}",
                style("PDF Note: 'the namespace option can only be used with username/password'")
                    .dim()
            );

            let use_namespace = Confirm::new()
                .with_prompt("Specify a target namespace?")
                .default(false)
                .interact()
                .map_err(|e| VqxError::Other(e.to_string()))?;

            if use_namespace {
                let namespace: String = Input::new()
                    .with_prompt("Namespace")
                    .interact_text()
                    .map_err(|e| VqxError::Other(e.to_string()))?;
                profile.namespace = Some(namespace);
            }
        }
        _ => unreachable!(),
    }

    // Trust SSL
    let trust_ssl = Confirm::new()
        .with_prompt("Trust SSL certificates? (PDF: '-trust' flag)")
        .default(false)
        .interact()
        .map_err(|e| VqxError::Other(e.to_string()))?;

    profile.trust_ssl = trust_ssl;

    // Store securely
    #[cfg(feature = "keyring-storage")]
    let use_secure = Confirm::new()
        .with_prompt("Store credentials in secure storage (keyring)?")
        .default(true)
        .interact()
        .map_err(|e| VqxError::Other(e.to_string()))?;

    #[cfg(not(feature = "keyring-storage"))]
    let use_secure = false;

    // Description
    let description: String = Input::new()
        .with_prompt("Description (optional)")
        .allow_empty(true)
        .interact_text()
        .map_err(|e| VqxError::Other(e.to_string()))?;

    if !description.is_empty() {
        profile.description = Some(description);
    }

    // Validate
    profile.validate()?;

    // Save
    let mut manager = ProfileManager::new()?;

    if use_secure {
        if let Some(ref password) = profile.password {
            manager.set_secret(&name, "password", password)?;
            profile.password = None;
            profile.use_secure_storage = true;
        }
        if let Some(ref token) = profile.token {
            manager.set_secret(&name, "token", token)?;
            profile.token = None;
            profile.use_secure_storage = true;
        }
    }

    // Set as default if it's the first profile
    let is_first = manager.store().list_names().is_empty();

    manager.store_mut().set(&name, profile);

    if is_first {
        manager.store_mut().set_default(&name)?;
    }

    manager.save()?;

    println!();
    println!("{}", style("─".repeat(40)).dim());
    println!(
        "{} Profile '{}' created successfully!",
        style("✓").green(),
        style(&name).bold()
    );

    if is_first {
        println!("  This profile has been set as the default.");
    }

    println!();
    println!("Test your connection with:");
    println!(
        "  {}",
        style(format!("vqx --profile {} doctor --test-connection", name)).cyan()
    );
    println!();

    Ok(())
}
