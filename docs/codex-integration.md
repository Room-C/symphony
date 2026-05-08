# Codex Integration

Symphony launches Codex with:

```text
bash -lc "<codex.command>"
```

The process cwd is always the per-issue workspace. The client uses app-server JSON-RPC over stdio and sends:

1. `initialize`
2. `thread/start`
3. one or more `turn/start` calls on the same thread

`build.rs` attempts to run:

```bash
codex app-server generate-json-schema --experimental --out "$OUT_DIR/codex-schemas"
```

If Codex is missing at build time, a minimal fallback schema marker is written so the crate remains buildable.

The `github_issue` dynamic tool is advertised. Full protocol-specific dynamic tool-call response handling should be verified against the active Codex app-server version before production use.
