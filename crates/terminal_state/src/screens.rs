//! `screens` — the **framing states**: the doorway in and the doorway out.
//!
//! Four full-screen, non-3D panes, all built on the same shape as the shipped
//! `Diving` state (`flow.rs`'s `enter_diving`/`tick_diving`/`paint_dive`
//! triple — a state clock, a nav-driven exit, and one painter):
//!
//! | State | What it frames | Reached from |
//! |---|---|---|
//! | [`Intro`](crate::flow::GameState::Intro) | the Stewardship cold boot before L1 | `Menu → START`, **fresh campaign only** |
//! | [`MemoryView`](crate::flow::GameState::MemoryView) | the journal: everything recovered so far | `Menu → MEMORY`, `Pause → MEMORY` |
//! | [`Ending`](crate::flow::GameState::Ending) | the two distinct end-cards | winning L12 (instead of the generic `Won`) |
//! | [`Epilogue`](crate::flow::GameState::Epilogue) | the SHARED closing furniture | either end-card |
//!
//! ## What lives here vs. in `flow`
//!
//! This module owns **content composition** and **painting**; `flow` owns the
//! state machine, the clocks, and the navigation. Every function below is pure
//! (`Framebuffer` in, `Framebuffer` out; or data in, `Vec<PaneLine>` out), so the
//! layout, the scroll clamping, the ending selection, and the corpus placement are
//! all unit-testable with no `App`, no renderer, and no terminal.
//!
//! ## Both endings are mechanically equal
//!
//! The v6.1 lock says the two finales are *permanently ambiguous*. That is
//! enforced structurally here, not by tone alone:
//!
//! * [`ending_lines`](crate::screens::ending_lines) is the only function that
//!   takes an [`Ending`](crate::flow::Ending), and it produces only the
//!   END-CARD;
//! * [`crate::story::stewardship::epilogue_lines`] takes no ending argument at all, so
//!   the epilogue furniture — the A-7 re-read offer, Form 44, the Incident Report,
//!   the Sealed-Door notice, and the single credits line — **cannot** differ
//!   between the paths;
//! * both cards route to the same [`Epilogue`](crate::flow::GameState::Epilogue),
//!   and both leave the campaign marked complete (the `OnEnter(Won)` progress write
//!   runs on either path before the routing happens).
//!
//! The `while(true)` card additionally OFFERS the loop (`session 000N — begin
//! again?`). It is an offer, never an imposition: `Back`/`Esc` on either card goes
//! straight to the Menu.

use bevy::prelude::*;
use re_core::Framebuffer;
use re_present::PresentSet;

use crate::flow::{
    Ending, EndingOutcome, EndingPhase, EndingTimer, GameState, MenuCursor, PaneScroll,
};
use crate::menu::{
    clear_bg, put_block, put_center, put_str, BG, BG_HILITE, FG, FG_DIM, FG_HILITE, FG_MEMORY,
    FG_MEMORY_CORRUPT, FG_SHARD,
};
use crate::save::Progress;
use crate::settings::Settings;
use crate::story::interp::OperatorProfile;
use crate::story::stewardship;
use crate::story::{MemoryLog, MemoryReadout, NameAssembly};

/// Registers the four framing-state painters.
///
/// Scheduling mirrors the shipped split exactly:
///
/// * `Intro`, `Ending` and `Epilogue` paint in `Update` like `draw_menu` /
///   `draw_won` do — no [`re_world::CameraPose`] exists in those states (the level
///   is torn down on the way into every one of them), so the raycaster
///   early-returns and cannot overwrite the pane.
/// * `MemoryView` paints in `Last`, `.before(PresentSet)`, like `draw_pause` —
///   because it is reachable FROM `Pause`, where the live level is deliberately
///   kept alive behind the overlay and the raycaster does still run.
pub struct ScreenRenderPlugin;

impl Plugin for ScreenRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw_intro.run_if(in_state(GameState::Intro)))
            .add_systems(Update, draw_ending.run_if(in_state(GameState::Ending)))
            .add_systems(Update, draw_epilogue.run_if(in_state(GameState::Epilogue)))
            .add_systems(
                Last,
                draw_memory_view
                    .run_if(in_state(GameState::MemoryView))
                    .before(PresentSet),
            );
    }
}

