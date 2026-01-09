# vqx - Vantiq CLI Wrapper

A safe, feature-rich Rust wrapper for the Vantiq CLI.

## Overview

vqx provides workflow automation, safety guards, and developer-friendly features around the underlying Vantiq CLI.

**Based on**: [Vantiq CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/)

## Features

- **Profile Management** - Secure credential storage with keyring support
- **Export/Import** - JSON normalization for git-friendly diffs
- **Environment Comparison** - Diff between profiles or directories
- **Synchronization** - Bidirectional sync with pull/push workflow
- **Safe Operations** - Backup and confirmation for destructive operations
- **Environment Promotion** - Automated workflow for cross-environment deployment
- **Test Execution** - Run tests, test suites, and procedures
- **Direct CLI Access** - Passthrough to underlying Vantiq CLI

## Prerequisites

From [Vantiq CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/):

> The Vantiq CLI is a Java (Groovy) application and requires an installation of Java 11.

- Java 11 or later
- Vantiq CLI installed and in PATH

## Installation

### From GitHub Releases (Recommended)

Download the pre-built binary for your platform from [GitHub Releases](https://github.com/shun-sonohara/vqx/releases).

| Platform | File |
|----------|------|
| Linux x86_64 | `vqx-linux-x86_64.tar.gz` |
| macOS Intel | `vqx-macos-x86_64.tar.gz` |
| macOS Apple Silicon | `vqx-macos-aarch64.tar.gz` |
| Windows x86_64 | `vqx-windows-x86_64.zip` |

#### Linux / macOS

```bash
# Download and extract (example for macOS Apple Silicon)
curl -LO https://github.com/shun-sonohara/vqx/releases/latest/download/vqx-macos-aarch64.tar.gz
tar xzf vqx-macos-aarch64.tar.gz

# Move to a directory in your PATH
sudo mv vqx /usr/local/bin/

# Verify installation
vqx --version
```

#### macOS Security (Gatekeeper)

If macOS blocks the binary, remove the quarantine attribute:

```bash
xattr -d com.apple.quarantine /usr/local/bin/vqx
```

#### Windows

1. Download `vqx-windows-x86_64.zip` from [Releases](https://github.com/shun-sonohara/vqx/releases)
2. Extract the ZIP file
3. Move `vqx.exe` to a directory in your PATH

### From Source

```bash
cargo install --path .
# Or build manually
cargo build --release
./target/release/vqx --help
```

## Quick Start

### 1. Check Environment

```bash
vqx doctor
```

### 2. Create a Profile

```bash
# Interactive setup
vqx profile init

# Or manual setup
vqx profile set dev \
    --url https://dev.vantiq.com \
    --token YOUR_ACCESS_TOKEN
```

### 3. Use Commands

```bash
# Export resources
vqx -s dev export -d ./export

# List types (direct CLI access)
vqx -s dev list types
```

## Configuration

### Profile Storage

Profiles are stored at:
- macOS/Linux: `~/.config/vqx/profiles.toml`
- Windows: `%APPDATA%\vqx\profiles.toml`

```toml
default_profile = "dev"

[profiles.dev]
url = "https://dev.vantiq.com"
token = "YOUR_ACCESS_TOKEN"
trust_ssl = false
description = "Development environment"
use_secure_storage = false  # true = store in keyring
```

**Authentication Options** (from [CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/)):

| Field | CLI Flag | Description |
|-------|----------|-------------|
| `url` | `-b` | Vantiq server URL |
| `token` | `-t` | Access token (required for public clouds) |
| `username` | `-u` | Username (Edge servers only) |
| `password` | `-p` | Password (Edge servers only) |
| `namespace` | `-n` | Target namespace (username/password only) |
| `trust_ssl` | `-trust` | Trust SSL certificates |

### Global Configuration

Configuration file location:
- macOS/Linux: `~/.config/vqx/config.toml`
- Windows: `%APPDATA%\vqx\config.toml`

```toml
cli_path = "vantiq"
timeout_seconds = 120
max_retries = 3

[normalization]
sort_keys = true
sort_arrays = true
excluded_fields = [
    "ars_modifiedAt",
    "ars_createdAt",
    "ars_modifiedBy",
    "ars_createdBy",
    "_id",
    "ars_version",
]
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `VQX_CLI_PATH` | Path to Vantiq CLI executable |
| `VQX_PROFILE` | Default profile name |
| `VQX_CONFIG` | Path to config.toml |

## Commands

### Global Options

```
-s, --profile <name>  Profile to use for connection
--cli <path>          Path to Vantiq CLI executable
--config <path>       Path to config file
-v, --verbose         Enable verbose output
-q, --quiet           Suppress non-essential output
--output <format>     Output format: text, json, csv
```

---

### doctor

Check environment and CLI prerequisites.

```bash
vqx doctor                    # Full check
vqx doctor --java-only        # Only check Java
vqx doctor --cli-only         # Only check CLI
vqx doctor --test-connection  # Also test server connection
```

---

### profile

Manage connection profiles with secure credential storage.

```bash
# List all profiles
vqx profile list

# Show profile details
vqx profile show dev

# Interactive profile creation
vqx profile init

# Create/update profile
vqx profile set dev --url https://dev.vantiq.com --token YOUR_TOKEN

# Create profile with secure storage (keyring)
vqx profile set dev --url https://dev.vantiq.com --token YOUR_TOKEN --secure

# Set default profile
vqx profile default dev

# Delete profile
vqx profile delete dev

# Export/import profiles
vqx profile export profiles.toml
vqx profile import profiles.toml --overwrite
```

**Profile Options:**

| Option | CLI Flag | Description |
|--------|----------|-------------|
| `--url` | `-b` | Base URL |
| `--username` | `-u` | Username (Edge servers only) |
| `--password` | `-p` | Password |
| `--token` | `-t` | Access token (recommended) |
| `--namespace` | `-n` | Target namespace |
| `--trust-ssl` | `-trust` | Trust SSL certificates |
| `--secure` | - | Store credentials in keyring |

---

### export

Export resources from Vantiq with JSON normalization for git-friendly diffs.

```bash
# Export metadata (default)
vqx -s dev export -d ./export

# Export data
vqx -s dev export data -d ./data

# Export project
vqx -s dev export project --project MyProject -d ./export

# Export with chunking (for large exports)
vqx -s dev export -d ./export --chunk 5000

# Export specific types only
vqx -s dev export --include types --include procedures

# Exclude specific types
vqx -s dev export --exclude rules

# Disable JSON normalization
vqx -s dev export -d ./export --normalize false
```

**Export Options:**

| Option | CLI Flag | Description |
|--------|----------|-------------|
| `-d, --directory` | `-d` | Output directory |
| `--chunk` | `-chunk` | Chunk size for large exports |
| `--include` | `-include` | Types to include (repeatable) |
| `--exclude` | `-exclude` | Types to exclude (repeatable) |
| `--until` | `-until` | Export data until timestamp |
| `--ignore-errors` | `-ignoreErrors` | Continue on errors |
| `--normalize` | - | JSON normalization (default: true) |

**JSON Normalization:**
- Sorts object keys alphabetically
- Stabilizes array ordering by `name` field
- Removes volatile timestamps (`ars_createdAt`, `ars_modifiedAt`, etc.)
- Consistent 2-space indentation

---

### import

Import resources to Vantiq with safety confirmations.

```bash
# Import metadata (with confirmation)
vqx -s dev import -d ./export

# Import data
vqx -s dev import data -d ./data

# Import with chunking
vqx -s dev import -d ./export --chunk 5000

# Import specific types only
vqx -s dev import --include types

# Skip confirmation (for CI/CD)
vqx -s dev import -d ./export --yes
```

**Import Options:**

| Option | CLI Flag | Description |
|--------|----------|-------------|
| `-d, --directory` | `-d` | Input directory |
| `--chunk` | `-chunk` | Chunk size for large imports |
| `--include` | `-include` | Types to include |
| `--exclude` | `-exclude` | Types to exclude |
| `--ignore` | `-ignore` | Resource types to ignore |
| `-y, --yes` | - | Skip confirmation prompt |

---

### diff

Compare resources between two sources (profiles or directories).

```bash
# Compare two directories
vqx diff ./local ./other

# Compare profile with directory
vqx diff dev ./local

# Compare two profiles
vqx diff dev prod

# Show full diff output
vqx diff ./source ./target --full

# Filter by resource type
vqx diff ./source ./target --resource types --resource procedures
```

**Diff Options:**

| Option | Description |
|--------|-------------|
| `--full` | Show complete diff output |
| `--resource` | Filter to specific resource types (repeatable) |

**Features:**
- Automatically exports from profiles for comparison
- JSON normalization ensures accurate comparisons
- Color-coded output (green: added, red: removed, yellow: modified)

---

### sync

Synchronize resources between local directories and Vantiq servers.

#### sync pull

Export from remote to local directory.

```bash
# Pull from server
vqx -s dev sync pull -d ./local

# Force overwrite local changes
vqx -s dev sync pull -d ./local --force
```

#### sync push

Import from local to remote with diff preview.

```bash
# Push with diff preview and confirmation
vqx -s dev sync push -d ./local

# Dry run - show what would be pushed
vqx -s dev sync push -d ./local --dry-run

# Skip confirmation (for CI/CD)
vqx -s dev sync push -d ./local --yes
```

**Sync Options:**

| Subcommand | Option | Description |
|------------|--------|-------------|
| `pull` | `-d, --directory` | Local directory |
| `pull` | `--force` | Force overwrite |
| `push` | `-d, --directory` | Local directory |
| `push` | `--dry-run` | Preview changes only |
| `push` | `-y, --yes` | Skip confirmation |

---

### run

Run tests, test suites, or procedures on Vantiq.

#### run test

```bash
vqx -s dev run test MyTest
```

#### run testsuite

```bash
# Run test suite
vqx -s dev run testsuite MyTestSuite

# Start from specific test
vqx -s dev run testsuite MyTestSuite --start-from SpecificTest
```

#### run procedure

```bash
# Run procedure
vqx -s dev run procedure MyProcedure

# Run with parameters
vqx -s dev run procedure MyProcedure param1:value1 param2:value2
```

**Run Options:**

| Subcommand | Option | Description |
|------------|--------|-------------|
| `test` | `<name>` | Test name |
| `testsuite` | `<name>` | Test suite name |
| `testsuite` | `--start-from` | Start from specific test |
| `procedure` | `<name>` | Procedure name |
| `procedure` | `[params...]` | Parameters as `name:value` |

---

### safe-delete

Safely delete resources with backup and confirmation.

```bash
# Delete single resource
vqx -s dev safe-delete types MyType

# Delete with query (deleteMatching)
vqx -s dev safe-delete types '{"name": {"$regex": "Test.*"}}'

# Dry run - preview only
vqx -s dev safe-delete types MyType --dry-run

# Skip backup
vqx -s dev safe-delete types MyType --no-backup

# Skip confirmation
vqx -s dev safe-delete types MyType --yes

# Force delete over 100 items
vqx -s dev safe-delete types '{"obsolete": true}' --force
```

**Safe-Delete Options:**

| Option | Description |
|--------|-------------|
| `--dry-run` | Preview without deleting |
| `--no-backup` | Skip automatic backup |
| `-y, --yes` | Skip confirmation |
| `--force` | Allow deleting over 100 items |

**Safety Features:**
- Automatic backup to `~/.local/share/vqx/backups/`
- Confirmation prompt showing items to delete
- 100 item limit for deleteMatching (override with `--force`)
- Dry-run mode for safe preview

---

### promote

Promote resources from one environment to another.

```bash
# Basic promotion
vqx promote --from dev --to prod

# Skip diff display
vqx promote --from dev --to prod --no-diff

# Run test suite after promotion
vqx promote --from dev --to prod --testsuite SmokeTests

# Run procedure after promotion
vqx promote --from dev --to prod --procedure ValidateDeployment

# Skip confirmations (for CI/CD)
vqx promote --from dev --to prod --yes
```

**Promote Options:**

| Option | Description |
|--------|-------------|
| `--from` | Source profile |
| `--to` | Target profile |
| `--no-diff` | Skip diff display |
| `--no-test` | Skip post-promotion tests |
| `--testsuite` | Test suite to run after |
| `--procedure` | Procedure to run after |
| `-y, --yes` | Skip confirmations |

**Workflow:**
1. Export metadata from source
2. Compare with target (optional)
3. Confirm promotion
4. Import to target
5. Run tests (optional)

---

### Direct CLI Access

Any unrecognized command is passed directly to the underlying Vantiq CLI.

```bash
# List types
vqx -s dev list types

# Find resource
vqx -s dev find procedures MyProc

# Select with query
vqx -s dev select types

# Any other CLI command
vqx -s dev <command> [args...]
```

## CLI Reference Notes

From [Vantiq CLI Reference Guide](https://dev.vantiq.com/docs/system/cli/):

1. **Token vs Password**: "If a password is specified, it is used instead of the token."

2. **Token requirement**: "public clouds and any server using keycloak access require use of the token option"

3. **Username/Password limitation**: "username/password can only be used for Edge servers"

4. **Namespace limitation**: "the namespace option can only be used with username/password; it cannot be used with long-lived access tokens"

## Security

### Credential Storage

vqx supports multiple credential storage methods:

1. **Keyring** (recommended): System keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service)
2. **Plain text**: In profiles.toml (not recommended for production)

### Sensitive Data Handling

- Passwords and tokens are never logged
- CLI arguments are masked in verbose output
- Secure storage available via `--secure` flag

## Development

### Building

```bash
cargo build
cargo test
cargo build --release
```

### Project Structure

```
src/
  main.rs           # Entry point
  cli.rs            # CLI definitions (clap)
  config.rs         # Configuration
  error.rs          # Error types
  normalizer.rs     # JSON normalization
  profile.rs        # Profile management
  underlying.rs     # CLI execution layer
  commands/
    doctor.rs       # Environment checks
    profile.rs      # Profile management
    export.rs       # Export with normalization
    import.rs       # Import with confirmations
    diff.rs         # Environment comparison
    sync.rs         # Pull/push synchronization
    run.rs          # Test/procedure execution
    safe_delete.rs  # Safe deletion
    promote.rs      # Environment promotion
    external.rs     # Direct CLI passthrough
```

## Release Process

Releases are created manually via GitHub Actions.

### Creating a Release

1. Go to [Actions → Release](https://github.com/shun-sonohara/vqx/actions/workflows/auto-release.yml)
2. Click "Run workflow"
3. Select version type:
   - `patch` - Bug fixes (0.1.0 → 0.1.1)
   - `minor` - New features (0.1.0 → 0.2.0)
   - `major` - Breaking changes (0.1.0 → 1.0.0)
4. Click "Run workflow"

### CI/CD

| Workflow | Trigger | Description |
|----------|---------|-------------|
| CI | Pull requests | Format, lint, build, test |
| Release | Manual | Version bump + build + release |

## License

MIT

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
