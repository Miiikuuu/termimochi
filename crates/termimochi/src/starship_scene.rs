//! Simulated command transitions rendered with the user's complete prompt.
//! Declarative example context only: displayed commands are never executed.
use crate::{
    starship_draft::StarshipDraft,
    starship_modules::{ModuleSpec, module_index},
};
use std::collections::BTreeMap;
use toml::{Table, Value};

pub(crate) use crate::starship_draft::EditFocus as Focus;

#[derive(Clone)]
pub(crate) struct CommandScene {
    pub key: (usize, usize, usize),
    pub document: u64,
    pub command: &'static str,
    pub output: &'static str,
    pub title: String,
    pub diagnostic_source: String,
    pub config: Result<String, String>,
}
#[derive(Clone)]
pub(crate) struct RenderedScene {
    pub scene: CommandScene,
    pub ansi: Result<String, String>,
}

pub(crate) fn record(
    history: &mut std::collections::VecDeque<RenderedScene>,
    scene: RenderedScene,
) {
    if history
        .back()
        .is_some_and(|previous| previous.scene.document != scene.scene.document)
    {
        history.clear();
    }
    if history
        .back()
        .is_some_and(|previous| previous.scene.key == scene.scene.key)
    {
        history.pop_back();
    } else if history.len() == 6 {
        history.pop_front();
    }
    history.push_back(scene);
}

enum Variable {
    Format(String),
    Style(String),
    Data(String),
}

pub(crate) struct SceneRequest {
    source: String,
    module: usize,
    symbol: usize,
    style: usize,
    focus: Focus,
    document: u64,
}
impl SceneRequest {
    pub fn capture(draft: &StarshipDraft, document: u64) -> Self {
        Self {
            source: draft.contents().into(),
            module: draft.module,
            symbol: draft.symbol,
            style: draft.style,
            focus: draft.sample_focus,
            document,
        }
    }
    pub fn build(self) -> CommandScene {
        let mut draft =
            StarshipDraft::new("scene.toml".into(), self.source).expect("validated draft snapshot");
        draft.select_module(self.module).expect("reviewed module");
        draft.symbol = self.symbol;
        draft.style = self.style;
        let mut scene = CommandScene::new(&draft, self.focus);
        scene.document = self.document;
        scene
    }
}

