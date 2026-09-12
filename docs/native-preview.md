# Native interactive preview — experimental

This is implemented in the native Rust/GTK App behind `native-preview`, not a
web prototype or an external-terminal fallback. Both Kitty and Ptyxis run as
actual Wayland clients inside a Casilda widget. Client buffers are presented
directly; PNG captures are test evidence only. This is **not yet a stable,
all-environment replacement for design preview**. See the exact limitations and
NP01–NP24 results in [the QA report](native-preview-qa.md).

## Build and run

```sh
bash scripts/build-native-preview.sh --download-deps
bash scripts/run-native-preview.sh
```

The first command builds into `target/native-preview/prefix`, not `/usr` or
`~/.local`. The Ubuntu archive lock is platform-specific; see
[dependencies, versions and patches](native-preview-dependencies.md). The App
launcher sets library paths for its own process only. Default Cargo builds do
not link Casilda; enabled builds require that library at process startup. This
is build-time optionality, not an inaccurate promise of runtime fallback when
the shared library is missing.

Open a target theme, choose **Real terminal**, then **Start native terminal**.
Confirm the actual-command warning. The target is taken from the theme, not a
second selector. Use **Expand** for more terminal space, **Resync theme** after
temporary native changes, **Return to design** to observe Full/VTE again, and
**Stop…** to terminate this preview's programs. Ctrl+Alt+Shift+F12 returns from
terminal focus to design. Ordinary terminal shortcuts are forwarded to the
client before App accelerators.

Commands execute with your user permissions. A private HOME/configuration is
**not a filesystem or network sandbox**. Personal shell rc/profile files are
not loaded. Starting native preview neither applies a theme nor publishes a
daily launcher. Save/Apply/use/recovery retain their existing semantics.

## Architecture and write boundary

| Part | Responsibility | State ownership |
| --- | --- | --- |
| Existing Workbench / DesignDocument / Workspace | Theme, sparse intent, all five editors | Only mutable design source |
| `native_preview::Snapshot` | Immutable projection of current valid design | Never serialized as another theme |
| `Session` | Target, startup baseline, private assets, controlled argv/env | Runtime-only, owned by one theme ID |
| Small Casilda FFI boundary | GTK widget, connected Wayland FD, spawn | GTK main thread; no synthetic terminal |
| `window/native_terminal.rs` | Mode, confirmation, lifecycle, bounded synchronization | One running session and one worker per workbench |
| Ptyxis adapter + C settings helper | Explicit shared keyfile GSettings and palette watcher | Entire backend private, not merely a private profile |
| Kitty adapter + immutable RC policy | Safe existing appearance projection and restricted `load-config` | One private Unix endpoint; no TTY command authorization |
| Python supervisor | Private D-Bus, subreaper, pidfd-based descendant cleanup | Own process tree only; no process-name killing |

The host retains its desktop connection. Only child environments receive the
connected `WAYLAND_SOCKET`. No host bus is imported: the private D-Bus config
has no activation service directories or systemd user manager. The project
Ptyxis patch bypasses the unavailable user scope **only** with the explicit
private-session flag. The system Ptyxis binary is not replaced.

Ptyxis uses `g_keyfile_settings_backend_new()` in the helper, and the same
`XDG_CONFIG_HOME`/keyfile backend in the client. Invalid batch values are rejected
before applying the batch; no dconf fallback is constructed. Fixed preview
profile IDs are safe only because the entire settings/file/bus roots differ.
Font settings that are global within Ptyxis remain private to each instance.

Kitty's fixed command-line overrides contain only the private connection,
controlled shell and safety policy. Mutable appearance is in `kitty.conf` and
reload does not ignore those overrides. The socket policy permits only loading
that exact owned file: no arbitrary configuration paths, `launch`, `send-text`,
TTY-originated requests, or removal of safety overrides.

## Synchronization and baseline

