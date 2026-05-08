---
tracker:
  kind: github
  mode: labels
  owner: Room-C
  repo: PactPilot-OfficialSite
  api_key: $GITHUB_TOKEN
  active_states: [Todo, "In Progress", Rework]
  terminal_states: [Done, Closed, Cancelled]
polling:
  interval_ms: 30000
workspace:
  root: ~/code/symphony-workspaces/pactpilot-officialsite
hooks:
  timeout_ms: 60000
  after_create: |
    git clone --depth 1 git@github.com:Room-C/PactPilot-OfficialSite.git .
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
  thread_sandbox: danger-full-access
  turn_sandbox_policy: danger-full-access
  read_timeout_ms: 30000
  turn_timeout_ms: 3600000
  stall_timeout_ms: 300000
observability:
  http_bind: 127.0.0.1:8726
  json_logs: true
---
You are working on GitHub issue `{{ issue.identifier }}` in `Room-C/PactPilot-OfficialSite`.

{% if attempt %}
Continue the previous unfinished run, attempt #{{ attempt }}. First inspect `git status --short`, existing plan/tracking docs, and recent verification notes. Do not restart implementation if code and verification are already ready. Finish the remaining workflow steps: create or update the branch, commit and push the changes, open a PR, comment on the issue with summary, verification, and PR URL, then move the issue to Human Review. If an external operation fails, leave the workspace state intact and comment on the issue with the precise blocker when possible.
{% else %}
Title: {{ issue.title }}
State: {{ issue.state }}
URL: {{ issue.url }}

Description:
{{ issue.description }}

Workflow:
1. Read AGENTS.md, README.md, package scripts, and relevant deployment docs before editing.
2. Inspect the live app structure before deciding on a fix.
3. Implement the narrowest correct change for this issue.
4. Run focused verification. For frontend changes, use the repo's documented lint, typecheck, build, and browser checks when applicable.
5. If code changed, create a branch and open a PR.
6. Use the `github_issue` tool to add a concise result comment with summary, verification, screenshots or preview notes when relevant, and PR URL.
7. Move the issue to Human Review when a PR is ready. Move it to Done only when no PR is needed and verification passed.
{% endif %}
