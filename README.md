# Symphony

Rust implementation of the Symphony service specification.

Symphony is a long-running daemon that polls GitHub Issues, creates one isolated workspace per issue, and runs Codex app-server in that workspace until the issue leaves an active state or retry policy takes over.

## Quick Start

```bash
cargo build
export GITHUB_TOKEN=...
cargo run -- run --workflow WORKFLOW.md
```

Check configuration without starting the daemon:

```bash
cargo run -- check --workflow WORKFLOW.md
```

Status surfaces:

- `GET /health`
- `GET /status`
- `GET /api/v1/state`
- `GET /api/v1/<url-encoded issue identifier>`
- `POST /api/v1/refresh`

## Workflow Contract

Runtime behavior is configured by `WORKFLOW.md`, using YAML front matter plus a Liquid-style prompt body.

Important fields:

- `tracker.kind: github`
- `tracker.mode: labels | projects_v2`
- `tracker.api_key: $GITHUB_TOKEN`
- `tracker.active_states`
- `tracker.terminal_states`
- `workspace.root`
- `hooks.after_create`, `hooks.before_run`, `hooks.after_run`, `hooks.before_remove`
- `agent.max_concurrent_agents`
- `agent.max_turns`
- `codex.command`
- `observability.http_bind`

See `examples/` for labels and Projects v2 examples.

## Implementation Status

Implemented:

- WORKFLOW parser and typed config defaults.
- Strict prompt rendering for supported `issue.*` and `attempt` variables.
- Workspace key sanitization and path safety checks.
- GitHub labels-mode tracker reads and basic writes.
- GitHub Projects v2 reads.
- Orchestrator state, dispatch quotas, reconcile, and retry scheduling.
- Codex app-server JSON-RPC client boundary with schema generation fallback.
- JSON logs and HTTP status API.

Known v0.1 limitations:

- Projects v2 write operations are explicitly rejected until field-option mutation lookup is added.
- Codex dynamic `github_issue` tool advertisement is present, but protocol-specific tool-call response handling needs a real app-server integration pass.
- Runtime state is in memory, as allowed by the spec; restart recovery is tracker/filesystem driven.

## Verification

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build --release
```
