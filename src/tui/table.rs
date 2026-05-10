//! Inline table rendering that keeps output in the terminal scrollback.

use std::io::{self, Write as _};

use crossterm::{
    execute,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
};

/// A column definition for [`render_table`].
pub struct Column {
    /// Header label.
    pub header: &'static str,
    /// Display width in characters.
    pub width: usize,
}

/// A single styled cell value.
pub struct StyledCell {
    /// Text content.
    pub text: String,
    /// Optional crossterm foreground colour override.
    pub color: Option<Color>,
    /// Whether to render the text bold.
    pub bold: bool,
}

impl StyledCell {
    /// Plain cell with no special styling.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: None,
            bold: false,
        }
    }

    /// Cell with a foreground colour.
    pub fn colored(text: impl Into<String>, color: Color) -> Self {
        Self {
            text: text.into(),
            color: Some(color),
            bold: false,
        }
    }

    /// Cell rendered in dim (dark-gray) colour.
    pub fn dim(text: impl Into<String>) -> Self {
        Self::colored(text, Color::DarkGrey)
    }

    /// Cell rendered in yellow — indicates a warning state.
    pub fn warning(text: impl Into<String>) -> Self {
        Self::colored(text, Color::Yellow)
    }
}

/// A table row.
pub struct Row {
    /// Cells in column order.
    pub cells: Vec<StyledCell>,
}

/// Render a styled table inline to stdout (no alternate screen).
///
/// Output stays in the terminal scrollback buffer just like `docker ps` or `ls -l`.
///
/// # Errors
///
/// Returns an error if writing to stdout fails.
pub fn render_table(
    title: &str,
    cols: &[Column],
    rows: &[Row],
    empty_msg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut out = io::stdout();

    // Title
    execute!(
        out,
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold),
        Print(title),
        Print("\n"),
        ResetColor,
        SetAttribute(Attribute::Reset),
    )?;
    execute!(out, Print("\n"))?;

    if rows.is_empty() {
        execute!(
            out,
            SetForegroundColor(Color::DarkGrey),
            Print("  "),
            Print(empty_msg),
            Print("\n"),
            ResetColor,
        )?;
        out.flush()?;
        return Ok(());
    }

    // Header row
    execute!(
        out,
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold),
        Print("  "),
    )?;
    for col in cols {
        let padded = pad_or_trunc(&col.header.to_uppercase(), col.width);
        execute!(out, Print(padded), Print("  "))?;
    }
    execute!(out, Print("\n"), ResetColor, SetAttribute(Attribute::Reset),)?;

    // Separator line
    execute!(out, SetForegroundColor(Color::DarkGrey), Print("  "))?;
    for col in cols {
        execute!(out, Print("─".repeat(col.width)), Print("  "))?;
    }
    execute!(out, Print("\n"), ResetColor)?;

    // Data rows
    for (i, row) in rows.iter().enumerate() {
        // Subtle alternating row via dim attribute on odd rows.
        if i % 2 == 1 {
            execute!(out, SetAttribute(Attribute::Dim))?;
        }
        execute!(out, Print("  "))?;

        for (ci, cell) in row.cells.iter().enumerate() {
            let width = cols.get(ci).map(|c| c.width).unwrap_or(0);
            let text = pad_or_trunc(&cell.text, width);

            if let Some(color) = cell.color {
                execute!(out, SetForegroundColor(color))?;
            }
            if cell.bold {
                execute!(out, SetAttribute(Attribute::Bold))?;
            }

            execute!(out, Print(text), Print("  "))?;

            if cell.color.is_some() {
                execute!(out, ResetColor)?;
            }
            if cell.bold {
                execute!(out, SetAttribute(Attribute::Reset))?;
            }
        }

        if i % 2 == 1 {
            execute!(out, SetAttribute(Attribute::Reset))?;
        }
        execute!(out, Print("\n"))?;
    }

    execute!(out, Print("\n"))?;
    out.flush()?;
    Ok(())
}

/// Truncates `s` to `width` chars, appending `…` if needed; pads with spaces to fill `width`.
pub fn pad_or_trunc(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count <= width {
        let padding = width - count;
        format!("{}{}", s, " ".repeat(padding))
    } else {
        let mut t: String = s.chars().take(width.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// Print a single styled status line to stderr (no alternate screen, no raw mode).
///
/// Used for progress/info messages that should appear in the normal scrollback.
///
/// # Errors
///
/// Returns an error if writing to stderr fails.
pub fn print_status(
    prefix: &str,
    prefix_color: Color,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut out = io::stderr();
    execute!(
        out,
        SetForegroundColor(prefix_color),
        SetAttribute(Attribute::Bold),
        Print(prefix),
        ResetColor,
        SetAttribute(Attribute::Reset),
        Print(" "),
        Print(message),
        Print("\n"),
    )?;
    out.flush()?;
    Ok(())
}

/// Print a dim info line to stderr (no alternate screen, no raw mode).
///
/// # Errors
///
/// Returns an error if writing to stderr fails.
pub fn print_info(message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut out = io::stderr();
    execute!(
        out,
        SetForegroundColor(Color::DarkGrey),
        Print(message),
        Print("\n"),
        ResetColor,
    )?;
    out.flush()?;
    Ok(())
}
