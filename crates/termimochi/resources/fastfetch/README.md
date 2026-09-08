# Fastfetch assets

The files in `logos/` and `presets/` are unmodified upstream assets from
[Fastfetch 2.57.1](https://github.com/fastfetch-cli/fastfetch/tree/2.57.1).
`example-8.jsonc` and `example-9.jsonc` come from `presets/examples/`.
See the accompanying MIT license. These are bundled so installing a different
Fastfetch version cannot silently change saved presets. The application's
TermiMochi artwork is separate and is not an upstream Fastfetch asset.

Only the five reviewed, local-information presets are exposed. Examples that
launch commands, access the network or depend on external images are not run.
The original module objects (including formats, duplicate types and decorative
modules) are retained; explicit field switches and ordering are applied by ID.
The workbench can customize artwork, placement and accent. The installed
Fastfetch executable renders the retained modules in a read-only offline
Bubblewrap sandbox; no arbitrary config file is imported or executed.
