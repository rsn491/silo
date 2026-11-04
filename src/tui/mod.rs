//! Terminal UI components for Silo, built on ratatui + crossterm.

pub mod confirm;
pub mod input;
pub mod prompt;
pub mod select;
pub mod table;
#[cfg(test)]
pub mod test_support;

pub use confirm::run_confirm;
pub use input::run_input;
pub use prompt::{Prompt, TerminalPrompt};
pub use select::{SelectItem, run_select};
pub use table::{Column, Row, StyledCell, pad_or_trunc, print_info, print_status, render_table};
