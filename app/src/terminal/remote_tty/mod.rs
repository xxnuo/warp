mod event_loop;
mod terminal_manager;

use crate::terminal::shell::ShellType;

pub use terminal_manager::TerminalManager;

const REMOTE_TTY_SHELL: ShellType = ShellType::Bash;
