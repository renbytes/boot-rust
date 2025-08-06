// boot-rust/src/server.rs

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use toml::Value;
use tonic::{Request, Response, Status};

use crate::boot_plugin::{
    boot_code_plugin_server::BootCodePlugin, GetPromptComponentsRequest, GetPromptComponentsResponse,
};

#[derive(Debug, Default)]
pub struct MyBootCodePlugin {}

/**
 * Gets the absolute path to the 'prompts' directory for local development.
 *
 * This function assumes a sibling-directory layout (`boot-core/` and `boot-rust/`)
 * and constructs the path from the current working directory.
 *
 * @returns The absolute path to the `boot-rust/prompts` directory.
 */
fn get_prompts_path() -> Result<PathBuf> {
    // Get the current working directory from where `boot` was run (e.g., /path/to/boot-core)
    let current_dir = env::current_dir()?;

    // Go up one level to the parent workspace directory
    let workspace_dir = current_dir
        .parent()
        .ok_or_else(|| anyhow!("Failed to get parent directory of {:?}", current_dir))?;

    // Construct the path to the sibling `boot-rust/prompts` directory
    let prompts_path = workspace_dir.join("boot-rust").join("prompts");

    if !prompts_path.is_dir() {
        return Err(anyhow!(
            "Could not find 'prompts' directory at expected dev path: {}. Ensure boot-rust is a sibling to boot-core.",
            prompts_path.display()
        ));
    }
    Ok(prompts_path)
}

// format_spec_for_prompt function remains the same...
fn format_spec_for_prompt(spec_toml_content: &str) -> Result<String> {
    let spec: Value = toml::from_str(spec_toml_content)?;
    let description = spec
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("No description provided.");
    let project_name = spec
        .get("project")
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("Unnamed project");
    Ok(format!(
        "--- USER SPECIFICATION ---\nProject Name: {}\nDescription: {}",
        project_name, description
    ))
}


#[tonic::async_trait]
impl BootCodePlugin for MyBootCodePlugin {
    async fn get_prompt_components(
        &self,
        request: Request<GetPromptComponentsRequest>,
    ) -> Result<Response<GetPromptComponentsResponse>, Status> {
        let spec_content = &request.get_ref().spec_toml_content;

        let mut components = HashMap::new();
        let prompts_dir =
            get_prompts_path().map_err(|e| Status::internal(e.to_string()))?;
        let entries = fs::read_dir(&prompts_dir)
            .map_err(|e| Status::internal(format!("Could not read prompts directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| Status::internal(format!("Invalid directory entry: {}", e)))?;
            let path = entry.path();
            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    if file_name == "Dockerfile" {
                        continue;
                    }
                    let content = fs::read_to_string(&path)
                        .map_err(|e| Status::internal(format!("Could not read file {:?}: {}", path, e)))?;
                    components.insert(file_name.to_string(), content);
                }
            }
        }

        let response = GetPromptComponentsResponse {
            components,
            user_spec_prompt: format_spec_for_prompt(spec_content)
                .map_err(|e| Status::internal(e.to_string()))?,
        };

        Ok(Response::new(response))
    }
}