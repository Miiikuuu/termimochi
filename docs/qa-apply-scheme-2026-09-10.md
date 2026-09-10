# Apply Scheme QA — 2026-09-10

## Scope

Complete-workspace application review, per-module result reporting, durable
per-run recovery, palette activation, and preservation of existing Save and
standalone module operations. Tests use isolated XDG directories, in-memory
GSettings, private files and a dedicated Xvfb display; no user terminal settings
or shell startup files were changed.

## Checks

- `cargo fmt --all -- --check`: passed.
- `cargo test --workspace --locked --quiet`: **332 passed**, 45 opt-in tests
  excluded from the ordinary run.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- Desktop resource tests: **9 passed**; metadata plus **17 embedded resources**
  and packaged copies validated.
- Release workspace build: passed.
- Expanded integration run: **12/12 passed**, six cases at 1x and 2x.
- Final focused integration run after header/discovery refinements:
  **6/6 passed**, three cases at 1x and 2x.

Expanded cases: workspace/layout save and safety; Greeting field apply/restore;
Fastfetch synchronization/conflicts; all contextual output actions and narrow
geometry; complete scheme review/apply/reopen/recovery; settings transaction
conflicts and inheritance restoration.

Ordinary tests additionally cover read-only cancellation/empty selection,
independent partial failures, export versus apply, external changes blocking
only their affected recovery, no-op receipts, later standalone applications,
palette late edits and symlink rejection, unsafe latest-record paths, and
interrupted-operation journals.

The settings matrix checks unset values, unrelated opacity preservation,
confirmation-time palette conflicts, global-font conflicts during restore,
independent layout recovery, and failed palette installation preventing
activation while a selected prompt export still succeeds.

The GUI flow starts with no selected changes, confirms unavailable activation
until installation is selected, checks preview-only controls, cancels without
writes, applies all supported parts, verifies separate exported-prompt status,
reopens the persisted result, confirms restoration, and prepares another
Fastfetch apply using the refreshed conflict baseline. The module-output test
also confirms Ctrl+S remains local workspace persistence, not a Starship write.

## Local evidence

Expanded results:
`/home/xiaozhouchao/.cache/termimochi-scheme-qa.4LyAzK/termimochi-regression-_wrlbqog/results.json`

Final focused results and per-case logs:
`/home/xiaozhouchao/.cache/termimochi-scheme-qa.4LyAzK/termimochi-regression-ebak5vm7/results.json`

Both scale cases save `cache/scheme-review.png` and `cache/scheme-results.png`.
Rendered captures were inspected for readable controls, scrollable review,
fixed confirmation buttons, destination display and explicit outcome labels.

The installed binary matches the release build:
`5f460811adecc6131ba9b80e15438228be906fc13360b7d5e763fef75046acce`.

## Boundaries

Direct settings application still targets Ptyxis, not Kitty or every terminal.
Designer/detached workspace prompts are explicitly export-only. Startup hooks
are unchanged. The profile-tab launcher uses a validated UUID argument and is
suppressed in tests; normal user shell startup was not executed by this QA.
Dedicated Kitty/Sixel/GIF rendering paths were not changed or rerun here.
This validates X11/GTK review and isolated settings/file transactions, not every
desktop, display backend, terminal version or external shell integration.

The local application was updated without terminating the user's running app.
No commit or push was made during this step.
