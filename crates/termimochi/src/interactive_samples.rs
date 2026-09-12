//! Bounded, in-memory samples. Deliberately no filesystem, process, network,
//! clock or theme dependency. Actions cannot carry an executable command.
use crate::preview::PreviewScenario;
use std::collections::VecDeque;

pub(crate) const MAX_TABS: usize = 8;
pub(crate) const MAX_BLOCKS: usize = 48;
pub(crate) const MAX_HISTORY: usize = 100;
pub(crate) const HELP: &str = "Interactive samples — no local commands are executed.\r\n\
help | pwd | ls | ls -l | cd / | cd demo | cd src | cd ..\r\n\
git status | git diff | cargo test | fastfetch | clear\r\n\
Directories: /, /demo, /demo/src. All files and test results are fixtures.\r\n";
pub(crate) const COMMANDS: &[&str] = &[
    "help",
    "pwd",
    "ls",
    "ls -l",
    "cd /",
    "cd demo",
    "cd src",
    "cd ..",
    "git status",
    "git diff",
    "cargo test",
    "fastfetch",
    "clear",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Directory {
    Root,
    Demo,
    Src,
}
impl Directory {
    pub(crate) fn path(self) -> &'static str {
        match self {
            Self::Root => "/",
            Self::Demo => "/demo",
            Self::Src => "/demo/src",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Help,
    Pwd,
    List(bool),
    Cd(&'static str),
    Sample(PreviewScenario),
    Greeting,
    Clear,
}
impl Action {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Help => "help".into(),
            Self::Pwd => "pwd".into(),
            Self::List(false) => "ls".into(),
            Self::List(true) => "ls -l".into(),
            Self::Cd(path) => format!("cd {path}"),
            Self::Greeting => "fastfetch".into(),
            Self::Clear => "clear".into(),
            Self::Sample(PreviewScenario::Tests) => "cargo test".into(),
            Self::Sample(scenario) => scenario.terminal_title().into(),
        }
    }
}
pub(crate) fn parse(input: &str) -> Result<Action, &'static str> {
    if input.len() > 1024
        || input.chars().any(char::is_control)
        || [";", "&", "|", "<", ">", "`", "$", "\\"]
            .iter()
            .any(|s| input.contains(s))
    {
        return Err(
            "Only one built-in sample at a time; shell syntax and control characters are rejected.",
        );
    }
    Ok(match input.trim() {
        "help" => Action::Help,
        "pwd" => Action::Pwd,
        "ls" => Action::List(false),
        "ls -l" => Action::List(true),
        "cd /" => Action::Cd("/"),
        "cd demo" => Action::Cd("demo"),
        "cd src" => Action::Cd("src"),
        "cd .." => Action::Cd(".."),
        "git status" => Action::Sample(PreviewScenario::GitStatus),
        "git diff" => Action::Sample(PreviewScenario::GitDiff),
        "cargo test" => Action::Sample(PreviewScenario::Tests),
        "fastfetch" => Action::Greeting,
        "clear" => Action::Clear,
        _ => return Err("Unknown sample. Type help — local commands are never executed."),
    })
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Output {
    Text(String),
    Sample(PreviewScenario),
    Greeting,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Block {
    pub id: u64,
    pub directory: Directory,
    pub command: String,
    pub output: Output,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tab {
    pub id: u64,
    pub directory: Directory,
    pub draft: String,
    pub history: VecDeque<String>,
    pub blocks: VecDeque<Block>,
    pub history_index: Option<usize>,
    pub history_draft: String,
    /// Character-row observation anchor, independent of font/zoom pixels.
    pub scroll_row: f64,
    pub scroll_anchor: Option<(u64, usize)>,
    pub follow: bool,
    next_block: u64,
}
impl Tab {
    fn new(id: u64) -> Self {
        let mut tab = Self {
            id,
            directory: Directory::Demo,
            draft: String::new(),
            history: VecDeque::new(),
            blocks: VecDeque::new(),
            history_index: None,
            history_draft: String::new(),
            scroll_row: 0.0,
            scroll_anchor: None,
            follow: false,
            next_block: 0,
        };
        tab.append("", Output::Greeting);
        tab
    }
    fn append(&mut self, command: &str, output: Output) {
        self.next_block += 1;
        self.blocks.push_back(Block {
            id: self.next_block,
            directory: self.directory,
            command: command.into(),
            output,
        });
        while self.blocks.len() > MAX_BLOCKS {
            self.blocks.pop_front();
        }
    }
    pub(crate) fn submit(&mut self, input: &str) -> Result<(), &'static str> {
        let action = parse(input)?;
        self.history.push_back(input.trim().into());
        while self.history.len() > MAX_HISTORY {
            self.history.pop_front();
        }
        self.history_index = None;
        self.draft.clear();
        self.act(action, input.trim());
        Ok(())
    }
    pub(crate) fn act(&mut self, action: Action, command: &str) {
        self.follow = true;
        let output = match action {
            Action::Clear => {
                self.blocks.clear();
                self.scroll_row = 0.0;
                self.scroll_anchor = None;
                return;
            }
            Action::Help => Output::Text(HELP.into()),
            Action::Pwd => Output::Text(format!("{}\r\n", self.directory.path())),
            Action::List(long) => {
                let names = match self.directory {
                    Directory::Root => "demo/",
                    Directory::Demo => "src/  Cargo.toml  README.md",
                    Directory::Src => "main.rs  preview.rs",
                };
                Output::Text(format!(
                    "{}{}\r\n",
                    if long {
                        "Fixture listing · synthetic permissions/size/date\r\n-rw-r--r--  demo demo 128  Jan 01  "
                    } else {
                        "Fixture files: "
                    },
                    names
                ))
            }
            Action::Cd(path) => {
                let next = match (self.directory, path) {
                    (_, "/") => Some(Directory::Root),
                    (Directory::Root, "demo") => Some(Directory::Demo),
                    (Directory::Demo, "src") => Some(Directory::Src),
                    (Directory::Src, "..") => Some(Directory::Demo),
                    (_, "..") => Some(Directory::Root),
                    _ => None,
                };
                self.append(
                    command,
                    Output::Text(if next.is_some() {
                        String::new()
                    } else {
                        "No such fixture directory. Try cd / then cd demo.\r\n".into()
                    }),
                );
                if let Some(next) = next {
                    self.directory = next;
                }
                return;
            }
            Action::Sample(PreviewScenario::GitDiff | PreviewScenario::GitStatus)
                if self.directory == Directory::Root =>
            {
                Output::Text("Not a fixture repository. Try cd demo.\r\n".into())
            }
            Action::Sample(PreviewScenario::CurrentFolder) => Output::Text(
                "Current Folder is a separate, explicit read-only snapshot in Terminal scene.\r\n"
                    .into(),
            ),
            Action::Sample(scenario) => Output::Sample(scenario),
            Action::Greeting => Output::Greeting,
        };
        self.append(command, output);
    }
    pub(crate) fn recall(&mut self, previous: bool) -> String {
        if self.history_index.is_none() {
            self.history_draft = self.draft.clone();
        }
        self.history_index = if previous {
            self.history_index
                .unwrap_or(self.history.len())
                .checked_sub(1)
                .or(self.history_index)
        } else {
            self.history_index
                .and_then(|i| (i + 1 < self.history.len()).then_some(i + 1))
        };
        self.draft = self
            .history_index
            .map(|i| self.history[i].clone())
            .unwrap_or_else(|| self.history_draft.clone());
        self.draft.clone()
    }
}
#[derive(Debug)]
pub(crate) struct Session {
    pub tabs: Vec<Tab>,
    pub active: usize,
    next_id: u64,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            tabs: vec![Tab::new(1)],
            active: 0,
            next_id: 2,
        }
    }
}
impl Session {
    pub(crate) fn current(&self) -> &Tab {
        &self.tabs[self.active]
    }
    pub(crate) fn current_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }
    pub(crate) fn add(&mut self) -> bool {
        if self.tabs.len() == MAX_TABS {
            return false;
        }
        self.tabs.push(Tab::new(self.next_id));
        self.next_id += 1;
        self.active = self.tabs.len() - 1;
        true
    }
    pub(crate) fn close(&mut self) {
        self.tabs.remove(self.active);
        if self.tabs.is_empty() {
            self.add();
        }
        self.active = self.active.min(self.tabs.len() - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_cannot_reach_an_execution_action() {
        for s in [
            "touch /tmp/oops",
            "ls; pwd",
            "ls && pwd",
            "ls || pwd",
            "ls > x",
            "`id`",
            "$(id)",
            "ls\npwd",
            "\x1b]52;c;eA==\x07",
            "ls\r",
        ] {
            let mut tab = Tab::new(1);
            let before = tab.clone();
            assert!(tab.submit(s).is_err(), "{s:?}");
            assert_eq!(tab, before);
        }
        for command in COMMANDS {
            assert!(parse(command).is_ok(), "{command}");
        }
    }
    #[test]
    fn tabs_history_directory_and_budgets_are_independent() {
        let mut session = Session::default();
        session.current_mut().draft = "未提交 👩‍💻".into();
        let before = session.current().clone();
        session.add();
        session.current_mut().submit("cd /").unwrap();
        assert_eq!(session.tabs[0], before);
        for _ in 0..2000 {
            session.current_mut().submit("git diff").unwrap();
        }
        assert_eq!(session.current().history.len(), MAX_HISTORY);
        assert_eq!(session.current().blocks.len(), MAX_BLOCKS);
        session.current_mut().submit("clear").unwrap();
        assert!(session.current().blocks.is_empty());
        assert_eq!(session.current().directory, Directory::Root);
        for _ in 0..50 {
            session.add();
            session.close();
        }
        assert_eq!(session.tabs.len(), 2);
        while session.add() {}
        assert_eq!(session.tabs.len(), MAX_TABS);
    }
    #[test]
    fn history_restores_draft_and_greeting_has_stable_identity() {
        let mut tab = Tab::new(1);
        tab.submit("help").unwrap();
        tab.submit("fastfetch").unwrap();
        assert_ne!(tab.blocks[0].id, tab.blocks[2].id);
        tab.draft = "unfinished".into();
        assert_eq!(tab.recall(true), "fastfetch");
        assert_eq!(tab.recall(true), "help");
        assert_eq!(tab.recall(false), "fastfetch");
        assert_eq!(tab.recall(false), "unfinished");
    }
}
