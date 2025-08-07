#!/bin/zsh
#
# Creates compressed ZIP archives of the release binary for macOS platforms.
#
# This script builds for both Apple Silicon and Intel architectures and creates
# correctly named asset files that the `boot` CLI can discover and download.
#
# Prerequisites:
#   - `rustup` must be installed.
#
# Usage:
#   From the root of the boot-rust repository, run:
#   ./scripts/create_release_zip.sh
#
# The output will be multiple .zip files in the `target/` directory.

set -e # Exit immediately if a command exits with a non-zero status.

# --- Configuration ---
# The name of your plugin binary. This should match the `name` in your Cargo.toml.
PLUGIN_NAME="boot-rust"
# The root directory where release assets will be created.
RELEASE_DIR="target"

# An array of Rust target triples to build for.
# We are now only building for macOS targets.
TARGETS=(
  "aarch64-apple-darwin"      # For Apple Silicon (M1/M2/M3)
  "x86_64-apple-darwin"       # For Intel-based Macs
)

# --- Main Logic ---

# Function to print a styled message.
function log() {
  echo -e "\n\e[1;34m>> $1\e[0m"
}

log "Starting macOS release build for ${PLUGIN_NAME}..."

# Ensure the release directory exists.
mkdir -p "${RELEASE_DIR}"

for target in "${TARGETS[@]}"; do
  log "Building for target: ${target}"

  # 1. Install the Rust toolchain for the target platform.
  rustup target add "${target}"

  # 2. Build the release binary using the standard `cargo build`.
  cargo build --release --target "${target}"

  # 3. Define platform-specific variables for naming the final asset.
  asset_arch=""
  asset_os="apple-darwin" # This is constant for both macOS targets

  case "${target}" in
    aarch64-apple-darwin)
      asset_arch="arm64"
      ;;
    x86_64-apple-darwin)
      asset_arch="x86_64"
      ;;
    *)
      echo "Warning: Unhandled target '${target}' for asset naming. Skipping."
      continue
      ;;
  esac

  # 4. Create the ZIP archive.
  binary_path="${RELEASE_DIR}/${target}/release/${PLUGIN_NAME}"
  zip_filename="${PLUGIN_NAME}-${asset_arch}-${asset_os}.zip"
  zip_path="${RELEASE_DIR}/${zip_filename}"

  if [ ! -f "${binary_path}" ]; then
      echo "Warning: Build for ${target} failed or binary not found. Skipping."
      continue
  fi

  echo "Creating ZIP archive: ${zip_path}"
  # Use `zip -j` to store files at the root of the archive without directory structure.
  zip -j "${zip_path}" "${binary_path}"
done

# 5. Final Summary
log "✅ All builds complete!"
echo "Release assets have been created in the '${RELEASE_DIR}/' directory."
echo "You can now upload these .zip files to your GitHub release."
