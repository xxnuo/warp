use std::borrow::Cow;
use std::fmt;

use warp_i18n::{tr, tr_with};
use warpui::Action;

use crate::server::telemetry::AddTabWithShellSource;
use crate::terminal::available_shells::AvailableShell;
use crate::terminal::view::TerminalAction;
use crate::WorkspaceAction;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NewSessionOptionId(pub(crate) String);
impl NewSessionOptionId {
    #[cfg_attr(not(feature = "local_tty"), allow(dead_code))]
    pub(super) fn new(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug)]
pub(super) enum Direction {
    Down,
    Right,
    Up,
    Left,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Direction::Down => "Down",
                Direction::Right => "Right",
                Direction::Up => "Up",
                Direction::Left => "Left",
            }
        )
    }
}

#[derive(Debug)]
pub(super) enum NewSessionConfig {
    NewTab(AvailableShell),
    NewWindow(AvailableShell),
    Split(Direction, AvailableShell),
}

impl NewSessionConfig {
    fn shell(&self) -> &AvailableShell {
        match self {
            NewSessionConfig::NewTab(shell) => shell,
            NewSessionConfig::NewWindow(shell) => shell,
            NewSessionConfig::Split(_, shell) => shell,
        }
    }

    fn search_description(&self) -> String {
        match self {
            NewSessionConfig::NewTab(shell) => format!("Create New Tab {}", shell.short_name()),
            NewSessionConfig::NewWindow(shell) => {
                format!("Create New Window {}", shell.short_name())
            }
            NewSessionConfig::Split(direction, shell) => {
                format!("Split Pane {direction} {}", shell.short_name())
            }
        }
    }
}

#[derive(Debug)]
/// An option for creating a new terminal session
///
/// Contains configuration information like:
/// - which shell to use
/// - how to display the option in the command palette
pub struct NewSessionOption {
    id: NewSessionOptionId,
    description: String,
    config: NewSessionConfig,
}

impl NewSessionOption {
    pub fn id(&self) -> &NewSessionOptionId {
        &self.id
    }

    /// Returns the description (a.k.a. the top line in the command palette entry)
    pub fn description(&self) -> &str {
        self.description.as_str()
    }

    pub fn search_text(&self) -> String {
        let mut text = self.description.clone();
        text.push(' ');
        text.push_str(&self.config.search_description());
        text
    }
}

impl NewSessionOption {
    pub(super) fn new(id: NewSessionOptionId, config: NewSessionConfig) -> Self {
        let description = match &config {
            NewSessionConfig::NewTab(shell) => {
                let shell_name = shell.short_name();
                tr_with(
                    "command_palette.new_session.create_new_tab",
                    &[("shell", shell_name.as_ref())],
                )
            }
            NewSessionConfig::NewWindow(shell) => {
                let shell_name = shell.short_name();
                tr_with(
                    "command_palette.new_session.create_new_window",
                    &[("shell", shell_name.as_ref())],
                )
            }
            NewSessionConfig::Split(direction, shell) => {
                let direction = direction.localized_name();
                let shell_name = shell.short_name();
                tr_with(
                    "command_palette.new_session.split_pane",
                    &[("direction", &direction), ("shell", shell_name.as_ref())],
                )
            }
        };
        Self {
            id,
            description,
            config,
        }
    }

    /// Returns an action that should be triggered if this entry is accepted
    pub fn action(&self) -> Box<dyn Action> {
        match &self.config {
            NewSessionConfig::NewTab(shell) => Box::new(WorkspaceAction::AddTabWithShell {
                shell: shell.clone(),
                source: AddTabWithShellSource::CommandPalette,
            }),
            NewSessionConfig::NewWindow(shell) => Box::new(WorkspaceAction::AddWindowWithShell {
                shell: shell.clone(),
            }),
            NewSessionConfig::Split(Direction::Down, shell) => {
                Box::new(TerminalAction::SplitDown(Some(shell.clone())))
            }
            NewSessionConfig::Split(Direction::Up, shell) => {
                Box::new(TerminalAction::SplitUp(Some(shell.clone())))
            }
            NewSessionConfig::Split(Direction::Right, shell) => {
                Box::new(TerminalAction::SplitRight(Some(shell.clone())))
            }
            NewSessionConfig::Split(Direction::Left, shell) => {
                Box::new(TerminalAction::SplitLeft(Some(shell.clone())))
            }
        }
    }

    /// Returns the details (a.k.a. the second line in the command palette entry)
    pub fn details(&self) -> Cow<'_, str> {
        self.config.shell().details()
    }
}

impl Direction {
    fn localized_name(&self) -> String {
        match self {
            Direction::Down => tr("command_palette.new_session.direction.down"),
            Direction::Right => tr("command_palette.new_session.direction.right"),
            Direction::Up => tr("command_palette.new_session.direction.up"),
            Direction::Left => tr("command_palette.new_session.direction.left"),
        }
    }
}
