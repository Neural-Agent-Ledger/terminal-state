//! `guide` — the in-game GUIDE (Swaggerfall study report item 1).
//!
//! Reachable from the main [`crate::flow::GameState::Menu`] AND the in-play
//! [`crate::flow::GameState::Pause`] overlay, so a player can look the rules
//! up mid-run without losing progress. Five chapters, a left-hand chapter
//! list + right-hand body.
//!
//! Every sentence is grounded in the REAL mechanics and the REAL key bindings:
//! * `crates/re_puzzle/src/lib.rs`'s module doc + `PuzzleNode`/`Pipe`/
//!   `network_forward` doc comments (nodes, pipes, switches, the Kahn forward
//!   pass, cycle refusal, the win condition, the power budget).
//! * `crates/re_present/src/terminal.rs`'s `key_intent` (the real WASD/arrow/
//!   Space/E/Shift/M/`[`/`]`/Z/G/Esc bindings) — nothing here is invented.
//! * `crate::flow::faced_switch` (the same faced-cell probe the crosshair
//!   itself uses) for the "brightens when you can act" claim.
//!
//! Content composition + painting live here (this module); the state machine
//! (the `Guide` state itself, `GuideReturn`, the nav system) lives in
//! `crate::flow`, mirroring the split `crate::screens`'s module doc documents
//! for the other framing panes.
//!
//! All copy is in [`crate::strings`], English only for now (owner decision,
//! `strings.rs`'s module doc).

use bevy::prelude::*;
use re_core::Framebuffer;
use re_present::PresentSet;

use crate::flow::{GameState, MenuCursor, PaneScroll};
use crate::menu::{clear_bg, put_center, put_str, BG, BG_HILITE, FG_DIM, FG_HILITE};
use crate::screens::clamp_scroll;
use crate::strings;

/// One guide chapter, in the order the left-hand list shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuideChapter {
    /// What the player is trying to achieve.
    Goal,
    /// The real key bindings, grounded in `re_present`'s input mapping.
    Controls,
    /// What nodes, pipes and switches are and how they interact.
    NodesAndSwitches,
    /// How the forward pass evaluates the network and what wins the level.
    ForwardPass,
    /// What each rendered material/surface means.
    Materials,
}

/// Every chapter, in display order. `crate::flow::enter_guide` sizes the
/// chapter cursor to this; `draw_guide` indexes into it with the cursor.
pub const GUIDE_CHAPTERS: [GuideChapter; 5] = [
    GuideChapter::Goal,
    GuideChapter::Controls,
    GuideChapter::NodesAndSwitches,
    GuideChapter::ForwardPass,
    GuideChapter::Materials,
];

impl GuideChapter {
    /// The left-hand list label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Goal => strings::GUIDE_CHAPTER_GOAL,
            Self::Controls => strings::GUIDE_CHAPTER_CONTROLS,
            Self::NodesAndSwitches => strings::GUIDE_CHAPTER_NODES_SWITCHES,
            Self::ForwardPass => strings::GUIDE_CHAPTER_FORWARD_PASS,
            Self::Materials => strings::GUIDE_CHAPTER_MATERIALS,
        }
    }

    /// The chapter's body, one entry per rendered line (a blank entry is a
    /// spacer row — plain terse lab-log prose, no markup).
    #[must_use]
    pub const fn body(self) -> &'static [&'static str] {
        match self {
            Self::Goal => strings::GUIDE_GOAL_BODY,
            Self::Controls => strings::GUIDE_CONTROLS_BODY,
            Self::NodesAndSwitches => strings::GUIDE_NODES_SWITCHES_BODY,
            Self::ForwardPass => strings::GUIDE_FORWARD_PASS_BODY,
            Self::Materials => strings::GUIDE_MATERIALS_BODY,
        }
    }
}

/// Left column width for the chapter list — Swaggerfall's own layout
/// (`24_guide.png`): a chapter list on the left, the body on the right.
const GUIDE_LIST_W: usize = 20;
/// The row both columns start painting at (below the title + a blank row).
const GUIDE_TOP: usize = 3;

/// How many body rows are visible given `rows` (title + blank + footer chrome).
#[must_use]
pub const fn guide_visible_rows(rows: usize) -> usize {
    rows.saturating_sub(GUIDE_TOP + 2)
}

/// Pure layout: produce exactly the lines [`paint_guide`] draws.
///
/// Given the framebuffer size, the chosen chapter and a scroll offset, split
/// out so the scroll-clamping and pagination math is unit-testable with no
/// `Framebuffer`.
#[must_use]
pub fn guide_body_window(rows: usize, chapter: GuideChapter, scroll: usize) -> (usize, usize) {
    let body = chapter.body();
    let visible = guide_visible_rows(rows);
    let scroll = clamp_scroll(scroll, body.len(), visible);
    let shown_to = (scroll + visible).min(body.len());
    (scroll, shown_to)
}

