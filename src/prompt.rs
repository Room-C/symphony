use regex::Regex;

use crate::error::{Result, SymphonyError};
use crate::events::Issue;

pub fn render_prompt(template: &str, issue: &Issue, attempt: Option<u32>) -> Result<String> {
    if template.trim().is_empty() {
        return Err(SymphonyError::prompt(
            "empty_prompt",
            "prompt template is empty",
        ));
    }
    liquid::ParserBuilder::with_stdlib()
        .build()
        .map_err(|error| SymphonyError::prompt("liquid_parser", error.to_string()))?
        .parse(template)
        .map_err(|error| SymphonyError::prompt("liquid_parse", error.to_string()))?;
    let rendered_conditionals = render_attempt_conditionals(template, attempt)?;
    render_variables(&rendered_conditionals, issue, attempt)
}

fn render_attempt_conditionals(template: &str, attempt: Option<u32>) -> Result<String> {
    let re = Regex::new(r"(?s)\{%\s*if\s+attempt\s*%\}(?P<then>.*?)(?:\{%\s*else\s*%\}(?P<else>.*?))?\{%\s*endif\s*%\}").unwrap();
    let mut output = template.to_string();
    while let Some(caps) = re.captures(&output) {
        let whole = caps.get(0).unwrap();
        let selected = if attempt.is_some() {
            caps.name("then").map(|m| m.as_str()).unwrap_or("")
        } else {
            caps.name("else").map(|m| m.as_str()).unwrap_or("")
        }
        .to_string();
        let range = whole.start()..whole.end();
        output.replace_range(range, &selected);
    }
    let tag_re = Regex::new(r"\{%\s*([^%]+?)\s*%\}").unwrap();
    if let Some(caps) = tag_re.captures(&output) {
        return Err(SymphonyError::prompt(
            "unknown_tag",
            format!("unsupported liquid tag {:?}", caps.get(1).unwrap().as_str()),
        ));
    }
    Ok(output)
}

fn render_variables(template: &str, issue: &Issue, attempt: Option<u32>) -> Result<String> {
    let re = Regex::new(r"\{\{\s*([^}]+?)\s*\}\}").unwrap();
    let mut rendered = String::with_capacity(template.len());
    let mut last = 0usize;
    for caps in re.captures_iter(template) {
        let whole = caps.get(0).unwrap();
        rendered.push_str(&template[last..whole.start()]);
        let expr = caps.get(1).unwrap().as_str().trim();
        rendered.push_str(&resolve_expr(expr, issue, attempt)?);
        last = whole.end();
    }
    rendered.push_str(&template[last..]);
    Ok(rendered)
}

fn resolve_expr(expr: &str, issue: &Issue, attempt: Option<u32>) -> Result<String> {
    if expr.contains('|') {
        return Err(SymphonyError::prompt(
            "unknown_filter",
            format!("filters are not enabled in the strict prompt renderer: {expr}"),
        ));
    }
    match expr {
        "attempt" => Ok(attempt.map(|value| value.to_string()).unwrap_or_default()),
        "issue.id" => Ok(issue.id.clone()),
        "issue.identifier" => Ok(issue.identifier.clone()),
        "issue.title" => Ok(issue.title.clone()),
        "issue.state" => Ok(issue.state.clone()),
        "issue.description" => Ok(issue.description.clone().unwrap_or_default()),
        "issue.priority" => Ok(issue
            .priority
            .map(|value| value.to_string())
            .unwrap_or_default()),
        "issue.branch_name" => Ok(issue.branch_name.clone().unwrap_or_default()),
        "issue.url" => Ok(issue.url.clone()),
        "issue.labels" => Ok(issue.labels.join(", ")),
        "issue.blocked_by" => Ok(issue.blocked_by.join(", ")),
        "issue.created_at" => Ok(issue.created_at.to_rfc3339()),
        "issue.updated_at" => Ok(issue.updated_at.to_rfc3339()),
        _ => Err(SymphonyError::prompt(
            "unknown_variable",
            format!("unknown prompt variable {expr:?}"),
        )),
    }
}
