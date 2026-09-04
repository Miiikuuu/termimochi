#!/usr/bin/env bash

set -euo pipefail
IFS=$'\n\t'

readonly APP_ID="io.github.miiikuuu.termimochi"
readonly LEGACY_APP_ID="io.github.termimochi.TermiMochi"

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
REPOSITORY_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd -P)"
readonly REPOSITORY_DIR

readonly -a ICON_PATHS=(
  "16x16/apps/$APP_ID.png"
  "24x24/apps/$APP_ID.png"
  "32x32/apps/$APP_ID.png"
  "48x48/apps/$APP_ID.png"
  "64x64/apps/$APP_ID.png"
  "128x128/apps/$APP_ID.png"
  "256x256/apps/$APP_ID.png"
  "512x512/apps/$APP_ID.png"
  "scalable/apps/$APP_ID.svg"
  "scalable/apps/termimochi-header-logo.svg"
  "symbolic/apps/$APP_ID-symbolic.svg"
)

readonly -a LEGACY_DATA_PATHS=(
  "applications/$LEGACY_APP_ID.desktop"
  "metainfo/$LEGACY_APP_ID.metainfo.xml"
  "appdata/$LEGACY_APP_ID.appdata.xml"
  "icons/hicolor/16x16/apps/$LEGACY_APP_ID.png"
  "icons/hicolor/24x24/apps/$LEGACY_APP_ID.png"
  "icons/hicolor/32x32/apps/$LEGACY_APP_ID.png"
  "icons/hicolor/48x48/apps/$LEGACY_APP_ID.png"
  "icons/hicolor/64x64/apps/$LEGACY_APP_ID.png"
  "icons/hicolor/128x128/apps/$LEGACY_APP_ID.png"
  "icons/hicolor/256x256/apps/$LEGACY_APP_ID.png"
  "icons/hicolor/512x512/apps/$LEGACY_APP_ID.png"
  "icons/hicolor/scalable/apps/$LEGACY_APP_ID.svg"
  "icons/hicolor/symbolic/apps/$LEGACY_APP_ID-symbolic.svg"
)

dry_run=false
skip_build=false
prefix_value="${PREFIX:-}"
data_home_value="${XDG_DATA_HOME:-}"
state_home_value="${XDG_STATE_HOME:-}"
desktop_temporary=""

usage() {
  cat <<'EOF'
Install or uninstall TermiMochi for one user.

Usage:
  scripts/install.sh install [OPTIONS]
  scripts/install.sh uninstall [OPTIONS]

Options:
  --prefix PATH       Binary prefix (default: $PREFIX or $HOME/.local)
  --data-home PATH    Desktop data root (default: $XDG_DATA_HOME or PREFIX/share)
  --state-home PATH   Migration backup root (default: $XDG_STATE_HOME or
                      $HOME/.local/state)
  --no-build          Install already-built release binaries
  --dry-run           Print every mutation without performing it
  -h, --help          Show this help

Use the same path options for uninstall that were used for install.
EOF
}

die() {
  printf 'TermiMochi install: %s\n' "$*" >&2
  exit 2
}

warn() {
  printf 'TermiMochi install: warning: %s\n' "$*" >&2
}

print_command() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'
}

run() {
  if "$dry_run"; then
    print_command "$@"
  else
    "$@"
  fi
}

cleanup() {
  if [[ -n "$desktop_temporary" && -e "$desktop_temporary" ]]; then
    rm -f -- "$desktop_temporary"
  fi
}
trap cleanup EXIT

require_value() {
  local option="$1"
  local value="${2:-}"
  [[ -n "$value" ]] || die "$option expects a non-empty path"
}

