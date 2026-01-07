# vqx - Vantiq CLI Wrapper

A safe, feature-rich Rust wrapper for the Vantiq CLI.

## Overview

vqx provides workflow automation, safety guards, and developer-friendly features around the underlying Vantiq CLI.

**Based on**: CLI Reference Guide PDF from Vantiq

## Features

### Phase 1 (Implemented)

| Command | Description | PDF Reference |
|---------|-------------|---------------|
| `doctor` | Check environment prerequisites | Prerequisites, Installation sections |
| `profile` | Manage connection profiles | Profile, Command Line Options sections |
| `passthrough` | Direct CLI access | All commands |

### Phase 2 (Implemented)

| Command | Description | PDF Reference |
|---------|-------------|---------------|
| `export` | Export resources with JSON normalization | Export section |
| `import` | Import resources with safety confirmations | Import section |

**Key Features:**
- JSON normalization for git-friendly diffs (sorted keys, stable arrays, timestamp removal)
- Confirmation prompts before destructive operations
- Progress indicators and detailed output

### Phase 3 (Implemented)

| Command | Description | PDF Reference |
|---------|-------------|---------------|
| `diff` | Compare resources between environments or directories | - |
| `sync pull` | Export from remote to local with normalization | Export section |
| `sync push` | Import to remote with diff preview and confirmation | Import section |

**Key Features:**
- Compare profiles (remote) or directories (local)
- Automatic export and normalization for comparison
- Diff preview before push operations
- Confirmation prompts for safety

### Phase 4+ (Planned)

- `safe-delete` - Destructive operations with confirmation and backup
- `promote` - Workflow: export -> diff -> confirm -> import -> test
- `run` - Test suites and procedures

## Prerequisites

From CLI Reference Guide PDF, "Prerequisites" section:

> The Vantiq CLI is a Java (Groovy) application and requires an installation of Java 11.

- Java 11 or later
- Vantiq CLI installed and in PATH

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
./target/release/vqx --help
```

## Quick Start

### 1. Check Environment

```bash
# Verify Java and CLI are properly installed
vqx doctor

# Verbose output with PDF references
vqx doctor --verbose
```

### 2. Create a Profile

```bash
# Interactive setup
vqx profile init

# Or manual setup
vqx profile set myprofile \
    --url https://dev.vantiq.com \
    --token YOUR_ACCESS_TOKEN \
    --description "Development environment"
```

**PDF Note**: "public clouds and any server using keycloak access require use of the token option"

### 3. Use the CLI

```bash
# List all types using a profile
vqx --profile myprofile passthrough list types

# Export metadata
vqx --profile myprofile passthrough export metadata -d ./export
```

## Configuration

### Profile Storage

Profiles are stored in TOML format at:
- macOS/Linux: `~/.config/vqx/profiles.toml`
- Windows: `%APPDATA%\vqx\profiles.toml`

### Global Configuration

Configuration file location:
- macOS/Linux: `~/.config/vqx/config.toml`
- Windows: `%APPDATA%\vqx\config.toml`

See [examples/config.toml](examples/config.toml) for a sample configuration.

## CLI Usage

### Global Options

```
--cli <path>      Path to Vantiq CLI executable (default: vantiq)
--profile <name>  Profile to use for connection
--config <path>   Path to vqx config file
--verbose         Enable verbose output
--quiet           Suppress non-essential output
--output <fmt>    Output format: text, json, csv
```

### Commands

#### doctor

Check environment and CLI prerequisites.

```bash
vqx doctor                    # Full check
vqx doctor --java-only        # Only check Java
vqx doctor --cli-only         # Only check CLI
vqx doctor --test-connection  # Also test server connection
```

#### profile

Manage connection profiles.

```bash
vqx profile list              # List all profiles
vqx profile show myprofile    # Show profile details
vqx profile init              # Interactive profile creation
vqx profile set <name> ...    # Create/update a profile
vqx profile delete <name>     # Delete a profile
vqx profile default <name>    # Set default profile
vqx profile export <file>     # Export profiles to file
vqx profile import <file>     # Import profiles from file
```

Profile options (based on PDF "Command Line Options"):

| Option | PDF Flag | Description |
|--------|----------|-------------|
| `--url` | `-b` | Base URL (default: https://dev.vantiq.com) |
| `--username` | `-u` | Username (Edge servers only) |
| `--password` | `-p` | Password |
| `--token` | `-t` | Access token (recommended for public clouds) |
| `--namespace` | `-n` | Target namespace (username/password only) |
| `--trust-ssl` | `-trust` | Trust SSL certificates |

#### passthrough

Pass commands directly to the underlying CLI.

```bash
vqx passthrough list types
vqx passthrough find procedures MyProc
vqx passthrough export metadata -d ./export
vqx --profile prod passthrough run procedure Utils.getNamespaceAndProfiles
```

#### export

Export resources from Vantiq with JSON normalization for git-friendly diffs.

```bash
# Export metadata (default)
vqx export -d ./export

# Export with specific type
vqx export metadata -d ./export
vqx export data -d ./export

# Export project
vqx export project --project MyProject -d ./export

# Export with chunking (for large exports)
vqx export metadata -d ./export --chunk 5000

# Export specific types only
vqx export metadata --include types --include procedures

