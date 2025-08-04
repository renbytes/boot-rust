# spex-rust

A "Prompt Provider" plugin for **Spex**.

This Rust application is a lightweight gRPC server that serves language-specific prompt components to `spex-core`. Its sole responsibility is to provide the building blocks that the core application uses to construct high-quality prompts for generating Rust code.

- [cite_start]**Handshake (stdout)**: Prints a single handshake line required by `spex-core` to establish a connection. [cite: 4, 5, 6, 11]
- [cite_start]**Logs (stderr)**: All logging is directed to stderr to keep stdout clean. [cite: 7, 51]
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

### Install for `spex-core`:

To make the `spex-rust` executable discoverable by `spex-core`, install it to your cargo binary path.

```bash
# Install the binary
cargo install --path . --force

# Verify it's in your path
which spex-rust
```

Expected output:
```
spex-rust % which spex-rust
/Users/your_user_name/.cargo/bin/spex-rust
```