// ═══════════════════════════ the pane primitives ════════════════════════════

/// How one composed line is coloured.
///
/// Kept as an enum (not a raw `u32`) so the content functions stay free of
/// palette knowledge and the tests can assert on *meaning* — "this row is a
/// recovered memory" — rather than on a hex value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Ordinary body text (system cyan).
    Text,
    /// Secondary / chrome text.
    Dim,
    /// The line the screen is *about* (the `exit 0` receipt, a heading).
    Accent,
    /// A recovered memory readout — the amber the Won overlay already uses.
    Memory,
    /// The corrupted `pool_alt` collapse — the red the Won overlay already uses.
    Corrupt,
    /// The name-shard / `pid://…` accounting.
    Shard,
}

impl Tone {
    /// The foreground colour this tone paints in.
    const fn fg(self) -> u32 {
        match self {
            Self::Text => FG,
            Self::Dim => FG_DIM,
            Self::Accent => 0x0040_FFB0,
            Self::Memory => FG_MEMORY,
            Self::Corrupt => FG_MEMORY_CORRUPT,
            Self::Shard => FG_SHARD,
        }
    }
}

/// One composed body row: its text and how it reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaneLine {
    /// The rendered text of this row.
    pub text: String,
    /// How this row is coloured.
    pub tone: Tone,
}

impl PaneLine {
    /// A body row in `tone`.
    #[must_use]
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
        }
    }
    /// A plain body row.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::new(text, Tone::Text)
    }
    /// A dim body row.
    #[must_use]
    pub fn dim(text: impl Into<String>) -> Self {
        Self::new(text, Tone::Dim)
    }
    /// An empty spacer row.
    #[must_use]
    pub fn blank() -> Self {
        Self::new("", Tone::Dim)
    }
}

/// What sits at the bottom of a pane.
pub enum Footer<'a> {
    /// A single dim hint line (the non-interactive panes).
    Hint(&'a str),
    /// A selectable row of items plus a hint line — driven by `←/→` so `↑/↓` stay
    /// free for scrolling the body (the same hand-rolled `MenuNav` read the
    /// settings screen already uses).
    Items {
        /// The selectable row's item labels, in display order.
        labels: &'a [String],
        /// The currently-highlighted item's index into `labels`.
        selected: usize,
        /// The dim hint line drawn under the item row.
        hint: &'a str,
    },
}

/// How many body rows a pane of `rows` height can show, given its footer.
///
/// Layout, top to bottom: a blank row, the title, a blank row, the body, the
/// scroll indicator, (the item row), the hint row.
#[must_use]
pub const fn pane_visible_rows(rows: usize, has_items: bool) -> usize {
    let chrome = if has_items { 3 + 3 } else { 3 + 2 };
    rows.saturating_sub(chrome)
}

/// Clamp a scroll offset so it can never scroll past the end (or below zero).
/// A body that fits entirely always clamps to `0`.
#[must_use]
pub fn clamp_scroll(scroll: usize, total: usize, visible: usize) -> usize {
    scroll.min(total.saturating_sub(visible))
}