# Disable JSON normalization
vqx export metadata -d ./export --normalize false
```

Export options (based on PDF "Export" section):

| Option | PDF Flag | Description |
|--------|----------|-------------|
| `-d, --directory` | `-d` | Output directory |
| `--chunk` | `-chunk` | Chunk size for large exports |
| `--include` | `-include` | Types to include |
| `--exclude` | `-exclude` | Types to exclude |
| `--until` | `-until` | Export data until timestamp |
| `--ignore-errors` | `-ignoreErrors` | Ignore errors during export |
| `--normalize` | (vqx extension) | Normalize JSON for git diffs (default: true) |

**JSON Normalization** (vqx extension):
- Sorts object keys alphabetically
- Stabilizes array ordering by `name` or `ars_version`
- Removes volatile timestamps (`ars_createdAt`, `ars_modifiedAt`, etc.)
- Consistent indentation (2 spaces)

#### import

Import resources to Vantiq with safety confirmations.

```bash
# Import metadata (with confirmation prompt)
vqx import metadata -d ./export

# Import data
vqx import data -d ./data

# Import with chunking
vqx import metadata -d ./export --chunk 5000

# Import specific types only
vqx import metadata --include types --exclude rules

# Skip confirmation (for CI/CD)
vqx import metadata -d ./export --yes
```

Import options (based on PDF "Import" section):

| Option | PDF Flag | Description |
|--------|----------|-------------|
| `-d, --directory` | `-d` | Input directory |
| `--chunk` | `-chunk` | Chunk size for large imports |
| `--include` | `-include` | Types to include |
| `--exclude` | `-exclude` | Types to exclude |
| `--ignore` | `-ignore` | Resource types to ignore |
| `-y, --yes` | (vqx extension) | Skip confirmation prompt |

**Safety Features** (vqx extension):
- Confirmation prompt before import (prevents accidental overwrites)
- File count preview
- Server and profile display

#### diff

Compare resources between two sources (profiles or directories).

```bash
# Compare two directories
vqx diff ./local-export ./other-export

# Compare a profile with a local directory
vqx diff my-profile ./local-export

# Compare two profiles (remote-to-remote)
vqx diff dev-profile prod-profile

# Show full diff output
vqx diff ./source ./target --full

# Filter by resource type
vqx diff ./source ./target --resource types --resource procedures
```

Diff options:

| Option | Description |
|--------|-------------|
| `--full` | Show complete diff output (not just summary) |
| `--resource` | Filter to specific resource types |

**Features:**
- Automatically exports from profiles for comparison
- JSON normalization ensures accurate comparisons
- Shows added, removed, and modified resources
- Color-coded output for easy reading

#### sync

Synchronize resources between local directories and Vantiq servers.

**sync pull** - Export from remote to local:

```bash
# Pull from server to local directory
vqx sync pull -d ./local

# Force overwrite (skip confirmation)
vqx sync pull -d ./local --force
```

**sync push** - Import from local to remote:

```bash
# Push with diff preview and confirmation
vqx sync push -d ./local

# Dry run - show what would be pushed
vqx sync push -d ./local --dry-run

# Skip confirmation (for CI/CD)
vqx sync push -d ./local --yes
```

Sync options:

| Subcommand | Option | Description |
|------------|--------|-------------|
| `pull` | `-d, --directory` | Local directory to sync to |
| `pull` | `--force` | Force overwrite without confirmation |
| `push` | `-d, --directory` | Local directory to sync from |
| `push` | `--dry-run` | Show changes without applying |
| `push` | `-y, --yes` | Skip confirmation prompt |

**Features:**
- Automatic diff preview before push
- JSON normalization on pull
- Confirmation prompts for safety
- Progress indicators

## PDF Mapping

### Connection Options

| vqx | PDF CLI Flag | Description |
|-----|--------------|-------------|
| `--profile` | `-s` | Profile name |
| Profile.url | `-b` | Base URL |
| Profile.username | `-u` | Username |
| Profile.password | `-p` | Password |
| Profile.token | `-t` | Access token |
| Profile.namespace | `-n` | Target namespace |
| Profile.trust_ssl | `-trust` | Trust SSL |

### Important PDF Notes

1. **Token vs Password**: "If a password is specified, it is used instead of the token."

2. **Token requirement**: "public clouds and any server using keycloak access require use of the token option"

3. **Username/Password limitation**: "username/password can only be used for Edge servers"

4. **Namespace limitation**: "the namespace option can only be used with username/password; it cannot be used with long-lived access tokens"

5. **Deprecated commands**: "the execute command is deprecated in favor of the run procedure command as of release 1.37"

## Security

### Credential Storage

vqx supports secure credential storage:

1. **Keyring** (default): Uses system keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service)

2. **Encrypted file**: Fallback when keyring is unavailable (uses `age` encryption)

3. **Plain text**: Not recommended, but available for development

### Sensitive Data Handling

- Passwords and tokens are never logged
- CLI arguments are masked in verbose output
- Secure storage is used by default when available

## Development

### Building

```bash
cargo build
cargo test
```

### Project Structure

```
src/
  main.rs           # Entry point
  cli.rs            # CLI command definitions (clap)
  config.rs         # Global configuration
  error.rs          # Error types
  normalizer.rs     # JSON normalization for git-friendly diffs
  profile.rs        # Profile management
  underlying.rs     # CLI execution layer (PDF-based)
  commands/
    mod.rs
    diff.rs         # Environment comparison
    doctor.rs       # Environment checks
    export.rs       # Export with normalization
    import.rs       # Import with safety confirmations
    passthrough.rs  # Direct CLI access
    profile.rs      # Profile management
    sync.rs         # Bidirectional sync (pull/push)
```

### Adding New Commands

1. Define command in `src/cli.rs`
2. Create implementation in `src/commands/`
3. Add to dispatch in `src/main.rs`
4. Document PDF mapping in code comments

## License

MIT

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
