# boot-rust

A "Prompt Provider" plugin for **BootCode**.

This Rust application is a lightweight gRPC server that serves language-specific prompt components to `boot-core`. Its sole responsibility is to provide the building blocks that the core application uses to construct high-quality prompts for generating Rust code.

- **Handshake (stdout)**: Prints a single handshake line required by `boot-core` to establish a connection.
- **Logs (stderr)**: All logging is directed to stderr to keep stdout clean.
- **Prompts**: All prompt logic is contained in simple text files within the `/prompts` directory.

---

## Build & Install

**Prerequisites:**
- Rust (2021 edition or later)
- `cargo`

**Build the Release Binary:**
```bash
cargo build --release
```

### Install for `boot-core`:

To make the `boot-rust` executable discoverable by `boot-core`, install it to your cargo binary path.

```bash
# Install the binary
cargo install --path . --force

# Verify it's in your path
which boot-rust
```

Expected output:
```
boot-rust % which boot-rust
/Users/your_user_name/.cargo/bin/boot-rust
```