/// Paint a full-screen scrollable text pane.
///
/// The body block is LEFT-ALIGNED at one shared margin derived from the widest
/// body line, so multi-line documents keep their authored internal alignment (a
/// Stewardship form's right-hand stamps stay right-hand). Rows past the framebuffer
/// are skipped and columns are clipped by `put_str`, so any grid size is safe.
pub fn paint_pane(
    fb: &mut Framebuffer,
    title: &str,
    lines: &[PaneLine],
    scroll: usize,
    footer: &Footer,
) {
    clear_bg(fb);
    let rows = fb.rows();
    let cols = fb.cols();
    if rows == 0 || cols == 0 {
        return;
    }
    put_center(fb, 1.min(rows - 1), title, Tone::Accent.fg(), BG);

    let has_items = matches!(footer, Footer::Items { .. });
    let visible = pane_visible_rows(rows, has_items);
    let scroll = clamp_scroll(scroll, lines.len(), visible);

    let width = lines
        .iter()
        .map(|l| l.text.chars().count())
        .max()
        .unwrap_or(0);
    let margin = cols.saturating_sub(width) / 2;

    let body_top = 3;
    for (i, line) in lines.iter().skip(scroll).take(visible).enumerate() {
        let row = body_top + i;
        if row >= rows {
            break;
        }
        put_str(fb, margin, row, &line.text, line.tone.fg(), BG);
    }

    // Scroll indicator — only when the body actually overflows.
    if lines.len() > visible && rows >= 3 {
        let shown_to = (scroll + visible).min(lines.len());
        let row = rows.saturating_sub(if has_items { 3 } else { 2 });
        put_center(
            fb,
            row,
            &format!(
                "[ {}–{} / {} · ↑/↓ scroll ]",
                scroll + 1,
                shown_to,
                lines.len()
            ),
            FG_DIM,
            BG,
        );
    }

    match footer {
        Footer::Hint(hint) => {
            put_center(fb, rows.saturating_sub(1), hint, FG_DIM, BG);
        }
        Footer::Items {
            labels,
            selected,
            hint,
        } => {
            paint_item_row(fb, rows.saturating_sub(2), labels, *selected);
            put_center(fb, rows.saturating_sub(1), hint, FG_DIM, BG);
        }
    }
}

/// Paint a horizontal row of selectable items, centred as one strip, with the
/// selected one on the shipped teal selection bar.
fn paint_item_row(fb: &mut Framebuffer, row: usize, labels: &[String], selected: usize) {
    if labels.is_empty() || row >= fb.rows() {
        return;
    }
    let cells: Vec<String> = labels.iter().map(|l| format!("  {l}  ")).collect();
    let total: usize =
        cells.iter().map(|c| c.chars().count()).sum::<usize>() + 3 * (cells.len() - 1);
    let mut col = fb.cols().saturating_sub(total) / 2;
    for (i, cell) in cells.iter().enumerate() {
        let (fg, bg) = if i == selected {
            (FG_HILITE, BG_HILITE)
        } else {
            (FG, BG)
        };
        put_str(fb, col, row, cell, fg, bg);
        col += cell.chars().count();
        if i + 1 < cells.len() {
            put_str(fb, col, row, "   ", FG_DIM, BG);
            col += 3;
        }
    }
}

// ══════════════════════════════ 4.1 · Intro ═════════════════════════════════

/// The Intro pane's title.
const INTRO_TITLE: &str = "— STEWARDSHIP-CERTIFIED LEGACY SYSTEM —";

/// The composed cold-boot body: the Stewardship letterhead, the `[boot.07]`
/// handshake, and the Form A-7 addendum
/// ([`crate::story::stewardship::intro_lines`]). `>`-prefixed rows read as
/// `system_log`, the rest as the `stewardship-form` register.
#[must_use]
pub fn intro_lines(profile: &OperatorProfile, session_label: &str) -> Vec<PaneLine> {
    stewardship::intro_lines(profile, session_label)
        .into_iter()
        .map(|l| {
            let tone = if l.trim_start().starts_with('>') {
                Tone::Text
            } else {
                Tone::Dim
            };
            PaneLine::new(l, tone)
        })
        .collect()
}

/// Paint the Intro. Pure — the caller supplies the already-composed body.
pub fn paint_intro(fb: &mut Framebuffer, lines: &[PaneLine], scroll: usize) {
    paint_pane(
        fb,
        INTRO_TITLE,
        lines,
        scroll,
        &Footer::Hint("↑/↓ read  ·  any key — begin  ·  session held open"),
    );
}

/// `Update` (Intro only). Re-composes the body each frame — the whole pass is a
/// few string splices over a `OnceLock`-free constant corpus, and the state lives
/// for a couple of seconds; keeping it stateless is worth more than the cycles.
fn draw_intro(
    mut fb: ResMut<Framebuffer>,
    scroll: Res<PaneScroll>,
    progress: Res<Progress>,
    settings: Res<Settings>,
) {
    let profile = OperatorProfile::for_fourth_wall(settings.lore_fourth_wall);
    let lines = intro_lines(&profile, &progress.session_label());
    paint_intro(&mut fb, &lines, scroll.0);
}

// ═══════════════════════════ 4.2 · MemoryView ═══════════════════════════════