struct Context {
    module: &'static str,
    symbol: &'static str,
    style: &'static str,
    path: &'static str,
    command: &'static str,
    output: &'static str,
    status: &'static str,
    root: bool,
}
impl Context {
    fn new(draft: &StarshipDraft, focus: Focus) -> Self {
        let module = draft.spec().id;
        let mut symbol = draft.symbol_spec().map_or("", |f| f.key);
        let style = if focus == Focus::Style {
            draft.style_spec().map_or("style", |f| f.key)
        } else {
            "style"
        };
        if module == "status" && focus == Focus::Style {
            if style == "success_style" {
                symbol = "success_symbol";
            } else if style == "failure_style" {
                symbol = "symbol";
            }
        }
        let root = module == "username" && style == "style_root";
        let (command, path, output, status) = match module {
            "rust" => ("cd ~/projects/rust-app", "~/projects/rust-app", "", "0"),
            "nodejs" => ("cd ~/projects/node-app", "~/projects/node-app", "", "0"),
            "python" => ("cd ~/projects/python-api", "~/projects/python-api", "", "0"),
            "golang" => ("cd ~/projects/go-service", "~/projects/go-service", "", "0"),
            "git_branch" => (
                "git switch feature/terminal-preview",
                "~/projects/TermiMochi",
                "Switched to branch 'feature/terminal-preview'",
                "0",
            ),
            "git_status" => (
                match symbol {
                    "staged" => "git add README.md src/",
                    "untracked" => "touch notes.txt todo.txt",
                    "modified" => "printf '\\n# Preview' >> README.md",
                    "deleted" => "rm old-config.toml",
                    "renamed" => "git mv draft.md README.md",
                    "conflicted" => "git merge feature/settings",
                    "stashed" => "git stash push -m preview",
                    "ahead" => "git commit -m 'Update theme'",
                    "behind" | "diverged" | "up_to_date" => "git fetch origin",
                    "typechanged" => "ln -sf settings.toml active-config",
                    _ => "git status --short",
                },
                "~/projects/TermiMochi",
                match symbol {
                    "conflicted" => "CONFLICT (content): Merge conflict in settings.toml",
                    "stashed" => "Saved working directory and index state",
                    _ => "",
                },
                if symbol == "conflicted" { "1" } else { "0" },
            ),
            "directory"
                if focus == Focus::Style
                    && matches!(style, "repo_root_style" | "before_repo_root_style") =>
            {
                (
                    "cd ~/projects/TermiMochi/src/ui/widgets",
                    "~/projects/TermiMochi/src/ui/widgets",
                    "",
                    "0",
                )
            }
            "directory" if focus == Focus::Style && style == "read_only_style" => {
                ("cd /usr/share", "/usr/share", "", "0")
            }
            "directory" => (
                if symbol == "home_symbol" {
                    "cd ~"
                } else if symbol == "read_only" {
                    "cd /usr/share"
                } else {
                    "cd ~/projects/TermiMochi/src/ui/widgets"
                },
                if symbol == "home_symbol" {
                    "~"
                } else if symbol == "read_only" {
                    "/usr/share"
                } else {
                    "~/projects/TermiMochi/src/ui/widgets"
                },
                "",
                "0",
            ),
            "character" => (
                match symbol {
                    "error_symbol" => "false",
                    "success_symbol" => "true",
                    _ => "set -o vi",
                },
                "~/projects/TermiMochi",
                "",
                if symbol == "error_symbol" { "1" } else { "0" },
            ),
            "username" if root => ("sudo -i", "/root", "", "0"),
            "username" => ("whoami", "~/projects/TermiMochi", "mii", "0"),
            "hostname" => ("ssh dev@workstation", "~/projects/TermiMochi", "", "0"),
            "conda" => ("conda activate dev", "~/projects/TermiMochi", "", "0"),
            "cmd_duration" => ("sleep 3", "~/projects/TermiMochi", "", "0"),
            "status" => match if focus == Focus::Style && style == "success_style" {
                "success_symbol"
            } else {
                symbol
            } {
                "success_symbol" => ("true", "~/projects/TermiMochi", "", "0"),
                "not_executable_symbol" => (
                    "./script.sh",
                    "~/projects/TermiMochi",
                    "bash: ./script.sh: Permission denied",
                    "126",
                ),
                "not_found_symbol" => (
                    "missing-command",
                    "~/projects/TermiMochi",
                    "bash: missing-command: command not found",
                    "127",
                ),
                "sigint_symbol" => ("sleep 60  # Ctrl+C", "~/projects/TermiMochi", "^C", "130"),
                "signal_symbol" => (
                    "./worker  # terminated",
                    "~/projects/TermiMochi",
                    "Terminated",
                    "143",
                ),
                _ => ("false", "~/projects/TermiMochi", "", "1"),
            },
            "jobs" => ("sleep 60 &", "~/projects/TermiMochi", "[1] 4242", "0"),
            "time" => ("date +%T", "~/projects/TermiMochi", "14:32:08", "0"),
            _ => ("pwd", "~/projects/TermiMochi", "", "0"),
        };
        Self {
            module,
            symbol,
            style,
            path,
            command,
            output,
            status,
            root,
        }
    }
}

impl CommandScene {
    pub(crate) fn new(draft: &StarshipDraft, focus: Focus) -> Self {
        let spec = draft.spec();
        let field = match focus {
            Focus::Symbol => draft.symbol_spec().map_or("Symbol", |f| f.label),
            Focus::Style => draft.style_spec().map_or("Style", |f| f.label),
            Focus::Format => "Format",
        };
        let title = format!(
            "{} · {} · simulated{}",
            spec.label,
            field,
            if draft.enabled() {
                ""
            } else {
                " · disabled in draft"
            }
        );
        let context = Context::new(draft, focus);
        Self {
            key: (draft.module, draft.symbol, draft.style),
            document: 0,
            command: context.command,
            output: context.output,
            title,
            diagnostic_source: draft.contents().into(),
            config: make_config(draft, &context),
        }
    }
}

