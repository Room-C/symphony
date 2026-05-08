use std::path::Path;

use symphony::path_safety::{ensure_workspace_child, sanitize_workspace_key};
use symphony::retry::next_backoff;

#[test]
fn sanitizes_workspace_key_per_spec() {
    assert_eq!(
        sanitize_workspace_key("Room-C/symphony#42: fix auth"),
        "Room-C_symphony_42__fix_auth"
    );
}

#[test]
fn keeps_workspace_inside_root() {
    let dir = tempfile::tempdir().unwrap();
    let path = ensure_workspace_child(dir.path(), "Room-C_symphony_42").unwrap();
    assert!(path.starts_with(dir.path()));
}

#[test]
fn rejects_separator_in_workspace_key() {
    let dir = tempfile::tempdir().unwrap();
    let err = ensure_workspace_child(dir.path(), "../escape").unwrap_err();
    assert!(err.to_string().contains("workspace"));
}

#[test]
fn spec_failure_backoff_uses_ten_second_base_with_jitter() {
    let first = next_backoff(1, 300_000).as_millis();
    assert!((7_500..=10_000).contains(&first), "first={first}");

    let second = next_backoff(2, 300_000).as_millis();
    assert!((15_000..=20_000).contains(&second), "second={second}");

    let capped = next_backoff(30, 30_000).as_millis();
    assert!((22_500..=30_000).contains(&capped), "capped={capped}");
}

#[test]
fn relative_root_is_normalized_against_current_dir_for_safety_helpers() {
    let path = ensure_workspace_child(Path::new("target/test-workspaces"), "issue").unwrap();
    assert!(path.is_absolute());
}
