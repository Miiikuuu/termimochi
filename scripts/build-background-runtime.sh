#!/usr/bin/env bash
# Optional local-only runtime. No apt install, desktop entry or system upgrade.
set -euo pipefail
bg_repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
bg_root="$bg_repo/target/background-runtime"
bg_deps="$bg_repo/target/native-preview/prefix"
bg_revision=0f045a653db114e22fe7bf4b409fcb30d7763fa9
mkdir -p "$bg_root"
if [[ ! -d "$bg_root/source/.git" ]]; then
  if [[ -d "$bg_repo/target/native-preview/src/ptyxis/.git" ]]; then
    git clone --no-hardlinks "$bg_repo/target/native-preview/src/ptyxis" "$bg_root/source"
  else
    git init "$bg_root/source"
    git -C "$bg_root/source" fetch --depth=1 https://gitlab.gnome.org/chergert/ptyxis.git "$bg_revision"
    git -C "$bg_root/source" switch --detach FETCH_HEAD
  fi
fi
[[ "$(git -C "$bg_root/source" rev-parse HEAD)" == "$bg_revision" ]] || { echo 'Unexpected source revision; not overwriting.' >&2; exit 1; }
for bg_patch in native/patches/ptyxis-private-scope.patch native/experiments/ptyxis-background.patch; do
  if git -C "$bg_root/source" apply --reverse --check "$bg_repo/$bg_patch" 2>/dev/null; then :
  else git -C "$bg_root/source" apply --check "$bg_repo/$bg_patch"
       git -C "$bg_root/source" apply "$bg_repo/$bg_patch"
  fi
done
export PKG_CONFIG_PATH="$bg_deps/usr/lib/x86_64-linux-gnu/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export LD_LIBRARY_PATH="$bg_deps/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$bg_deps/usr/bin:$PATH"
export XDG_DATA_DIRS="$bg_deps/usr/share:/usr/share"
export CCACHE_DIR="$bg_root/ccache"
if [[ ! -f "$bg_root/build/build.ninja" ]]; then
  meson setup "$bg_root/build" "$bg_root/source" --prefix "$bg_root/prefix" --libdir lib \
    -Dbuildtype=debugoptimized -Dapp-id=io.github.Miiikuuu.Termimochi.PtyxisBackgroundResearch \
    -Dgschema-id=io.github.Miiikuuu.Termimochi.PtyxisBackgroundResearch \
    -Dgschema-path=/io/github/Miiikuuu/Termimochi/PtyxisBackgroundResearch/
fi
meson compile -C "$bg_root/build" -j 4
meson install -C "$bg_root/build" --no-rebuild
install -m 644 "$bg_repo/native/experiments/background-canvas-version" "$bg_root/prefix/share/termimochi-background-canvas-version"
