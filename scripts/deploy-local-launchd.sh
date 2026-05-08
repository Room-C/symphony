#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
runner="$repo_root/scripts/run-symphony-workflow.sh"
binary="$repo_root/target/release/symphony"
launch_agents_dir="$HOME/Library/LaunchAgents"
log_dir="$HOME/Library/Logs/symphony"
launch_domain="gui/$(id -u)"

service_records() {
  cat <<'SERVICES'
pactpilot|com.roomc.symphony.pactpilot|WORKFLOW.pactpilot.md|Room-C/PactPilot
backend|com.roomc.symphony.backend|WORKFLOW.backend.md|Room-C/PactPilot-Backend
officialsite|com.roomc.symphony.officialsite|WORKFLOW.officialsite.md|Room-C/PactPilot-OfficialSite
SERVICES
}

label_records() {
  cat <<'LABELS'
symphony:todo|0969DA|Ready for Symphony
symphony:in-progress|1D76DB|Currently handled by Symphony
symphony:rework|FBCA04|Needs another Symphony pass
symphony:human-review|8957E5|Waiting for human review
symphony:done|1F883D|Completed by Symphony
symphony:closed|BFDADC|Closed
symphony:cancelled|D93F0B|Cancelled
priority:1|B60205|Highest priority
priority:2|D93F0B|High priority
priority:3|FBCA04|Normal priority
priority:4|C2E0C6|Low priority
LABELS
}

usage() {
  cat <<'USAGE'
usage: scripts/deploy-local-launchd.sh <command> [options]

Commands:
  check                 Validate prerequisites and workflow files.
  install [--sync-labels]
                        Build release binary, install three launchd services, and start them.
  start                 Start the installed launchd services.
  stop                  Stop the launchd services.
  restart               Restart the launchd services.
  status                Print launchd status for each service.
  logs                  Tail Symphony launchd logs.
  sync-labels           Create or update Symphony labels in the three target repositories.
  uninstall             Stop services and remove generated plist files.

Optional environment:
  SYMPHONY_ENV_FILE     File sourced by the runner before startup. Defaults to ~/.config/symphony/env.
  SYMPHONY_GH           Absolute path to gh if it is not on PATH.

Example:
  scripts/deploy-local-launchd.sh install --sync-labels
  scripts/deploy-local-launchd.sh status
USAGE
}

die() {
  echo "error: $*" >&2
  exit 1
}

require_macos() {
  [ "$(uname -s)" = "Darwin" ] || die "macOS launchd deployment requires macOS"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "$1 is required but was not found on PATH"
}

gh_bin() {
  if [ -n "${SYMPHONY_GH:-}" ]; then
    printf '%s\n' "$SYMPHONY_GH"
  else
    command -v gh || true
  fi
}

xml_escape() {
  printf '%s' "$1" \
    | sed -e 's/&/\&amp;/g' \
          -e 's/</\&lt;/g' \
          -e 's/>/\&gt;/g' \
          -e 's/"/\&quot;/g' \
          -e "s/'/\&apos;/g"
}

plist_path_for_label() {
  printf '%s/com.roomc.%s.plist\n' "$launch_agents_dir" "$1"
}

ensure_prerequisites() {
  require_macos
  require_command cargo
  require_command codex
  require_command git
  require_command launchctl

  local gh_path
  gh_path="$(gh_bin)"
  [ -n "$gh_path" ] || die "gh is required but was not found on PATH"
  [ -x "$gh_path" ] || die "gh exists but is not executable: $gh_path"

  "$gh_path" auth token >/dev/null
  codex app-server --help >/dev/null
}

build_release() {
  echo "==> Building Symphony release binary"
  (cd "$repo_root" && cargo build --release)
}

check_workflows() {
  local token gh_path name label workflow repo
  gh_path="$(gh_bin)"
  token="$("$gh_path" auth token)"

  while IFS='|' read -r name label workflow repo; do
    [ -n "$name" ] || continue
    [ -f "$repo_root/$workflow" ] || die "workflow not found: $workflow"
    echo "==> Checking $workflow for $repo"
    GITHUB_TOKEN="$token" "$binary" check --workflow "$repo_root/$workflow"
  done < <(service_records)
}

write_plist() {
  local name="$1"
  local label="$2"
  local workflow="$3"
  local plist="$4"
  local gh_path path_value

  gh_path="$(gh_bin)"
  path_value="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

  cat >"$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$(xml_escape "$label")</string>
  <key>ProgramArguments</key>
  <array>
    <string>$(xml_escape "$runner")</string>
    <string>$(xml_escape "$workflow")</string>
  </array>
  <key>WorkingDirectory</key>
  <string>$(xml_escape "$repo_root")</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>$(xml_escape "$path_value")</string>
    <key>SYMPHONY_GH</key>
    <string>$(xml_escape "$gh_path")</string>
    <key>RUST_LOG</key>
    <string>symphony=info,info</string>
  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>$(xml_escape "$log_dir/$name.out.log")</string>
  <key>StandardErrorPath</key>
  <string>$(xml_escape "$log_dir/$name.err.log")</string>
</dict>
</plist>
EOF
}

