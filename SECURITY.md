# Security Policy

## Credential Storage

vqx provides multiple options for storing credentials, with secure storage as the default.

### Storage Options

1. **System Keyring (Default, Recommended)**
   - macOS: Keychain Access
   - Windows: Credential Manager
   - Linux: Secret Service (GNOME Keyring, KWallet)

   ```bash
   vqx profile set myprofile --token YOUR_TOKEN --secure
   ```

2. **Encrypted File Fallback**
   - Used when keyring is unavailable
   - Uses `age` encryption
   - Key derived from user password

3. **Plain Text (Not Recommended)**
   - Only for development/testing
   - Credentials stored in `~/.config/vqx/profiles.toml`
   - File permissions should be restricted (600)

### Best Practices

1. **Always use `--secure` flag** when setting credentials:
   ```bash
   vqx profile set production --token $VANTIQ_TOKEN --secure
   ```

2. **Use environment variables** for CI/CD:
   ```bash
   export VQX_PROFILE=ci
   # Or pass token directly via underlying CLI
   vqx passthrough -t $VANTIQ_TOKEN list types
   ```

3. **Restrict file permissions**:
   ```bash
   chmod 600 ~/.config/vqx/profiles.toml
   chmod 700 ~/.config/vqx/
   ```

## Sensitive Data Handling

### Logging

- Passwords and tokens are **never** logged
- CLI commands show masked arguments:
  ```
  Args: -b https://dev.vantiq.com -u user -p ******** list types
  ```

### Memory

- Credentials are not stored longer than necessary
- Rust's ownership model prevents accidental leaks

### Network

- All connections use HTTPS by default
- `--trust-ssl` should only be used for Edge servers with self-signed certificates

## Destructive Operations (Phase 4)

vqx wraps destructive CLI operations with safety guards:

### Protected Commands

Based on CLI Reference Guide PDF:
- `delete <resource> <resourceId>`
- `deleteMatching <resource> <query>`
- `undeploy <configurationName>`

### Safety Measures

1. **Dry Run**: Preview what would be affected
   ```bash
   vqx safe-delete types MyType --dry-run
   ```

2. **Confirmation Required**: Interactive prompt by default
   ```bash
   vqx safe-delete types MyType
   # Are you sure you want to delete 1 item(s)? [y/N]
   ```

3. **Automatic Backup**: Export before deletion
   ```bash
   vqx safe-delete types MyType
   # Backup saved to ~/.vqx/backups/2024-01-15T10-30-00/
   ```

4. **Blocklist**: Prevent deletion of system resources
   ```toml
   [safe_delete]
   blocked_prefixes = ["System", "ARS"]
   ```

5. **Quantity Limit**: Require `--force` for bulk deletions
   ```toml
   [safe_delete]
   max_items_without_force = 10
   ```

## Reporting Security Issues

If you discover a security vulnerability, please:

1. **Do not** open a public issue
2. Email security concerns to the maintainers
3. Include steps to reproduce
4. Allow time for a fix before disclosure

## Audit Trail

When enabled, vqx logs all operations with:
- Profile name (not credentials)
- Server URL
- Command executed
- Timestamp
- Result status

Example log entry:
```
2024-01-15T10:30:00Z INFO vqx: Executing CLI command
    profile=production
    url=https://prod.vantiq.com
    command=export
    args=["metadata", "-d", "./export"]
```
