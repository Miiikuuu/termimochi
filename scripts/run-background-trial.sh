#!/usr/bin/env bash
set -euo pipefail
bg_repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
export TERMIMOCHI_BACKGROUND_PREFIX="${TERMIMOCHI_BACKGROUND_PREFIX:-$bg_repo/target/background-runtime/prefix}"
bg_app="${TERMIMOCHI_BACKGROUND_APP:-$bg_repo/target/background-trial-build/release/termimochi}"
if [[ "${1:-}" != --workbench && ! -x "$TERMIMOCHI_BACKGROUND_PREFIX/bin/ptyxis" ]]; then
  echo 'Build the optional runtime first: bash scripts/build-background-runtime.sh' >&2; exit 1
fi
if [[ ! -x "$bg_app" ]]; then
  echo 'Build the App first: CARGO_TARGET_DIR=target/background-trial-build cargo build --release --locked -p termimochi' >&2; exit 1
fi
export LD_LIBRARY_PATH="$bg_repo/target/native-preview/prefix/lib:$bg_repo/target/native-preview/prefix/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
case "${1:-}" in
  '') exec "$bg_app" --ptyxis-background-trial ;;
  --workbench) exec "$bg_app" --background-trial-workbench ;;
  *) echo 'Usage: bash scripts/run-background-trial.sh [--workbench]' >&2; exit 2 ;;
esac
