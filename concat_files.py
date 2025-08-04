# concat_files.py
"""
Usage
python concat_files.py
"""

import argparse
import pathlib

# --- Configuration ---
# List of directories to always ignore during the file search.
DEFAULT_IGNORE_DIRS = [
    ".git",
    "target",
    "generated_jobs",
    ".idea",
    ".vscode",
    "__pycache__",
    "examples",
    ".build",
    ".swiftpm",
    "Sources",
    "Tests",
    "docs",
    ".pytest_cache",
    ".mypy_cache",
    "poetry.lock",
    "target",
]

# List of specific files to always ignore.
DEFAULT_IGNORE_FILES = [
    "concat_files.py",
    "all_code_python.txt",
    "Cargo.lock",
    ".DS_Store",
    ".env",
]


def concatenate_files(
    folders_to_search: list[str], file_types: list[str], output_file: str
):
    """
    Finds and concatenates the content of specified files into a single output file.

    Args:
        folders_to_search (list[str]): A list of specific folders to search in.
                                       If empty, searches from the current directory down.
        file_types (list[str]): A list of file extensions to include (e.g., ['.rs', '.toml']).
                                If empty, includes all files.
        output_file (str): The name of the file to write the concatenated content to.
    """
    root_path = pathlib.Path(".")
    all_content = []

    search_paths = (
        [pathlib.Path(p) for p in folders_to_search]
        if folders_to_search
        else [root_path]
    )

    print("Starting file concatenation...")
    print(f"Searching in: {', '.join(map(str, search_paths)) or 'current directory'}")
    print(f"Looking for file types: {', '.join(file_types) or 'all'}")

    for search_path in search_paths:
        for path in sorted(search_path.rglob("*")):  # rglob searches recursively
            if path.is_file():
                # --- Filtering Logic ---
                if (
                    any(d in path.parts for d in DEFAULT_IGNORE_DIRS)
                    or path.name in DEFAULT_IGNORE_FILES
                ):
                    continue

                if file_types and path.suffix not in file_types:
                    continue

                # --- File Processing ---
                try:
                    with open(path, encoding="utf-8") as f:
                        content = f.read()
                    header = f"\n--- START OF FILE: {path} ---\n"
                    footer = f"\n--- END OF FILE: {path} ---\n"
                    all_content.append(header + content + footer)
                    print(f"  ✓ Added: {path}")
                except Exception as e:
                    print(f"  ✗ Could not read file {path}: {e}")

    # --- Write Output File ---
    try:
        with open(output_file, "w", encoding="utf-8") as f:
            f.write("\n".join(all_content))
        print(f"\n✅ Successfully concatenated all code into '{output_file}'")
    except Exception as e:
        print(f"\n❌ Failed to write output file: {e}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Concatenate the contents of all files in the repo into a single file for debugging."
    )
    parser.add_argument(
        "--folders",
        nargs="*",
        default=[],
        help="A list of specific folders to search (e.g., src configs). Searches all if empty.",
    )
    parser.add_argument(
        "--types",
        nargs="*",
        default=[],
        help="A list of file extensions to include (e.g., .rs .toml). Includes all if empty.",
    )
    parser.add_argument(
        "--output",
        default="all_code_spex_rust.txt",
        help="The name of the output file.",
    )

    args = parser.parse_args()

    # Ensure file types have a leading dot if they don't already
    sanitized_types = [f".{t.lstrip('.')}" for t in args.types if t]

    concatenate_files(args.folders, sanitized_types, args.output)
