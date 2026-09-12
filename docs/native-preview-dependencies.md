# Native preview dependency lock

This is an opt-in native GTK build, not a replacement for the default build.
Source baseline: main `3f3024216814ae79750db3e9fa37ac6afe6ecb66`.
No daily terminal, shell startup file, global session environment or system
package has been modified. All additional binaries live in `target/native-preview/prefix`.

| Component | Actual version / source | State |
| --- | --- | --- |
| OS | Ubuntu 26.04.1, x86_64 | Installed |
| GTK / GLib / libadwaita | 4.22.4 / 2.88.0 / 1.9.1 | Installed |
| VTE | 0.84.0 | Installed |
| Rust | 1.92.0, matching CI | Existing project-local toolchain |
| Casilda | 1.4.0, `0c6ff1490a0199024b30bf9a237b5d41856a0e81` | Local build, LGPL-2.1-only |
| wlroots | 0.20.2, `d783533489e1f75d6886c2ab5c5960090ef268f8` | Local build, MIT |
| Ptyxis | 50.1, `0f045a653db114e22fe7bf4b409fcb30d7763fa9` | Local patched build, GPL-3.0-or-later |
| System Ptyxis | 50.1-1ubuntu2 | Unmodified; private scope startup fails |
| Kitty | 0.45.0 | Installed, actual native executable |
| Fcitx5 / Chinese addons / GTK frontend | 5.1.19 / 5.1.12 / 5.1.6 | Installed; private Pinyin test instance |
| IBus / libpinyin | 1.5.34rc2 / 1.16.5 | Installed; not yet validated |
| Xvfb + Mesa software rendering | Existing project-local Xvfb; llvmpipe | Isolated native tests |
| Host GNOME Wayland, hardware GPU, fractional scaling | Separate from Xvfb evidence | Not yet verified |

Casilda's ABI is named **1.0**, but the locked release is **1.4.0**. It requires
GTK >= 4.22.2 and wlroots 0.20; the stock Ubuntu Casilda/wlroots pairing was not
silently substituted. Sources, licenses and builds remain under
`target/native-preview/src` and `target/native-preview/build-*`.

`native/debs.sha256` locks Ubuntu dependency archives. They are downloaded and
extracted, not installed with apt. Bootstrap:

```sh
bash scripts/build-native-preview.sh --download-deps
bash scripts/run-native-preview.sh
```

The Debian archive lock is Ubuntu 26.04 amd64-specific. Other distributions must
supply compatible development packages; the script never upgrades host GTK.

## Local patches and maintenance boundary

- `native/patches/ptyxis-private-scope.patch`: only an explicit private-session
  flag bypasses unavailable systemd user scopes; file-monitor palette changes
  rebind existing native terminals. System Ptyxis is untouched.
- `native/patches/casilda-input-bridge.patch`: initial keymap, combined modifiers,
  GTK IM ↔ Wayland text-input-v3, bounded asynchronous text clipboard, and
  conservative native SHM rendering. These changes are not upstream releases.
- DMA-BUF under the tested Xvfb/Mesa combination produced corrupt Kitty glyphs.
  The local compositor defaults to actual native SHM client surfaces, not a
  screenshot stream. `CASILDA_ENABLE_DMABUF=1` is an unverified developer opt-in.

The private bus has no activation directories and never imports a host session
environment. The settings helper constructs an explicit keyfile backend; it
does not construct the default dconf backend. A subreaper owns the private bus,
terminal and descendant shell lifecycle. This is configuration/lifetime
isolation, **not** a filesystem or network sandbox.
