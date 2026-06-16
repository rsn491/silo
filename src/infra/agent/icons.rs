//! Half-block terminal icon rendering for agent brand logos.
//!
//! Each icon is a 5-column × 6-row boolean pixel grid, scaled via nearest-neighbour
//! sampling to any requested cell size, then rendered with Unicode half-block characters
//! (▀ ▄) and per-character ANSI colour.  No external crates beyond ratatui are needed.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use super::Agent;

/// Terminal columns for a compact table icon.
pub const ICON_CHAR_W: u16 = 5;
/// Terminal rows for a compact table icon (each row encodes 2 pixel rows).
pub const ICON_CHAR_H: u16 = 3;

/// Terminal columns for the larger `init`-selector preview icon.
pub const PREVIEW_CHAR_W: u16 = 10;
/// Terminal rows for the larger `init`-selector preview icon.
pub const PREVIEW_CHAR_H: u16 = 8;

/// 5-column × 6-row boolean pixel grid (rows top-to-bottom, columns left-to-right).
type Px = [[bool; 5]; 6];

// ── Brand pixel patterns ──────────────────────────────────────────────────────

/// Anthropic "A" shape (ClaudeCode).
const CLAUDE: Px = [
    [false, false, true,  false, false],
    [false, true,  false, true,  false],
    [true,  false, false, false, true ],
    [true,  true,  true,  true,  true ],
    [true,  false, false, false, true ],
    [true,  false, false, false, true ],
];

/// Hollow rectangle — matches the OpenCode logo (outer square, inner cutout).
const OPENCODE: Px = [
    [true,  true,  true,  true,  true ],
    [true,  false, false, false, true ],
    [true,  false, false, false, true ],
    [true,  false, false, false, true ],
    [true,  false, false, false, true ],
    [true,  true,  true,  true,  true ],
];

/// Stylised "C" for Codex (OpenAI Codex CLI).
const CODEX: Px = [
    [false, true,  true,  true,  false],
    [true,  false, false, false, false],
    [true,  false, false, false, false],
    [true,  false, false, false, false],
    [true,  false, false, false, false],
    [false, true,  true,  true,  false],
];

/// 4-pointed star / sparkle — matches the Google Gemini brand icon.
const GEMINI: Px = [
    [false, false, true,  false, false],
    [false, false, true,  false, false],
    [true,  true,  true,  true,  true ],
    [false, false, true,  false, false],
    [false, false, true,  false, false],
    [false, false, false, false, false],
];

/// Simplified robot face (Droid / factory.ai).
const DROID: Px = [
    [false, true,  true,  true,  false],
    [true,  false, true,  false, true ],
    [true,  true,  true,  true,  true ],
    [true,  true,  false, true,  true ],
    [true,  true,  true,  true,  true ],
    [false, true,  false, true,  false],
];

// ── Per-agent brand colour ────────────────────────────────────────────────────

fn tint(agent: &Agent) -> Color {
    match agent {
        Agent::ClaudeCode => Color::Rgb(212, 168,  75), // Anthropic amber
        Agent::OpenCode   => Color::Rgb(200, 100, 255), // purple
        Agent::Codex      => Color::Rgb(100, 210, 210), // teal
        Agent::Gemini     => Color::Rgb(100, 150, 255), // Google blue
        Agent::Droid      => Color::Rgb( 50, 200,  50), // green
    }
}

fn pixel_pattern(agent: &Agent) -> &'static Px {
    match agent {
        Agent::ClaudeCode => &CLAUDE,
        Agent::OpenCode   => &OPENCODE,
        Agent::Codex      => &CODEX,
        Agent::Gemini     => &GEMINI,
        Agent::Droid      => &DROID,
    }
}

// ── Renderer ──────────────────────────────────────────────────────────────────

/// Scale the 5×6 source pattern to `(px_w × px_h)` using nearest-neighbour sampling.
fn scale(src: &Px, px_w: usize, px_h: usize) -> Vec<Vec<bool>> {
    (0..px_h)
        .map(|dy| {
            (0..px_w)
                .map(|dx| {
                    let sx = (dx * 5 / px_w.max(1)).min(4);
                    let sy = (dy * 6 / px_h.max(1)).min(5);
                    src[sy][sx]
                })
                .collect()
        })
        .collect()
}

/// Convert a scaled pixel grid to ratatui half-block `Line`s tinted with `color`.
///
/// Pairs of pixel rows map to one terminal row via `'▀'` (upper half block):
/// - both off   →  `' '`
/// - top on     →  `'▀'` fg=color
/// - bottom on  →  `'▄'` fg=color
/// - both on    →  `'▀'` fg=color bg=color
fn pixels_to_lines(pixels: &[Vec<bool>], color: Color) -> Vec<Line<'static>> {
    let px_h = pixels.len();
    let mut lines = Vec::with_capacity(px_h / 2);
    let mut y = 0;
    while y + 1 < px_h {
        let spans: Vec<Span<'static>> = pixels[y]
            .iter()
            .zip(pixels[y + 1].iter())
            .map(|(&top, &bot)| match (top, bot) {
                (false, false) => Span::raw(" "),
                (true,  false) => Span::styled("▀", Style::default().fg(color).bg(Color::Reset)),
                (false, true)  => Span::styled("▄", Style::default().fg(color).bg(Color::Reset)),
                (true,  true)  => Span::styled("▀", Style::default().fg(color).bg(color)),
            })
            .collect();
        lines.push(Line::from(spans));
        y += 2;
    }
    lines
}

/// Render the agent's icon at `char_w` columns × `char_h` rows.
fn render(agent: &Agent, char_w: u16, char_h: u16) -> Vec<Line<'static>> {
    let px_w = char_w as usize;
    let px_h = char_h as usize * 2;
    let scaled = scale(pixel_pattern(agent), px_w, px_h);
    pixels_to_lines(&scaled, tint(agent))
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Returns a compact 3-row icon for use as a table cell in `silo ps`.
pub fn table_icon(agent: &Agent) -> Vec<Line<'static>> {
    render(agent, ICON_CHAR_W, ICON_CHAR_H)
}

/// Returns an 8-row preview icon for use in the `init` agent-selector side panel.
pub fn preview_icon(agent: &Agent) -> Vec<Line<'static>> {
    render(agent, PREVIEW_CHAR_W, PREVIEW_CHAR_H)
}

/// Pre-renders table icons for all five agent variants in enum declaration order.
///
/// Index mapping: 0 = ClaudeCode, 1 = OpenCode, 2 = Codex, 3 = Gemini, 4 = Droid.
pub fn all_table_icons() -> [Vec<Line<'static>>; 5] {
    use strum::IntoEnumIterator;
    Agent::iter()
        .map(|a| table_icon(&a))
        .collect::<Vec<_>>()
        .try_into()
        .expect("exactly 5 agent variants")
}

/// Returns the index into the array produced by [`all_table_icons`] for the given agent.
pub fn agent_icon_index(agent: &Agent) -> usize {
    match agent {
        Agent::ClaudeCode => 0,
        Agent::OpenCode   => 1,
        Agent::Codex      => 2,
        Agent::Gemini     => 3,
        Agent::Droid      => 4,
    }
}