fn module_expression(
    draft: &StarshipDraft,
    context: &Context,
    data: &mut Table,
) -> Result<String, String> {
    let spec = draft.spec();
    if !draft.enabled() {
        return Ok(String::new());
    }
    if spec.version && spec.id != context.module {
        return Ok(String::new());
    }
    if matches!(
        spec.id,
        "git_status" | "conda" | "cmd_duration" | "jobs" | "time"
    ) && spec.id != context.module
    {
        return Ok(String::new());
    }
    if spec.id == "status" && context.status == "0" && context.module != "status" {
        return Ok(String::new());
    }
    let mut variables = BTreeMap::new();
    for field in spec.symbols {
        let value = draft
            .field(field.key)
            .unwrap_or_else(|| field.default.into());
        variables.insert(
            field.key.to_owned(),
            if field.escaped || field.formatted {
                Variable::Format(value)
            } else {
                Variable::Data(value)
            },
        );
    }
    for field in spec.styles {
        let value = draft.field(field.key).unwrap_or_else(|| {
            if matches!(
                field.key,
                "repo_root_style" | "before_repo_root_style" | "success_style" | "failure_style"
            ) {
                draft
                    .field("style")
                    .unwrap_or_else(|| spec.styles[0].default.into())
            } else {
                field.default.into()
            }
        });
        variables.insert(field.key.to_owned(), Variable::Style(value));
    }
    // These are intentionally fixed examples, not claims about the user's shell.
    for (key, value) in [
        ("user", "mii"),
        ("hostname", "workstation"),
        ("path", "~/TermiMochi/src"),
        ("branch", "feature/preview"),
        ("remote_branch", "main"),
        ("remote_name", "origin"),
        ("before_root_path", "~/Projects/"),
        ("repo_root", "TermiMochi"),
        ("duration", "2s842ms"),
        ("status", "1"),
        ("int", "1"),
        ("maybe_int", "1"),
        ("hex_status", "0x1"),
        ("common_meaning", "ERROR"),
        ("signal_name", "SIGINT"),
        ("signal_number", "2"),
        ("number", "2"),
        ("time", "14:32:08"),
        ("environment", "dev"),
        ("virtualenv", "venv"),
        ("pyenv_prefix", ""),
        ("count", "2"),
        ("ahead_count", "2"),
        ("behind_count", "1"),
        ("pipestatus", "0|1"),
    ] {
        variables.insert(key.into(), Variable::Data(value.into()));
    }
    variables.insert(
        "user".into(),
        Variable::Data(
            if context.root {
                "root"
            } else if context.module == "hostname" {
                "dev"
            } else {
                "mii"
            }
            .into(),
        ),
    );
    variables.insert("path".into(), Variable::Data(context.path.into()));
    for key in ["status", "int", "maybe_int"] {
        variables.insert(key.into(), Variable::Data(context.status.into()));
    }
    variables.insert("duration".into(), Variable::Data("3s".into()));
    variables.insert("number".into(), Variable::Data("1".into()));
    variables.insert(
        "branch".into(),
        Variable::Data("feature/terminal-preview".into()),
    );
    variables.insert("remote_branch".into(), Variable::Data(String::new()));
    let style_key = if spec.id == "username" {
        if context.root {
            "style_root"
        } else {
            "style_user"
        }
    } else if spec.id == "status" {
        if context.status == "0" {
            "success_style"
        } else {
            "failure_style"
        }
    } else if spec.id == context.module && context.style == "not_capable_style" {
        "not_capable_style"
    } else {
        "style"
    };
    if style_key != "style"
        && let Some(Variable::Style(value)) = variables.get(style_key)
    {
        let value = value.clone();
        variables.insert("style".into(), Variable::Style(value));
    }
    if spec.version {
        let (major, minor, patch) = match spec.id {
            "rust" => ("1", "89", "0"),
            "nodejs" => ("24", "2", "0"),
            "python" => ("3", "13", "2"),
            _ => ("1", "24", "1"),
        };
        let version = substitute(
            &draft
                .field("version_format")
                .unwrap_or_else(|| "v${raw}".into()),
            |key| {
                Ok(match key {
                    "raw" => format!("{major}.{minor}.{patch}"),
                    "major" => major.into(),
                    "minor" => minor.into(),
                    "patch" => patch.into(),
                    "pre" | "build" => String::new(),
                    _ => return Err(format!("No example for version variable ${key}.")),
                })
            },
        )?;
        variables.insert("version".into(), Variable::Data(version));
    }
    if spec.id == "character" {
        let symbol = if context.module == "character" {
            context.symbol
        } else if context.status != "0" {
            "error_symbol"
        } else {
            "success_symbol"
        };
        variables.insert("symbol".into(), Variable::Format(format!("${{{symbol}}}")));
    }
    if spec.id == "git_status" {
        for field in spec.symbols {
            if field.key != context.symbol {
                variables.insert(field.key.into(), Variable::Data(String::new()));
            }
        }
        variables.insert(
            "all_status".into(),
            Variable::Format(
                "$conflicted$stashed$deleted$renamed$modified$staged$untracked$typechanged".into(),
            ),
        );
        variables.insert(
            "ahead_behind".into(),
            Variable::Format("$ahead$behind$diverged$up_to_date".into()),
        );
    }
    if spec.id == "status" {
        let field = if context.status == "0" {
            "success_symbol"
        } else if context.module == "status" {
            context.symbol
        } else {
            "symbol"
        };
        if field != "symbol" {
            variables.insert("symbol".into(), Variable::Format(format!("${{{field}}}")));
        }
    }
    if spec.id == "directory" {
        if context.path != "/usr/share" {
            variables.insert("read_only".into(), Variable::Data(String::new()));
        }
        if context.module == "directory" {
            if context.command == "cd ~" {
                variables.insert(
                    "path".into(),
                    Variable::Data(draft.field("home_symbol").unwrap_or_else(|| "~".into())),
                );
            } else if context.symbol == "truncation_symbol" && context.path != "/usr/share" {
                variables.insert(
                    "path".into(),
                    Variable::Data(format!(
                        "{}src/ui/widgets",
                        draft.field("truncation_symbol").unwrap_or_default()
                    )),
                );
            }
        }
    }
    if spec.id == "git_branch"
        && context.module == "git_branch"
        && context.symbol == "truncation_symbol"
    {
        variables.insert(
            "branch".into(),
            Variable::Data(format!(
                "feature/{}",
                draft
                    .field("truncation_symbol")
                    .unwrap_or_else(|| "…".into())
            )),
        );
    }
    let format = if spec.id == "directory"
        && context.module == "directory"
        && matches!(context.style, "repo_root_style" | "before_repo_root_style")
    {
        variables.insert("path".into(), Variable::Data("/src/ui/widgets".into()));
        draft.field("repo_root_format").unwrap_or_else(|| "[$before_root_path]($before_repo_root_style)[$repo_root]($repo_root_style)[$path]($style) ".into())
    } else {
        draft
            .field("format")
            .unwrap_or_else(|| default_format(spec).into())
    };
    expand(&format, &variables, data, &mut Vec::new(), spec.id)
}

