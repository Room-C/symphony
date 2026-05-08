use std::fs;

use symphony::Workflow;

#[test]
fn parses_front_matter_and_prompt_with_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("WORKFLOW.md");
    fs::write(
        &path,
        r#"---
tracker:
  kind: github
  mode: labels
  owner: Room-C
  repo: symphony
  api_key: literal-token
unknown_extension:
  ignored: true
workspace:
  root: workspaces
---
Handle {{ issue.identifier }}.
"#,
    )
    .unwrap();

    let workflow = Workflow::load(&path).unwrap();

    assert_eq!(workflow.config.tracker.owner.as_deref(), Some("Room-C"));
    assert_eq!(workflow.config.polling.interval_ms, 30_000);
    assert!(workflow.config.workspace.root.ends_with("workspaces"));
    assert!(workflow.prompt_template.contains("issue.identifier"));
}

#[test]
fn rejects_non_map_front_matter() {
    let err = Workflow::parse("WORKFLOW.md", "---\n- nope\n---\nprompt").unwrap_err();
    assert!(
        err.to_string()
            .contains("front matter must decode to a map")
    );
}

#[test]
fn rejects_empty_prompt() {
    let err = Workflow::parse(
        "WORKFLOW.md",
        r#"---
tracker:
  kind: github
  mode: labels
  owner: Room-C
  repo: symphony
  api_key: token
---
"#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("prompt body is empty"));
}
