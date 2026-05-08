use std::fs;
use std::path::{Path, PathBuf};

use serde_yaml::Value;

use crate::config::Config;
use crate::error::{Result, SymphonyError};

#[derive(Debug, Clone)]
pub struct Workflow {
    pub path: PathBuf,
    pub config: Config,
    pub prompt_template: String,
}

impl Workflow {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(SymphonyError::WorkflowNotFound {
                path: path.to_path_buf(),
            });
        }
        let content = fs::read_to_string(path)?;
        Self::parse(path, &content)
    }

    pub fn parse(path: impl AsRef<Path>, content: &str) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let (front_matter, prompt_template) = split_front_matter(content)?;
        let yaml_value: Value = serde_yaml::from_str(front_matter)?;
        if !matches!(yaml_value, Value::Mapping(_)) {
            return Err(SymphonyError::FrontMatterNotMap);
        }
        let config: Config = serde_yaml::from_value(yaml_value)?;
        let workflow_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let config = config.resolve(workflow_dir)?;
        if prompt_template.trim().is_empty() {
            return Err(SymphonyError::EmptyPrompt);
        }
        Ok(Self {
            path,
            config,
            prompt_template: prompt_template.to_string(),
        })
    }
}

fn split_front_matter(content: &str) -> Result<(&str, &str)> {
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return Err(SymphonyError::MissingFrontMatter);
    }
    let mut offset = 4usize;
    for line in lines {
        if line == "---" {
            let front_matter = &content[4..offset - 1];
            let prompt = content[offset + 3..].trim_start_matches(['\r', '\n']);
            return Ok((front_matter, prompt));
        }
        offset += line.len() + 1;
    }
    Err(SymphonyError::MissingFrontMatter)
}

#[cfg(test)]
mod tests {
    use super::split_front_matter;

    #[test]
    fn splits_front_matter() {
        let (yaml, prompt) = split_front_matter("---\na: b\n---\nhello").unwrap();
        assert_eq!(yaml, "a: b");
        assert_eq!(prompt, "hello");
    }
}