fn make_config(draft: &StarshipDraft, context: &Context) -> Result<String, String> {
    let original: Table = toml::from_str(draft.contents()).map_err(|e| e.to_string())?;
    let root = original
        .get("format")
        .and_then(Value::as_str)
        .filter(|f| !f.is_empty())
        .unwrap_or("$all");
    let mut root = crate::starship_import::filter_format(root, &mut Default::default());
    let mut contains_selected = false;
    substitute(&root, |key| {
        contains_selected |= key == context.module;
        Ok(String::new())
    })?;
    // Omitted modules are inserted only in the simulated prompt, never the draft.
    if !contains_selected {
        let mut inserted = false;
        root = substitute(&root, |key| {
            if key == "character" && !inserted {
                inserted = true;
                Ok(format!("${{{}}}${{character}}", context.module))
            } else {
                Ok(format!("${{{key}}}"))
            }
        })?;
        if !inserted {
            root.push_str(&format!(" ${{{}}}", context.module));
        }
    }
    let mut data = Table::new();
    let format = substitute(&root, |key| {
        if key == "line_break" {
            let disabled = original
                .get("line_break")
                .and_then(|m| m.get("disabled"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            return Ok(if disabled { "" } else { "\n" }.into());
        }
        let index =
            module_index(key).ok_or_else(|| format!("Unsupported preview module: {key}"))?;
        let mut module = StarshipDraft::new("scene.toml".into(), draft.contents().into())?;
        module.select_module(index)?;
        if key == context.module {
            module.symbol = draft.symbol;
            module.style = draft.style;
        }
        module_expression(&module, context, &mut data)
    })?;
    // Even an imported style string must not inject another runtime module
    // into the generated root format. Only our declarative example slots exist.
    substitute(&format, |key| {
        if key
            .strip_prefix("env_var.")
            .is_some_and(|name| data.contains_key(name))
        {
            Ok(format!("${{{key}}}"))
        } else {
            Err("The sample contains an unsupported module reference.".into())
        }
    })?;
    let mut config = Table::new();
    for key in ["palette", "palettes"] {
        if let Some(value) = original.get(key) {
            config.insert(key.into(), value.clone());
        }
    }
    // All source-provided runtime modules and commands are intentionally absent.
    config.insert("format".into(), Value::String(format));
    config.insert("add_newline".into(), Value::Boolean(false));
    config.insert("env_var".into(), Value::Table(data));
    config.insert("command_timeout".into(), Value::Integer(250));
    toml::to_string(&config).map_err(|e| e.to_string())
}

fn expand(
    text: &str,
    variables: &BTreeMap<String, Variable>,
    data: &mut Table,
    stack: &mut Vec<String>,
    namespace: &str,
) -> Result<String, String> {
    if stack.len() > 8 {
        return Err("Sample format is too deeply nested.".into());
    }
    substitute(text, |key| {
        if stack.iter().any(|item| item == key) {
            return Err(format!("Sample format contains a recursive ${key}."));
        }
        let variable = variables.get(key).ok_or_else(|| {
            format!("No example for ${key}; the original format is kept for export.")
        })?;
        match variable {
            Variable::Style(value) => Ok(value.clone()),
            Variable::Format(value) => {
                // Keep literal meta variables as real formatter variables. If
                // inlined as plain text, enclosing Starship conditional groups
                // would incorrectly hide e.g. Git's default modified symbol.
                if let Some(literal) = literal_text(value) {
                    return Ok(data_variable(namespace, key, &literal, data));
                }
                if let Some((text, style)) = value
                    .strip_prefix('[')
                    .and_then(|v| v.strip_suffix(')'))
                    .and_then(|v| v.split_once("]("))
                    && let Some(literal) = literal_text(text)
                {
                    stack.push(key.into());
                    let style = expand(style, variables, data, stack, namespace);
                    stack.pop();
                    return Ok(format!(
                        "[{}]({})",
                        data_variable(namespace, key, &literal, data),
                        style?
                    ));
                }
                stack.push(key.into());
                let result = expand(value, variables, data, stack, namespace);
                stack.pop();
                result
            }
            Variable::Data(value) => Ok(data_variable(namespace, key, value, data)),
        }
    })
}

fn data_variable(namespace: &str, key: &str, value: &str, data: &mut Table) -> String {
    let mut field = Table::new();
    field.insert(
        "variable".into(),
        Value::String(format!("TERMIMOCHI_SCENE_{namespace}_{key}")),
    );
    field.insert("default".into(), Value::String(value.into()));
    field.insert("format".into(), Value::String("$env_value".into()));
    data.insert(format!("{namespace}_{key}"), Value::Table(field));
    format!("${{env_var.{namespace}_{key}}}")
}
fn literal_text(text: &str) -> Option<String> {
    let mut chars = text.chars();
    let mut out = String::new();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            out.push(chars.next()?);
        } else if "$[]()".contains(ch) {
            return None;
        } else {
            out.push(ch);
        }
    }
    Some(out)
}

