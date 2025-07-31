// FILE: src/server.rs

// This file xclusively defines the gRPC service
// implementation. The server startup logic, including binding the TCP listener
// and printing the handshake string, has been moved to `src/main.rs`. This
// separation of concerns resolves the startup race condition and aligns with
// standard Rust application architecture.

use crate::{llm_client::LlmClient, project_builder, prompt_builder, spec::SpexSpecification};
use anyhow::Result;
use std::path::Path;
use tera::Tera;
use tonic::{Request, Response, Status};
use tracing::{error, info};

// Use the gRPC types generated in `main.rs` via the `spex_plugin` module.
// This avoids including the proto file in multiple places.
use crate::spex_plugin::{
    spex_plugin_server::SpexPlugin, GenerateRequest, GenerateResponse,
};

/// Implements the gRPC service for the Rust plugin.
///
/// This struct holds the state required for the service, such as the template
/// rendering engine.
pub struct RustPluginServicer {
    tera: Tera,
}

impl RustPluginServicer {
    /// Creates a new instance of the plugin servicer.
    ///
    /// This function is now public so it can be called from `main.rs`. It
    /// initializes the Tera template engine by loading all templates from the
    /// `templates` directory.
    pub fn new() -> Result<Self> {
        let templates_path = Path::new(env!("CARGO_MANIFEST_DIR"))
           .join("templates")
           .join("**")
           .join("*.{tera,template}");

        let tera = Tera::new(templates_path.to_str().unwrap())?;
        info!("Jinja2-like template environment loaded successfully.");
        Ok(Self { tera })
    }
}

#[tonic::async_trait]
impl SpexPlugin for RustPluginServicer {
    /// The core RPC method that handles the project generation request.
    ///
    /// This function orchestrates the entire code generation process within the
    /// plugin, from parsing the spec to calling the LLM and packaging the
    /// resulting files.
    async fn generate_project(
        &self,
        request: Request<GenerateRequest>,
    ) -> Result<Response<GenerateResponse>, Status> {
        info!("Received GenerateProject request.");
        let req = request.into_inner();

        // 1. Parse the specification from the request.
        let spec: SpexSpecification = toml::from_str(&req.spec_toml_content).map_err(|e| {
            error!("Failed to parse spec.toml content: {}", e);
            Status::invalid_argument(format!("Invalid spec.toml: {}", e))
        })?;

        // 2. Get LLM configuration from the request.
        let llm_config = req.llm_config.clone().ok_or_else(|| {
            error!("LLMConfig is missing from the request.");
            Status::invalid_argument("LLMConfig is required")
        })?;

        // 3. Initialize the LLM client.
        let llm_client = LlmClient::new(llm_config);

        // 4. Render the prompt using the specification and templates.
        let prompt = prompt_builder::render_prompt(&self.tera, &spec, &req).map_err(|e| {
            error!("Failed to render generation prompt: {}", e);
            Status::internal(format!("Failed to render prompt: {}", e))
        })?;

        // 5. Call the LLM to generate the code.
        let llm_output = llm_client.generate(&prompt).await.map_err(|e| {
            error!("LLM generation failed: {}", e);
            Status::internal(format!("LLM generation failed: {}", e))
        })?;

        // 6. Assemble the response by packaging the generated files.
        let mut response = GenerateResponse::default();
        project_builder::package_code_files(&llm_output, &mut response);
        project_builder::package_infrastructure_files(&self.tera, &spec, &mut response).map_err(

|e| {
                error!("Failed to package infrastructure files: {}", e);
                Status::internal(format!("Failed to package infrastructure files: {}", e))
            },
        )?;

        info!("Successfully generated project files. Sending response.");
        Ok(Response::new(response))
    }
}