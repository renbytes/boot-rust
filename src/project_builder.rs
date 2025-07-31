// FILE: src/project_builder.rs
//! Robust packaging of generated files and rendered infrastructure templates.
//!
//! Goals:
//! - Parse LLM output reliably (### FILE: headers + triple-backtick fences).
//! - Sanitize non-markdown files so accidental code fences / FILE markers don’t leak into artifacts.
//! - Keep stdout-sensitive files (e.g., Cargo.toml) clean.
//! - Be defensive about paths (no absolute paths, no traversal).
//! - Make behavior observable via `tracing`.

use crate::{
    spec::SpexSpecification,
    spex_plugin::{File, GenerateResponse},
};
use anyhow::{Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::borrow::Cow;
use std::path::{Component, Path};
use tera::{Context as TeraContext, Tera};
use tracing::{debug, info, warn};

lazy_static! {
    /// Match LLM file blocks, e.g.:
    ///
    /// ### FILE: path/to/file.ext
    /// ```lang
    /// <content>
    /// ```
    ///
    /// - Supports optional language after the opening backticks.
    /// - Handles `\n` or `\r\n`.
    /// - Content is captured non-greedily up to the first matching closing fence.
    static ref FILE_BLOCK_REGEX: Regex = Regex::new(
        r"(?s)^\s*###\s*FILE:\s*(?P<path>[^\r\n]+)\r?\n```(?P<lang>[^\r\n`]*)\r?\n(?P<content>.*?)\r?\n```(?=\r?\n|$)"
    ).expect("valid FILE_BLOCK_REGEX");

    /// Matches an entire file wrapped in a single triple-backtick fence, with optional language.
    /// We use this to strip accidental full-file fencing in non-markdown outputs.
    static ref FULL_FENCE_RE: Regex =
        Regex::new(r"(?s)^\s*```[a-zA-Z0-9_-]*\s*\r?\n(.*)\r?\n```\s*$")
            .expect("valid FULL_FENCE_RE");

    /// Matches a leading "### FILE: ..." header line.
    static ref FILE_MARKER_RE: Regex =
        Regex::new(r"(?m)^\s*###\s+FILE:.*\r?\n")
            .expect("valid FILE_MARKER_RE");

    /// Matches a UTF-8 BOM at the beginning of a string.
    static ref LEADING_BOM_RE: Regex =
        Regex::new(r"^\u{FEFF}")
            .expect("valid LEADING_BOM_RE");
}

/// Returns `true` if the path should be treated as markdown-like
/// (we do not strip code fences for these).
fn is_markdown_like(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.ends_with(".md") || p.ends_with(".markdown") || p.ends_with(".rst")
}

/// Normalize newlines to `\n`.
fn normalize_newlines(s: &str) -> Cow<'_, str> {
    // Fast path: if no CR present, return borrowed.
    if !s.as_bytes().contains(&b'\r') {
        return Cow::Borrowed(s);
    }
    Cow::Owned(s.replace("\r\n", "\n").replace('\r', "\n"))
}

/// Strip a single leading UTF-8 BOM if present.
fn strip_bom(s: &str) -> Cow<'_, str> {
    if LEADING_BOM_RE.is_match(s) {
        Cow::Owned(LEADING_BOM_RE.replace(s, "").into_owned())
    } else {
        Cow::Borrowed(s)
    }
}

/// Remove a single leading "### FILE: ..." marker line.
fn strip_leading_file_marker(s: &str) -> Cow<'_, str> {
    if FILE_MARKER_RE.is_match(s) {
        Cow::Owned(FILE_MARKER_RE.replace(s, "").into_owned())
    } else {
        Cow::Borrowed(s)
    }
}

