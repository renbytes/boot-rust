//! Packaging of generated files and rendered infrastructure templates.
//! - Parses ### FILE: blocks by header scanning (no look-around).
//! - Sanitizes non-markdown outputs (strip accidental fences / headers).
//! - Normalizes paths (no absolute, no parent traversal), skips templates/.
//! - Renders infra from a list of candidate template names and falls back
//!   to a built-in .gitignore if none found.

use crate::{
    spec::SpexSpecification,
    spex_plugin::{File, GenerateResponse},
};
use anyhow::{anyhow, Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Component, Path};
use tera::{Context as TeraContext, Tera};
use tracing::{debug, info, warn};

lazy_static! {
    /// `### FILE: path/to/file.ext`
    static ref FILE_HEADER_RE: Regex =
        Regex::new(r"(?m)^###\s*FILE:\s*(?P<path>[^\r\n]+)\s*\r?$")
            .expect("valid FILE_HEADER_RE");

    /// Entire file fenced in ```lang ... ```
    static ref FULL_FENCE_RE: Regex =
        Regex::new(r"(?s)^\s*```[a-zA-Z0-9_-]*\s*\r?\n(.*)\r?\n```\s*$")
            .expect("valid FULL_FENCE_RE");

    /// Remove a leading `### FILE:` line inside a file if present
    static ref FILE_MARKER_RE: Regex =
        Regex::new(r"(?m)^\s*###\s+FILE:.*\r?\n")
            .expect("valid FILE_MARKER_RE");

    /// UTF-8 BOM
    static ref LEADING_BOM_RE: Regex =
        Regex::new(r"^\u{FEFF}")
            .expect("valid LEADING_BOM_RE");
}

fn is_markdown_like(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.ends_with(".md") || p.ends_with(".markdown") || p.ends_with(".rst")
}

fn normalize_newlines(s: &str) -> Cow<'_, str> {
    if !s.as_bytes().contains(&b'\r') {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(s.replace("\r\n", "\n").replace('\r', "\n"))
    }
}

fn strip_bom(s: &str) -> Cow<'_, str> {
    if LEADING_BOM_RE.is_match(s) {
        Cow::Owned(LEADING_BOM_RE.replace(s, "").into_owned())
    } else {
        Cow::Borrowed(s)
    }
}

fn strip_leading_file_marker(s: &str) -> Cow<'_, str> {
    if FILE_MARKER_RE.is_match(s) {
        Cow::Owned(FILE_MARKER_RE.replace(s, "").into_owned())
    } else {
        Cow::Borrowed(s)
    }
}

