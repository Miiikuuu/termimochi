# Kitty cold-start protection QA — 2026-09-10

## Finding and fix scope

Immediate Fastfetch output intermittently left the image area blank in Kitty
0.45.0 on an isolated X11 display. Complete system fields reproduced the problem;
the first minimal-field experiment did not. Switching from file transfer to an
inline stream, or disabling stdout buffering, did not reliably fix it.
Raw terminal byte captures still contained the image command. The child did not
record SIGWINCH during its short Fastfetch run, so the exact upstream renderer
race is not established; “initial resize clears it” was a hypothesis, not a proven
root cause.

The application now protects newly managed Kitty PNG/GIF configurations with a
reviewed `general.preRun` helper. Fastfetch documents this
[pre-logo command in its versioned schema](https://github.com/fastfetch-cli/fastfetch/blob/2.57.1/doc/json_schema.json).
The helper runs before GTK, observes row/column dimensions via `stty size` and
requires a 300 ms quiet interval, with a 1.5 s overall sampling limit. It performs
no keyboard reads, termios writes, clear/redraw operations or configuration writes.
It runs Fastfetch modules zero times: the enclosing Fastfetch process runs them
once after the guard. Redirected/non-Kitty output skips the guard.

This is a bounded startup workaround in TermiMochi's installation path, not a
patch to the system Kitty binary. It adds roughly 0.3 seconds to eligible runs.
Existing preRun commands/comments are preserved; a repeated installation does
not duplicate the same guard prefix. The helper path is absolute and shell-quoted.
Existing `.bashrc`, user configurations and old bundles are not auto-migrated.

## Evidence

- Diagnostic direct-versus-guarded run: unguarded **2/8 blank**, guarded **8/8
  visible**. Captured full-run durations were roughly 0.36–0.42 s with protection.
  Report: `termimochi-kitty-startup-eklrw9h4/results.json` in the isolated QA cache.
- Managed-config startup regression: **32/32 cold launches passed** — eight PNG
  and eight GIF launches under each GTK scale setting (1x and 2x). The terminal
  launchers invoke Fastfetch immediately, with no pre-render sleep. Trial files
  are removed before installation rendering; commands run from `/`.
- The native checker checks real pixels before accepting the output, and GIF
  requires moving bounds across four frames without accumulated trails. Terminal
  font size is fixed; GTK scale coverage is not every possible terminal DPI.
- Expanded native/GTK matrix: **8/8 passed** (`termimochi-regression-twru5mzk`):
  repeated startup, real protocol/fallback trials, installation gate/cancel/restore,
  and the separate Bash startup switch's cancellation/conflict/recovery flow.
- Workspace tests: **342 passed**, 49 opt-in tests excluded from ordinary runs.
  Formatting and Clippy (`-D warnings`) passed. Desktop resource tests: **9 passed**.
- Release build/local installation and validation of **17 embedded resources**
  plus packaged copies passed. Built/installed app SHA-256 matched:
  `47545a25f05017b6f3010d4de013987d8d024293aa72728ec89a363eacf4554b`.

The final focused matrix **passed 6/6** and also covers a real Python PTY: resize twice during the
guard, queue a line of keyboard input before it completes, then verify the exact
line is still readable and termios is unchanged. Redirected helper output must
exit quietly. See `termimochi-regression-461l73ls/results.json` for this run and
the final installation/startup UI checks.

## User adoption and limits

Restart the updated app, then repeat **Test in Terminal → Review Install… →
Install & Apply** for an existing image scheme. The review names the new preRun
helper and its timing/dependency. No startup script needs to be edited to add
the guard; an existing startup command reading that config uses it automatically.
Keep the referenced TermiMochi executable installed. Restore uses the existing
Fastfetch backup path and returns the exact prior configuration.

Portable image bundles and manually authored configurations remain unchanged.
The guard is not terminal capability proof, and does not fix unsupported image
protocols, arbitrary later window resizing, user commands that clear the display,
or every Wayland/compositor/remote-session timing. Those still need visual checks.
No user terminal configuration or shell startup file was modified during QA.
