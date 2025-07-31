use crate::{spec::SpexSpecification, spex_plugin::GenerateRequest};
use anyhow::{Context, Result};
use tera::{Context as TeraContext, Tera};

pub fn render_prompt(tera: &Tera, spec: &SpexSpecification, request: &GenerateRequest) -> Result<String> {
    let template_type = if request.is_review_pass { "review" } else { "generation" };
    let template_path = format!("rust/prompt_templates/{}.tera", template_type);

    let mut context = TeraContext::new();
    context.insert("spec", spec);
    if request.is_review_pass {
        context.insert("initial_code", &request.initial_code);
    }

    tera.render(&template_path, &context)
        .context(format!("Failed to render template: {}", template_path))
}