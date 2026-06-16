//! Half-block terminal icon rendering for agent brand logos using Simple Icons SVGs.
//!
//! Icons are rendered at compile time into pixel grids and displayed using Unicode
//! half-block characters (▀ ▄) with per-character foreground/background colours.

use resvg::{tiny_skia, usvg};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use super::Agent;

/// Terminal columns occupied by a table icon (also the pixel width).
pub const ICON_CHAR_W: u16 = 5;
/// Terminal rows occupied by a table icon. Each row encodes 2 pixel rows.
pub const ICON_CHAR_H: u16 = 3;

/// Terminal columns used by the preview icon in the `init` agent selector.
pub const PREVIEW_CHAR_W: u16 = 10;
/// Terminal rows used by the preview icon in the `init` agent selector.
pub const PREVIEW_CHAR_H: u16 = 8;

/// Rasterise an SVG to a flat RGBA byte buffer at the requested pixel dimensions.
fn svg_to_pixels(svg_bytes: &[u8], px_w: u32, px_h: u32) -> Vec<u8> {
    let opt = usvg::Options::default();
    let tree = match usvg::Tree::from_data(svg_bytes, &opt) {
        Ok(t) => t,
        Err(_) => return vec![0u8; (px_w * px_h * 4) as usize],
    };
    let mut pixmap = tiny_skia::Pixmap::new(px_w, px_h)
        .expect("icon dimensions must be non-zero");
    let size = tree.size();
    let transform = tiny_skia::Transform::from_scale(
        px_w as f32 / size.width(),
        px_h as f32 / size.height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap.take()
}

/// Convert raw RGBA pixels to ratatui half-block `Line`s tinted with the given RGB colour.
///
/// Each terminal row consumes two pixel rows: the upper pixel maps to the foreground
/// colour of `'▀'` and the lower pixel maps to its background colour.
fn pixels_to_lines(rgba: &[u8], px_w: u32, px_h: u32, tint: (u8, u8, u8)) -> Vec<Line<'static>> {
    let (tr, tg, tb) = tint;
    let mut lines = Vec::with_capacity((px_h / 2) as usize);
    let mut y = 0u32;
    while y + 1 < px_h {
        let mut spans = Vec::with_capacity(px_w as usize);
        for x in 0..px_w {
            let top = &rgba[((y * px_w + x) * 4) as usize..][..4];
            let bot = &rgba[(((y + 1) * px_w + x) * 4) as usize..][..4];

            let mk_color = |pixel: &[u8]| -> Option<Color> {
                let a = pixel[3];
                if a == 0 { return None; }
                Some(Color::Rgb(
                    (tr as u16 * a as u16 / 255) as u8,
                    (tg as u16 * a as u16 / 255) as u8,
                    (tb as u16 * a as u16 / 255) as u8,
                ))
            };

            let top_col = mk_color(top);
            let bot_col = mk_color(bot);

            let (ch, style) = match (top_col, bot_col) {
                (None, None) => (" ", Style::default()),
                (Some(fg), None) => ("▀", Style::default().fg(fg).bg(Color::Reset)),
                (None, Some(bg)) => ("▄", Style::default().fg(bg).bg(Color::Reset)),
                (Some(fg), Some(bg)) => ("▀", Style::default().fg(fg).bg(bg)),
            };
            spans.push(Span::styled(ch, style));
        }
        lines.push(Line::from(spans));
        y += 2;
    }
    lines
}

/// A set of ratatui `Line`s that render an agent's brand icon using half-block characters.
#[derive(Clone)]
pub struct AgentIcon {
    /// Half-block rows; length equals the number of terminal rows requested at creation.
    pub lines: Vec<Line<'static>>,
}

impl AgentIcon {
    fn from_svg(svg_bytes: &[u8], tint: (u8, u8, u8), char_w: u16, char_h: u16) -> Self {
        let px_h = char_h as u32 * 2;
        let rgba = svg_to_pixels(svg_bytes, char_w as u32, px_h);
        let lines = pixels_to_lines(&rgba, char_w as u32, px_h, tint);
        Self { lines }
    }
}

/// Returns the compact table icon for an agent (fits in a single table row of height [`ICON_CHAR_H`]).
pub fn table_icon(agent: &Agent) -> AgentIcon {
    let (svg, tint) = svg_and_tint(agent);
    AgentIcon::from_svg(svg, tint, ICON_CHAR_W, ICON_CHAR_H)
}

/// Returns the larger preview icon for an agent (for the `init` selector side panel).
pub fn preview_icon(agent: &Agent) -> AgentIcon {
    let (svg, tint) = svg_and_tint(agent);
    AgentIcon::from_svg(svg, tint, PREVIEW_CHAR_W, PREVIEW_CHAR_H)
}

/// Maps each agent variant to its SVG bytes and brand tint colour.
fn svg_and_tint(agent: &Agent) -> (&'static [u8], (u8, u8, u8)) {
    match agent {
        Agent::ClaudeCode => (
            include_bytes!("../../../assets/icons/anthropic.svg"),
            (212, 168, 75),
        ),
        Agent::OpenCode => (
            include_bytes!("../../../assets/icons/opencode.svg"),
            (200, 100, 255),
        ),
        Agent::Codex => (
            include_bytes!("../../../assets/icons/codex.svg"),
            (100, 210, 210),
        ),
        Agent::Gemini => (
            include_bytes!("../../../assets/icons/googlegemini.svg"),
            (100, 150, 255),
        ),
        Agent::Droid => (
            include_bytes!("../../../assets/icons/droid.svg"),
            (50, 200, 50),
        ),
    }
}

/// Returns pre-rendered table icons for all five agent variants in enum declaration order.
///
/// Index mapping: 0 = ClaudeCode, 1 = OpenCode, 2 = Codex, 3 = Gemini, 4 = Droid.
pub fn all_table_icons() -> [AgentIcon; 5] {
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
        Agent::OpenCode => 1,
        Agent::Codex => 2,
        Agent::Gemini => 3,
        Agent::Droid => 4,
    }
}
