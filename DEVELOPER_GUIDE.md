# Spex-Rust Developer Guide

This guide provides technical details for developing and troubleshooting the `spex-rust` plugin.

---

## The Handshake Contract

`spex-core` discovers and communicates with plugins based on a simple contract:

- **First line on `stdout`**: Must be the handshake string in the format `1|1|tcp|HOST:PORT|grpc`.
- **All other output**: All logs, warnings, and errors **must** be sent to `stderr`. This keeps `stdout` clean so the handshake is not corrupted.

---

## Testing the Binary

You can quickly test that the binary is producing the correct handshake and that logs are properly sent to `stderr`.

```bash
# From the spex-rust project root, run the release binary and grab the first line of stdout
./target/release/spex-rust 2>/dev/null | head -1
```

**Expected Output:**

```
1|1|tcp|127.0.0.1:<PORT>|grpc
```

**Note**: If you see a "Broken pipe" error, it's often because a previous version of the binary was logging to stdout. Rebuild with `cargo build --release` to ensure logs are correctly sent to stderr.

---

## Debugging Plugin Discovery

`spex-core` discovers the plugin by finding an executable named `spex-rust` on its `PATH`.

### Verify what `spex-core` Sees

From the `spex-core` directory, run this command to see which `spex-rust` executable Poetry's environment will use:

```bash
poetry run which -a spex-rust
```

### Alternative: Running Without Installing

For rapid testing, you can run `spex-core` and temporarily add your local plugin build to the `PATH` for that single command:

```bash
# Run this from the spex-core directory
poetry run env PATH="/path/to/your/spex-rust/target/release:$PATH" \
  spex generate my_rust_spec.toml
```

### Removing Stale Binaries

If `which spex-rust` points to an old version, remove it:

```bash
# If installed via cargo
cargo uninstall spex-rust

# Or remove a manually copied file
rm -f ~/.cargo/bin/spex-rust

# Clear the shell's command cache
hash -r
```

---

## Development Workflow

- **Format Code:** `cargo fmt --all`
- **Lint Code:** `cargo clippy --all-targets -- -D warnings`
- **Run Unit Tests:** `cargo test`

---

## Common `spex-core` Errors

- **`Plugin executable not found`**: This is a `PATH` issue. `spex-core` cannot find the `spex-rust` binary. Ensure you have run `cargo install --path . --force` and that `~/.cargo/bin` is in your shell's `PATH`.

- **`Invalid handshake` / `not enough values to unpack`**: This means the plugin wrote something to `stdout` before the handshake string. Use the "Testing the Binary" command above to verify the output is clean.