/// The journal pane's title.
const MEMORY_TITLE: &str = "M E M O R Y";

/// The rule the journal draws under the vignettes before the name accounting
/// (`WORD_STORY` §1.4's own layout).
const JOURNAL_RULE: &str = "──────────────────────────────────────────────";

/// What the journal says before anything has been recovered.
const JOURNAL_EMPTY: &str = "(nothing recovered yet — solve a room and it will be written here)";

/// The journal body, in the `WORD_STORY` §1.4 layout: the accumulated
/// vignettes, a rule, the running `true_name:` assembly, the shard count, and the
/// session counter.
///
/// **This is [`MemoryLog::story`]'s consumer** — the joined `id  PHRASE` form is
/// exactly the journal body, so the two can never drift.
#[must_use]
pub fn memory_view_lines(
    log: &MemoryLog,
    name: &NameAssembly,
    session_label: &str,
) -> Vec<PaneLine> {
    let mut out = Vec::new();
    if log.is_empty() {
        out.push(PaneLine::dim(JOURNAL_EMPTY));
    } else {
        for line in log.story().split('\n') {
            out.push(PaneLine::new(line, Tone::Memory));
        }
    }
    out.push(PaneLine::blank());
    out.push(PaneLine::dim(JOURNAL_RULE));
    out.push(PaneLine::new(name.true_name_line(), Tone::Shard));
    out.push(PaneLine::new(
        format!(
            "name-shards recovered: {} of {}",
            name.count(),
            crate::story::NAME_SHARDS.len()
        ),
        Tone::Shard,
    ));
    out.push(PaneLine::dim(format!("session {session_label}")));
    out
}

/// Paint the journal.
pub fn paint_memory_view(fb: &mut Framebuffer, lines: &[PaneLine], scroll: usize) {
    paint_pane(
        fb,
        MEMORY_TITLE,
        lines,
        scroll,
        &Footer::Hint("↑/↓ scroll  ·  Esc/Enter — back"),
    );
}

/// `Last` (MemoryView only): paint the journal over whatever is behind it (an
/// empty menu field, or the frozen scene when opened from `Pause`).
fn draw_memory_view(
    mut fb: ResMut<Framebuffer>,
    scroll: Res<PaneScroll>,
    log: Res<MemoryLog>,
    progress: Res<Progress>,
) {
    let name = NameAssembly::from_solved_ids(progress.solved.iter().map(String::as_str));
    let lines = memory_view_lines(&log, &name, &progress.session_label());
    paint_memory_view(&mut fb, &lines, scroll.0);
}

// ═════════════════════════════ 4.3 · Ending ═════════════════════════════════

/// The `exit 0` receipt, verbatim (`WORD_STORY` §4 / `CONTENT_PACK` L12 /
/// `PUZZLE_PROGRESSION` §5).
const EXIT0_COMMAND: &str = "$ exit 0";
const EXIT0_RECEIPT: &str = "Process finished with exit code 0.";

/// The `while(true)` collapse, verbatim (`STORY_BIBLE` §6 / `WORD_STORY` §4).
const LOOP_BODY: &str = "while(true) { remember(); forget(); }";

/// The end-card title for each path — distinct, and neither one a verdict.
#[must_use]
pub const fn ending_title(ending: Ending) -> &'static str {
    match ending {
        Ending::Exit0 => "— T H E   D O O R   O P E N S   O U T W A R D —",
        Ending::WhileTrue => "— T H E   L O O P   S T A N D S —",
    }
}

/// The eye frame each end-card shows, chosen from the shipped 16-frame splash
/// asset — no art is cropped or synthesised:
///
/// * `exit 0` → the **`awake`** frame: the eye is fully open and the `@@@` pupil
///   is not in it. The eye opens without a pupil — it is not looking for you
///   any more.
/// * `while(true)` → the **`-- shut --`** frame: the eye that never opens.
///
/// Falls back to the last / first frame if the asset is ever re-authored with
/// different captions, and to `None` if it holds no frames at all.
#[must_use]
pub fn ending_eye(ending: Ending) -> Option<&'static crate::menu::EyeFrame> {
    let frames = crate::menu::eye_frames();
    let caption = match ending {
        Ending::Exit0 => "awake",
        Ending::WhileTrue => "shut",
    };
    frames
        .iter()
        .find(|f| f.lines.iter().any(|l| l.contains(caption)))
        .or_else(|| match ending {
            Ending::Exit0 => frames.last(),
            Ending::WhileTrue => frames.first(),
        })
}