fn substitute(
    text: &str,
    mut replacement: impl FnMut(&str) -> Result<String, String>,
) -> Result<String, String> {
    if text.len() > 16 * 1024 {
        return Err("Sample format exceeds 16 KiB.".into());
    }
    let mut chars = text.chars().peekable();
    let mut out = String::new();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            out.push(ch);
            out.extend(chars.next());
        } else if ch == '$' {
            let mut key = String::new();
            if chars.peek() == Some(&'{') {
                chars.next();
                let mut closed = false;
                for ch in chars.by_ref() {
                    if ch == '}' {
                        closed = true;
                        break;
                    }
                    key.push(ch);
                }
                if !closed {
                    return Err("Unclosed sample variable.".into());
                }
            } else {
                while chars
                    .peek()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                {
                    key.push(chars.next().unwrap());
                }
            }
            if key.is_empty() {
                return Err("Escape a literal dollar sign with a backslash.".into());
            }
            out.push_str(&replacement(&key)?);
        } else {
            out.push(ch);
        }
        if out.len() > 16 * 1024 {
            return Err("Expanded sample exceeds 16 KiB.".into());
        }
    }
    Ok(out)
}

fn default_format(spec: &ModuleSpec) -> &'static str {
    match spec.id {
        "rust" | "nodejs" | "golang" => "via [$symbol($version )]($style)",
        "python" => "via [${symbol}(${version} )(\\($virtualenv\\) )]($style)",
        "git_branch" => "on [$symbol$branch(:$remote_branch)]($style) ",
        "git_status" => "([\\[$all_status$ahead_behind\\]]($style) )",
        "directory" => "[$path]($style)[$read_only]($read_only_style) ",
        "username" => "[$user]($style) in ",
        "hostname" => "[$ssh_symbol$hostname]($style) in ",
        "character" => "$symbol ",
        "conda" => "via [$symbol$environment]($style) ",
        "cmd_duration" => "took [$duration]($style) ",
        "status" => "[$symbol$status]($style) ",
        "jobs" => "[$symbol$number]($style) ",
        "time" => "at [$time]($style) ",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        starship_draft::ModuleEdit,
        starship_import::{can_render_samples, render_scene},
        starship_modules::{MODULES, module_index},
    };

    const SOURCE: &str = "format='$directory'\npalette='cute'\n[palettes.cute]\npink='#eb6f92'\n[custom.private]\ncommand='must never run'\n";
    fn draft(id: &str) -> StarshipDraft {
        let mut draft = StarshipDraft::new("/tmp/source.toml".into(), SOURCE.into()).unwrap();
        draft.select_module(module_index(id).unwrap()).unwrap();
        draft
    }
    #[test]
    fn every_module_and_field_has_a_full_command_scene() {
        for spec in MODULES {
            let mut draft = draft(spec.id);
            for focus in [Focus::Symbol, Focus::Style, Focus::Format] {
                let count = match focus {
                    Focus::Symbol => spec.symbols.len(),
                    Focus::Style => spec.styles.len(),
                    Focus::Format => 1,
                };
                for i in 0..count {
                    if focus == Focus::Symbol {
                        draft.symbol = i;
                    } else if focus == Focus::Style {
                        draft.style = i;
                    }
                    let sample = CommandScene::new(&draft, focus);
                    let config: Table = toml::from_str(
                        sample
                            .config
                            .as_ref()
                            .unwrap_or_else(|e| panic!("{}: {e}", sample.title)),
                    )
                    .unwrap();
                    assert!(sample.title.contains("simulated"));
                    assert!(!sample.command.is_empty());
                    assert!(!config.contains_key("custom"));
                    assert!(config.keys().all(|k| {
                        [
                            "format",
                            "add_newline",
                            "palette",
                            "palettes",
                            "env_var",
                            "command_timeout",
                        ]
                        .contains(&k.as_str())
                    }));
                    assert_eq!(draft.contents(), SOURCE);
                    assert!(!draft.dirty() && !draft.can_undo());
                }
            }
        }
    }
    #[test]
    fn actual_renderer_shows_all_modules_even_outside_a_matching_project() {
        let temp = tempfile::tempdir().unwrap();
        if !can_render_samples(temp.path()) {
            return;
        }
        for spec in MODULES {
            let mut draft = draft(spec.id);
            draft.edit(ModuleEdit::Enabled(true)).unwrap();
            let focus = if spec.symbols.is_empty() {
                Focus::Style
            } else {
                Focus::Symbol
            };
            if !spec.symbols.is_empty() {
                draft
                    .edit(ModuleEdit::Symbol("sample-icon".into()))
                    .unwrap();
            }
            let sample = CommandScene::new(&draft, focus);
            let ansi = render_scene(&sample, temp.path(), 80).unwrap();
            assert!(!ansi.trim().is_empty(), "{}", sample.title);
            if !spec.symbols.is_empty() {
                assert!(ansi.contains("sample-icon"), "{}: {ansi:?}", spec.id);
            }
        }
    }
    #[test]
    fn actual_git_counts_colors_versions_and_raw_symbols_are_visible() {
        let temp = tempfile::tempdir().unwrap();
        if !can_render_samples(temp.path()) {
            return;
        }
        let render = |draft: &StarshipDraft, focus| {
            render_scene(&CommandScene::new(draft, focus), temp.path(), 80).unwrap()
        };
        let mut git = draft("git_status");
        git.symbol = 2;
        git.edit(ModuleEdit::Symbol("+${count}".into())).unwrap();
        git.edit(ModuleEdit::Color("#123456".into())).unwrap();
        let ansi = render(&git, Focus::Symbol);
        assert!(ansi.contains("+2"), "{ansi:?}");
        assert!(ansi.contains("38;2;18;52;86"), "{ansi:?}");
        let mut python = draft("python");
        python.edit(ModuleEdit::Layout(1)).unwrap();
        python.edit(ModuleEdit::Version(2)).unwrap();
        let ansi = render(&python, Focus::Format);
        assert!(ansi.contains("v3.13"), "{ansi:?}");
        assert!(!ansi.contains("v3.13.2"));
        let mut directory = draft("directory");
        directory.symbol = 1;
        directory
            .edit(ModuleEdit::Symbol("[$path](red)\\".into()))
            .unwrap();
        assert!(render(&directory, Focus::Symbol).contains("[$path](red)\\"));
        let mut character = draft("character");
        character.symbol = 1;
        character
            .edit(ModuleEdit::Symbol("[FAIL](pink)".into()))
            .unwrap();
        assert!(render(&character, Focus::Symbol).contains("FAIL"));
        character.edit(ModuleEdit::Symbol("".into())).unwrap();
        assert!(!render(&character, Focus::Symbol).contains("FAIL"));
        assert!(render(&character, Focus::Symbol).contains("~/projects/TermiMochi"));
    }

    #[test]
    fn actual_full_prompts_expose_every_symbol_and_style_selector() {
        let temp = tempfile::tempdir().unwrap();
        if !can_render_samples(temp.path()) {
            return;
        }
        for spec in MODULES {
            for (index, field) in spec.symbols.iter().enumerate() {
                let mut draft = draft(spec.id);
                draft.edit(ModuleEdit::Enabled(true)).unwrap();
                draft.symbol = index;
                draft.edit(ModuleEdit::Symbol("EDITED".into())).unwrap();
                let scene = CommandScene::new(&draft, Focus::Symbol);
                let ansi = render_scene(&scene, temp.path(), 120).unwrap();
                assert!(
                    ansi.contains("EDITED"),
                    "{}.{}: {ansi:?}",
                    spec.id,
                    field.key
                );
            }
            for (index, field) in spec.styles.iter().enumerate() {
                let mut draft = draft(spec.id);
                draft.edit(ModuleEdit::Enabled(true)).unwrap();
                draft.style = index;
                draft.edit(ModuleEdit::Color("#123456".into())).unwrap();
                let scene = CommandScene::new(&draft, Focus::Style);
                let ansi = render_scene(&scene, temp.path(), 120).unwrap();
                assert!(
                    ansi.contains("38;2;18;52;86"),
                    "{}.{}: {ansi:?}",
                    spec.id,
                    field.key
                );
            }
        }
    }
    #[test]
    fn unsupported_recursive_and_injected_formats_cannot_run_modules() {
        for source in [
            "[rust]\nsymbol='$symbol'",
            "[rust]\nsymbol='${custom.bad}'\n[custom.bad]\ncommand='touch /tmp/never-run'",
            "[rust]\nstyle=') ] ${custom.bad}'",
        ] {
            let draft = StarshipDraft::new("/tmp/source.toml".into(), source.into()).unwrap();
            assert!(CommandScene::new(&draft, Focus::Symbol).config.is_err());
            assert_eq!(draft.contents(), source);
        }
        assert!(substitute("${unfinished", |_| Ok("".into())).is_err());
        assert!(substitute(&"x".repeat(17000), |_| Ok("".into())).is_err());
    }

    #[test]
    fn selection_appends_typing_replaces_and_new_copy_clears_history() {
        let mut history = std::collections::VecDeque::new();
        let mut draft = draft("git_status");
        let frame = |draft: &StarshipDraft, document, ansi: &str| RenderedScene {
            scene: SceneRequest::capture(draft, document).build(),
            ansi: Ok(ansi.into()),
        };
        draft.symbol = 2;
        record(&mut history, frame(&draft, 0, "+2"));
        draft.symbol = 1;
        record(&mut history, frame(&draft, 0, "?2"));
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].scene.command, "git add README.md src/");
        assert_eq!(history[1].scene.command, "touch notes.txt todo.txt");
        draft.edit(ModuleEdit::Symbol("U${count}".into())).unwrap();
        record(&mut history, frame(&draft, 0, "U2"));
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].ansi.as_ref().unwrap(), "U2");
        for index in 0..12 {
            draft.symbol = index;
            record(&mut history, frame(&draft, 0, "updated"));
            assert!(history.len() <= 6);
        }
        assert_eq!(history.len(), 6);
        record(&mut history, frame(&draft, 1, "new copy"));
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].ansi.as_ref().unwrap(), "new copy");
    }

    #[test]
    fn actual_commands_render_complete_multiline_prompts_without_changing_export() {
        let temp = tempfile::tempdir().unwrap();
        if !can_render_samples(temp.path()) {
            return;
        }
        let source = "format='╭─$directory$git_branch$line_break╰─$character'\n[character]\nerror_symbol='[FAIL](red)'\n[git_status]\nstaged='+${count}'\n";
        let mut draft = StarshipDraft::new("source.toml".into(), source.into()).unwrap();
        let render = |draft: &StarshipDraft| {
            let scene = CommandScene::new(draft, Focus::Symbol);
            let ansi = render_scene(&scene, temp.path(), 120).unwrap();
            assert!(ansi.contains('╭') && ansi.contains('╰'), "{ansi:?}");
            assert!(ansi.contains('\n'), "{ansi:?}");
            assert!(ansi.contains("feature/terminal-preview"), "{ansi:?}");
            assert_eq!(draft.contents(), source);
            assert!(!draft.dirty());
            (scene, ansi)
        };
        draft
            .select_module(module_index("python").unwrap())
            .unwrap();
        let (scene, ansi) = render(&draft);
        assert_eq!(scene.command, "cd ~/projects/python-api");
        assert!(
            ansi.contains("~/projects/python-api") && ansi.contains("v3.13.2"),
            "{ansi:?}"
        );
        draft
            .select_module(module_index("git_status").unwrap())
            .unwrap();
        draft.symbol = 2;
        let (scene, ansi) = render(&draft);
        assert_eq!(scene.command, "git add README.md src/");
        assert!(ansi.contains("+2"), "{ansi:?}");
        assert!(
            ansi.find('╰').unwrap() < ansi.find("+2").unwrap(),
            "omitted module is inserted beside the final character"
        );
        draft
            .select_module(module_index("character").unwrap())
            .unwrap();
        draft.symbol = 1;
        let (scene, ansi) = render(&draft);
        assert_eq!(scene.command, "false");
        assert!(ansi.contains("FAIL"), "{ansi:?}");
    }

    #[test]
    fn disabled_flags_and_literal_status_conditionals_survive_full_scene_rendering() {
        let temp = tempfile::tempdir().unwrap();
        if !can_render_samples(temp.path()) {
            return;
        }
        let mut draft = StarshipDraft::new("source.toml".into(),
            "format='$directory$line_break$rust$git_status$character'\n[rust]\ndisabled=true\nsymbol='RUST-ICON'\n[line_break]\ndisabled=true\n[git_status]\nstaged='[STAGED](red)'\n".into()).unwrap();
        let render = |draft: &StarshipDraft| {
            render_scene(&CommandScene::new(draft, Focus::Symbol), temp.path(), 120).unwrap()
        };
        assert!(!render(&draft).contains("RUST-ICON"));
        assert!(!render(&draft).contains('\n'));
        draft
            .select_module(module_index("git_status").unwrap())
            .unwrap();
        draft.symbol = 2;
        assert!(render(&draft).contains("STAGED"));
        draft.edit(ModuleEdit::Symbol("+".into())).unwrap();
        assert!(
            render(&draft).contains('+'),
            "literal symbols remain present inside conditional groups"
        );
        draft.edit(ModuleEdit::Symbol("".into())).unwrap();
        assert!(!render(&draft).contains('+'));
    }

    #[test]
    fn alternate_styles_choose_matching_commands_and_inherit_base_style() {
        let temp = tempfile::tempdir().unwrap();
        if !can_render_samples(temp.path()) {
            return;
        }
        let render = |draft: &StarshipDraft, focus| {
            let scene = CommandScene::new(draft, focus);
            let ansi = render_scene(&scene, temp.path(), 120).unwrap();
            (scene, ansi)
        };
        let mut draft = draft("username");
        draft.style = 1;
        draft.edit(ModuleEdit::Color("#334455".into())).unwrap();
        let (scene, ansi) = render(&draft, Focus::Style);
        assert_eq!(scene.command, "sudo -i");
        assert!(
            ansi.contains("root") && ansi.contains("38;2;51;68;85"),
            "{ansi:?}"
        );
        draft
            .select_module(module_index("status").unwrap())
            .unwrap();
        draft.edit(ModuleEdit::Enabled(true)).unwrap();
        draft.style = 0;
        draft.edit(ModuleEdit::Color("#123456".into())).unwrap();
        draft.style = draft
            .spec()
            .styles
            .iter()
            .position(|f| f.key == "failure_style")
            .unwrap();
        let (scene, ansi) = render(&draft, Focus::Style);
        assert_eq!(scene.command, "false");
        assert!(
            ansi.contains("38;2;18;52;86"),
            "unset alternate style inherits main style: {ansi:?}"
        );
        draft
            .select_module(module_index("directory").unwrap())
            .unwrap();
        draft.symbol = 1; // Home symbol must not override the read-only context.
        draft.style = draft
            .spec()
            .styles
            .iter()
            .position(|f| f.key == "read_only_style")
            .unwrap();
        let (scene, ansi) = render(&draft, Focus::Style);
        assert_eq!(scene.command, "cd /usr/share");
        assert!(ansi.contains("/usr/share"), "{ansi:?}");
        let (scene, ansi) = render(&draft, Focus::Symbol);
        assert_eq!(scene.command, "cd ~");
        assert!(!ansi.contains("/usr/share"), "{ansi:?}");
    }
}
