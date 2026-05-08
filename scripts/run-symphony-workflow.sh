#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <WORKFLOW.md>" >&2
  exit 64
fi

workflow="$1"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:${PATH:-}"

env_file="${SYMPHONY_ENV_FILE:-$HOME/.config/symphony/env}"
if [ -f "$env_file" ]; then
  set -a
  # shellcheck disable=SC1090
  source "$env_file"
  set +a
fi

gh_bin="${SYMPHONY_GH:-$(command -v gh || true)}"
if [ -z "$gh_bin" ]; then
  echo "gh not found. Install GitHub CLI or set SYMPHONY_GH in $env_file." >&2
  exit 127
fi

if [ -z "${GITHUB_TOKEN:-}" ]; then
  GITHUB_TOKEN="$("$gh_bin" auth token)"
  export GITHUB_TOKEN
fi

if [ -z "${GH_TOKEN:-}" ]; then
  GH_TOKEN="$GITHUB_TOKEN"
  export GH_TOKEN
fi

export RUST_LOG="${RUST_LOG:-symphony=info,info}"

cd "$repo_root"
exec "$repo_root/target/release/symphony" run --workflow "$workflow"
