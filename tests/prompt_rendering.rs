use chrono::Utc;
use symphony::events::Issue;
use symphony::prompt::render_prompt;

#[test]
fn renders_issue_and_attempt_variables() {
    let issue = issue();
    let rendered = render_prompt(
        "{% if attempt %}retry {{ attempt }} {{ issue.identifier }}{% else %}new {{ issue.title }}{% endif %}",
        &issue,
        Some(2),
    )
    .unwrap();
    assert_eq!(rendered.trim(), "retry 2 Room-C/symphony#1");
}

#[test]
fn fails_unknown_variable() {
    let err = render_prompt("{{ issue.missing }}", &issue(), None).unwrap_err();
    assert!(err.to_string().contains("unknown"));
}

#[test]
fn fails_unknown_filter() {
    let err = render_prompt("{{ issue.title | missing_filter }}", &issue(), None).unwrap_err();
    assert!(err.to_string().contains("filter") || err.to_string().contains("parse"));
}

fn issue() -> Issue {
    Issue {
        id: "I_kw".to_string(),
        identifier: "Room-C/symphony#1".to_string(),
        title: "Build it".to_string(),
        state: "Todo".to_string(),
        description: Some("body".to_string()),
        priority: Some(1),
        branch_name: None,
        url: "https://github.com/Room-C/symphony/issues/1".to_string(),
        labels: vec!["bug".to_string()],
        blocked_by: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
