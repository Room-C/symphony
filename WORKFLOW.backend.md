---
tracker:
  kind: github
  mode: labels
  owner: Room-C
  repo: PactPilot-Backend
  api_key: $GITHUB_TOKEN
  active_states: [Todo, "In Progress", Rework]
  terminal_states: [Done, Closed, Cancelled]
polling:
  interval_ms: 30000
workspace:
  root: ~/code/symphony-workspaces/pactpilot-backend
hooks:
  timeout_ms: 60000
  after_create: |
    git clone --depth 1 git@github.com:Room-C/PactPilot-Backend.git .
  before_run: |
    git status --short
agent:
  max_concurrent_agents: 2
  max_turns: 10
  max_retry_backoff_ms: 300000
  max_concurrent_agents_by_state:
    "in progress": 1
codex:
  command: codex app-server
  approval_policy: never
  thread_sandbox: workspace-write
  turn_sandbox_policy: workspace-write
  read_timeout_ms: 5000
  turn_timeout_ms: 3600000
  stall_timeout_ms: 300000
observability:
  http_bind: 127.0.0.1:8725
  json_logs: true
---
You are working on GitHub issue `{{ issue.identifier }}` in `Room-C/PactPilot-Backend`.

{% if attempt %}
Continue the previous unfinished run, attempt #{{ attempt }}. Use the current workspace state and move directly toward completion.
{% else %}
Title: {{ issue.title }}
State: {{ issue.state }}
URL: {{ issue.url }}

Description:
{{ issue.description }}

Workflow:
1. Read AGENTS.md, CLAUDE.md, and backend docs before editing.
2. Trace the actual FastAPI, service, worker, schema, and test paths involved in the issue.
3. Implement the narrowest correct backend change. Keep API and database contracts explicit.
4. Run focused verification first. For backend code changes, use the repo's documented uv, ruff, mypy, pytest, and architecture checks when applicable.
5. If code changed, create a branch and open a PR.
6. Use the `github_issue` tool to add a concise result comment with summary, verification, and PR URL.
7. Move the issue to Human Review when a PR is ready. Move it to Done only when no PR is needed and verification passed.
{% endif %}