install_plists() {
  local name label workflow repo plist
  mkdir -p "$launch_agents_dir" "$log_dir"

  while IFS='|' read -r name label workflow repo; do
    [ -n "$name" ] || continue
    plist="$(plist_path_for_label "$label")"
    echo "==> Writing $plist"
    write_plist "$name" "$label" "$workflow" "$plist"
  done < <(service_records)
}

stop_services() {
  local name label workflow repo plist
  while IFS='|' read -r name label workflow repo; do
    [ -n "$name" ] || continue
    plist="$(plist_path_for_label "$label")"
    echo "==> Stopping $label"
    launchctl bootout "$launch_domain" "$plist" >/dev/null 2>&1 || true
  done < <(service_records)
}

start_services() {
  local name label workflow repo plist
  while IFS='|' read -r name label workflow repo; do
    [ -n "$name" ] || continue
    plist="$(plist_path_for_label "$label")"
    [ -f "$plist" ] || die "plist not found: $plist. Run install first."
    echo "==> Starting $label"
    launchctl bootout "$launch_domain" "$plist" >/dev/null 2>&1 || true
    launchctl bootstrap "$launch_domain" "$plist"
    launchctl enable "$launch_domain/$label" >/dev/null 2>&1 || true
    launchctl kickstart -k "$launch_domain/$label" >/dev/null 2>&1 || true
  done < <(service_records)
}

status_services() {
  local name label workflow repo
  while IFS='|' read -r name label workflow repo; do
    [ -n "$name" ] || continue
    if launchctl print "$launch_domain/$label" >/dev/null 2>&1; then
      echo "$label: loaded"
    else
      echo "$label: not loaded"
    fi
  done < <(service_records)
}

remove_plists() {
  local name label workflow repo plist
  while IFS='|' read -r name label workflow repo; do
    [ -n "$name" ] || continue
    plist="$(plist_path_for_label "$label")"
    if [ -f "$plist" ]; then
      echo "==> Removing $plist"
      rm -f "$plist"
    fi
  done < <(service_records)
}

sync_labels() {
  local gh_path repo label color description
  gh_path="$(gh_bin)"
  [ -n "$gh_path" ] || die "gh is required but was not found on PATH"
  "$gh_path" auth token >/dev/null

  while IFS='|' read -r _name _service_label _workflow repo; do
    [ -n "$repo" ] || continue
    echo "==> Syncing labels for $repo"
    while IFS='|' read -r label color description; do
      [ -n "$label" ] || continue
      if "$gh_path" label view "$label" --repo "$repo" >/dev/null 2>&1; then
        "$gh_path" label edit "$label" --repo "$repo" --color "$color" --description "$description"
      else
        "$gh_path" label create "$label" --repo "$repo" --color "$color" --description "$description"
      fi
    done < <(label_records)
  done < <(service_records)
}

tail_logs() {
  mkdir -p "$log_dir"
  local files=("$log_dir"/*.log)
  if [ ! -e "${files[0]}" ]; then
    echo "No logs found in $log_dir yet."
    return
  fi
  tail -n 80 -f "${files[@]}"
}

run_check() {
  ensure_prerequisites
  build_release
  check_workflows
}

install_all() {
  local should_sync_labels="$1"
  run_check
  install_plists
  stop_services
  start_services
  if [ "$should_sync_labels" = "1" ]; then
    sync_labels
  fi
  echo "==> Installed Symphony launchd services"
  status_services
}

main() {
  local command="${1:-help}"
  shift || true

  local should_sync_labels=0
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --sync-labels)
        should_sync_labels=1
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        die "unknown option: $1"
        ;;
    esac
    shift
  done

  case "$command" in
    check)
      run_check
      ;;
    install)
      install_all "$should_sync_labels"
      ;;
    start)
      require_macos
      start_services
      ;;
    stop)
      require_macos
      stop_services
      ;;
    restart)
      require_macos
      stop_services
      start_services
      ;;
    status)
      require_macos
      status_services
      ;;
    logs)
      tail_logs
      ;;
    sync-labels)
      sync_labels
      ;;
    uninstall)
      require_macos
      stop_services
      remove_plists
      ;;
    help|-h|--help)
      usage
      ;;
    *)
      usage
      exit 64
      ;;
  esac
}

main "$@"
