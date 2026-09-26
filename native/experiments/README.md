# Native runtime experiments

These patches are **not applied by the normal build** and are not shipped in the
public preview package. Do not add them to the runtime bootstrap without the
corresponding integration/security/native acceptance work.

## Ptyxis image background

`ptyxis-background.patch` targets Ptyxis 50.1 commit
`0f045a653db114e22fe7bf4b409fcb30d7763fa9`. It changes only `src/ptyxis-tab.c` and
can coexist with `../patches/ptyxis-private-scope.patch`.

This is a **trusted-fixture drawing experiment**, not a finished file importer:
synchronous per-tab texture loading, process-wide environment options, no live
reload, no hostile-input/resource limits, no theme persistence or launcher
integration. It is gated by `TERMIMOCHI_PRIVATE_SESSION=1` and the
`TERMIMOCHI_RESEARCH_BACKGROUND` path. `TERMIMOCHI_RESEARCH_LAYOUT=contain`
selects contain (otherwise cover); `TERMIMOCHI_RESEARCH_TINT` defaults to 0.35.
Do not enable this against arbitrary user files as a production feature.

The optional [background workflow](../../docs/background-design-preview.md)
bounds and converts static user images before passing an owned PNG to this patch.
The App now saves backgrounds in themes and can publish an independent appearance
entry; that integration lives outside this drawing patch. Its dedicated build
script does not change the normal native-preview runtime. Live updates and
distribution of the enhanced runtime are not implemented.

The tested executable has its own application/schema ID, prefix, private
GSettings backend and D-Bus. Do not replace `/usr/bin/ptyxis` or reuse a daily
configuration/backend to reproduce it. The existing native-preview prefix is
also left untouched.

See [research, evidence, failed attempts, remaining work and local reproduction](../../docs/ptyxis-background-research-2026-09-23.md).
The upstream file carries `GPL-3.0-or-later`; this experimental modification
retains that license context. Distribution of an enhanced runtime requires its
own packaging/provenance review.
