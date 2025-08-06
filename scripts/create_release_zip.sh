#!/bin/bash
#
# Creates a compressed ZIP archive of the release binary for distribution
# on GitHub.
#
# This script automatically detects the host operating system and architecture
# to create a correctly named asset file that the `boot` CLI can discover
# and download.
#
# Usage:
#   From the root of the boot-rust repository, run:
#   ./scripts/create_release_zip.sh
#
# The output will be a file like `target/release/boot-rust-x86_64-apple-darwin.zip`.

set -e # Exit immediately if a command exits with a non-zero status.

# --- Configuration ---
# The name of your plugin binary. This should match the `name` in your Cargo.toml.
PLUGIN_NAME="boot-rust"
# The directory where the final zip file will be placed.
RELEASE_DIR="target/release"

# --- Main Logic ---

# Function to print a styled message.
function log() {
  echo -e "\n\e[1;34m>> $1\e[0m"
}

# 1. Determine OS and Architecture
# -----------------------------------------------------------------------------
log "Detecting OS and architecture..."

os=""
arch=$(uname -m)

case "$(uname -s)" in
  Linux*)
    os="unknown-linux-gnu"
    ;;
  Darwin*)
    os="apple-darwin"
    ;;
  *)
    echo "Error: Unsupported operating system." >&2
    exit 1
    ;;
esac

echo "OS: $os"
echo "Arch: $arch"

# 2. Build the Release Binary
# -----------------------------------------------------------------------------
log "Building the release binary with Cargo..."
cargo build --release

# Path to the compiled binary.
BINARY_PATH="${RELEASE_DIR}/${PLUGIN_NAME}"

if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Release binary not found at ${BINARY_PATH}" >&2
    exit 1
fi

echo "Build complete. Binary is at ${BINARY_PATH}"

# 3. Create the ZIP Archive
# -----------------------------------------------------------------------------
# Construct the final filename based on the detected platform.
# e.g., boot-rust-x86_64-apple-darwin.zip
ZIP_FILENAME="${PLUGIN_NAME}-${arch}-${os}.zip"
ZIP_PATH="${RELEASE_DIR}/${ZIP_FILENAME}"

log "Creating ZIP archive: ${ZIP_PATH}"

# Use `zip` to create the archive.
# -j "junk paths" flag stores files at the root of the zip, removing the directory structure.
zip -j "$ZIP_PATH" "$BINARY_PATH"

# 4. Final Summary
# -----------------------------------------------------------------------------
log "✅ Success!"
echo "Release asset created at: ${ZIP_PATH}"
echo "You can now upload this file to your GitHub release."