fn strip_full_file_fence(s: &str) -> Option<String> {
    FULL_FENCE_RE
        .captures(s)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

/// For non-markdown files:
/// - strip a single leading "### FILE:" header if present
/// - strip a single surrounding ``` fence if the *whole* file is fenced
/// Always drop BOM & normalize newlines.
fn sanitize_nonmarkdown_output(path: &str, content: &str) -> String {
    let content = strip_bom(content);
    let content = normalize_newlines(&content);

    if is_markdown_like(path) {
        return content.into_owned();
    }

    let no_marker = strip_leading_file_marker(&content).into_owned();
    if let Some(inner) = strip_full_file_fence(&no_marker) {
        normalize_newlines(&inner).into_owned()
    } else {
        no_marker
    }
}

/// Return `Some(sanitized_path)` if acceptable (no absolute, no `..`, not under templates/).
fn sanitize_path(path: &str) -> Option<String> {
    let mut p = path.replace('\\', "/").trim().to_string();
    if p.is_empty() { return None; }
    if let Some(stripped) = p.strip_prefix("./") { p = stripped.to_string(); }
    if Path::new(&p).is_absolute() { return None; }
    if p.starts_with("templates/") { return None; }
    let has_traversal = Path::new(&p)
        .components()
        .any(|c| matches!(c, Component::ParentDir));
    if has_traversal { return None; }
    Some(p)
}

fn upsert_file(response: &mut GenerateResponse, path: String, content: String) {
    if let Some(idx) = response.files.iter().position(|f| f.path == path) {
        debug!("Replacing existing file: {}", path);
        response.files[idx].content = content;
    } else {
        response.files.push(File { path: path.clone(), content });
        debug!("Packaged code file: {}", path);
    }
}

/// Parse `### FILE:` blocks by index slicing (no look-arounds).
fn slice_file_blocks(llm_output: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();

    let headers: Vec<(usize, usize, String)> = FILE_HEADER_RE
        .captures_iter(llm_output)
        .filter_map(|cap| {
            let m = cap.get(0)?;
            let start = m.start();
            let end = m.end();
            let raw_path = cap.name("path")?.as_str().trim().to_string();
            Some((start, end, raw_path))
        })
        .collect();

    if headers.is_empty() { return blocks; }

    let total_len = llm_output.len();
    for i in 0..headers.len() {
        let (_h_start, h_end, raw_path) = &headers[i];
        let content_start = *h_end;
        let next_start = if i + 1 < headers.len() { headers[i + 1].0 } else { total_len };

        if let Some(path) = sanitize_path(raw_path) {
            let mut slice = &llm_output[content_start..next_start];
            if let Some(stripped) = slice.strip_prefix("\r\n") {
                slice = stripped;
            } else if let Some(stripped) = slice.strip_prefix('\n') {
                slice = stripped;
            }
            let cleaned = sanitize_nonmarkdown_output(&path, slice);
            blocks.push((path, cleaned));
        } else {
            warn!("Skipping invalid or disallowed path in LLM output: {}", raw_path);
        }
    }
    blocks
}

/// Extract code blocks from LLM output and package them as files.
/// Returns number of files packaged.
pub fn package_code_files(llm_output: &str, response: &mut GenerateResponse) -> usize {
    let blocks = slice_file_blocks(llm_output);
    let count = blocks.len();
    for (path, content) in blocks {
        upsert_file(response, path, content);
    }
    if count == 0 {
        warn!("No code files matched the expected '### FILE:' block format.");
    } else {
        info!("Total packaged code files: {}", count);
    }
    count
}

/// Built-in .gitignore fallback content (used only if no template is found).
fn default_gitignore() -> &'static str {
    r#"# Rust / Cargo
/target/
**/*.rs.bk

# Editors & IDEs
.vscode/
.idea/
*.iml

# OS junk
.DS_Store
Thumbs.db

# Coverage / profiling
*.profraw
*.profdata
/target/llvm-cov/
coverage/

# Python (if present)
/.venv/
__pycache__/
*.pyc
"#
}

/// Render the first existing template from `candidates`.
fn render_first_existing(tera: &Tera, candidates: &[&str], ctx: &TeraContext) -> Result<String> {
    let names: HashSet<_> = tera.get_template_names().collect();
    for &name in candidates {
        if names.contains(name) {
            return tera.render(name, ctx).with_context(|| format!("Failed to render template: {}", name));
        }
    }
    Err(anyhow!("None of the candidate templates exist: {}", candidates.join(", ")))
}

/// Render infrastructure templates (Cargo.toml, Makefile, README, .gitignore).
/// Try multiple candidate names per file; fall back to built-in .gitignore if needed.
pub fn package_infrastructure_files(
    tera: &Tera,
    spec: &SpexSpecification,
    response: &mut GenerateResponse,
) -> Result<()> {
    info!("Packaging infrastructure files...");
    let mut ctx = TeraContext::new();
    ctx.insert("spec", spec);
    for (key, value) in &spec.extras {
        ctx.insert(key, value);
    }

    // For each output path, list candidate template names (first existing will be used)
    let plan: Vec<(&str, Vec<&str>)> = vec![
        ("Cargo.toml", vec![
            "rust/Cargo.toml.template",
            "rust/Cargo.toml.tera",
            "shared/Cargo.toml.template",
            "shared/Cargo.toml.tera",
        ]),
        ("Makefile", vec![
            "rust/Makefile.template",
            "rust/Makefile.tera",
            "shared/Makefile.template",
            "shared/Makefile.tera",
        ]),
        ("README.md", vec![
            "rust/README.md.template",
            "rust/README.md.tera",
            "shared/README.md.template",
            "shared/README.md.tera",
        ]),
        (".gitignore", vec![
            "rust/gitignore.template",
            "rust/gitignore.tera",
            "shared/gitignore.template",
            "shared/gitignore.tera",
        ]),
    ];

    for (out_path, candidates) in plan {
        let rendered = if out_path == ".gitignore" {
            match render_first_existing(tera, &candidates, &ctx) {
                Ok(s) => s,
                Err(e) => {
                    warn!("{} — using built-in default .gitignore", e);
                    default_gitignore().to_string()
                }
            }
        } else {
            render_first_existing(tera, &candidates, &ctx)?
        };

        let content = sanitize_nonmarkdown_output(out_path, &rendered);
        upsert_file(response, out_path.to_string(), content);
        info!("Packaged infrastructure file: {}", out_path);
    }

    Ok(())
}

/// Bootstrap a minimal compilable project if the LLM returned no code files.
/// Returns number of files rendered.
pub fn package_bootstrap_files(
    tera: &Tera,
    spec: &SpexSpecification,
    response: &mut GenerateResponse,
) -> Result<usize> {
    info!("Bootstrapping minimal project for project_type='{}'", spec.project_type);
    let mut ctx = TeraContext::new();
    ctx.insert("spec", spec);
    for (key, value) in &spec.extras {
        ctx.insert(key, value);
    }

    let pt = spec.project_type.to_ascii_lowercase();
    let files: Vec<(String, &'static str)> = match pt.as_str() {
        "service" => vec![
            ("src/main.rs".into(),   "rust/bootstrap/service/main.rs.tera"),
            ("src/lib.rs".into(),    "rust/bootstrap/service/lib.rs.tera"),
            ("src/routes.rs".into(), "rust/bootstrap/service/routes.rs.tera"),
            ("tests/health.rs".into(),"rust/bootstrap/service/tests_health.rs.tera"),
        ],
        "library" => vec![
            ("src/lib.rs".into(),    "rust/bootstrap/library/lib.rs.tera"),
            ("tests/lib.rs".into(),  "rust/bootstrap/library/tests_lib.rs.tera"),
        ],
        _ => vec![
            ("src/main.rs".into(),   "rust/bootstrap/cli/main.rs.tera"),
            ("src/lib.rs".into(),    "rust/bootstrap/cli/lib.rs.tera"),
            ("tests/cli.rs".into(),  "rust/bootstrap/cli/tests_cli.rs.tera"),
        ],
    };

    let names: HashSet<_> = tera.get_template_names().collect();
    let mut rendered_count = 0usize;
    for (path, tpl) in files {
        if !names.contains(tpl) {
            warn!("Bootstrap template missing: {}", tpl);
            continue;
        }
        let rendered = tera.render(tpl, &ctx)
            .with_context(|| format!("Failed to render bootstrap template: {}", tpl))?;
        let content = sanitize_nonmarkdown_output(&path, &rendered);
        upsert_file(response, path, content);
        rendered_count += 1;
    }

    info!("Bootstrapped {} file(s).", rendered_count);
    Ok(rendered_count)
}