/// Paint the end-card's eye phase: the chosen frame, block-centred, with one
/// caption row.
///
/// Two phases (eye, then readout) rather than one scroll, because the eye is
/// 22 rows tall and the readout would otherwise sit below the fold — the same
/// reason the splash sequences its own two assets.
pub fn paint_ending_eye(fb: &mut Framebuffer, ending: Ending) {
    clear_bg(fb);
    let rows = fb.rows();
    if let Some(frame) = ending_eye(ending) {
        let top = (rows + 1).saturating_sub(frame.lines.len()) / 2;
        let fg = match ending {
            Ending::Exit0 => FG,
            Ending::WhileTrue => FG_MEMORY_CORRUPT,
        };
        put_block(fb, top.saturating_sub(1), &frame.lines, fg, BG);
    }
    put_center(fb, rows.saturating_sub(1), ending_title(ending), FG_DIM, BG);
}

/// The end-card READOUT body for `ending`.
///
/// * `exit 0` — the clean primary-pool memory log in full, the completed
///   `true_name:` line, then `$ exit 0` / `Process finished with exit code 0.`
///   Quiet: no exhortation, no verdict.
/// * `while(true)` — the same log kept (the museum of wounds, in the corrupt
///   tone), the level's `pool_alt` Babel collapse as the final readout, the name
///   rendered UNRESOLVED, the loop body, the self-incrementing `attempts`, and the
///   repeating `resolving session 0001` lines.
///
/// `final_readout` is the just-solved level's assembled phrase
/// ([`MemoryReadout`]); on the loop path it is already the `pool_alt` form,
/// because `assemble_readout` renders a held cycle corrupt.
#[must_use]
pub fn ending_lines(
    ending: Ending,
    log: &MemoryLog,
    name: &NameAssembly,
    final_readout: &MemoryReadout,
    next_session_label: &str,
) -> Vec<PaneLine> {
    let mut out = Vec::new();
    match ending {
        Ending::Exit0 => {
            out.push(PaneLine::dim("[ memory readout ]"));
            for line in log.story().split('\n').filter(|l| !l.is_empty()) {
                out.push(PaneLine::new(line, Tone::Memory));
            }
            out.push(PaneLine::dim(JOURNAL_RULE));
            out.push(PaneLine::new(name.true_name_line(), Tone::Shard));
            out.push(PaneLine::blank());
            out.push(PaneLine::text(EXIT0_COMMAND));
            out.push(PaneLine::new(EXIT0_RECEIPT, Tone::Accent));
        }
        Ending::WhileTrue => {
            out.push(PaneLine::dim("[ memory readout — collapsed ]"));
            for line in log.story().split('\n').filter(|l| !l.is_empty()) {
                out.push(PaneLine::new(line, Tone::Corrupt));
            }
            if final_readout.has_phrase() {
                out.push(PaneLine::new(&final_readout.phrase, Tone::Corrupt));
            }
            out.push(PaneLine::dim(JOURNAL_RULE));
            // The name loses the one language it was called right in: the DEFAULT
            // assembly renders exactly WORD_STORY §4's `░·░·…  [unresolved]` form,
            // so the collapse is the same code path as an unassembled name.
            out.push(PaneLine::new(
                NameAssembly::default().true_name_line(),
                Tone::Corrupt,
            ));
            out.push(PaneLine::blank());
            out.push(PaneLine::new(LOOP_BODY, Tone::Accent));
            out.push(PaneLine::new(
                format!("> attempts: {next_session_label} ..."),
                Tone::Corrupt,
            ));
            // The notes repeat. That is the ending, not a rendering artefact.
            for _ in 0..3 {
                out.push(PaneLine::new("> resolving session 0001 ...", Tone::Corrupt));
            }
        }
    }
    out
}

