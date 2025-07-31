//! Packaging of generated files and rendered infrastructure templates.
//! - Parses ### FILE: blocks with fenced content
//! - Sanitizes non-markdown outputs (strip accidental fences / headers)
//! - Normalizes paths (no absolute, no parent traversal), skips templates/

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
    // No leading ^ so we can find multiple blocks anywhere.
    static ref FILE_BLOCK_REGEX: Regex = Regex::new(
        r"(?s)###\s*FILE:\s*(?P<path>[^\r\n]+)\r?\n```(?P<lang>[^\r\n`]*)\r?\n(?P<content>.*?)\r?\n```"
    ).expect("valid FILE_BLOCK_REGEX");

    static ref FULL_FENCE_RE: Regex =
        Regex::new(r"(?s)^\s*```[a-zA-Z0-9_-]*\s*\r?\n(.*)\r?\n```\s*$").expect("valid FULL_FENCE_RE");

    static ref FILE_MARKER_RE: Regex =
        Regex::new(r"(?m)^\s*###\s+FILE:.*\r?\n").expect("valid FILE_MARKER_RE");

    static ref LEADING_BOM_RE: Regex =
        Regex::new(r"^\u{FEFF}").expect("valid LEADING_BOM_RE");
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

fn sanitize_path(path: &str) -> Option<String> {
    let mut p = path.replace('\\', "/").trim().to_string();
    if p.is_empty() {
        return None;
    }
    if let Some(stripped) = p.strip_prefix("./") {
        p = stripped.to_string();
    }
    if Path::new(&p).is_absolute() {
        return None;
    }
    if p.starts_with("templates/") {
        return None;
    }
    let has_traversal = Path::new(&p)
        .components()
        .any(|c| matches!(c, Component::ParentDir));
    if has_traversal {
        return None;
    }
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

pub fn package_code_files(llm_output: &str, response: &mut GenerateResponse) {
    let mut count = 0usize;
    for cap in FILE_BLOCK_REGEX.captures_iter(llm_output) {
        let raw_path = cap.name("path").map_or("", |m| m.as_str()).trim();
        let lang = cap.name("lang").map(|m| m.as_str().trim()).unwrap_or_default();
        let content = cap.name("content").map_or("", |m| m.as_str());

        match sanitize_path(raw_path) {
            Some(path) => {
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

pub fn package_infrastructure_files(
    tera: &Tera,
    spec: &SpexSpecification,
    response: &mut GenerateResponse,
) -> Result<()> {
    info!("Packaging infrastructure files...");
    let mut context = TeraContext::new();
    context.insert("spec", spec);

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

        let content = sanitize_nonmarkdown_output(&path, &rendered);
        upsert_file(response, path.clone(), content);
        info!("Packaged infrastructure file from template: {}", template_name);
    }
    Ok(())
}