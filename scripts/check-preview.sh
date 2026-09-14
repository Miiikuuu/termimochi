#!/usr/bin/env bash
set -euo pipefail
preview_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$preview_root"
test -f PREVIEW.json
sha256sum --check SHA256SUMS
for binary in target/release/termimochi target/release/termimochi-cli; do
  output="$(ldd "$binary")"
  printf '%s\n' "$output"
  if [[ "$output" == *'not found'* ]]; then
    printf 'Missing system libraries; do not install this build.\n' >&2
    exit 1
  fi
done
printf 'Package integrity and library resolution passed; desktop usability is not verified.\n'
