//! Abstraction over interactive prompts so commands can be tested without a terminal.

use crate::tui::{SelectItem, run_confirm, run_input, run_select};

/// Interactive user prompts used by CLI commands.
#[cfg_attr(test, mockall::automock)]
pub trait Prompt {
    /// Asks the user to pick one of `items`; returns `None` when cancelled.
    ///
    /// # Errors
    ///
    /// Returns an error if the terminal interaction fails.
    fn select(
        &self,
        prompt: &str,
        items: &[SelectItem],
        default: usize,
    ) -> Result<Option<usize>, Box<dyn std::error::Error>>;

    /// Asks a yes/no question.
    ///
    /// # Errors
    ///
    /// Returns an error if the terminal interaction fails.
    fn confirm(&self, prompt: &str, default_yes: bool) -> Result<bool, Box<dyn std::error::Error>>;

    /// Asks for a line of text; returns `None` when cancelled.
    ///
    /// # Errors
    ///
    /// Returns an error if the terminal interaction fails.
    // mockall cannot mock an elided lifetime inside `Option<&str>`.
    #[allow(clippy::needless_lifetimes)]
    fn input<'a>(
        &self,
        prompt: &str,
        initial: Option<&'a str>,
    ) -> Result<Option<String>, Box<dyn std::error::Error>>;
}

/// [`Prompt`] implementation backed by the ratatui widgets in this module.
#[derive(Debug, Default, Clone, Copy)]
pub struct TerminalPrompt;

impl Prompt for TerminalPrompt {
    fn select(
        &self,
        prompt: &str,
        items: &[SelectItem],
        default: usize,
    ) -> Result<Option<usize>, Box<dyn std::error::Error>> {
        run_select(prompt, items, default)
    }

    fn confirm(&self, prompt: &str, default_yes: bool) -> Result<bool, Box<dyn std::error::Error>> {
        run_confirm(prompt, default_yes)
    }

    fn input(
        &self,
        prompt: &str,
        initial: Option<&str>,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        run_input(prompt, initial)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_select_without_items_needs_no_terminal() {
        assert_eq!(TerminalPrompt.select("Pick", &[], 0).unwrap(), None);
    }
}
