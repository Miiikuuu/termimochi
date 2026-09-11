#!/usr/bin/env bash
# Launch the built GUI with disposable application/configuration state.
# This is state isolation, NOT a filesystem sandbox: only open test copies.
set -euo pipefail
task_repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
task_binary="$task_repo/target/release/termimochi"
if [[ ! -x "$task_binary" ]]; then
  echo 'Build first: cargo build --workspace --release --locked' >&2
  exit 1
fi
if [[ "${1:-}" == "--help" ]]; then
  echo 'Usage: bash scripts/run-isolated.sh [--session EXISTING_DIRECTORY] [DESIGN_OR_NATIVE_COPY]'
  echo 'Creates or reuses state under target/qa. Never install from this test session.'
  exit 0
fi
mkdir -p -- "$task_repo/target/qa"
if [[ "${1:-}" == "--session" ]]; then
  [[ $# -ge 2 ]] || { echo 'Missing session directory' >&2; exit 2; }
  task_session="$(realpath -e -- "$2")"
  [[ "$task_session" == "$task_repo/target/qa/session-"* && -f "$task_session/.termimochi-isolated" ]] || {
    echo 'Only a session created by this launcher may be reused.' >&2; exit 2;
  }
  shift 2
else
  task_session="$(mktemp -d "$task_repo/target/qa/session-XXXXXXXX")"
  touch -- "$task_session/.termimochi-isolated"
fi
mkdir -p -- "$task_session/home" "$task_session/config" "$task_session/data" "$task_session/state" "$task_session/cache"
echo "Isolated session: $task_session"
echo "Reopen: bash scripts/run-isolated.sh --session '$task_session'"
echo 'Only use copies in this session. Memory GSettings reset on each launch.'
echo 'No startup hook is enabled automatically. Session files are retained.'
# Keep the compositor socket for an ordinary visible desktop window. Each GUI
# and child terminal still get their own HOME, XDG state and D-Bus session.
exec env -u BASH_ENV -u ENV -u STARSHIP_CONFIG -u PTYXIS_PROFILE -u TERM_PROGRAM \
  HOME="$task_session/home" XDG_CONFIG_HOME="$task_session/config" \
  XDG_DATA_HOME="$task_session/data" XDG_STATE_HOME="$task_session/state" \
  XDG_CACHE_HOME="$task_session/cache" GSETTINGS_BACKEND=memory \
  dbus-run-session -- "$task_binary" "$@"
