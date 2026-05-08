---
tracker:
  kind: github
  mode: labels
  owner: Room-C
  repo: your-repo
  api_key: $GITHUB_TOKEN
  active_states: [Todo, "In Progress", Rework]
  terminal_states: [Done, Closed, Cancelled]
workspace:
  root: ~/code/symphony-workspaces
agent:
  max_concurrent_agents: 2
  max_turns: 6
codex:
  command: codex app-server
---
Handle {{ issue.identifier }}.
