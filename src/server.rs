use crate::{llm_client::LlmClient, project_builder, prompt_builder, spec::SpexSpecification};
use futures::FutureExt;
use serde_json::json;
use std::backtrace::Backtrace;
use std::collections::HashSet;
use tera::Tera;
use tonic::{Request, Response, Status};
use tracing::{error, info};

use crate::spex_plugin::{spex_plugin_server::SpexPlugin, GenerateRequest, GenerateResponse};

pub struct RustPluginServicer {
    tera: Tera,
}

impl RustPluginServicer {
    pub fn new() -> anyhow::Result<Self> {
        let pattern = format!("{}/templates/**/*", env!("CARGO_MANIFEST_DIR"));
        let mut tera = Tera::new(&pattern)?;
        tera.autoescape_on(vec![]);
        info!("Tera template environment loaded (pattern: {pattern})");

        // Log what Tera sees (helps diagnose naming mismatches)
        let names: Vec<_> = tera.get_template_names().collect();
        info!("Loaded {} templates: {:?}", names.len(), names);

        // Fail fast if core infra templates are missing (we’ll fallback only for .gitignore)
        ensure_required_templates(&tera)?;

        Ok(Self { tera })
    }

    async fn handle_generate(&self, req: GenerateRequest) -> Result<GenerateResponse, Status> {
        info!("Received GenerateProject request.");

        let spec: SpexSpecification = toml::from_str(&req.spec_toml_content).map_err(|e| {
            error!("Failed to parse spec.toml content: {}", e);
            Status::invalid_argument(format!("Invalid spec.toml: {}", e))
        })?;

        let llm_config = req.llm_config.clone().ok_or_else(|| {
            error!("LLMConfig is missing from the request.");
            Status::invalid_argument("LLMConfig is required")
        })?;

        let llm_client = LlmClient::new(llm_config);

        let prompt = prompt_builder::render_prompt(&self.tera, &spec, &req).map_err(|e| {
            error!("Failed to render generation prompt: {:?}", e);
            Status::internal(format!("Failed to render prompt: {e:#}"))
        })?;

        let llm_output = llm_client.generate(&prompt).await.map_err(|e| {
            error!("LLM generation failed: {:?}", e);
            Status::internal(format!("LLM generation failed: {e:#}"))
        })?;

        let mut response = GenerateResponse::default();
        let code_count = project_builder::package_code_files(&llm_output, &mut response);

        project_builder::package_infrastructure_files(&self.tera, &spec, &mut response).map_err(
            |e| {
                error!("Failed to package infrastructure files: {:?}", e);
                Status::internal(format!("Failed to package infrastructure files: {e:#}"))
            },
        )?;

        if code_count == 0 {
            info!("No code files from LLM; rendering bootstrap skeleton");
            project_builder::package_bootstrap_files(&self.tera, &spec, &mut response).map_err(
                |e| {
                    error!("Failed to package bootstrap files: {:?}", e);
                    Status::internal(format!("Failed to package bootstrap files: {e:#}"))
                },
            )?;
        }

        // append manifest
        let manifest = json!({
            "files": response.files.iter().map(|f| json!({"path": f.path})).collect::<Vec<_>>()
        }).to_string();
        response.files.push(crate::spex_plugin::File { path: ".spex_manifest.json".into(), content: manifest });

        Ok(response)
    }
}

fn ensure_required_templates(tera: &Tera) -> anyhow::Result<()> {
    use anyhow::anyhow;
    let required: &[&str] = &[
        // we’ll fallback for .gitignore so it’s not required here
        "rust/Cargo.toml.template",
        "rust/Makefile.template",
        "rust/README.md.template",
        "rust/instructions/rust_rules.tera",
        "rust/prompt_templates/generation.tera",
        "rust/prompt_templates/review.tera",
    ];

    let names: HashSet<_> = tera.get_template_names().collect();
    let mut missing = Vec::new();
    for t in required {
        if !names.contains(*t) {
            missing.push(*t);
        }
    }
    if !missing.is_empty() {
        return Err(anyhow!("Missing required templates: {}", missing.join(", ")));
    }
    Ok(())
}

#[tonic::async_trait]
impl SpexPlugin for RustPluginServicer {
    async fn generate_project(
        &self,
        request: Request<GenerateRequest>,
    ) -> Result<Response<GenerateResponse>, Status> {
        let fut = async { self.handle_generate(request.into_inner()).await };
        match std::panic::AssertUnwindSafe(fut).catch_unwind().await {
            Ok(Ok(resp)) => Ok(Response::new(resp)),
            Ok(Err(status)) => Err(status),
            Err(panic) => {
                let msg = if let Some(s) = panic.downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = panic.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic payload".to_string()
                };
                let bt = Backtrace::force_capture();
                error!("panic in generate_project: {msg}\nBacktrace:\n{bt}");
                Err(Status::internal(format!(
                    "plugin panic in generate_project: {msg}\n{bt}"
                )))
            }
        }
    }
}