/// Paint the guide: the chapter list (left), the selected chapter's body
/// (right, scrollable), and the footer hint.
pub fn paint_guide(fb: &mut Framebuffer, chapter: GuideChapter, scroll: usize) {
    clear_bg(fb);
    let rows = fb.rows();
    let cols = fb.cols();
    if rows == 0 || cols == 0 {
        return;
    }
    put_center(fb, 1.min(rows - 1), strings::GUIDE_TITLE, FG_DIM, BG);

    // Chapter list, left column.
    for (i, &ch) in GUIDE_CHAPTERS.iter().enumerate() {
        let row = GUIDE_TOP + i;
        if row >= rows {
            break;
        }
        let selected = ch == chapter;
        let (fg, bg) = if selected {
            (FG_HILITE, BG_HILITE)
        } else {
            (FG_DIM, BG)
        };
        let text = if selected {
            format!("> {:<w$}", ch.label(), w = GUIDE_LIST_W.saturating_sub(2))
        } else {
            format!("  {:<w$}", ch.label(), w = GUIDE_LIST_W.saturating_sub(2))
        };
        put_str(fb, 1, row, &text, fg, bg);
    }

    // Body, right column — scrollable.
    let body = chapter.body();
    let (scroll, shown_to) = guide_body_window(rows, chapter, scroll);
    let visible = guide_visible_rows(rows);
    let body_col = (GUIDE_LIST_W + 2).min(cols.saturating_sub(1));
    for (i, line) in body.iter().skip(scroll).take(visible).enumerate() {
        let row = GUIDE_TOP + i;
        if row >= rows {
            break;
        }
        put_str(fb, body_col, row, line, FG_DIM, BG);
    }

    if body.len() > visible && rows >= 2 {
        put_center(
            fb,
            rows.saturating_sub(2),
            &format!("[ {}-{} / {} ]", scroll + 1, shown_to, body.len()),
            FG_DIM,
            BG,
        );
    }
    put_center(
        fb,
        rows.saturating_sub(1),
        strings::GUIDE_FOOTER_HINT,
        FG_DIM,
        BG,
    );
}

/// `Last` (Guide only), `.before(PresentSet)`: like `draw_pause`/
/// `draw_memory_view`, this must run AFTER the raycaster/glitch produced the
/// frame — the guide is reachable from `Pause`, where the live level is kept
/// alive (frozen) behind the overlay, exactly as the journal is.
fn draw_guide(mut fb: ResMut<Framebuffer>, cursor: Res<MenuCursor>, scroll: Res<PaneScroll>) {
    let idx = cursor.index.min(GUIDE_CHAPTERS.len() - 1);
    paint_guide(&mut fb, GUIDE_CHAPTERS[idx], scroll.0);
}

/// Registers the guide's painter.
///
/// Content composition + painting are presentation, so — mirroring
/// `crate::screens::ScreenRenderPlugin` — this is added from
/// `crate::menu::MenuRenderPlugin`, not from the binary.
pub struct GuideRenderPlugin;

impl Plugin for GuideRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Last,
            draw_guide
                .run_if(in_state(GameState::Guide))
                .before(PresentSet),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_chapter_has_a_label_and_a_nonempty_body() {
        for &ch in &GUIDE_CHAPTERS {
            assert!(!ch.label().is_empty());
            assert!(!ch.body().is_empty(), "{ch:?} has no body");
        }
    }

    #[test]
    fn body_window_never_scrolls_past_the_end() {
        for &ch in &GUIDE_CHAPTERS {
            let len = ch.body().len();
            let (scroll, shown_to) = guide_body_window(32, ch, usize::MAX);
            assert!(shown_to <= len);
            assert!(scroll <= len);
        }
    }

    #[test]
    fn body_window_is_full_when_it_fits() {
        // A tall grid can show every line of the shortest chapter at once.
        let (scroll, shown_to) = guide_body_window(200, GuideChapter::Controls, 0);
        assert_eq!(scroll, 0);
        assert_eq!(shown_to, GuideChapter::Controls.body().len());
    }

    #[test]
    fn paint_guide_does_not_panic_on_a_tiny_grid() {
        let mut fb = Framebuffer::new(1, 1);
        paint_guide(&mut fb, GuideChapter::Goal, 0);
        let mut fb0 = Framebuffer::new(0, 0);
        paint_guide(&mut fb0, GuideChapter::Goal, 0);
    }

    /// Text evidence (not just synthetic assertions) for the delegation's
    /// acceptance criterion: a headless render of the guide pane at the real
    /// 100x32 grid, printed row by row (`cargo test ... -- --nocapture`).
    #[test]
    fn guide_screen_text_dump_for_evidence() {
        let mut fb = Framebuffer::new(100, 32);
        paint_guide(&mut fb, GuideChapter::Controls, 0);
        println!("--- GUIDE screen (Controls chapter), 100x32 ---");
        for row in 0..fb.rows() {
            let line: String = (0..fb.cols())
                .map(|c| char::from_u32(fb.get(c, row).unwrap().glyph).unwrap_or(' '))
                .collect();
            println!("{:2} |{}|", row, line.trim_end());
        }
    }

    #[test]
    fn controls_chapter_matches_the_real_bindings() {
        // Grounding check: every control the guide claims must be a real
        // `key_intent` binding (`re_present::terminal::key_intent`), not an
        // invented one. This can't import the private mapping, so it asserts
        // the ones this module's own doc cites are all mentioned in the body.
        let body = GuideChapter::Controls.body().join("\n");
        for token in ["WASD", "SPACE", "E or ENTER", "SHIFT", "[ / ]"] {
            assert!(
                body.contains(token),
                "controls chapter must mention {token:?}"
            );
        }
    }
}
