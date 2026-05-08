use symphony::config::{TrackerConfig, TrackerKind, TrackerMode};
use symphony::tracker::Tracker;
use symphony::tracker::github::labels::GithubLabelsTracker;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn labels_mode_normalizes_candidate_issues() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/Room-C/symphony/issues"))
        .and(query_param("state", "open"))
        .and(query_param("labels", "symphony:todo"))
        .and(query_param("per_page", "50"))
        .and(header("authorization", "Bearer token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "node_id": "I_kw1",
                "number": 42,
                "title": "Implement worker",
                "body": "- [ ] #7\n- [ ] Other/repo#8",
                "html_url": "https://github.com/Room-C/symphony/issues/42",
                "labels": [
                    { "name": "symphony:todo" },
                    { "name": "priority:2" },
                    { "name": "backend" }
                ],
                "created_at": "2026-05-08T00:00:00Z",
                "updated_at": "2026-05-08T01:00:00Z"
            }
        ])))
        .mount(&server)
        .await;

    let tracker = GithubLabelsTracker::new(&TrackerConfig {
        kind: TrackerKind::Github,
        mode: TrackerMode::Labels,
        owner: Some("Room-C".to_string()),
        repo: Some("symphony".to_string()),
        org: None,
        project_number: None,
        status_field: "Status".to_string(),
        api_key: Some("token".to_string()),
        endpoint: server.uri(),
        active_states: vec!["Todo".to_string()],
        terminal_states: vec!["Done".to_string()],
    })
    .unwrap();

    let issues = tracker.fetch_candidate_issues().await.unwrap();

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].identifier, "Room-C/symphony#42");
    assert_eq!(issues[0].state, "todo");
    assert_eq!(issues[0].priority, Some(2));
    assert_eq!(issues[0].labels, vec!["backend"]);
    assert_eq!(
        issues[0].blocked_by,
        vec!["Room-C/symphony#7", "Other/repo#8"]
    );
}
