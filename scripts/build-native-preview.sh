#!/usr/bin/env bash
# Project-local dependencies only. Does not install into ~/.local or /usr.
set -euo pipefail
np_repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$np_repo"
np_base="$np_repo/target/native-preview"
np_prefix="$np_base/prefix"
mkdir -p "$np_base/src" "$np_base/debs" "$np_prefix"
if [[ "${1:-}" == "--download-deps" ]]; then
  (
    cd "$np_base/debs"
    while read -r checksum filename; do
      if [[ ! -f "$filename" ]]; then
        IFS=_ read -r package version architecture <<< "$filename"
        apt-get download "$package=$version"
      fi
    done < "$np_repo/native/debs.sha256"
  )
fi
if [[ -f "$np_base/debs/libjson-glib-dev_1.10.8+ds-2_amd64.deb" ]]; then
  (cd "$np_base/debs" && sha256sum --check "$np_repo/native/debs.sha256")
  while read -r np_checksum np_filename; do
    dpkg-deb -x "$np_base/debs/$np_filename" "$np_prefix"
  done < "$np_repo/native/debs.sha256"
  # Relocate only the extracted dependency metadata, never host pkg-config.
  sed -i "s|^prefix=/usr|prefix=$np_prefix/usr|" "$np_prefix"/usr/lib/x86_64-linux-gnu/pkgconfig/*.pc
  sed -i "s|^g_ir_scanner=.*|g_ir_scanner=$np_prefix/usr/bin/g-ir-scanner|;s|^g_ir_compiler=.*|g_ir_compiler=$np_prefix/usr/bin/g-ir-compiler|" "$np_prefix/usr/lib/x86_64-linux-gnu/pkgconfig/gobject-introspection-1.0.pc"
fi
export PATH="$np_prefix/usr/bin:$PATH"
export PKG_CONFIG_PATH="$np_prefix/lib/pkgconfig:$np_prefix/usr/lib/x86_64-linux-gnu/pkgconfig"
export LD_LIBRARY_PATH="$np_prefix/lib:$np_prefix/usr/lib/x86_64-linux-gnu"
export PYTHONPATH="$np_prefix/usr/lib/x86_64-linux-gnu/gobject-introspection"
export XDG_DATA_DIRS="$np_prefix/usr/share:/usr/share"
pkg-config --atleast-version=4.22.2 gtk4 || { echo 'GTK >= 4.22.2 required; host GTK is not upgraded automatically.' >&2; exit 1; }
np_source() {
  local name="$1" url="$2" revision="$3"
  if [[ ! -d "$np_base/src/$name/.git" ]]; then
    git init "$np_base/src/$name"
    git -C "$np_base/src/$name" fetch --depth=1 "$url" "$revision"
    git -C "$np_base/src/$name" switch --detach FETCH_HEAD
  fi
  [[ "$(git -C "$np_base/src/$name" rev-parse HEAD)" == "$revision" ]] || { echo "Unexpected $name checkout; not overwriting it" >&2; exit 1; }
}
np_source wlroots https://gitlab.freedesktop.org/wlroots/wlroots.git d783533489e1f75d6886c2ab5c5960090ef268f8
np_source casilda https://gitlab.gnome.org/jpu/casilda.git 0c6ff1490a0199024b30bf9a237b5d41856a0e81
np_source ptyxis https://gitlab.gnome.org/chergert/ptyxis.git 0f045a653db114e22fe7bf4b409fcb30d7763fa9
for np_pair in 'casilda casilda-input-bridge' 'ptyxis ptyxis-private-scope'; do
  read -r np_name np_patch <<< "$np_pair"
  if git -C "$np_base/src/$np_name" apply --reverse --check "$np_repo/native/patches/$np_patch.patch" 2>/dev/null; then :
  else git -C "$np_base/src/$np_name" apply --check "$np_repo/native/patches/$np_patch.patch"
       git -C "$np_base/src/$np_name" apply "$np_repo/native/patches/$np_patch.patch"
  fi
done
np_build() {
  local name="$1"; shift
  if [[ ! -f "$np_base/build-$name/build.ninja" ]]; then
    meson setup "$np_base/build-$name" "$np_base/src/$name" --prefix "$np_prefix" --libdir lib "$@"
  fi
  meson install -C "$np_base/build-$name"
}
np_build wlroots -Dbackends=[] -Drenderers=[] -Dxwayland=disabled -Dexamples=false -Dsession=disabled
np_build casilda
np_build ptyxis
mkdir -p "$np_prefix/libexec" "$np_prefix/share/termimochi-native"
mkdir -p "$np_prefix/share/licenses/termimochi-native"
install -m 644 "$np_base/src/casilda/COPYING" "$np_prefix/share/licenses/termimochi-native/casilda-COPYING"
install -m 644 "$np_base/src/ptyxis/COPYING" "$np_prefix/share/licenses/termimochi-native/ptyxis-COPYING"
install -m 644 "$np_base/src/wlroots/LICENSE" "$np_prefix/share/licenses/termimochi-native/wlroots-LICENSE"
install -m 644 native/session-supervisor.py "$np_prefix/libexec/session-supervisor.py"
install -m 644 "$np_base/src/ptyxis/src/palettes/gnome.palette" "$np_prefix/share/termimochi-native/gnome.palette"
cc -Wall -Wextra -Werror native/settings-helper.c $(pkg-config --cflags --libs gio-2.0) -o "$np_prefix/bin/termimochi-settings-helper"
cc -Wall -Wextra -Werror native/probe-host.c $(pkg-config --cflags --libs casilda-1.0) -Wl,-rpath,"$np_prefix/lib" -o "$np_base/probe-host"
echo "Local native dependencies ready: $np_prefix"
