// gRPC service implementation & panic-safe wrapper.

use crate::{llm_client::LlmClient, project_builder, prompt_builder, spec::SpexSpecification};
use futures::FutureExt;
use tera::Tera;
use tonic::{Request, Response, Status};
use tracing::{error, info};

// gRPC types generated in main.rs module
use crate::spex_plugin::{spex_plugin_server::SpexPlugin, GenerateRequest, GenerateResponse};

pub struct RustPluginServicer {
    tera: Tera,
}

impl RustPluginServicer {
    pub fn new() -> anyhow::Result<Self> {
        // Load all templates under templates/**
        let pattern = format!("{}/templates/**/*", env!("CARGO_MANIFEST_DIR"));
        let mut tera = Tera::new(&pattern)?;
        tera.autoescape_on(vec![]); // we're not rendering HTML
        info!("Tera template environment loaded (pattern: {pattern})");
        Ok(Self { tera })
    }

    // The inner logic, returning Status on failure (no panics).
    async fn handle_generate(&self, req: GenerateRequest) -> Result<GenerateResponse, Status> {
        info!("Received GenerateProject request.");

        // 1) Parse spec
        let spec: SpexSpecification = toml::from_str(&req.spec_toml_content).map_err(|e| {
            error!("Failed to parse spec.toml content: {}", e);
            Status::invalid_argument(format!("Invalid spec.toml: {}", e))
        })?;

        // 2) LLM config
        let llm_config = req.llm_config.clone().ok_or_else(|| {
            error!("LLMConfig is missing from the request.");
            Status::invalid_argument("LLMConfig is required")
        })?;

        // 3) LLM client
        let llm_client = LlmClient::new(llm_config);

        // 4) Prompt
        let prompt = prompt_builder::render_prompt(&self.tera, &spec, &req).map_err(|e| {
            error!("Failed to render generation prompt: {}", e);
            Status::internal(format!("Failed to render prompt: {}", e))
        })?;

        // 5) Call LLM
        let llm_output = llm_client.generate(&prompt).await.map_err(|e| {
            error!("LLM generation failed: {}", e);
            Status::internal(format!("LLM generation failed: {}", e))
        })?;

        // 6) Package files
        let mut response = GenerateResponse::default();
        project_builder::package_code_files(&llm_output, &mut response);
        project_builder::package_infrastructure_files(&self.tera, &spec, &mut response).map_err(
            |e| {
                error!("Failed to package infrastructure files: {}", e);
                Status::internal(format!("Failed to package infrastructure files: {}", e))
            },
        )?;

        Ok(response)
    }
}

#[tonic::async_trait]
impl SpexPlugin for RustPluginServicer {
    async fn generate_project(
        &self,
        request: Request<GenerateRequest>,
    ) -> Result<Response<GenerateResponse>, Status> {
        // Panic-safe wrapper: a panic becomes INTERNAL instead of RST_STREAM.
        let fut = async { self.handle_generate(request.into_inner()).await };
        match std::panic::AssertUnwindSafe(fut).catch_unwind().await {
            Ok(Ok(resp)) => Ok(Response::new(resp)),
            Ok(Err(status)) => Err(status),
            Err(panic) => {
                error!("panic in generate_project: {:?}", panic);
                Err(Status::internal("plugin panic in generate_project"))
            }
        }
    }
}