# spex-rust

Rust plugin for **Spex**. It launches a local `gRPC` server, prints a single handshake line to **stdout**, and renders Rust project files from Tera templates.

- Handshake (stdout): `1|1|tcp|127.0.0.1:<PORT>|grpc`
- Logs (stderr): structured with `tracing` (no logs to stdout)

---

## Prereqs

- Rust stable (edition 2021)
- `cargo`
- (Optional) Python + Poetry (to run `spex-core` locally)

---

## Build

```bash
# debug
cargo build

# release
cargo build --release
```

--- 

## Fast test of the binary:

```bash
# add local release to PATH for this shell
export PATH="$(pwd)/target/release:$PATH"

# the first stdout line should be the handshake (logs go to stderr)
spex-rust 2>/dev/null | head -1
# expected: 1|1|tcp|127.0.0.1:<PORT>|grpc
```
> If you see a “Broken pipe” after head -1: that’s an old build that logs to stdout. Rebuild; the current code logs to stderr only.

---

## Install (so spex-core can find it)

Two options—use one:

### A) Install to `~/.cargo/bin` (recommended)
```bash
cargo install --path . --force
# verify
which -a spex-rust
```

### B) Don’t install; just export PATH when running core
```bash
# from spex-core
poetry run env PATH="/abs/path/to/spex-rust/target/release:$PATH" \
  spex generate my_rust_spec.toml
```

---

## Debugging: PATH & Plugin Discovery

Core discovers the plugin by name spex-rust on its PATH.

Check what Poetry sees:
```bash
# from spex-core dir
poetry run which -a spex-rust
```
If that shows `~/.cargo/bin/spex-rust` but you want your local build, either:
    •   Install your latest (`cargo install --path . --force`), or
    •   Override PATH for that run (see option B above).

Test the binary from Poetry’s env:
```bash
poetry run bash -lc 'spex-rust 2>/dev/null | head -1'
# expect: 1|1|tcp|127.0.0.1:<PORT>|grpc
```
Remove stale binary if needed:
```bash
cargo uninstall spex-rust           # if installed by cargo
rm -f ~/.cargo/bin/spex-rust        # if it was just a stray file
hash -r
```

---

Template Gotchas (Tera)
- Default filter requires a named arg:
```
{% set pkg_name = spec.package_name | default(value="spex_app") %}
```

- Escaping examples that contain {{ ... }}:
```
{% raw %}Command::cargo_bin("{{ spec.binary_name | default(value="app") }}"){% endraw %}
```

---

## LLM Response Parsing (Rust client)

When parsing provider responses, index arrays correctly:
```rust
// OpenAI chat
data["choices"][0]["message"]["content"].as_str()

// Gemini
data["candidates"][0]["content"]["parts"][0]["text"].as_str()
```

Avoid `Result<_, String>` in the library layer; prefer `anyhow::Result` (apps) or typed errors with thiserror (libs). This keeps `.context("...")` usable at call sites.

⸻

Development Tips

# format & lint
cargo fmt --all
cargo clippy --all-targets -- -D warnings

# run unit tests (if any)
cargo test

Handshake contract:
- First line on stdout: `1|1|tcp|HOST:PORT|grpc`
- Everything else (info, warnings, errors) → `stderr`

If spex-core errors with:
- Plugin executable not found: `spex-rust` → `PATH` issue (see "Install/Using with spex-core").
- not enough values to unpack / invalid handshake → wrong binary or stdout noise before handshake (verify with head -1 test).
- Tera expected an identifier → use `default(value="...")` or escape example `{{ ... }} with {% raw %}`.