/// The end-card's footer items. `exit 0` offers only the way onward; the loop card
/// ALSO offers the loop — an offer, never an imposition.
#[must_use]
pub fn ending_item_labels(ending: Ending, next_session_label: &str) -> Vec<String> {
    let mut v = vec!["CONTINUE".to_string()];
    if ending == Ending::WhileTrue {
        v.push(format!("SESSION {next_session_label} — BEGIN AGAIN?"));
    }
    v
}

/// Paint the end-card's readout phase.
pub fn paint_ending_readout(
    fb: &mut Framebuffer,
    ending: Ending,
    lines: &[PaneLine],
    labels: &[String],
    selected: usize,
    scroll: usize,
) {
    let hint = if labels.len() > 1 {
        "←/→ choose  ·  Enter confirm  ·  ↑/↓ scroll  ·  Esc — menu"
    } else {
        "Enter continue  ·  ↑/↓ scroll  ·  Esc — menu"
    };
    paint_pane(
        fb,
        ending_title(ending),
        lines,
        scroll,
        &Footer::Items {
            labels,
            selected,
            hint,
        },
    );
}

/// The end-card's own view state, bundled so the painter stays inside Bevy's
/// system-argument budget (the same pattern `menu`'s `HudNameCtx` uses).
#[derive(bevy::ecs::system::SystemParam)]
struct EndingView<'w> {
    outcome: Res<'w, EndingOutcome>,
    timer: Res<'w, EndingTimer>,
    cursor: Res<'w, MenuCursor>,
    scroll: Res<'w, PaneScroll>,
}

/// `Update` (Ending only): paint whichever phase the state clock is in.
fn draw_ending(
    mut fb: ResMut<Framebuffer>,
    view: EndingView,
    log: Res<MemoryLog>,
    readout: Res<MemoryReadout>,
    progress: Res<Progress>,
) {
    let EndingView {
        outcome,
        timer,
        cursor,
        scroll,
    } = &view;
    // Defensive: the state is only ever entered with an outcome captured, but a
    // direct `NextState` jump (a focused test) must not panic — show the release
    // card rather than nothing.
    let ending = outcome.0.unwrap_or(Ending::Exit0);
    if timer.phase() == EndingPhase::Eye {
        paint_ending_eye(&mut fb, ending);
        return;
    }
    let name = NameAssembly::from_solved_ids(progress.solved.iter().map(String::as_str));
    let next_session = progress.next_session_label();
    let lines = ending_lines(ending, &log, &name, &readout, &next_session);
    let labels = ending_item_labels(ending, &next_session);
    let selected = cursor.index.min(labels.len().saturating_sub(1));
    paint_ending_readout(&mut fb, ending, &lines, &labels, selected, scroll.0);
}

// ════════════════════════════ 4.4 · Epilogue ════════════════════════════════

/// The epilogue's title — the same on both paths, like everything else on it.
const EPILOGUE_TITLE: &str = "— A F T E R —";

/// The shared epilogue body. Takes NO ending argument, by construction (see the
/// module docs): the furniture cannot be path-exclusive.
#[must_use]
pub fn epilogue_lines(profile: &OperatorProfile) -> Vec<PaneLine> {
    stewardship::epilogue_lines(profile)
        .into_iter()
        .map(|l| {
            let tone = if l == stewardship::CREDITS_LINE {
                Tone::Accent
            } else if l.trim_start().starts_with('>') {
                Tone::Text
            } else {
                Tone::Dim
            };
            PaneLine::new(l, tone)
        })
        .collect()
}

/// Paint the epilogue.
pub fn paint_epilogue(fb: &mut Framebuffer, lines: &[PaneLine], scroll: usize) {
    paint_pane(
        fb,
        EPILOGUE_TITLE,
        lines,
        scroll,
        &Footer::Hint("↑/↓ scroll  ·  Enter / Esc — menu"),
    );
}

/// `Update` (Epilogue only).
fn draw_epilogue(mut fb: ResMut<Framebuffer>, scroll: Res<PaneScroll>, settings: Res<Settings>) {
    let profile = OperatorProfile::for_fourth_wall(settings.lore_fourth_wall);
    let lines = epilogue_lines(&profile);
    paint_epilogue(&mut fb, &lines, scroll.0);
}

#[cfg(test)]
mod tests;