/// If the entire string is wrapped in a single triple-backtick fence, return the inner content.
fn strip_full_file_fence(s: &str) -> Option<String> {
    FULL_FENCE_RE
        .captures(s)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

/// For non-markdown files:
/// - strip a single leading "### FILE:" header if present
/// - strip a single surrounding ```fence``` if the *whole* file is fenced
/// Always:
/// - drop a leading BOM if present
/// - normalize newlines to `\n`
fn sanitize_nonmarkdown_output(path: &str, content: &str) -> String {
    // Always canonicalize simple text issues first.
    let content = strip_bom(content);
    let content = normalize_newlines(&content);

    if is_markdown_like(path) {
        return content.into_owned();
    }

    // Remove accidental FILE markers and full-file fences for non-markdown.
    let no_marker = strip_leading_file_marker(&content).into_owned();
    if let Some(inner) = strip_full_file_fence(&no_marker) {
        normalize_newlines(&inner).into_owned()
    } else {
        no_marker
    }
}

/// Return `Some(sanitized_path)` if the path is acceptable and within the project dir.
/// Rejects absolute paths and paths containing `..` components.
fn sanitize_path(path: &str) -> Option<String> {
    // Convert Windows backslashes to forward slashes for consistency.
    let mut p = path.replace('\\', "/").trim().to_string();

    if p.is_empty() {
        return None;
    }
    // Strip a leading "./"
    if let Some(stripped) = p.strip_prefix("./") {
        p = stripped.to_string();
    }
    // No absolute paths
    if Path::new(&p).is_absolute() {
        return None;
    }
    // No templates/ files in the final artifact (these are source templates)
    if p.starts_with("templates/") {
        return None;
    }
    // Disallow path traversal
    let has_traversal = Path::new(&p)
        .components()
        .any(|c| matches!(c, Component::ParentDir));
    if has_traversal {
        return None;
    }
    Some(p)
}

/// Insert or replace a file in `response.files` by `path`.
fn upsert_file(response: &mut GenerateResponse, path: String, content: String) {
    if let Some(idx) = response.files.iter().position(|f| f.path == path) {
        debug!("Replacing existing file: {}", path);
        response.files[idx].content = content;
    } else {
        response.files.push(File { path: path.clone(), content });
        debug!("Packaged code file: {}", path);
    }
}

/// Extract code blocks from LLM output and package them as files.
///
/// Expected block shape:
/// ```text
/// ### FILE: relative/path.ext
/// ```<lang>
/// <content>
/// ```
/// ```
///
/// Any block with an invalid or disallowed path is skipped with a warning.
pub fn package_code_files(llm_output: &str, response: &mut GenerateResponse) {
    let mut count = 0usize;
    for cap in FILE_BLOCK_REGEX.captures_iter(llm_output) {
        let raw_path = cap.name("path").map_or("", |m| m.as_str()).trim();
        let lang = cap.name("lang").map(|m| m.as_str().trim()).unwrap_or_default();
        let content = cap.name("content").map_or("", |m| m.as_str());

        match sanitize_path(raw_path) {
            Some(path) => {
                // For non-markdown files, perform extra cleanup; for markdown, keep as-is (with normalized newlines/BOM stripping).
                let cleaned = sanitize_nonmarkdown_output(&path, content);
                upsert_file(response, path.clone(), cleaned);
                info!("Packaged code file: {} (lang='{}')", path, lang);
                count += 1;
            }
            None => {
                warn!("Skipping invalid or disallowed path in LLM output: {}", raw_path);
            }
        }
    }
    if count == 0 {
        warn!("No code files matched the expected ### FILE:/``` block format.");
    } else {
        info!("Total packaged code files: {}", count);
    }
}

/// Render infrastructure templates (Cargo.toml, Makefile, README, etc.)
/// Applies the same sanitization to prevent accidental fences in non-markdown outputs.
pub fn package_infrastructure_files(
    tera: &Tera,
    spec: &SpexSpecification,
    response: &mut GenerateResponse,
) -> Result<()> {
    info!("Packaging infrastructure files...");
    let mut context = TeraContext::new();
    context.insert("spec", spec);

    // Flatten extras into the context (e.g., features, binary_name, etc.).
    for (key, value) in &spec.extras {
        context.insert(key, value);
    }

    let templates = vec![
        ("Cargo.toml".to_string(), "rust/Cargo.toml.template"),
        ("Makefile".to_string(), "rust/Makefile.template"),
        ("README.md".to_string(), "rust/README.md.template"),
    ];

    for (path, template_name) in templates {
        let rendered = tera
            .render(template_name, &context)
            .with_context(|| format!("Failed to render template: {}", template_name))?;

        // For non-markdown files, strip accidental FILE markers / top-level fences.
        let content = sanitize_nonmarkdown_output(&path, &rendered);
        upsert_file(response, path.clone(), content);
        info!("Packaged infrastructure file from template: {}", template_name);
    }
    Ok(())
}