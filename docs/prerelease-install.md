# Installable preview (not a stable release)

This archive contains optimized GUI/CLI binaries, icons, desktop metadata and
the existing reversible user installer. It does not bundle system libraries,
Kitty, Ptyxis, Starship or Fastfetch. The experimental native-preview feature is
not bundled. `PREVIEW.json` records the build OS, architecture and dependencies.
A build made on Ubuntu 26.04 is **not** certified for Ubuntu 24.04 or other
distributions. Build from source on incompatible systems. No root access is needed.

After extracting the archive, run these commands in its directory:

```sh
bash scripts/check-preview.sh
./target/release/termimochi
```

The check verifies local integrity and dynamic-library resolution, not publisher
authenticity or desktop compatibility. This is an unsigned local preview.

Only when you explicitly want to install:

```sh
bash scripts/install.sh install --dry-run
bash scripts/install.sh install
```

Defaults: binaries in `~/.local/bin`, desktop data in `~/.local/share` (respecting
XDG overrides). No terminal configuration, shell startup file or default terminal
is changed. Existing TermiMochi application binaries are replaced; retain the
previous archive if you need to reinstall that build. Theme versions, retained
launcher runtimes and recovery receipts are not removed.

Uninstall with the same prefix/data/state options used for installation:

```sh
bash scripts/install.sh uninstall --dry-run
bash scripts/install.sh uninstall
```

For an isolated install, pass explicit absolute `--prefix`, `--data-home` and
`--state-home` paths under a test directory. Do not use daily XDG paths in tests.
Uninstall removes only the installer's enumerated application files, not themes
or independent theme launchers. Those have their own reviewed recovery flow.

Real desktop-menu indexing, personal Conda/custom Bash, Wayland, multi-monitor,
fractional scaling and logout/login persistence remain a **separate validation
task**. Isolated X11 does not replace it; unverified is not unimplemented.
