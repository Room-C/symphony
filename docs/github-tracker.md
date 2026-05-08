# GitHub Tracker

## Labels Mode

Candidate fetch runs one GitHub REST query per active state:

```text
GET /repos/{owner}/{repo}/issues?state=open&labels=symphony:{state}&per_page=50
```

Normalization:

- `issue.id`: GitHub `node_id`
- `issue.identifier`: `{owner}/{repo}#{number}`
- `issue.state`: first `symphony:*` label, default `Todo`
- `issue.priority`: `priority:N`
- `issue.labels`: labels excluding `symphony:*` and `priority:*`
- `issue.blocked_by`: unchecked task-list references like `- [ ] #7`

Writes supported in labels mode:

- `comment`
- `set_state`
- `close`
- `link_pr` as a comment

## Projects v2 Mode

Candidate fetch uses GitHub GraphQL organization Projects v2 data and maps the configured single-select status field to Symphony state.

Read support is implemented for v0.1. Writes are intentionally rejected with `unsupported_tracker_write` until project item field mutation lookup is implemented.
