//! Reviewed declarative editor fields, following Starship's configuration schema.
//! No command, executable, detection or shell configuration is editable here.
#[derive(Clone, Copy)]
pub(crate) struct TextField {
    pub key: &'static str,
    pub label: &'static str,
    pub default: &'static str,
    pub formatted: bool,
    pub escaped: bool,
}

const fn literal(key: &'static str, label: &'static str, default: &'static str) -> TextField {
    TextField {
        key,
        label,
        default,
        formatted: false,
        escaped: true,
    }
}
const fn formatted(key: &'static str, label: &'static str, default: &'static str) -> TextField {
    TextField {
        key,
        label,
        default,
        formatted: true,
        escaped: false,
    }
}
const fn raw(key: &'static str, label: &'static str, default: &'static str) -> TextField {
    TextField {
        escaped: false,
        ..literal(key, label, default)
    }
}

pub(crate) struct ModuleSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub symbols: &'static [TextField],
    pub styles: &'static [TextField],
    pub version: bool,
    pub disabled: bool,
    pub presets: Option<[&'static str; 3]>,
}

macro_rules! module {
    ($id:literal, $label:literal, $symbols:expr, $styles:expr) => {
        ModuleSpec {
            id: $id,
            label: $label,
            symbols: $symbols,
            styles: $styles,
            version: false,
            disabled: false,
            presets: None,
        }
    };
}

pub(crate) const MODULES: &[ModuleSpec] = &[
    ModuleSpec {
        version: true,
        presets: Some([" rs ", " 🦀 ", " \u{e7a8} "]),
        ..module!(
            "rust",
            "Rust",
            &[literal("symbol", "Symbol", "🦀 ")],
            &[literal("style", "Style", "bold red")]
        )
    },
    ModuleSpec {
        version: true,
        presets: Some(["node ", "⬢ ", "\u{e718} "]),
        ..module!(
            "nodejs",
            "Node.js",
            &[literal("symbol", "Symbol", "\u{e718} ")],
            &[
                literal("style", "Style", "bold green"),
                literal("not_capable_style", "Unsupported version", "bold red")
            ]
        )
    },
    ModuleSpec {
        version: true,
        presets: Some(["py ", "🐍 ", "\u{e73c} "]),
        ..module!(
            "python",
            "Python",
            &[literal("symbol", "Symbol", "🐍 ")],
            &[literal("style", "Style", "yellow bold")]
        )
    },
    ModuleSpec {
        version: true,
        presets: Some(["go ", "🐹 ", "\u{e627} "]),
        ..module!(
            "golang",
            "Go",
            &[literal("symbol", "Symbol", "🐹 ")],
            &[
                literal("style", "Style", "bold cyan"),
                literal("not_capable_style", "Unsupported version", "bold red")
            ]
        )
    },
    module!(
        "git_branch",
        "Git Branch",
        &[
            literal("symbol", "Branch", "\u{e0a0} "),
            raw("truncation_symbol", "Truncation", "…")
        ],
        &[literal("style", "Style", "bold purple")]
    ),
    module!(
        "git_status",
        "Git Status",
        &[
            formatted("modified", "Modified", "!"),
            formatted("untracked", "Untracked", "?"),
            formatted("staged", "Staged", "+"),
            formatted("deleted", "Deleted", "✘"),
            formatted("renamed", "Renamed", "»"),
            formatted("conflicted", "Conflicted", "="),
            formatted("stashed", "Stashed", "\\$"),
            formatted("ahead", "Ahead", "⇡"),
            formatted("behind", "Behind", "⇣"),
            formatted("diverged", "Diverged", "⇕"),
            formatted("up_to_date", "Up to date", ""),
            formatted("typechanged", "Type changed", "")
        ],
        &[literal("style", "Style", "red bold")]
    ),
    module!(
        "directory",
        "Directory",
        &[
            raw("read_only", "Read-only", "🔒"),
            raw("home_symbol", "Home", "~"),
            raw("truncation_symbol", "Truncation", "")
        ],
        &[
            literal("style", "Path", "cyan bold"),
            literal("read_only_style", "Read-only", "red"),
            literal("repo_root_style", "Repository root", ""),
            literal("before_repo_root_style", "Before repository root", "")
        ]
    ),
    module!(
        "character",
        "Prompt Symbol",
        &[
            formatted("success_symbol", "Success", "[❯](bold green)"),
            formatted("error_symbol", "Error", "[❯](bold red)"),
            formatted("vimcmd_symbol", "Vim command", "[❮](bold green)"),
            formatted("vimcmd_visual_symbol", "Vim visual", "[❮](bold yellow)"),
            formatted("vimcmd_replace_symbol", "Vim replace", "[❮](bold purple)"),
            formatted(
                "vimcmd_replace_one_symbol",
                "Vim replace once",
                "[❮](bold purple)"
            )
        ],
        &[]
    ),
    module!(
        "username",
        "User",
        &[],
        &[
            literal("style_user", "User", "yellow bold"),
            literal("style_root", "Root", "red bold")
        ]
    ),
    module!(
        "hostname",
        "Host",
        &[literal("ssh_symbol", "SSH", "🌐 ")],
        &[literal("style", "Style", "green dimmed bold")]
    ),
    module!(
        "conda",
        "Conda",
        &[literal("symbol", "Symbol", "🅒 ")],
        &[literal("style", "Style", "green bold")]
    ),
    module!(
        "cmd_duration",
        "Command Time",
        &[],
        &[literal("style", "Style", "yellow bold")]
    ),
    ModuleSpec {
        disabled: true,
        ..module!(
            "status",
            "Exit Status",
            &[
                literal("symbol", "Error", "❌"),
                literal("success_symbol", "Success", ""),
                literal("not_executable_symbol", "Not executable", "🚫"),
                literal("not_found_symbol", "Not found", "🔍"),
                literal("sigint_symbol", "Interrupted", "🧱"),
                literal("signal_symbol", "Signal", "⚡")
            ],
            &[
                literal("style", "Style", "bold red"),
                literal("success_style", "Success", ""),
                literal("failure_style", "Failure", "")
            ]
        )
    },
    module!(
        "jobs",
        "Background Jobs",
        &[literal("symbol", "Symbol", "✦")],
        &[literal("style", "Style", "bold blue")]
    ),
    ModuleSpec {
        disabled: true,
        ..module!(
            "time",
            "Time",
            &[],
            &[literal("style", "Style", "bold yellow")]
        )
    },
];

pub(crate) fn module_index(id: &str) -> Option<usize> {
    MODULES.iter().position(|module| module.id == id)
}
