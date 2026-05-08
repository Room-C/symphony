---
tracker:
  kind: github
  mode: labels
  owner: Room-C
  repo: symphony
  api_key: $GITHUB_TOKEN
  active_states: [Todo, "In Progress", Rework]
  terminal_states: [Done, Closed, Cancelled]
polling:
  interval_ms: 30000
workspace:
  root: ~/code/symphony-workspaces
hooks:
  timeout_ms: 60000
  after_create: |
    git clone --depth 1 git@github.com:Room-C/symphony.git .
    cargo fetch
agent:
  max_concurrent_agents: 3
  max_turns: 10
  max_retry_backoff_ms: 300000
  max_concurrent_agents_by_state:
    "in progress": 2
codex:
  command: codex app-server
  approval_policy: never
  thread_sandbox: workspace-write
  turn_sandbox_policy: workspace-write
  read_timeout_ms: 5000
  turn_timeout_ms: 3600000
  stall_timeout_ms: 300000
observability:
  http_bind: 127.0.0.1:8723
  json_logs: true
---
You are working on GitHub issue `{{ issue.identifier }}`.

{% if attempt %}
Continue the previous unfinished run, attempt #{{ attempt }}. Use the current workspace state and move directly toward completion.
{% else %}
Title: {{ issue.title }}
State: {{ issue.state }}
URL: {{ issue.url }}

Description:
{{ issue.description }}

Tasks:
1. Read the issue and add a short tracking comment.
2. Implement the smallest correct change.
3. Run focused verification.
4. Open a pull request and move the issue to Human Review using the `github_issue` tool.
{% endif %}