Editor changes mark the projection dirty; a 150 ms timer coalesces continuous
input. There is one in-flight worker; changes arriving during it become the
latest next projection, not an unbounded thread/command queue. Idle ticks do
not repeatedly clone image/GIF design data. GTK changes remain on the main
thread. Preparation and update results check theme ID, target, epoch, revision
and current snapshot; settings observations also check document identity.

Invalid/incomplete editor values leave the last valid native appearance in
place. File/control success is reported as **Appearance sent**, not visual
approval. Native errors remain visible. Ptyxis managed-key differences are
observed on the bounded worker every two seconds. Kitty font shortcuts show a
temporary-difference warning; arbitrary Kitty runtime differences are not yet
fully queried. Native preferences never write back into the design.

Inheritance uses controlled terminal defaults, not an undisclosed copy of
daily configuration. Kitty reloads a fresh sparse safe configuration; Ptyxis
uses the locked GNOME palette and Monospace 11 baseline. Removing overrides
restores these baselines. Existing theme save/apply still records sparse intent,
not the extra private values needed to start a reproducible terminal.

Prompt/Greeting share the existing safe `kitty_session::Plan` preparation and
retained assets. All initial/new tabs use controlled Bash and the same startup
projection. **Changing Prompt/Greeting requires Stop / Start**; the current
bootstrap remains frozen, including for tabs opened before restarting. Nothing
is injected into an active shell/TUI. Ptyxis rejects unverified pixel output
with an explicit Character/disable choice; it never overlays a GTK GIF. An
implementation debt is that this shared bootstrap planner currently also
requires Kitty to be installed for Ptyxis Prompt/Greeting preparation.

Switching editor pages or returning to design keeps the native session alive.
Stopping/closing asks before terminating its programs. The supervisor owns the
private bus and reaps descendant shells before asset deletion. Native client
crashes produce an unexpected-exit state with a usable Start action. A local
Casilda input-context reference cycle and stale cursor listener were found by
the tests and fixed; the host is destroyed after stopping, not just hidden.

## Preserved functionality / placement

| Existing function | Entry after this change | Regression evidence |
| --- | --- | --- |
| One theme / one target, sparse intent | Existing theme workspace, unchanged | Workspace/unit and theme-window regressions |
| Palette, fonts, Layout, Prompt, Greeting | Same editor pages | Shared projection; existing tests retained |
| Full / Terminal / Prompt / Greeting | Design preview | Full composition/navigation regression |
| Inspect | Design preview only; explicitly disabled on native surfaces | Existing VTE hit-test regression |
| Artwork algorithms, GIF/SVG source editing | Same Greeting editors | Existing unit tests and native GIF fixture |
| Save / reopen / undo | Existing theme document actions | Existing persistence and same-file window regressions |
| Try / Apply / daily Kitty opening | Existing review and use flows | Existing review/stale-result/launcher regressions |
| Source retention / conflict checks / backup / restore | Existing adapters and review flows | Existing safety suite; not replaced by preview code |
| Native interaction | New Real terminal mode | Enabled native tests, separately from default tests |

## Deliberate experimental boundaries

- Actual client SHM buffers and software rendering are the tested path. DMA-BUF
  produced corrupt Kitty glyphs in Xvfb/llvmpipe; it is off by default. This is
  not a screenshot/video stream, but it has software-rendering performance cost.
- The text clipboard bridge is bounded (1 MiB, 3-second timeout) and asynchronous.
  Single-line Unicode paste and native selection copy back to the host were
  tested in both targets at 1×/2×. Primary selection and non-text formats are
  not implemented; long/multiline and bracketed-paste coverage remains open.
- Fcitx5 private Pinyin under outer X11 / inner Wayland was tested. Host GNOME
  Wayland, IBus, mixed GPU, fractional scale and cross-monitor behavior are
  separate, unverified environments.
- Native CSD/tabs are genuine client surfaces, not copies of the host GNOME
  decorations. Not every window-top/spacing field is proven hot-applicable.
- No stable-default enablement or distribution package for this patched native
  dependency set is implied. No commit, push, merge, release, daily install,
  default-terminal change or shell startup edit is part of this work.
