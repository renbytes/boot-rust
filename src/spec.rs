use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Project specification (top-level).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpexSpecification {
    pub language: String,

    #[serde(rename = "project_type")]
    pub project_type: String,

    pub description: String,

    pub project: Project,

    #[serde(flatten)]
    pub extras: HashMap<String, Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub version: String,
    pub description: String,
}