normalize_directory() {
  local label="$1"
  local value="$2"
  local normalized

  [[ -n "$value" ]] || die "$label is empty"
  [[ "$value" == /* ]] || die "$label must be an absolute path: $value"
  [[ "$value" != *$'\n'* && "$value" != *$'\r'* ]] ||
    die "$label cannot contain a newline"
  normalized="$(realpath --canonicalize-missing -- "$value")" ||
    die "cannot normalize $label: $value"
  [[ "$normalized" != "/" ]] || die "$label cannot be the filesystem root"
  printf '%s\n' "$normalized"
}

require_source_file() {
  [[ -f "$1" ]] || die "required source file is missing: $1"
}

desktop_quote() {
  local value="$1"
  local escaped=""
  local character
  local index

  for ((index = 0; index < ${#value}; index++)); do
    character="${value:index:1}"
    case "$character" in
      '"'|'`'|'$'|\\) escaped+="\\$character" ;;
      '%') escaped+='%%' ;;
      *) escaped+="$character" ;;
    esac
  done
  printf '"%s"' "$escaped"
}

render_desktop_file() {
  local source="$1"
  local destination="$2"
  local executable="$3"
  local line
  local replaced=false
  local quoted_executable
  quoted_executable="$(desktop_quote "$executable")"

  : >"$destination"
  while IFS= read -r line || [[ -n "$line" ]]; do
    if [[ "$line" == Exec=* ]]; then
      printf 'Exec=%s %%f\n' "$quoted_executable" >>"$destination"
      replaced=true
    else
      printf '%s\n' "$line" >>"$destination"
    fi
  done <"$source"
  "$replaced" || die "desktop template has no Exec entry"
}

refresh_caches() {
  local applications_dir="$1"
  local icon_theme_dir="$2"

  if [[ -d "$applications_dir" ]] || "$dry_run"; then
    if ! command -v update-desktop-database >/dev/null 2>&1; then
      warn "update-desktop-database is unavailable; desktop caches were not refreshed"
    elif "$dry_run"; then
      print_command update-desktop-database "$applications_dir"
    elif ! update-desktop-database "$applications_dir"; then
      warn "could not refresh the desktop database"
    fi
  fi

  if [[ -d "$icon_theme_dir" ]] || "$dry_run"; then
    if ! command -v gtk-update-icon-cache >/dev/null 2>&1; then
      warn "gtk-update-icon-cache is unavailable; icon caches were not refreshed"
    elif "$dry_run"; then
      print_command gtk-update-icon-cache --force --ignore-theme-index "$icon_theme_dir"
    elif ! gtk-update-icon-cache --force --ignore-theme-index "$icon_theme_dir"; then
      warn "could not refresh the hicolor icon cache"
    fi
  fi
}

migrate_legacy_installation() {
  local data_home="$1"
  local state_home="$2"
  local backup_parent="$state_home/termimochi/legacy-backups"
  local backup_dir=""
  local relative
  local source
  local destination
  local -a present=()

  for relative in "${LEGACY_DATA_PATHS[@]}"; do
    source="$data_home/$relative"
    if [[ -e "$source" || -L "$source" ]]; then
      present+=("$relative")
    fi
  done
  ((${#present[@]} > 0)) || return 0

  if "$dry_run"; then
    backup_dir="$backup_parent/$LEGACY_APP_ID.DRY-RUN"
    print_command install -d -m 0700 "$backup_dir"
  else
    install -d -m 0700 "$backup_parent"
    backup_dir="$(mktemp -d "$backup_parent/$LEGACY_APP_ID.XXXXXXXX")"
    chmod 0700 "$backup_dir"
  fi

  for relative in "${present[@]}"; do
    source="$data_home/$relative"
    destination="$backup_dir/$relative"
    run install -d -m 0700 "$(dirname -- "$destination")"
    run mv -- "$source" "$destination"
  done

  printf 'Legacy TermiMochi desktop files were backed up to %s\n' "$backup_dir"
}

remove_exact_file() {
  local path="$1"
  if [[ -e "$path" || -L "$path" ]]; then
    run rm -- "$path"
  fi
}

[[ $# -gt 0 ]] || {
  usage >&2
  exit 2
}

action="$1"
shift
case "$action" in
  install|uninstall) ;;
  -h|--help)
    usage
    exit 0
    ;;
  *) die "expected install or uninstall, got: $action" ;;
esac

while (($# > 0)); do
  case "$1" in
    --prefix)
      require_value "$1" "${2:-}"
      prefix_value="$2"
      shift 2
      ;;
    --data-home)
      require_value "$1" "${2:-}"
      data_home_value="$2"
      shift 2
      ;;
    --state-home)
      require_value "$1" "${2:-}"
      state_home_value="$2"
      shift 2
      ;;
    --no-build)
      skip_build=true
      shift
      ;;
    --dry-run)
      dry_run=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *) die "unknown option: $1" ;;
  esac
done

if [[ -z "$prefix_value" ]]; then
  [[ -n "${HOME:-}" ]] || die "HOME is unavailable; pass --prefix"
  prefix_value="$HOME/.local"
fi
prefix_value="$(normalize_directory PREFIX "$prefix_value")"

if [[ -z "$data_home_value" ]]; then
  data_home_value="$prefix_value/share"
fi
data_home_value="$(normalize_directory XDG_DATA_HOME "$data_home_value")"

if [[ -z "$state_home_value" ]]; then
  if [[ -n "${HOME:-}" ]]; then
    state_home_value="$HOME/.local/state"
  else
    state_home_value="$prefix_value/state"
  fi
fi
state_home_value="$(normalize_directory XDG_STATE_HOME "$state_home_value")"

binary_dir="$prefix_value/bin"
applications_dir="$data_home_value/applications"
metainfo_dir="$data_home_value/metainfo"
icon_theme_dir="$data_home_value/icons/hicolor"
desktop_source="$REPOSITORY_DIR/data/$APP_ID.desktop"
metainfo_source="$REPOSITORY_DIR/data/$APP_ID.metainfo.xml"
desktop_target="$applications_dir/$APP_ID.desktop"
metainfo_target="$metainfo_dir/$APP_ID.metainfo.xml"
installed_gui="$binary_dir/termimochi"
installed_cli="$binary_dir/termimochi-cli"

cargo_target_dir="${CARGO_TARGET_DIR:-$REPOSITORY_DIR/target}"
if [[ "$cargo_target_dir" != /* ]]; then
  cargo_target_dir="$REPOSITORY_DIR/$cargo_target_dir"
fi
cargo_target_dir="$(realpath --canonicalize-missing -- "$cargo_target_dir")"
gui_binary="$cargo_target_dir/release/termimochi"
cli_binary="$cargo_target_dir/release/termimochi-cli"

case "$action" in
  install)
    require_source_file "$desktop_source"
    require_source_file "$metainfo_source"
    for relative in "${ICON_PATHS[@]}"; do
      require_source_file "$REPOSITORY_DIR/data/icons/hicolor/$relative"
    done

    if ! "$skip_build"; then
      if "$dry_run"; then
        printf '+ cd %q &&' "$REPOSITORY_DIR"
        printf ' %q' cargo build --workspace --release --locked
        printf '\n'
      else
        (cd -- "$REPOSITORY_DIR" && cargo build --workspace --release --locked)
      fi
    fi
    if ! "$dry_run"; then
      [[ -x "$gui_binary" ]] || die "release GUI binary is missing: $gui_binary"
      [[ -x "$cli_binary" ]] || die "release CLI binary is missing: $cli_binary"
    fi

    run install -d -m 0755 "$binary_dir" "$applications_dir" "$metainfo_dir"
    run install -m 0755 "$gui_binary" "$installed_gui"
    run install -m 0755 "$cli_binary" "$installed_cli"

    if "$dry_run"; then
      printf '+ render desktop Exec=%q to %q\n' "$installed_gui" "$desktop_target"
    else
      desktop_temporary="$(mktemp --suffix=.desktop "${TMPDIR:-/tmp}/termimochi-desktop.XXXXXXXX")"
      render_desktop_file "$desktop_source" "$desktop_temporary" "$installed_gui"
      if command -v desktop-file-validate >/dev/null 2>&1; then
        desktop-file-validate "$desktop_temporary"
      fi
      install -m 0644 "$desktop_temporary" "$desktop_target"
      rm -f -- "$desktop_temporary"
      desktop_temporary=""
    fi
    run install -m 0644 "$metainfo_source" "$metainfo_target"

    for relative in "${ICON_PATHS[@]}"; do
      run install -D -m 0644 \
        "$REPOSITORY_DIR/data/icons/hicolor/$relative" \
        "$icon_theme_dir/$relative"
    done

    migrate_legacy_installation "$data_home_value" "$state_home_value"
    refresh_caches "$applications_dir" "$icon_theme_dir"
    printf 'TermiMochi was installed under %s\n' "$prefix_value"
    ;;
  uninstall)
    remove_exact_file "$installed_gui"
    remove_exact_file "$installed_cli"
    remove_exact_file "$desktop_target"
    remove_exact_file "$metainfo_target"
    for relative in "${ICON_PATHS[@]}"; do
      remove_exact_file "$icon_theme_dir/$relative"
    done
    refresh_caches "$applications_dir" "$icon_theme_dir"
    printf 'TermiMochi files were removed from %s\n' "$prefix_value"
    ;;
esac
