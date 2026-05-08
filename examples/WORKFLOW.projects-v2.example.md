---
tracker:
  kind: github
  mode: projects_v2
  org: Room-C
  project_number: 1
  status_field: Status
  api_key: $GITHUB_TOKEN
  active_states: [Todo, "In Progress", Rework]
  terminal_states: [Done, Closed, Cancelled]
workspace:
  root: ~/code/symphony-project-workspaces
agent:
  max_concurrent_agents: 2
  max_turns: 6
codex:
  command: codex app-server
---
Handle {{ issue.identifier }} from GitHub Projects v2.
