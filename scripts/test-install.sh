#!/usr/bin/env bash

set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
REPOSITORY_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd -P)"
readonly REPOSITORY_DIR
readonly INSTALLER="$SCRIPT_DIR/install.sh"
readonly APP_ID="io.github.miiikuuu.termimochi"
readonly LEGACY_APP_ID="io.github.termimochi.TermiMochi"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/termimochi install test.XXXXXXXX")"
cleanup() {
  [[ -n "$test_root" && "$test_root" != "/" ]] || return
  rm -rf -- "$test_root"
}
trap cleanup EXIT

test_home="$test_root/home with spaces"
test_prefix="$test_root/prefix with spaces"
test_data="$test_root/data with spaces"
test_state="$test_root/state with spaces"
dry_prefix="$test_root/dry prefix"
dry_data="$test_root/dry data"
dry_state="$test_root/dry state"

mkdir -p \
  "$test_home" \
  "$test_data/applications" \
  "$test_data/icons/hicolor/64x64/apps" \
  "$test_data/unrelated"
printf 'legacy desktop\n' >"$test_data/applications/$LEGACY_APP_ID.desktop"
printf 'legacy icon\n' >"$test_data/icons/hicolor/64x64/apps/$LEGACY_APP_ID.png"
printf '[Desktop Entry]\nName=Unrelated\nExec=true\nType=Application\n' \
  >"$test_data/applications/unrelated.desktop"
printf 'keep me too\n' >"$test_data/unrelated/sentinel"

if [[ ! -x "$REPOSITORY_DIR/target/release/termimochi" ||
      ! -x "$REPOSITORY_DIR/target/release/termimochi-cli" ]]; then
  (cd -- "$REPOSITORY_DIR" && cargo build --workspace --release --locked)
fi

dry_output="$(
  HOME="$test_home" \
    XDG_DATA_HOME="$dry_data" \
    XDG_STATE_HOME="$dry_state" \
    CARGO_TARGET_DIR="$REPOSITORY_DIR/target" \
    "$INSTALLER" install \
      --prefix "$dry_prefix" \
      --data-home "$dry_data" \
      --state-home "$dry_state" \
      --no-build \
      --dry-run
)"
[[ "$dry_output" == *"TermiMochi was installed"* ]]
[[ ! -e "$dry_prefix" && ! -e "$dry_data" && ! -e "$dry_state" ]]

HOME="$test_home" \
  XDG_DATA_HOME="$test_data" \
  XDG_STATE_HOME="$test_state" \
  CARGO_TARGET_DIR="$REPOSITORY_DIR/target" \
  "$INSTALLER" install \
    --prefix "$test_prefix" \
    --data-home "$test_data" \
    --state-home "$test_state" \
    --no-build

cmp "$REPOSITORY_DIR/target/release/termimochi" "$test_prefix/bin/termimochi"
cmp "$REPOSITORY_DIR/target/release/termimochi-cli" "$test_prefix/bin/termimochi-cli"
cmp \
  "$REPOSITORY_DIR/data/$APP_ID.metainfo.xml" \
  "$test_data/metainfo/$APP_ID.metainfo.xml"
grep --fixed-strings --line-regexp --quiet \
  "Exec=\"$test_prefix/bin/termimochi\" %f" \
  "$test_data/applications/$APP_ID.desktop"
if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate "$test_data/applications/$APP_ID.desktop"
fi

icon_count=0
while IFS= read -r -d '' source; do
  relative="${source#"$REPOSITORY_DIR/data/icons/hicolor/"}"
  cmp "$source" "$test_data/icons/hicolor/$relative"
  ((icon_count += 1))
done < <(find "$REPOSITORY_DIR/data/icons/hicolor" -type f -print0)
[[ "$icon_count" -eq 11 ]]

[[ ! -e "$test_data/applications/$LEGACY_APP_ID.desktop" ]]
[[ ! -e "$test_data/icons/hicolor/64x64/apps/$LEGACY_APP_ID.png" ]]
legacy_backup="$(find "$test_state/termimochi/legacy-backups" \
  -mindepth 1 -maxdepth 1 -type d -print -quit)"
[[ -n "$legacy_backup" ]]
grep --fixed-strings --line-regexp --quiet \
  'legacy desktop' \
  "$legacy_backup/applications/$LEGACY_APP_ID.desktop"
grep --fixed-strings --line-regexp --quiet \
  'legacy icon' \
  "$legacy_backup/icons/hicolor/64x64/apps/$LEGACY_APP_ID.png"

HOME="$test_home" \
  XDG_DATA_HOME="$test_data" \
  XDG_STATE_HOME="$test_state" \
  CARGO_TARGET_DIR="$REPOSITORY_DIR/target" \
  "$INSTALLER" uninstall \
    --prefix "$test_prefix" \
    --data-home "$test_data" \
    --state-home "$test_state"

[[ ! -e "$test_prefix/bin/termimochi" ]]
[[ ! -e "$test_prefix/bin/termimochi-cli" ]]
[[ ! -e "$test_data/applications/$APP_ID.desktop" ]]
[[ ! -e "$test_data/metainfo/$APP_ID.metainfo.xml" ]]
while IFS= read -r -d '' source; do
  relative="${source#"$REPOSITORY_DIR/data/icons/hicolor/"}"
  [[ ! -e "$test_data/icons/hicolor/$relative" ]]
done < <(find "$REPOSITORY_DIR/data/icons/hicolor" -type f -print0)

[[ -f "$test_data/applications/unrelated.desktop" ]]
[[ -f "$test_data/unrelated/sentinel" ]]
[[ -f "$legacy_backup/applications/$LEGACY_APP_ID.desktop" ]]

# Uninstall is intentionally idempotent and must still stay inside the same
# explicitly supplied roots.
HOME="$test_home" \
  XDG_DATA_HOME="$test_data" \
  XDG_STATE_HOME="$test_state" \
  CARGO_TARGET_DIR="$REPOSITORY_DIR/target" \
  "$INSTALLER" uninstall \
    --prefix "$test_prefix" \
    --data-home "$test_data" \
    --state-home "$test_state"

printf 'isolated install/uninstall test passed\n'
