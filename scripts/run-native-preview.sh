#!/usr/bin/env bash
set -euo pipefail
np_repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$np_repo"
export TERMIMOCHI_NATIVE_PREFIX="$np_repo/target/native-preview/prefix"
export PKG_CONFIG_PATH="$TERMIMOCHI_NATIVE_PREFIX/lib/pkgconfig:$TERMIMOCHI_NATIVE_PREFIX/usr/lib/x86_64-linux-gnu/pkgconfig"
export LD_LIBRARY_PATH="$TERMIMOCHI_NATIVE_PREFIX/lib:$TERMIMOCHI_NATIVE_PREFIX/usr/lib/x86_64-linux-gnu"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
if [[ -d "$np_repo/target/qa/typed-toolchain/rustup/toolchains/1.92.0-x86_64-unknown-linux-gnu" ]]; then
  export RUSTUP_HOME="$np_repo/target/qa/typed-toolchain/rustup" RUSTUP_TOOLCHAIN=1.92.0
fi
cargo build -p termimochi --features native-preview --locked
exec "$np_repo/target/debug/termimochi" "$@"
