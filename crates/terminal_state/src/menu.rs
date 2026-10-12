//! `menu` — text overlays painted directly into the
//! [`Framebuffer`](re_core::Framebuffer) for the non-playing states (splash,
//! menu, level-select, won).
//!
//! These run in `Update` (gated per state). In every non-playing state there is
//! no [`CameraPose`](re_world::CameraPose), so `re_render`'s raycaster
//! early-returns and leaves the framebuffer alone; we clear it and stamp ASCII
//! text, which `re_present` then blits to the real terminal in `Last`. That
//! makes these overlays show on the actual terminal backend — not just in
//! tests.
//!
//! The overlay for the [`GameState::Won`](crate::flow::GameState::Won) state is
//! drawn ON TOP of the last rendered 3D frame (we do NOT clear it) so the
//! solved scene stays visible behind the banner.

use bevy::prelude::*;
use re_core::{Cell, Framebuffer};

use crate::flow::{
    faced_switch, objective_progress, power_readout, sequence_progress, visible_categories,
    BrowseCategory, CurrentLevel, GameState, LevelCatalog, MenuCursor, PlayCamera, SettingsRow,
    SplashTimer, PAUSE_ITEMS, SETTINGS_ROWS, SPLASH_TIMEOUT, WON_ITEMS,
};
use crate::save::Progress;
use crate::settings::Settings;
use crate::story::beats::NarrativeBeat;
use crate::story::{shard_recovered_line, MemoryLog, MemoryReadout, NameAssembly, ReadoutForm};
use re_present::{ColorMode, MenuNav, PresentSet};
use re_puzzle::PuzzleState;
use re_world::CameraPose;

/// Adds the per-state overlay painters.
pub struct MenuRenderPlugin;

impl Plugin for MenuRenderPlugin {
    fn build(&self, app: &mut App) {
        // The F3 overlay's draw system + its `run_if` both read these re_core
        // resources, so init them defensively HERE (where the consumer lives).
        // `init_resource` is insert-if-absent, so build_core's copies still win
        // in the real app; headless tests that add only MenuRenderPlugin get the
        // re_core defaults (overlay hidden, zeroed metrics) — no missing-resource
        // panic when the `Last` schedule evaluates the overlay's run_if.
        app.init_resource::<re_core::PerfOverlay>()
            .init_resource::<re_core::FrameTimingMetrics>()
            .init_resource::<MenuOvergrowth>()
            .init_resource::<MenuBackdrop>();
        // The proximity-context label's per-level hysteresis latch.
        // Reset `OnEnter(Playing)` so a stale pipe index from
        // the PREVIOUS level's `PuzzleState` (indices are only stable for one
        // loaded level) is never carried into the next one — mirrors
        // `crate::first_steps`'s own `reset_watch_on_level_enter` pattern.
        app.init_resource::<re_runtime::context_line::ProximityTracker>()
            .add_systems(
                OnEnter(GameState::Playing),
                |mut tracker: ResMut<re_runtime::context_line::ProximityTracker>| {
                    *tracker = re_runtime::context_line::ProximityTracker::default();
                },
            );
        // The four framing states' painters. Registered from HERE (not from
        // the binary) so every consumer of `MenuRenderPlugin` gets the whole
        // overlay set: painting is presentation, and this plugin is where
        // presentation is added.
        app.add_plugins(crate::screens::ScreenRenderPlugin);
        // Mouse-driven menu navigation (hover + click), sharing this plugin's
        // painters' geometry — see `crate::menu_mouse`'s module doc for why it
        // lives here rather than being wired separately from the binary.
        app.add_plugins(crate::menu_mouse::MenuMousePlugin);
        // The in-game GUIDE — its own render plugin, registered here so every
        // `MenuRenderPlugin` consumer gets it, same reasoning as
        // `ScreenRenderPlugin` above.
        app.add_plugins(crate::guide::GuideRenderPlugin);
        // The FIRST STEPS onboarding checklist's observation/ticking systems.
        // Painting itself lives in `draw_play_hud` below, not in this plugin
        // — `FirstStepsPlugin` only owns the tick.
        app.add_plugins(crate::first_steps::FirstStepsPlugin);
        app.add_systems(Update, draw_splash.run_if(in_state(GameState::Splash)))
            .add_systems(Update, draw_menu.run_if(in_state(GameState::Menu)))
            .add_systems(
                Update,
                overgrowth_recoil_on_confirm.run_if(in_state(GameState::Menu)),
            )
            .add_systems(
                Update,
                draw_category_select.run_if(in_state(GameState::CategorySelect)),
            )
            .add_systems(
                Update,
                draw_level_select.run_if(in_state(GameState::LevelSelect)),
            )
            .add_systems(Update, draw_settings.run_if(in_state(GameState::Settings)))
            .add_systems(Update, draw_won.run_if(in_state(GameState::Won)))
            // The Pause overlay draws OVER the frozen 3D scene (like Won) — it must
            // run AFTER the raycaster/glitch produced the frame, so it lives in
            // `Last` (before present), not `Update`.
            .add_systems(
                Last,
                draw_pause
                    .run_if(in_state(GameState::Pause))
                    .before(PresentSet),
            )
            // The in-play HUD (objective line + crosshair) is painted OVER the live
            // 3D frame, so it ALSO runs in `Last` (after the PostUpdate raycast +
            // glitch, before the present blit) — painting it in `Update` would let
            // the raycaster overwrite it the same frame.
            //
            // `.after(entry_fx::tick_entry_fx_sys)`: BOTH systems write
            // `Framebuffer` in `Last`, so without an explicit order Bevy treats
            // them as ambiguous. Ordering the HUD after the entry transition
            // means the HUD paints ON TOP of it (visible throughout the
            // transition, not hidden then popping back in) — and since
            // `crate::entry_fx::EntryFxPlugin` is not yet added to the live app
            // (see that module's doc), this `.after(...)` is currently a no-op
            // ordering constraint against an unscheduled system, which Bevy
            // accepts without error.
            .add_systems(
                Last,
                draw_play_hud
                    .run_if(in_state(GameState::Playing))
                    .after(crate::entry_fx::tick_entry_fx_sys)
                    .before(PresentSet),
            )
            // The F3 performance overlay. Registered in the SAME `Last` schedule
            // as the other over-frame painters (after the PostUpdate composite,
            // before re_present's present), so it draws on top of the live/frozen
            // scene. Shown in BOTH Playing and Pause (so cost is inspectable while
            // paused) but never in the menu/splash states (no scene there), and
            // only when the player has toggled it on (`PerfOverlay.visible`).
            .add_systems(
                Last,
                draw_perf_overlay
                    .run_if(in_state(GameState::Playing).or_else(in_state(GameState::Pause)))
                    .run_if(|o: Res<re_core::PerfOverlay>| o.visible)
                    .before(PresentSet),
            );
    }
}

// ─────────────────────────── small text helpers ─────────────────────────────

// `pub(crate)` so the Wave-4 framing screens (`crate::screens`) paint with the
// SAME palette and the same stamping helpers as the shipped overlays — one look,
// one clipping/centring implementation, no second copy to drift.
pub(crate) const FG: u32 = 0x00C8_F0FF; // soft cyan-white
pub(crate) const FG_DIM: u32 = 0x0050_6878;
pub(crate) const FG_HILITE: u32 = 0x0000_0000;
pub(crate) const BG_HILITE: u32 = 0x0030_C0A0; // teal selection bar
pub(crate) const BG: u32 = 0x0000_0608;
/// A LOCKED campaign row: dimmer than `FG_DIM` but not invisible. The level's name is
/// deliberately readable — the campaign's shape is not a secret, only entry is earned.
const FG_LOCKED: u32 = 0x0028_3844;

/// Fill the whole framebuffer with the menu background (a near-black field).
/// Delegates to [`re_asciifx::paint::fill_bg`]; kept as a thin `pub(crate)`
/// wrapper so every existing `crate::menu::clear_bg(fb)` call site across the
/// game crate (`guide.rs`, `screens.rs`, `boot_console.rs`, …) compiles
/// unchanged.
pub(crate) fn clear_bg(fb: &mut Framebuffer) {
    re_asciifx::paint::fill_bg(fb, FG, BG);
}

/// Write `text` starting at `(col, row)`, clipped to the framebuffer, in `fg`
/// over `bg`. Delegates to [`re_asciifx::paint::put_str`]; kept as a thin
/// wrapper for the same reason as [`clear_bg`].
pub(crate) fn put_str(fb: &mut Framebuffer, col: usize, row: usize, text: &str, fg: u32, bg: u32) {
    re_asciifx::paint::put_str(fb, col, row, text, fg, bg);
}

/// Center `text` on `row`, in `fg` over `bg`. Delegates to
/// [`re_asciifx::paint::put_center`]; kept as a thin wrapper for the same
/// reason as [`clear_bg`].
pub(crate) fn put_center(fb: &mut Framebuffer, row: usize, text: &str, fg: u32, bg: u32) {
    re_asciifx::paint::put_center(fb, row, text, fg, bg);
}

// ─────────────────────── mouse hit-test geometry (shared with paint) ────────
//
// Every clickable menu item's rectangle is computed by ONE function per
// screen, called by BOTH the painter (below) and `crate::menu_mouse`'s
// hit-test — the discipline `ts_editor`'s `button_spans()` established: paint
// and hit-test read the SAME numbers, so a layout tweak can never silently
// move where a click lands out from under where the highlight is drawn.

/// One menu item's clickable rectangle: framebuffer `row`, inclusive
/// `col_start`, EXCLUSIVE `col_end`. `index` is the item's position in
/// whatever list this screen paints — the SAME index space as
/// [`crate::flow::MenuCursor::index`], so a hit directly becomes the new
/// cursor position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ItemRect {
    /// The item's position in the list this screen paints.
    pub index: usize,
    /// The framebuffer row this item occupies.
    pub row: usize,
    /// The first column this item covers (inclusive).
    pub col_start: usize,
    /// One past the last column this item covers (exclusive).
    pub col_end: usize,
}

impl ItemRect {
    #[inline]
    fn contains(&self, col: usize, row: usize) -> bool {
        row == self.row && col >= self.col_start && col < self.col_end
    }
}

/// The index of the first rect (if any) whose box contains `(col, row)`.
/// Pure — no ECS, no terminal — so it's headlessly unit-testable against
/// hand-built rects, and it's the exact function `menu_mouse` calls with the
/// REAL per-screen rects below.
pub(crate) fn hit_test(rects: &[ItemRect], col: usize, row: usize) -> Option<usize> {
    rects.iter().find(|r| r.contains(col, row)).map(|r| r.index)
}

/// Shared geometry for the [`put_menu_list`] layout (`draw_menu` / `draw_won`
/// / `draw_pause`): one row per item, stride 2, centered, `"  {label}  "`
/// width — EXACTLY the string `put_menu_list` stamps for the highlighted
/// item, so the clickable box is pixel-identical to the highlight bar.
pub(crate) fn menu_list_rects(cols: usize, top: usize, labels: &[&str]) -> Vec<ItemRect> {
    labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let row = top + i * 2;
            let w = label.chars().count() + 4; // "  " + label + "  "
            let col_start = cols.saturating_sub(w) / 2;
            ItemRect {
                index: i,
                row,
                col_start,
                col_end: col_start + w,
            }
        })
        .collect()
}

/// Shared geometry for a "row = `top + i * stride`, centred, fixed `pad`"
/// list — [`draw_category_select`]'s (`stride` 1) and [`draw_settings`]'s
/// (`stride` 2) layout. `lines` are the PRE-FORMATTED display strings
/// (already carrying any counts / values), so the widths match exactly what
/// those screens centre.
pub(crate) fn indexed_row_rects(
    cols: usize,
    top: usize,
    stride: usize,
    lines: &[String],
    pad: usize,
) -> Vec<ItemRect> {
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let row = top + i * stride;
            let w = line.chars().count() + pad;
            let col_start = cols.saturating_sub(w) / 2;
            ItemRect {
                index: i,
                row,
                col_start,
                col_end: col_start + w,
            }
        })
        .collect()
}

/// Draw a selectable menu list starting at `top`, centering each label and
/// highlighting `selected` with the selection bar. Items render as
/// `  LABEL  `; the selected one gets `▶ LABEL ◀` on the teal bar.
fn put_menu_list(fb: &mut Framebuffer, top: usize, items: &[&str], selected: usize) {
    let rects = menu_list_rects(fb.cols(), top, items);
    for (i, label) in items.iter().enumerate() {
        let row = top + i * 2;
        if i == selected {
            let txt = format!("  {label}  ");
            // selection bar — column comes from the SAME rect the hit-test uses.
            put_str(fb, rects[i].col_start, row, &txt, FG_HILITE, BG_HILITE);
        } else {
            put_center(fb, row, label, FG, BG);
        }
    }
}

// ────────────────────────────── splash ──────────────────────────────────────
//
// The splash draws from three authored ASCII assets:
//
//   assets/splash/eye_frames.txt          — a multi-frame eye animation
//   assets/splash/title_terminalstate.txt — the title card
//   assets/splash/press_key.txt           — the "press any key" strap
//
// All three are EMBEDDED with `include_str!`: the shipped binary is a static
// `crt-static` executable with no runtime asset path, and the splash must draw
// identically no matter which directory the game is launched from.
//
// The splash state's lifetime (`flow::SPLASH_TIMEOUT`) is divided into two phases
// — the eye animation, then the title card — so the TOTAL duration is unchanged
// (the state still auto-advances on the same timeout, and any key still skips it;
// both of those live in `flow::tick_splash`, untouched). Two phases rather than one
// screen because the eye alone is 22 rows and the title 12: they do not fit
// together on the 32-row grid, and cropping authored art is worse than sequencing
// it.
//
// The footer is the Sealed-Door notice (docs/lore/NORMATIVE_NOTES.md #13) — the
// campaign's outside-the-door frame, static across both phases.

/// The embedded eye animation source (`[frame N]` / `delay_ms N` records).
/// The SPLASH no longer paints this (see [`paint_splash`]'s `SplashPhase::Eye`
/// arm, which now paints [`re_asciifx::eye::Eye`]) — but
/// `crate::screens::ending_eye` still picks two of its captioned frames
/// (`awake` / `-- shut --`) for the campaign end-cards, so the asset and its
/// parser stay live.
const EYE_FRAMES_SRC: &str = include_str!("../../../assets/splash/eye_frames.txt");
/// The embedded title card.
const TITLE_SRC: &str = include_str!("../../../assets/splash/title_terminalstate.txt");
/// The embedded "press any key" strap.
const PRESS_KEY_SRC: &str = include_str!("../../../assets/splash/press_key.txt");

/// Frame delay used when a `[frame N]` record carries no (or an unparsable)
/// `delay_ms` line. Matches the authored cadence of the shipped asset.
const DEFAULT_FRAME_DELAY_MS: u64 = 80;

/// Share of the splash window the procedural eye gets before the title card
/// takes over — was "one full pass of the baked animation, capped at 65% of
/// the window" back when the eye was `assets/splash/eye_frames.txt`; the
/// procedural [`re_asciifx::eye::Eye`] has no frame cycle to measure, so
/// this is now just that same 65% share as a fixed fraction of the window.
const EYE_MAX_SHARE: f32 = 0.65;

/// NORMATIVE_NOTES #13 — "Posting on the Sealed Door", quoted verbatim (a subset
/// of its lines; nothing is reworded). The notice is the campaign's frame: the
/// building is shut, and the way you came in was not a door.
const SEALED_DOOR_NOTICE: [&str; 4] = [
    "NOTICE — POSTED ON THE FACILITY DOOR (exterior, sealed 2007)",
    "no personnel remain. one (1) steward remains. the steward is not personnel.",
    "if you are reading this from the inside, you did not come in through this door.",
    "you came in through the source.                          [ COPY 1 of 1 ]",
];

/// One parsed frame of the eye animation. Kept for `crate::screens::
/// ending_eye`'s campaign end-cards — see `EYE_FRAMES_SRC`'s doc comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EyeFrame {
    /// How long this frame is held.
    pub delay: std::time::Duration,
    /// The frame's art rows, in order (borrowed from the embedded asset).
    pub lines: Vec<&'static str>,
    /// The baked pupil's `(row, char-col)` within `lines`, if this frame
    /// carries one (a scan frame does; a blink/shut frame does not — see
    /// `find_pupil`). Row/col are CHAR indices, not byte offsets, because
    /// the art is full of multi-byte box-drawing/block glyphs.
    pub pupil: Option<(usize, usize)>,
}

/// Locate the baked `"@@@"` pupil marker (three consecutive `@` chars) in a
/// frame's art, returning its `(row, char-col)` — the first match, scanning
/// top to bottom. `None` when the frame carries no pupil (blink/shut/glitch
/// frames). Pure string search — no rendering, no ECS.
fn find_pupil(lines: &[&str]) -> Option<(usize, usize)> {
    for (r, line) in lines.iter().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        if let Some(c) = chars.windows(3).position(|w| w == ['@', '@', '@']) {
            return Some((r, c));
        }
    }
    None
}

/// Which half of the splash is on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplashPhase {
    /// The eye animation (frame chosen by time-in-state).
    Eye,
    /// The title card.
    Title,
}

/// Split an embedded art asset into its rows, dropping trailing blank rows.
/// Interior blank rows are KEPT (they are part of the art). `str::lines` strips a
/// trailing `\r`, so a CRLF checkout parses identically.
fn art_lines(src: &'static str) -> Vec<&'static str> {
    let mut lines: Vec<&'static str> = src.lines().collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines
}

/// Parse the eye-animation asset.
///
/// Format (as authored): a `[frame N]` header opens a frame, an optional
/// `delay_ms N` line sets its hold time, every other line is one row of art, and
/// a blank line separates frames. Unknown headers, a missing/garbled `delay_ms`,
/// and stray leading text are all tolerated — a malformed asset yields fewer
/// frames, never a panic.
fn parse_eye_frames(src: &'static str) -> Vec<EyeFrame> {
    let mut frames: Vec<EyeFrame> = Vec::new();
    for line in src.lines() {
        let t = line.trim_end();
        if t.trim_start().starts_with("[frame") {
            frames.push(EyeFrame {
                delay: std::time::Duration::from_millis(DEFAULT_FRAME_DELAY_MS),
                lines: Vec::new(),
                pupil: None,
            });
            continue;
        }
        let Some(cur) = frames.last_mut() else {
            continue; // art before the first header — ignore it
        };
        if let Some(rest) = t.trim_start().strip_prefix("delay_ms") {
            if let Ok(ms) = rest.trim().parse::<u64>() {
                cur.delay = std::time::Duration::from_millis(ms);
            }
            continue;
        }
        cur.lines.push(line);
    }
    // Drop each frame's trailing blank rows (the inter-frame separator), then drop
    // any frame that turned out to hold no art at all.
    for f in &mut frames {
        while f.lines.last().is_some_and(|l| l.trim().is_empty()) {
            f.lines.pop();
        }
    }
    frames.retain(|f| !f.lines.is_empty());
    // Locate each frame's pupil AFTER trimming — trimming only ever removes
    // TRAILING rows, so an interior row index found here is already final.
    for f in &mut frames {
        f.pupil = find_pupil(&f.lines);
    }
    frames
}

/// The parsed eye animation, parsed ONCE (the source is a compile-time constant,
/// so this is pure and deterministic — no I/O, no cwd dependence). Kept for
/// `crate::screens::ending_eye`; the splash itself no longer reads this (see
/// `EYE_FRAMES_SRC`'s doc comment).
#[must_use]
pub fn eye_frames() -> &'static [EyeFrame] {
    static FRAMES: std::sync::OnceLock<Vec<EyeFrame>> = std::sync::OnceLock::new();
    FRAMES.get_or_init(|| parse_eye_frames(EYE_FRAMES_SRC))
}

/// The parsed title card.
fn title_lines() -> &'static [&'static str] {
    static LINES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    LINES.get_or_init(|| art_lines(TITLE_SRC))
}

/// The "press any key" strap (its first non-empty row).
fn press_key_line() -> &'static str {
    static LINE: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    LINE.get_or_init(|| {
        art_lines(PRESS_KEY_SRC)
            .into_iter()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("[ press any key ]")
    })
}

/// How much of the splash window the procedural eye gets before the title
/// card takes over — see `EYE_MAX_SHARE`.
#[must_use]
pub fn eye_window(total: std::time::Duration) -> std::time::Duration {
    total.mul_f32(EYE_MAX_SHARE)
}

/// Which phase `elapsed` (time in the Splash state) falls in.
#[must_use]
pub fn splash_phase(elapsed: std::time::Duration, total: std::time::Duration) -> SplashPhase {
    if elapsed < eye_window(total) {
        SplashPhase::Eye
    } else {
        SplashPhase::Title
    }
}

/// Stamp a block of art rows so the BLOCK is centred as a whole (each row keeps
/// its own internal alignment — centring rows individually would shear the art).
/// Rows past the bottom of the framebuffer are skipped; `put_str` clips columns.
pub(crate) fn put_block(fb: &mut Framebuffer, top: usize, lines: &[&str], fg: u32, bg: u32) {
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let col = fb.cols().saturating_sub(width) / 2;
    for (i, line) in lines.iter().enumerate() {
        let row = top + i;
        if row >= fb.rows() {
            break;
        }
        put_str(fb, col, row, line, fg, bg);
    }
}

/// Top row for a `height`-row block vertically centred in `0..=bottom`.
fn centred_top(bottom: usize, height: usize) -> usize {
    (bottom + 1).saturating_sub(height) / 2
}

/// The procedural splash eye's rect: the whole area above the "press any
/// key" strap. [`re_asciifx::eye::Eye`] centres and sizes itself within
/// whatever rect it is given (it does not read `rect` at construction — see
/// its own doc), so handing it the full art area rather than a tightly-fit
/// box is enough; it does not need per-frame layout math the way the old
/// fixed-size baked art did.
fn eye_rect(fb: &Framebuffer, art_bottom: usize) -> re_asciifx::Rect {
    re_asciifx::Rect::new(0, 0, fb.cols(), art_bottom + 1)
}

/// Paint the whole splash for `elapsed` time-in-state into `fb`.
///
/// `eye_seed` is the per-launch seed for the procedural
/// [`re_asciifx::eye::Eye`] (see [`crate::flow::SplashEyeSeed`]) —
/// passed in rather than read from a resource so this stays a pure function
/// of its arguments: same `(elapsed, eye_seed, gaze_target)` always paints
/// the same frame, which is what makes both this module's own tests AND
/// `boot_console::breakdown_reaches_the_real_splash_frame` (which calls this
/// twice with a literal seed and diffs the two frames) meaningful.
///
/// `gaze_target` is the eye's look-at target in absolute framebuffer cell
/// coordinates (typically the live mouse position) — `None` lets the eye
/// fall back to its own autonomous saccade pattern.
pub fn paint_splash(
    fb: &mut Framebuffer,
    elapsed: std::time::Duration,
    eye_seed: u64,
    gaze_target: Option<(f32, f32)>,
) {
    clear_bg(fb);
    let rows = fb.rows();
    // The Sealed-Door notice is bottom-anchored; the strap sits two rows above it.
    let notice_top = rows.saturating_sub(SEALED_DOOR_NOTICE.len());
    let strap_row = notice_top.saturating_sub(2);
    // The phase art owns everything above the strap.
    let art_bottom = strap_row.saturating_sub(1);

    match splash_phase(elapsed, SPLASH_TIMEOUT) {
        SplashPhase::Eye => {
            let rect = eye_rect(fb, art_bottom);
            let eye = re_asciifx::eye::Eye::random(eye_seed, rect).with_target(gaze_target);
            re_asciifx::AsciiEffect::paint(&eye, fb, rect, elapsed);
        }
        SplashPhase::Title => {
            let title = title_lines();
            // title + one blank row + the tagline
            let top = centred_top(art_bottom, title.len() + 2);
            put_block(fb, top, title, FG, BG);
            let tag_row = top + title.len() + 1;
            if tag_row <= art_bottom {
                put_center(fb, tag_row, "a first-person ASCII puzzle", FG_DIM, BG);
            }
        }
    }

    put_center(fb, strap_row, press_key_line(), FG, BG);
    put_block(fb, notice_top, &SEALED_DOOR_NOTICE, FG_DIM, BG);
}

/// `Update` (Splash only): paint the splash for the current time-in-state. The
/// clock is [`SplashTimer`], which `flow::enter_splash` zeroes on entry and
/// `flow::tick_splash` advances — so the animation is driven by the state's own
/// existing cadence and needs no clock of its own. The eye's seed comes from
/// [`crate::flow::SplashEyeSeed`] (resolved once from the wall clock at app
/// start — "seeded from time, randomized per launch"); its gaze target comes
/// from [`crate::flow::SplashMouse`], folded from the live mouse position each
/// frame by `flow::track_splash_mouse`.
fn draw_splash(
    mut fb: ResMut<Framebuffer>,
    timer: Res<SplashTimer>,
    eye_seed: Res<crate::flow::SplashEyeSeed>,
    mouse: Res<crate::flow::SplashMouse>,
) {
    let rows = fb.rows();
    let cols = fb.cols();
    let gaze = mouse.0.map(|(c, r)| {
        // `SplashMouse` arrives in terminal cell coords already; clamp
        // defensively so an out-of-range event (a resize race) cannot hand
        // the eye a wildly off-screen target.
        (f32::from(c).min(cols as f32), f32::from(r).min(rows as f32))
    });
    paint_splash(&mut fb, timer.0, eye_seed.0, gaze);
}

/// §7.2 "Click … Release: every growth cell within 10 cells of the label
/// recoils". Reads `MenuNav` with its OWN `MessageReader` cursor, entirely
/// independent of whatever reader `flow.rs` uses to actually commit the
/// selection -- Bevy message readers each track their own read position, so
/// this never steals or duplicates the event flow.rs consumes.
fn overgrowth_recoil_on_confirm(
    mut nav: MessageReader<MenuNav>,
    cursor: Res<MenuCursor>,
    progress: Res<Progress>,
    fb: Res<Framebuffer>,
    mut fx: ResMut<MenuOvergrowth>,
) {
    if !nav.read().any(|n| matches!(n, MenuNav::Confirm)) {
        return;
    }
    let rects = menu_rects(fb.cols(), fb.rows(), &progress);
    let sel = cursor.index.min(rects.len().saturating_sub(1));
    if let Some(r) = rects.get(sel) {
        let center = (((r.col_start + r.col_end) / 2) as i32, r.row as i32);
        fx.state.recoil(center, OVERGROWTH_RECOIL_RADIUS);
    }
}

// ──────────────────────────────── menu ──────────────────────────────────────

/// The label for one main-menu item.
///
/// `BEGIN AGAIN?` carries the SESSION NUMBER the loop would open, so the offer is
/// diegetic (`session 0002 — begin again?`) rather than a bare UI verb — hence
/// `String`, not `&'static str`.
fn label_for_menu(item: crate::flow::MenuItem, progress: &Progress) -> String {
    match item {
        crate::flow::MenuItem::Start => "START".into(),
        crate::flow::MenuItem::Continue => "CONTINUE".into(),
        crate::flow::MenuItem::Memory => "MEMORY".into(),
        crate::flow::MenuItem::Select => "SELECT LEVEL".into(),
        crate::flow::MenuItem::Settings => "SETTINGS".into(),
        crate::flow::MenuItem::Guide => "GUIDE".into(),
        crate::flow::MenuItem::BeginAgain => {
            format!("session {} — begin again?", progress.next_session_label())
        }
        crate::flow::MenuItem::Quit => "QUIT".into(),
    }
}

/// The top row [`draw_menu`] starts its item list at, and the SAME value
/// `menu_mouse` uses to hit-test — kept as one function so the two can never
/// disagree about where row 0 of the list is.
pub(crate) fn menu_list_top(rows: usize) -> usize {
    (rows / 2).saturating_sub(4)
}

/// The main-menu's clickable rects for the CURRENT save (item count varies
/// with [`crate::flow::menu_items`]'s conditional `BeginAgain` entry).
pub(crate) fn menu_rects(cols: usize, rows: usize, progress: &Progress) -> Vec<ItemRect> {
    let items = crate::flow::menu_items(progress);
    let owned: Vec<String> = items.iter().map(|i| label_for_menu(*i, progress)).collect();
    let labels: Vec<&str> = owned.iter().map(String::as_str).collect();
    menu_list_rects(cols, menu_list_top(rows), &labels)
}

// ─────────────────────────── main-menu overgrowth ───────────────────────────
//
// The deterministic vine simulation itself lives in `crate::menu_fx::overgrowth`
// (pure, no ECS). Everything below is the thin ECS wiring: a resource holding
// the sim + its own growth-tick accumulator, reseeded whenever the terminal
// resizes or the save's session number changes (growth is seeded by the
// session number, so it reseeds on a loop-back), and a
// paint pass that runs BEFORE the title/list/hint stamps so those always win
// any pixel they touch — a second, structural guarantee (on top of the
// simulation's own repeller avoidance) that labels are never covered.

/// How often the overgrowth sim advances one growth tick — each tip advances
/// 1 cell per 180 ms, ± 40% jitter. The jitter is per-tip (baked into each
/// tip's own `speed` at spawn — see `menu_fx::overgrowth`), so this is just
/// the nominal cadence the accumulator drains at.
const OVERGROWTH_TICK: std::time::Duration = std::time::Duration::from_millis(180);

/// A click recoils every growth cell within this many cells of the label.
const OVERGROWTH_RECOIL_RADIUS: i32 = 10;

/// ECS-side state for the main-menu overgrowth effect: the pure sim plus the
/// bookkeeping needed to drive it from real frame `Time` and real `Framebuffer`
/// dimensions. `Default` seeds a fresh, empty field at the crate's nominal
/// view size; `draw_menu` reseeds it for real on first use (and on any resize
/// or session change) once it has a real `Framebuffer`/`Progress` to read.
#[derive(Resource)]
pub(crate) struct MenuOvergrowth {
    state: crate::menu_fx::overgrowth::OvergrowthState,
    accumulator: std::time::Duration,
    cols: usize,
    rows: usize,
    seeded_for_session: Option<u32>,
    hovered: Option<usize>,
}

impl Default for MenuOvergrowth {
    fn default() -> Self {
        MenuOvergrowth {
            state: crate::menu_fx::overgrowth::OvergrowthState::new(
                0,
                crate::VIEW_COLS,
                crate::VIEW_ROWS,
            ),
            accumulator: std::time::Duration::ZERO,
            cols: crate::VIEW_COLS,
            rows: crate::VIEW_ROWS,
            seeded_for_session: None,
            hovered: None,
        }
    }
}

// ─────────────── main-menu backdrop (random per launch) ────────────────────
//
// `menu_fx::backdrop` owns the pure "which kind, which seed" picking logic;
// this resource is the thin ECS side, same split as `MenuOvergrowth` above:
// picked ONCE per launch (from `flow::SplashEyeSeed`, on first `draw_menu`
// call), then the boxed effect is rebuilt whenever the terminal resizes
// (`Circuit`/`NeuralNet`/`EyeSwarm` bake absolute cell coordinates at
// construction — see `re_asciifx`'s `gallery.rs` doc on that gotcha — so a stale
// boxed effect from before a resize would paint into the wrong region).
#[derive(Resource, Default)]
pub(crate) struct MenuBackdrop {
    /// `None` until the first `draw_menu` call resolves it from
    /// `SplashEyeSeed` — after that it never changes for the rest of the run.
    kind: Option<crate::menu_fx::backdrop::BackdropKind>,
    effect: Option<Box<dyn re_asciifx::AsciiEffect + Send + Sync>>,
    cols: usize,
    rows: usize,
    /// This backdrop's own running clock (accumulated `Time::delta()`, same
    /// pattern `MenuOvergrowth.accumulator` uses) — `re_asciifx` effects are
    /// pure functions of `(self, t)` with no wall-clock of their own, so
    /// something has to own the clock they animate against.
    elapsed: std::time::Duration,
}

/// Paint every planted overgrowth cell into `fb`. Runs BEFORE the menu's own
/// text stamps (see the section doc above) so it can never sit on top of a
/// label even in an edge case the sim's own repeller-avoidance missed.
fn paint_overgrowth(fb: &mut Framebuffer, state: &crate::menu_fx::overgrowth::OvergrowthState) {
    for row in 0..fb.rows() {
        for col in 0..fb.cols() {
            let (c, r) = (col as i32, row as i32);
            let Some(glyph) = state.glyph_at(c, r) else {
                continue;
            };
            let age = state
                .planted_step_at(c, r)
                .map_or(0, |planted| state.step_count().saturating_sub(planted));
            let style = if age <= crate::menu_fx::overgrowth::AGE_BOLD_TICKS {
                re_core::CellStyle::NONE.with_bold()
            } else {
                re_core::CellStyle::NONE.with_flags(re_core::CellStyle::DIM)
            };
            fb.set(
                col,
                row,
                Cell {
                    glyph: u32::from(glyph),
                    fg: FG,
                    bg: BG,
                    style,
                    ..Default::default()
                },
            );
        }
    }
}

fn draw_menu(
    mut fb: ResMut<Framebuffer>,
    cursor: Res<MenuCursor>,
    progress: Res<Progress>,
    mut fx: ResMut<MenuOvergrowth>,
    mut backdrop: ResMut<MenuBackdrop>,
    eye_seed: Res<crate::flow::SplashEyeSeed>,
    time: Res<Time>,
) {
    if backdrop.kind.is_none() {
        backdrop.kind = Some(crate::menu_fx::backdrop::pick(eye_seed.0));
    }
    let is_overgrowth = backdrop.kind == Some(crate::menu_fx::backdrop::BackdropKind::Overgrowth);

    if fx.cols != fb.cols()
        || fx.rows != fb.rows()
        || fx.seeded_for_session != Some(progress.session)
    {
        fx.state = crate::menu_fx::overgrowth::OvergrowthState::new(
            u64::from(progress.session),
            fb.cols(),
            fb.rows(),
        );
        fx.cols = fb.cols();
        fx.rows = fb.rows();
        fx.seeded_for_session = Some(progress.session);
        fx.hovered = None;
        fx.accumulator = std::time::Duration::ZERO;
    }

    let rects = menu_rects(fb.cols(), fb.rows(), &progress);
    let sel = cursor.index.min(rects.len().saturating_sub(1));
    let hovered_rect = rects
        .get(sel)
        .map(|r| crate::menu_fx::overgrowth::HoverRect {
            row: r.row as i32,
            col_start: r.col_start as i32,
            col_end: r.col_end as i32,
        });
    if fx.hovered != Some(sel) {
        if let Some(hr) = hovered_rect {
            fx.state.on_hover_start(hr);
        }
        fx.hovered = Some(sel);
    }
    let repellers: Vec<crate::menu_fx::overgrowth::Repeller> = rects
        .iter()
        .map(|r| crate::menu_fx::overgrowth::Repeller {
            row: r.row as i32,
            col_start: r.col_start as i32,
            col_end: r.col_end as i32,
        })
        .collect();

    if is_overgrowth {
        fx.accumulator += time.delta();
        while fx.accumulator >= OVERGROWTH_TICK {
            fx.accumulator -= OVERGROWTH_TICK;
            let (cols, rows) = (fb.cols(), fb.rows());
            fx.state.step(cols, rows, &repellers, hovered_rect);
        }
    }

    clear_bg(&mut fb);
    if is_overgrowth {
        paint_overgrowth(&mut fb, &fx.state);
    } else {
        backdrop.elapsed += time.delta();
        let kind = backdrop.kind.expect("resolved above");
        if backdrop.effect.is_none() || backdrop.cols != fb.cols() || backdrop.rows != fb.rows() {
            let rect = re_asciifx::Rect::new(0, 0, fb.cols(), fb.rows());
            let seed = crate::menu_fx::backdrop::effect_seed(eye_seed.0);
            backdrop.effect = crate::menu_fx::backdrop::build(kind, seed, rect);
            backdrop.cols = fb.cols();
            backdrop.rows = fb.rows();
        }
        if let Some(effect) = &backdrop.effect {
            let rect = re_asciifx::Rect::new(0, 0, fb.cols(), fb.rows());
            let mut tmp = Framebuffer::new(fb.cols(), fb.rows());
            effect.paint(&mut tmp, rect, backdrop.elapsed);
            crate::menu_fx::backdrop::blit_dimmed(&mut fb, &tmp, crate::menu_fx::backdrop::DIM_K);
        }
    }
    let mid = fb.rows() / 2;
    put_center(
        &mut fb,
        mid.saturating_sub(8),
        "T E R M I N A L   S T A T E",
        FG,
        BG,
    );
    // A completed campaign says so, once, above the list — both endings reach this
    // line, which is the visible half of "both mark the campaign COMPLETE".
    let manifest = crate::flow::campaign_manifest();
    if manifest.is_complete(&progress) {
        put_center(
            &mut fb,
            mid.saturating_sub(6),
            &format!(
                "campaign complete — {}/{} rooms · session {}",
                manifest.len(),
                manifest.len(),
                progress.session_label()
            ),
            FG_DIM,
            BG,
        );
    }
    let items = crate::flow::menu_items(&progress);
    let owned: Vec<String> = items
        .iter()
        .map(|i| label_for_menu(*i, &progress))
        .collect();
    let labels: Vec<&str> = owned.iter().map(String::as_str).collect();
    let sel = cursor.index.min(items.len().saturating_sub(1));
    let top = menu_list_top(fb.rows());
    put_menu_list(&mut fb, top, &labels, sel);
    let hint_row = fb.rows().saturating_sub(2);
    put_center(
        &mut fb,
        hint_row,
        "W/S or ↑/↓ move  ·  E/Enter select  ·  Esc/q quit",
        FG_DIM,
        BG,
    );
}

// ─────────────────────────── category select ────────────────────────────────

/// Row 0 of [`draw_category_select`]'s list — shared with `menu_mouse`.
pub(crate) const CATEGORY_TOP: usize = 5;
/// Padding added to a category line's char length for its clickable width:
/// both the selected (`"  ▶ {line}  "`) and unselected (`"    {line}  "`)
/// formats are `line.len() + 6`, so one pad value covers either.
pub(crate) const CATEGORY_PAD: usize = 6;

/// The per-category display line (label + count, or campaign progress),
/// WITHOUT the selection marker — the text both the paint branches and the
/// hit-test geometry measure.
pub(crate) fn category_lines(catalog: &LevelCatalog, progress: &Progress) -> Vec<String> {
    let manifest = crate::flow::campaign_manifest();
    visible_categories(catalog)
        .iter()
        .map(|cat| {
            let count = catalog.count_in(*cat);
            if *cat == crate::flow::Category::Campaign && !manifest.is_empty() {
                format!(
                    "{:<12} ({}/{})",
                    cat.label(),
                    manifest.solved_count(progress),
                    manifest.len()
                )
            } else {
                format!("{:<12} ({count})", cat.label())
            }
        })
        .collect()
}

/// The category-select screen's clickable rects.
pub(crate) fn category_rects(
    cols: usize,
    catalog: &LevelCatalog,
    progress: &Progress,
) -> Vec<ItemRect> {
    indexed_row_rects(
        cols,
        CATEGORY_TOP,
        1,
        &category_lines(catalog, progress),
        CATEGORY_PAD,
    )
}

/// The first level of the two-level browse: list the category "folders" with a
/// per-category level count, highlighting the focused one. Confirming a folder
/// drops into [`draw_level_select`] scoped to that category.
fn draw_category_select(
    mut fb: ResMut<Framebuffer>,
    cursor: Res<MenuCursor>,
    catalog: Res<LevelCatalog>,
    progress: Res<Progress>,
) {
    clear_bg(&mut fb);
    put_center(&mut fb, 2, "SELECT CATEGORY", FG, BG);

    // Empty categories are not offered at all (`visible_categories`), so every
    // row painted below has at least one level — no dimmed "(0)" dead end.
    let visible = visible_categories(&catalog);
    let sel = cursor.index.min(visible.len().saturating_sub(1));
    let lines = category_lines(&catalog, &progress);
    let rects = indexed_row_rects(fb.cols(), CATEGORY_TOP, 1, &lines, CATEGORY_PAD);
    for (i, _cat) in visible.iter().enumerate() {
        let row = CATEGORY_TOP + i;
        let line = &lines[i];
        if i == sel {
            let txt = format!("  ▶ {line}  ");
            put_str(&mut fb, rects[i].col_start, row, &txt, FG_HILITE, BG_HILITE);
        } else {
            put_center(&mut fb, row, &format!("    {line}  "), FG, BG);
        }
    }
    let hint_row = fb.rows().saturating_sub(2);
    put_center(
        &mut fb,
        hint_row,
        "W/S or ↑/↓ move  ·  E/Enter open  ·  Esc/q back",
        FG_DIM,
        BG,
    );
}

// ──────────────────────────── level select ──────────────────────────────────

/// How many level rows fit on the level-select screen between the header and the
/// hint line. The list scrolls to keep the cursor visible when a category holds
/// more levels than this.
const LIST_VISIBLE_ROWS: usize = 22;

/// Row 0 of the level-select visible window — shared with `menu_mouse`.
pub(crate) const LEVEL_SELECT_TOP: usize = 5;
/// Padding added to a level row's title length for its clickable width: the
/// SELECTED format `"  ▶ {marker} {title}  "` is `title.len() + 8` (a 4-char
/// `"  ▶ "` prefix, the 1-char marker, a separating space, and a 2-char
/// suffix) — used uniformly so hovering an unselected (narrower-painted) row
/// still hits.
pub(crate) const LEVEL_SELECT_PAD: usize = 8;

/// One visible row of the level-select list: its cursor-domain `index` (a
/// position into `catalog.indices_in(browse)`, the SAME space
/// `MenuCursor::index` uses), display `row`/`marker`/`title`, and the
/// solved/unlocked flags the painter colors by.
///
/// Computed ONCE by [`level_select_rows`] and consumed by BOTH the painter
/// and `menu_mouse`'s hit-test, so the scroll-window math (which row shows
/// which level) can never disagree between the two.
pub(crate) struct LevelRow {
    pub index: usize,
    pub row: usize,
    pub marker: char,
    pub title: String,
    pub solved: bool,
    pub unlocked: bool,
}

/// Build the visible rows for `browse`'s level list, given the (already
/// clamped) cursor position `sel`. Empty when the category holds no levels.
pub(crate) fn level_select_rows(
    catalog: &LevelCatalog,
    browse: crate::flow::Category,
    progress: &Progress,
    sel: usize,
) -> Vec<LevelRow> {
    let indices = catalog.indices_in(browse);
    if indices.is_empty() {
        return Vec::new();
    }
    let sel = sel.min(indices.len() - 1);
    // Scroll window: keep `sel` visible. The first visible row index is chosen so
    // the cursor sits inside the [0, LIST_VISIBLE_ROWS) window.
    let visible = LIST_VISIBLE_ROWS.min(indices.len());
    let start = sel
        .saturating_sub(visible.saturating_sub(1))
        .min(indices.len().saturating_sub(visible));
    let manifest = crate::flow::campaign_manifest();
    indices[start..(start + visible)]
        .iter()
        .enumerate()
        .map(|(vis, &gidx)| {
            let i = start + vis;
            let path = &catalog.paths[gidx];
            // Progress keys on the stable file stem; the row SHOWS the display name.
            let stem = LevelCatalog::label(path);
            // An authored campaign name wins over the filename-derived title — "The
            // Soft-Footstep" reads as a chapter, "Soft Footstep" reads as a file.
            let title = manifest
                .get(stem)
                .map(|l| l.name.clone())
                .unwrap_or_else(|| crate::flow::display_title(path));
            let solved = progress.is_solved(stem);
            let unlocked = manifest.is_unlocked(stem, progress);
            // Three states, three markers: solved, available, locked. A locked row
            // still shows its NAME (the arc's shape is not a secret — only entry is
            // earned).
            let marker = if solved {
                '✓'
            } else if unlocked {
                '·'
            } else {
                '—'
            };
            LevelRow {
                index: i,
                row: LEVEL_SELECT_TOP + vis,
                marker,
                title,
                solved,
                unlocked,
            }
        })
        .collect()
}

/// The level-select screen's clickable rects, one per visible row.
pub(crate) fn level_select_rects(cols: usize, rows: &[LevelRow]) -> Vec<ItemRect> {
    rows.iter()
        .map(|r| {
            let w = r.title.chars().count() + LEVEL_SELECT_PAD;
            let col_start = cols.saturating_sub(w) / 2;
            ItemRect {
                index: r.index,
                row: r.row,
                col_start,
                col_end: col_start + w,
            }
        })
        .collect()
}

fn draw_level_select(
    mut fb: ResMut<Framebuffer>,
    cursor: Res<MenuCursor>,
    catalog: Res<LevelCatalog>,
    browse: Res<BrowseCategory>,
    progress: Res<Progress>,
) {
    clear_bg(&mut fb);
    put_center(&mut fb, 2, &format!("{} LEVELS", browse.0.label()), FG, BG);

    // Only this category's levels (global catalogue indices).
    let indices = catalog.indices_in(browse.0);
    if indices.is_empty() {
        put_center(&mut fb, 6, "(no levels in this category)", FG_DIM, BG);
        let hint_row = fb.rows().saturating_sub(2);
        put_center(&mut fb, hint_row, "Esc/q back", FG_DIM, BG);
        return;
    }

    let sel = cursor.index.min(indices.len() - 1);
    // The campaign manifest supplies BOTH the authored display name and the unlock
    // state for its twelve levels. Non-manifest levels keep the derived title and are
    // never gated (`is_unlocked` returns true for anything the manifest omits).
    let manifest = crate::flow::campaign_manifest();
    // On a campaign row, name the MOVEMENT the highlighted level belongs to. The
    // four-movement frame is the campaign's emotional structure (first light →
    // recognition → grief → the choice) and this is the one place the player can see
    // which of the four they are standing in.
    if let Some(m) = catalog
        .id_at(indices[sel])
        .and_then(|id| manifest.get(id))
        .map(|l| l.movement)
    {
        put_center(&mut fb, 3, m.title(), FG_DIM, BG);
    }
    let rows = level_select_rows(&catalog, browse.0, &progress, sel);
    let rects = level_select_rects(fb.cols(), &rows);
    let mut any_locked = false;
    for (r, rect) in rows.iter().zip(rects.iter()) {
        if !r.unlocked {
            any_locked = true;
        }
        let LevelRow {
            index: i,
            row,
            marker,
            title,
            solved,
            unlocked,
        } = r;
        if *i == sel {
            let txt = format!("  ▶ {marker} {title}  ");
            // A locked highlight is drawn DIM-on-highlight so the cursor is still
            // visible while the row reads plainly as unavailable.
            let (fg, bg) = if *unlocked {
                (FG_HILITE, BG_HILITE)
            } else {
                (FG_DIM, BG_HILITE)
            };
            put_str(&mut fb, rect.col_start, *row, &txt, fg, bg);
        } else {
            let fg = if *solved {
                FG
            } else if *unlocked {
                FG_DIM
            } else {
                FG_LOCKED
            };
            put_center(&mut fb, *row, &format!("  {marker} {title}  "), fg, BG);
        }
    }
    // Scroll hint when the list overflows the visible window.
    let visible = LIST_VISIBLE_ROWS.min(indices.len());
    if indices.len() > visible {
        let pos_row = LEVEL_SELECT_TOP + visible;
        put_center(
            &mut fb,
            pos_row,
            &format!("[ {}/{} ]", sel + 1, indices.len()),
            FG_DIM,
            BG,
        );
    }
    let hint_row = fb.rows().saturating_sub(2);
    // The legend only advertises the lock when a locked row is actually on screen.
    let legend = if any_locked {
        "✓ solved  ·  — locked (solve the level before it)  ·  E/Enter play  ·  Esc/q back"
    } else {
        "✓ solved  ·  W/S or ↑/↓ move  ·  E/Enter play  ·  Esc/q back"
    };
    put_center(&mut fb, hint_row, legend, FG_DIM, BG);
}

// ────────────────────────────── settings ────────────────────────────────────

/// The display name for a settings row.
fn settings_row_label(row: SettingsRow) -> &'static str {
    match row {
        SettingsRow::Color => "COLOR",
        SettingsRow::MouseEnabled => "MOUSE-LOOK",
        SettingsRow::MouseSens => "MOUSE SENS",
        SettingsRow::Mute => "AUDIO",
        SettingsRow::Volume => "VOLUME",
        SettingsRow::FontSize => "FONT SIZE",
        SettingsRow::Glitch => "CORRUPTION",
        SettingsRow::Fov => "FOV",
        SettingsRow::Stretch => "ASPECT/STRETCH",
        SettingsRow::Shadows => "SHADOWS",
        SettingsRow::Quality => "QUALITY",
        SettingsRow::DrawDistance => "DRAW DISTANCE",
        SettingsRow::Fps => "FPS LIMIT",
        SettingsRow::Lod => "LOD",
        SettingsRow::SubCell => "SUB-CELL",
        SettingsRow::Postfx => "POST-FX",
        SettingsRow::ColorUnlock => "COLOR UNLOCK",
        SettingsRow::SoundFields => "SOUND FIELDS",
        SettingsRow::SoundFieldsReduced => "FIELD EFFECTS",
        SettingsRow::Brightness => "BRIGHTNESS",
        SettingsRow::ColorSteps => "COLOUR STEPS",
        SettingsRow::FirstSteps => "FIRST STEPS",
    }
}

/// The current value of a settings row, formatted for display.
fn settings_row_value(row: SettingsRow, s: &Settings) -> String {
    match row {
        SettingsRow::Color => match s.color {
            ColorMode::Truecolor => "TRUECOLOR".to_string(),
            ColorMode::Ansi256 => "256-COLOR".to_string(),
        },
        SettingsRow::MouseEnabled => if s.mouse_enabled { "ON" } else { "OFF" }.to_string(),
        SettingsRow::MouseSens => s.mouse_sens.label().to_string(),
        SettingsRow::Mute => if s.muted { "MUTED" } else { "ON" }.to_string(),
        SettingsRow::Volume => format!("{}%", (s.volume * 100.0).round() as i32),
        // The grid IS the terminal emulator's own cell size, which WE cannot set —
        // the emulator owns it. So this row is an honest, inert note pointing the
        // player at their terminal's own zoom (Ctrl +/-), not a numeric control.
        SettingsRow::FontSize => "set by terminal (Ctrl +/-)".to_string(),
        SettingsRow::Glitch => if s.glitch_enabled { "ON" } else { "OFF" }.to_string(),
        SettingsRow::Fov => format!("{}°", s.fov_deg),
        SettingsRow::Stretch => format!("{:.2}", s.stretch),
        SettingsRow::Shadows => match s.shadows {
            re_render::ShadowQuality::Off => "OFF",
            re_render::ShadowQuality::Hard => "HARD",
            re_render::ShadowQuality::Soft => "SOFT",
        }
        .to_string(),
        SettingsRow::Quality => s.quality.label().to_string(),
        SettingsRow::DrawDistance => format!("{} CELLS", s.draw_distance),
        SettingsRow::Fps => s.fps_cap.label(),
        SettingsRow::Lod => s.lod_quality.label().to_string(),
        SettingsRow::SubCell => if s.subcell { "ON" } else { "OFF" }.to_string(),
        SettingsRow::Postfx => s.postfx.to_ascii_uppercase(),
        SettingsRow::ColorUnlock => if s.colour_unlock_enabled { "ON" } else { "OFF" }.to_string(),
        SettingsRow::SoundFields => s.sound_fields_label().to_string(),
        SettingsRow::SoundFieldsReduced => if s.sound_fields_reduced {
            "REDUCED"
        } else {
            "FULL"
        }
        .to_string(),
        SettingsRow::Brightness => s.brightness_label().to_string(),
        SettingsRow::ColorSteps => s.color_steps.label().to_string(),
        SettingsRow::FirstSteps => if s.first_steps_hud { "ON" } else { "OFF" }.to_string(),
    }
}

/// Row 0 of [`draw_settings`]'s list, its row stride, and its label column
/// width — all shared with `menu_mouse` (and the click-width pad below).
pub(crate) const SETTINGS_TOP: usize = 6;
pub(crate) const SETTINGS_STRIDE: usize = 2;
pub(crate) const SETTINGS_LABEL_W: usize = 12;
/// Padding added to a settings line's char length for its clickable width:
/// the selected format `"▶ {line} ◀"` is `line.len() + 4` (▶+space prefix,
/// space+◀ suffix).
pub(crate) const SETTINGS_PAD: usize = 4;

/// The formatted `"{label:<12}  {value}"` line for every settings row, in
/// display order — the text both paint branches and the hit-test geometry
/// measure.
pub(crate) fn settings_lines(s: &Settings) -> Vec<String> {
    SETTINGS_ROWS
        .iter()
        .map(|&row| {
            format!(
                "{label:<width$}  {value}",
                label = settings_row_label(row),
                value = settings_row_value(row, s),
                width = SETTINGS_LABEL_W,
            )
        })
        .collect()
}

/// The on-screen row (`y`) each entry in `rows` paints at, starting from `top`
/// and index-aligned 1:1 with `rows` — the SINGLE place that accounts for
/// [`crate::flow::settings_section_before`]'s section headers, so the cursor
/// (`SETTINGS_ROWS[i]`), the click rects ([`settings_rects`]), and the paint
/// ([`draw_settings`]) can never disagree about where a row lives. A header
/// costs one extra row directly above the row it names; [`SETTINGS_STRIDE`]
/// still separates every settings row from the next.
///
/// Takes `top` and an arbitrary slice (rather than always starting at
/// [`SETTINGS_TOP`] over the whole [`SETTINGS_ROWS`]) so a SCROLLED WINDOW —
/// some contiguous sub-range of the full list, repainted starting from a fixed
/// on-screen top — reuses the exact same header-aware layout math instead of a
/// second copy that could drift from it.
fn settings_window_row_ys(top: usize, rows: &[SettingsRow]) -> Vec<usize> {
    let mut y = top;
    let mut out = Vec::with_capacity(rows.len());
    for &row in rows {
        if crate::flow::settings_section_before(row).is_some() {
            y += 1;
        }
        out.push(y);
        y += SETTINGS_STRIDE;
    }
    out
}

/// One row reserved directly under the title for a `▲ more above` indicator,
/// and one reserved directly above the hint line for a `▼ more below`
/// indicator — present or not, so the list's vertical extent (and every row
/// under it) never shifts as the cursor scrolls into or out of overflow.
const SETTINGS_INDICATOR_ROWS: usize = 1;

/// The first on-screen body row available to settings ENTRIES (below the
/// reserved `▲` indicator row).
fn settings_body_top() -> usize {
    SETTINGS_TOP + SETTINGS_INDICATOR_ROWS
}

/// The on-screen row the hint / legend line sits on, and the exclusive bound
/// entries must stay above (leaving the reserved `▼` indicator row directly
/// beneath the last entry).
fn settings_hint_row(total_rows: usize) -> usize {
    total_rows.saturating_sub(2)
}

/// The scrolled window into [`SETTINGS_ROWS`] — `[start, end)` — that keeps
/// `sel` visible on a framebuffer of `total_rows` rows.
///
/// Mirrors `level_select_rows`'s "keep the cursor inside the visible window"
/// scroll math, but the settings list cannot use a single fixed
/// `LIST_VISIBLE_ROWS` constant the way the level list does: a header
/// ([`crate::flow::settings_section_before`]) costs an extra row of vertical
/// space only when it happens to fall inside the window, so how many entries
/// FIT is not a constant — it depends on which entries end up shown. This
/// grows the window outward from `sel` one entry at a time, re-measuring the
/// REAL header-aware layout (via [`settings_window_row_ys`]) after each grow,
/// so it can never claim a window fits when it doesn't (or vice-versa).
pub(crate) fn settings_window(sel: usize, total_rows: usize) -> (usize, usize) {
    let n = SETTINGS_ROWS.len();
    if n == 0 {
        return (0, 0);
    }
    let sel = sel.min(n - 1);
    let body_top = settings_body_top();
    // Entries must end strictly before the reserved `▼` row, which sits
    // directly above the hint line.
    let body_bottom_excl = settings_hint_row(total_rows).saturating_sub(SETTINGS_INDICATOR_ROWS);

    let fits = |start: usize, end: usize| -> bool {
        if start >= end {
            return true;
        }
        settings_window_row_ys(body_top, &SETTINGS_ROWS[start..end])
            .last()
            .is_some_and(|&y| y < body_bottom_excl)
    };

    let mut start = sel;
    let mut end = sel + 1;
    // The selected row alone must always fit — a terminal too small even for
    // one row is a degenerate case no scroll window can fix.
    loop {
        let mut grew = false;
        if end < n && fits(start, end + 1) {
            end += 1;
            grew = true;
        }
        if start > 0 && fits(start - 1, end) {
            start -= 1;
            grew = true;
        }
        if !grew {
            break;
        }
    }
    (start, end)
}

/// The settings screen's clickable rects, for whichever window is on screen
/// given the current cursor `sel` and framebuffer row count — so a hidden
/// (scrolled-off) row is never hoverable/clickable, and a click always lands
/// on the row the eye sees.
pub(crate) fn settings_rects(
    cols: usize,
    sel: usize,
    total_rows: usize,
    s: &Settings,
) -> Vec<ItemRect> {
    let lines = settings_lines(s);
    let (start, end) = settings_window(sel, total_rows);
    let ys = settings_window_row_ys(settings_body_top(), &SETTINGS_ROWS[start..end]);
    lines[start..end]
        .iter()
        .zip(ys.iter())
        .enumerate()
        .map(|(vis, (line, &row))| {
            let i = start + vis;
            let w = line.chars().count() + SETTINGS_PAD;
            let col_start = cols.saturating_sub(w) / 2;
            ItemRect {
                index: i,
                row,
                col_start,
                col_end: col_start + w,
            }
        })
        .collect()
}

/// Whether the settings row's current value sits in a costly performance
/// class, so the row can be tinted as a hint. Measured costs: `SUBCELL` alone
/// reaches ~1.6-4.1 ms/frame under rotation in classic scenarios, and `FAR`
/// re-bakes into the multi-millisecond range under motion at a long draw
/// distance; `SHADOWS=Soft` and `QUALITY=high` are the other known-costly
/// settings. Pure display: never gates a value, only tints it.
fn settings_row_costly(row: SettingsRow, s: &Settings) -> bool {
    use crate::settings::{LodQuality, QualityPreset};
    match row {
        SettingsRow::Shadows => s.shadows == re_render::ShadowQuality::Soft,
        SettingsRow::Quality => matches!(s.quality, QualityPreset::High | QualityPreset::Ultra),
        SettingsRow::DrawDistance => s.draw_distance >= 32,
        SettingsRow::Lod => s.lod_quality == LodQuality::Near,
        SettingsRow::SubCell => s.subcell,
        SettingsRow::Postfx => matches!(s.postfx.as_str(), "cinematic" | "crt" | "dream"),
        _ => false,
    }
}

/// Warm amber-orange — the cost tint. Distinct from every other HUD/menu hue
/// already in use (not the teal selection bar, not the memory amber).
const FG_COSTLY: u32 = 0x00FF_A040;

/// Paint one settings row at `row_y`: for the SELECTED row, the existing
/// `"▶ {line} ◀"` highlight bar (unchanged — a cost tint would fight the
/// highlight for attention on the one row already drawing the eye). For every
/// other row, the label and value are painted as two separate runs so the
/// value alone can carry the cost tint, while the combined text stays
/// centered exactly as [`settings_lines`]/[`settings_rects`] measure it.
#[allow(clippy::too_many_arguments)]
fn draw_settings_row(
    fb: &mut Framebuffer,
    cols: usize,
    row_y: usize,
    label: &str,
    value: &str,
    costly: bool,
    selected: bool,
) {
    let line = format!("{label:<width$}  {value}", width = SETTINGS_LABEL_W);
    if selected {
        let txt = format!("▶ {line} ◀");
        let col_start = cols.saturating_sub(txt.chars().count()) / 2;
        put_str(fb, col_start, row_y, &txt, FG_HILITE, BG_HILITE);
        return;
    }
    let col_start = cols.saturating_sub(line.chars().count()) / 2;
    let label_run = format!("{label:<width$}  ", width = SETTINGS_LABEL_W);
    put_str(fb, col_start, row_y, &label_run, FG, BG);
    let value_col = col_start + label_run.chars().count();
    let value_fg = if costly { FG_COSTLY } else { FG };
    put_str(fb, value_col, row_y, value, value_fg, BG);
}

/// The settings screen: one row per [`SettingsRow`], showing its label + current
/// value, with the focused row on the selection bar. `Left`/`Right` change the
/// focused value (see `settings_nav` in `flow`); `Back` returns to the menu.
///
/// SCROLLS to keep the selected row on screen: at 22 rows the full list
/// overflows any terminal this small (it needs ~51 lines; even a spacious
/// 100×32 grid has ~24 body rows to work with, and the minimum-supported
/// 80×24 has ~16), so only [`settings_window`]'s visible slice is painted,
/// with a `▲`/`▼` indicator whenever rows are hidden above/below it. The
/// COMFORT/PHOTOSENSITIVITY header and the cost-tint legend footer are
/// unaffected: the header paints correctly whenever its row is in the visible
/// window (same header-aware layout as the unscrolled list, just re-based at
/// the window's own top), and the footer stays anchored to
/// `fb.rows() - 2`/`- 1` regardless of scroll position.
fn draw_settings(mut fb: ResMut<Framebuffer>, cursor: Res<MenuCursor>, settings: Res<Settings>) {
    clear_bg(&mut fb);
    put_center(&mut fb, 2, "SETTINGS", FG, BG);

    let n = SETTINGS_ROWS.len();
    let sel = cursor.index.min(n.saturating_sub(1));
    let total_rows = fb.rows();
    let cols = fb.cols();
    let (start, end) = settings_window(sel, total_rows);
    let ys = settings_window_row_ys(settings_body_top(), &SETTINGS_ROWS[start..end]);

    if start > 0 {
        put_center(&mut fb, SETTINGS_TOP, "▲ more above", FG_DIM, BG);
    }
    for (vis, &row) in SETTINGS_ROWS[start..end].iter().enumerate() {
        let i = start + vis;
        let r = ys[vis];
        if let Some(header) = crate::flow::settings_section_before(row) {
            put_center(&mut fb, r - 1, &format!("── {header} ──"), FG_DIM, BG);
        }
        draw_settings_row(
            &mut fb,
            cols,
            r,
            settings_row_label(row),
            &settings_row_value(row, &settings),
            settings_row_costly(row, &settings),
            i == sel,
        );
    }
    let hint_row = settings_hint_row(total_rows);
    if end < n {
        put_center(
            &mut fb,
            hint_row.saturating_sub(SETTINGS_INDICATOR_ROWS),
            "▼ more below",
            FG_DIM,
            BG,
        );
    }

    put_center(
        &mut fb,
        hint_row,
        "amber = pricier settings  ·  W/S or ↑/↓ move  ·  A/D or ←/→ change  ·  Esc/q back",
        FG_DIM,
        BG,
    );
}

// ──────────────────────────────── won ───────────────────────────────────────

/// Recovered-memory amber — the readout line is styled distinctly from the
/// system-cyan UI (WORD_STORY: "the memory surfacing as letters", not a caption).
pub(crate) const FG_MEMORY: u32 = 0x00FF_D080;
/// The corrupted / denial readout glares red — the Babel collapse.
pub(crate) const FG_MEMORY_CORRUPT: u32 = 0x00FF_5060;

/// The name-shard amber — one shade cooler than the memory readout, so the shard
/// line reads as the *system's* accounting rather than a recovered memory.
pub(crate) const FG_SHARD: u32 = 0x00FF_B860;

/// The label for one won-overlay item.
pub(crate) fn won_item_label(w: crate::flow::WonItem) -> &'static str {
    match w {
        crate::flow::WonItem::NextLevel => "NEXT LEVEL",
        crate::flow::WonItem::Menu => "MENU",
    }
}

/// The top row [`draw_won`]'s item list starts at — shared with `menu_mouse`.
pub(crate) fn won_list_top(rows: usize) -> usize {
    rows / 2 + 2
}

/// The won-overlay's clickable rects.
pub(crate) fn won_rects(cols: usize, rows: usize) -> Vec<ItemRect> {
    let labels: Vec<&str> = crate::flow::WON_ITEMS
        .iter()
        .map(|w| won_item_label(*w))
        .collect();
    menu_list_rects(cols, won_list_top(rows), &labels)
}

/// Does this level id sit on the seven-shard name arc at all? (Used to decide
/// whether the `true_name:` line belongs on a screen where nothing is assembled
/// yet — it does on L1 before its own solve is counted, it does not on a demo.)
fn shard_for_this_level(level_id: &str) -> bool {
    crate::story::shard_for_level(level_id).is_some()
}

/// The won overlay does NOT clear the framebuffer — it stamps a banner over the
/// last-rendered solved scene so the 3D view stays visible behind it. The banner
/// carries the **memory readout** (this level's assembled phrase), the
/// **name-shard** this level yielded plus the running `pid://…` assembly
/// (`WORD_STORY` §5 item 5), and a scrollback of the accumulated story.
///
/// The shard lines are keyed off the SOLVED SET
/// ([`crate::save::Progress::solved`]) — `record_progress` runs on the same
/// `OnEnter(Won)`, so the level just finished is already in it — and a level that
/// carries no shard (a demo, or a campaign level past L7) simply shows neither
/// line.
fn draw_won(
    mut fb: ResMut<Framebuffer>,
    cursor: Res<MenuCursor>,
    readout: Res<MemoryReadout>,
    log: Res<MemoryLog>,
    // The same catalogue/current/progress trio the HUD's exit clause needs,
    // bundled for the same reason: to stay inside Bevy's argument budget.
    ctx: HudNameCtx,
    // `Option` so a focused harness that adds only `MenuRenderPlugin` (without
    // `FlowPlugin`, which owns the resource) still runs.
    ending: Option<Res<crate::flow::EndingOutcome>>,
) {
    let HudNameCtx {
        catalog,
        current,
        progress,
    } = &ctx;
    // THE FINALE STANDS DOWN THE GENERIC BANNER. `OnEnter(Won)` is still entered for
    // one frame on the finale — it is where progress is recorded and the win audio
    // fires — but `route_finale` has already queued `GameState::Ending`, so painting
    // "*** PUZZLE SOLVED ***" here would flash the generic overlay over the ending.
    if ending.is_some_and(|e| e.0.is_some()) {
        return;
    }
    let mid = fb.rows() / 2;
    // A solid banner strip so the text reads over the scene. Widen it to host
    // the readout line + a few rows of the accumulating story.
    let strip_top = mid.saturating_sub(5);
    let strip_bot = (mid + 7).min(fb.rows());
    for row in strip_top..strip_bot {
        for col in 0..fb.cols() {
            fb.set(
                col,
                row,
                Cell {
                    glyph: u32::from(' '),
                    fg: FG,
                    bg: BG,
                    ..Default::default()
                },
            );
        }
    }
    put_center(
        &mut fb,
        mid.saturating_sub(4),
        "*** PUZZLE SOLVED ***",
        0x0040_FFB0,
        BG,
    );
    put_center(
        &mut fb,
        mid.saturating_sub(3),
        &format!("level {} — the door opens", current.0 + 1),
        FG_DIM,
        BG,
    );

    // The NAME-SHARD this level yielded, announced above the readout (the door's
    // half of WORD_STORY §5 item 5). Silent on a level that carries no shard.
    let level_id = catalog.id_at(current.0).unwrap_or_default();
    if let Some(line) = shard_recovered_line(level_id) {
        put_center(&mut fb, mid.saturating_sub(2), &line, FG_SHARD, BG);
    }

    // The recovered-memory readout: this level's assembled phrase, in topo
    // order, styled distinctly (amber clean / red corrupt).
    if readout.has_phrase() {
        let mem_fg = match readout.form {
            ReadoutForm::Clean => FG_MEMORY,
            ReadoutForm::Corrupt => FG_MEMORY_CORRUPT,
        };
        put_center(
            &mut fb,
            mid.saturating_sub(1),
            "[ memory readout ]",
            FG_DIM,
            BG,
        );
        put_center(&mut fb, mid, &readout.phrase, mem_fg, BG);
    }

    // The running assembly of the true name — `true_name: pid://aleks_v@R·░·…`,
    // `[unresolved]` until all seven shards are in hand. Shown on every campaign
    // level (the name is the campaign's through-line), hidden on levels outside the
    // arc so a demo's Won screen stays clean.
    let name = NameAssembly::from_solved_ids(progress.solved.iter().map(String::as_str));
    if name.count() > 0 || shard_for_this_level(level_id) {
        put_center(&mut fb, mid + 1, &name.true_name_line(), FG_SHARD, BG);
    }

    let labels: Vec<&str> = WON_ITEMS.iter().map(|w| won_item_label(*w)).collect();
    let sel = cursor.index.min(WON_ITEMS.len() - 1);
    // inline horizontal-ish: stack the two items on consecutive rows
    let top = won_list_top(fb.rows());
    put_menu_list(&mut fb, top, &labels, sel);

    // The story so far — the last few memory-log lines beneath the menu, so the
    // player watches the vignettes accumulate into one story across levels.
    if !log.is_empty() {
        let foot_top = mid + 6;
        let max_lines = fb.rows().saturating_sub(foot_top);
        let n = log.entries.len();
        let start = n.saturating_sub(max_lines.max(1));
        for (i, entry) in log.entries[start..].iter().enumerate() {
            let row = foot_top + i;
            if row >= fb.rows() {
                break;
            }
            put_center(
                &mut fb,
                row,
                &format!("{}  {}", entry.level_id, entry.phrase),
                FG_DIM,
                BG,
            );
        }
    }
}

// ─────────────────────────────── pause ──────────────────────────────────────

/// The pause overlay (Esc during play). Like [`draw_won`], it does NOT clear the
/// framebuffer — it stamps a banner over the last-rendered (now frozen) 3D scene,
/// so the world stays visible behind it. Offers Resume / Settings / Quit-to-menu.
/// The label for one pause-overlay item.
pub(crate) fn pause_item_label(p: crate::flow::PauseItem) -> &'static str {
    match p {
        crate::flow::PauseItem::Resume => "RESUME",
        crate::flow::PauseItem::Memory => "MEMORY",
        crate::flow::PauseItem::Guide => "GUIDE",
        crate::flow::PauseItem::Settings => "SETTINGS",
        crate::flow::PauseItem::QuitToMenu => "QUIT TO MENU",
    }
}

/// The top row [`draw_pause`]'s item list starts at — shared with `menu_mouse`.
pub(crate) fn pause_list_top(rows: usize) -> usize {
    (rows / 2).saturating_sub(2)
}

/// The pause-overlay's clickable rects.
pub(crate) fn pause_rects(cols: usize, rows: usize) -> Vec<ItemRect> {
    let labels: Vec<&str> = PAUSE_ITEMS.iter().map(|p| pause_item_label(*p)).collect();
    menu_list_rects(cols, pause_list_top(rows), &labels)
}

fn draw_pause(mut fb: ResMut<Framebuffer>, cursor: Res<MenuCursor>) {
    let mid = fb.rows() / 2;
    // A solid banner strip so the menu reads over the scene. Sized to the item
    // count (four since MEMORY joined the overlay), which `put_menu_list` lays out
    // on every other row starting two above the middle.
    let strip_top = mid.saturating_sub(5);
    let strip_bot = (mid + 2 * PAUSE_ITEMS.len()).min(fb.rows());
    for row in strip_top..strip_bot {
        for col in 0..fb.cols() {
            fb.set(
                col,
                row,
                Cell {
                    glyph: u32::from(' '),
                    fg: FG,
                    bg: BG,
                    ..Default::default()
                },
            );
        }
    }
    put_center(&mut fb, mid.saturating_sub(4), "= PAUSED =", FG, BG);
    let labels: Vec<&str> = PAUSE_ITEMS.iter().map(|p| pause_item_label(*p)).collect();
    let sel = cursor.index.min(PAUSE_ITEMS.len() - 1);
    let top = pause_list_top(fb.rows());
    put_menu_list(&mut fb, top, &labels, sel);
    let hint_row = fb.rows().saturating_sub(1);
    put_center(
        &mut fb,
        hint_row,
        "W/S or ↑/↓ move  ·  E/Enter select  ·  Esc/q resume",
        FG_DIM,
        BG,
    );
}

// ───────────────────────────── in-play HUD ──────────────────────────────────

/// Idle crosshair color (a faint mark when nothing interactable is faced).
const CROSS_IDLE: u32 = 0x0060_7888;
/// Active reticle color (bright amber) — shown ONLY when the faced cell is an
/// interactable switch.
const CROSS_ACTIVE: u32 = 0x00FF_D060;
/// The objective HUD line color (system cyan), and a dimmer hint color.
const HUD_FG: u32 = 0x00C8_F0FF;
const HUD_HINT: u32 = 0x0070_90A0;
/// The narrative-beat line color — the `system_log` register's cold green, so a
/// story beat never reads as an objective (cyan) or a reticle (amber).
const HUD_BEAT: u32 = 0x0080_E0A8;
/// The `power N/M` readout: cool green while there is headroom, hot amber-red
/// the moment the grid is fully drawn. Painted as its own segment (not folded
/// into the objective line) precisely so it can change colour on its own — the
/// player must SEE the constraint arrive before a switch refuses them.
const HUD_POWER: u32 = 0x0070_D8A0;
const HUD_POWER_FULL: u32 = 0x00FF_9048;

/// The HUD row the timed narrative beat is painted on: directly under the two
/// objective rows, above the 3D view's middle where the reticle lives.
const HUD_BEAT_ROW: usize = 2;

/// The FIRST STEPS onboarding checklist's top row: two rows under
/// [`HUD_BEAT_ROW`], leaving one blank row between the (transient) narrative
/// beat and the (persistent, until complete) checklist so the two never
/// visually run together.
const FIRST_STEPS_ROW: usize = HUD_BEAT_ROW + 2;

/// The campaign's exit clause for the objective line: what the door knows about
/// who is leaving. Appended once the network is complete, on campaign levels only
/// (the name arc is the campaign's through-line; a demo door has no name to want).
///
/// Pure string work over the solved set — [`crate::story::NameAssembly`] does the
/// shard bookkeeping.
fn exit_name_clause(level_id: &str, progress: &Progress) -> Option<String> {
    if crate::flow::categorize(level_id) != crate::flow::Category::Campaign {
        return None;
    }
    let name = NameAssembly::from_solved_ids(progress.solved.iter().map(String::as_str));
    Some(format!("  ·  {}", name.door_line()))
}

/// The read-only campaign context the HUD's exit clause needs: which level is
/// being played and what the player has solved. Bundled as one [`SystemParam`] so
/// [`draw_play_hud`] stays inside the argument budget, and taken as `Option` so a
/// focused harness that adds only [`MenuRenderPlugin`] still runs.
#[derive(bevy::ecs::system::SystemParam)]
struct HudNameCtx<'w> {
    catalog: Res<'w, LevelCatalog>,
    current: Res<'w, CurrentLevel>,
    progress: Res<'w, Progress>,
}

/// The read-only state the FIRST STEPS checklist HUD block needs: whether the
/// row is enabled ([`Settings::first_steps_hud`]) and which items are already
/// ticked ([`Progress::first_steps`]). A separate bundle from [`HudNameCtx`]
/// (which couples `Progress` to the campaign catalog) — the checklist reads
/// `Progress` on its own, so it must not go missing just because a focused
/// harness omits `LevelCatalog`/`CurrentLevel`. `Option` for the same
/// "focused harness" reason as every other optional param here.
#[derive(bevy::ecs::system::SystemParam)]
struct FirstStepsHudCtx<'w> {
    progress: Res<'w, Progress>,
    settings: Res<'w, Settings>,
}

/// The in-play HUD: a center reticle + a compact objective / progress line.
///
/// Painted OVER the live 3D frame in `Last` (after the raycast + glitch, before
/// present). Two pieces:
///
/// * **Crosshair** — a faint `+` at the framebuffer center normally, switching to
///   a bright `◆` when the camera faces an INTERACTABLE switch. The active
///   predicate reuses [`faced_switch`] — the exact faced-cell logic the interact
///   path uses — so the reticle lights up precisely when an interact would land.
/// * **Objective line** — read from the live [`PuzzleState`]: the goal, the
///   `[lit/total]` door-target progress, and a hint at where to look. Drawn on the
///   top rows so it never collides with the crosshair.
/// * **Controls line** — a compact, always-visible cheat-sheet on the BOTTOM row
///   (the player reported it was unclear what to do / how to move). Painted last,
///   over the live frame, so it cannot be hidden by the raycaster.
/// * **Narrative beat** — the one `>`-prefixed line a fired `[trigger]` field put
///   up ([`NarrativeBeat`]), on [`HUD_BEAT_ROW`], gone after ~4 s.
#[allow(clippy::too_many_arguments)]
fn draw_play_hud(
    mut fb: ResMut<Framebuffer>,
    cam_q: Query<&CameraPose, With<PlayCamera>>,
    puzzle: Res<PuzzleState>,
    // Optional so a focused HUD test harness that omits the zoom plugin still runs;
    // in the real app `ZoomPlugin` always inserts both.
    zoom: Option<Res<re_runtime::zoom::ZoomState>>,
    postfx: Option<Res<re_runtime::zoom::PostFxCycleState>>,
    // The campaign name arc + the live trigger beat. `Option` for the same reason
    // as the two above: a focused harness may add only `MenuRenderPlugin`.
    name_ctx: Option<HudNameCtx>,
    beat: Option<Res<NarrativeBeat>>,
    // The loaded level's parsed data — read ONLY for the faced-object context
    // line's node-name labels (`re_runtime::context_line`). `Option` for the same
    // "focused harness" reason as the params above.
    level_data: Option<Res<crate::flow::CurrentLevelData>>,
    // The FIRST STEPS onboarding checklist's read-only state.
    // `Option` for the same "focused harness" reason as above.
    first_steps_ctx: Option<FirstStepsHudCtx>,
    // Proximity-picked context label state: the per-level hysteresis latch +
    // the live FOV the raycaster is using this frame. Both `Option` for the
    // same "focused harness" reason as above.
    proximity: Option<ResMut<re_runtime::context_line::ProximityTracker>>,
    fov: Option<Res<re_render::Fov>>,
) {
    // The door's exit clause, resolved once (used by both objective branches).
    let exit_clause = name_ctx
        .as_ref()
        .and_then(|c| {
            c.catalog
                .id_at(c.current.0)
                .map(|id| exit_name_clause(id, &c.progress))
        })
        .flatten()
        .unwrap_or_default();
    // Objective / progress line (top rows). `(lit, total)` from the live puzzle.
    // The goal + `[lit/total]` reads on row 0; a where-to-look hint on row 1.
    // Both rows are stamped with the HUD background so they never blend into
    // the 3D scene behind them, keeping the objective legible.
    // POWER READOUT (v0.2 `power_budget`). `None` ⇒ the level has no cap and the
    // HUD is byte-identical to before the rule existed. When a cap IS declared,
    // the objective line is followed by a `power N/M` segment in its own colour,
    // and the row-1 hint switches to the "free a unit" instruction the moment the
    // grid is full — so a refusal is never the first the player hears of it.
    let power = power_readout(&puzzle);
    let power_full = power.is_some_and(|(used, budget)| used >= budget);
    // Row 0 columns consumed by the objective line, so the power segment can be
    // appended after it rather than overlapping.
    let mut obj_end = 1usize;
    let power_hint = "the grid is FULL — switch a pipe OFF to free a unit before powering another";

    if let Some((locked, steps)) = sequence_progress(&puzzle) {
        // L11 sequence-lock: show the ordered checkpoints as P1✓/P2○/… so the
        // player can see how far the lock has advanced. A confirmed step is ✓,
        // the rest ○. The whole sequence done ⇒ head to the exit.
        let done = locked >= steps;
        let marks: String = (0..steps)
            .map(|i| {
                let mark = if i < locked { '✓' } else { '○' };
                format!("P{}{} ", i + 1, mark)
            })
            .collect();
        let line = if done {
            format!("SEQUENCE LOCKED — head to the exit ▒  [{marks}]{exit_clause}")
        } else {
            format!("SEQUENCE-LOCK: drive the output to each pattern IN ORDER  [{marks}]")
        };
        put_str(&mut fb, 1, 0, &line, HUD_FG, BG);
        obj_end = 1 + line.chars().count();
        // Row-1 hint: only meaningful while the sequence is still being driven.
        // Once `done`, row 0 already reads "head to the exit" — repeating the
        // mid-puzzle "a missed pattern resets…" hint underneath it would be
        // stale advice about a puzzle that is no longer live. `put_str` only
        // overwrites the cells its own text covers, and the framebuffer is NOT
        // implicitly cleared between HUD frames — simply skipping the call
        // once solved leaves the LAST-painted hint's glyphs sitting there
        // forever, which is the exact bug this branch exists to fix. Paint a
        // full-row blank instead so the row is genuinely empty once solved.
        let row1_blank = " ".repeat(fb.cols().saturating_sub(1));
        put_str(
            &mut fb,
            1,
            1,
            if done {
                &row1_blank
            } else if power_full {
                power_hint
            } else {
                "a missed pattern resets only the current step — flip switches, watch the readout"
            },
            HUD_HINT,
            BG,
        );
    } else {
        let (lit, total) = objective_progress(&puzzle);
        if total > 0 {
            let done = lit >= total;
            let line = if done {
                format!("OBJECTIVE: network complete — head to the exit ▒  [{lit}/{total}]{exit_clause}")
            } else {
                format!("OBJECTIVE: light every node — flip switches to power the network  [{lit}/{total}]")
            };
            put_str(&mut fb, 1, 0, &line, HUD_FG, BG);
            obj_end = 1 + line.chars().count();
            // Row-1 hint: only meaningful while there is still a switch worth
            // aiming at. Once `done`, row 0 already reads "head to the exit" —
            // leaving "aim at a glowing switch ◆ … press E" up underneath it
            // told a player who had already solved the puzzle to go do
            // something that no longer does anything (every node is lit, so
            // no switch flip changes the outcome). As in the sequence-lock
            // branch above, the row must be actively blanked, not skipped —
            // `put_str` never clears cells outside the text it is given, so a
            // skipped call would leave the pre-solve hint's glyphs on screen
            // forever (the exact bug being fixed here).
            let row1_blank = " ".repeat(fb.cols().saturating_sub(1));
            put_str(
                &mut fb,
                1,
                1,
                if done {
                    &row1_blank
                } else if power_full {
                    power_hint
                } else {
                    "aim at a glowing switch ◆ (the reticle brightens) and press E to flip it"
                },
                HUD_HINT,
                BG,
            );
        }
    }

    // The `power N/M` segment, appended to the objective row in its own colour.
    if let Some((used, budget)) = power {
        let seg = format!("  ·  power {used}/{budget}");
        let fg = if power_full {
            HUD_POWER_FULL
        } else {
            HUD_POWER
        };
        put_str(&mut fb, obj_end, 0, &seg, fg, BG);
    }

    // NARRATIVE BEAT — the one `>`-prefixed line a walked-into `[trigger]` field
    // put up. Painted under the objective rows and stamped with the HUD background
    // so it reads over the 3D scene; it disappears on its own after ~4 s (the
    // resource decays in `flow::trigger_subscribers`), so an unfired level's frame
    // is byte-identical to before.
    if let Some(line) = beat.as_ref().and_then(|b| b.line()) {
        put_str(&mut fb, 1, HUD_BEAT_ROW, line, HUD_BEAT, BG);
    }

    // FIRST STEPS onboarding checklist: a compact tick-list starting at
    // `FIRST_STEPS_ROW`. Empty once every item
    // is ticked or the player switched the row off in Settings — see
    // `crate::first_steps::hud_lines`. Clipped to the grid so a tiny test
    // framebuffer never panics.
    if let Some(ctx) = first_steps_ctx.as_ref() {
        let lines = crate::first_steps::hud_lines(&ctx.progress, ctx.settings.first_steps_hud);
        for (i, line) in lines.iter().enumerate() {
            let row = FIRST_STEPS_ROW + i;
            if row >= fb.rows() {
                break;
            }
            put_str(&mut fb, 1, row, line, HUD_HINT, BG);
        }
    }

    // Controls cheat-sheet — a single compact line pinned to the BOTTOM row, shown
    // every frame in Playing so movement + interaction are always discoverable.
    // `Z zoom` (the FOV-ZOOM toggle) and `G fx` (the post-fx cycle) are listed
    // alongside the existing bindings.
    let controls = "WASD move · ←→ look · Space jump · E use · [ ] FOV · Z zoom · G fx · Esc menu";
    let crow = fb.rows().saturating_sub(1);
    put_center(&mut fb, crow, controls, HUD_HINT, BG);

    // Active FOV-ZOOM / POST-FX status, top-right, so the player sees the modes
    // they toggled. Only shown when active (zoomed or a non-Off post-fx stage), so
    // the default frame is unchanged.
    let mut status = String::new();
    if zoom.as_ref().is_some_and(|z| z.zoomed) {
        status.push_str("◎ ZOOM");
    }
    let fx = postfx.as_ref().map_or("", |p| p.stage.hud_label());
    if !fx.is_empty() {
        if !status.is_empty() {
            status.push_str(" · ");
        }
        status.push_str("FX:");
        status.push_str(fx);
    }
    if !status.is_empty() {
        let col = fb.cols().saturating_sub(status.chars().count() + 1);
        put_str(&mut fb, col, 0, &status, HUD_HINT, BG);
    }

    // Proximity-picked context label: a verb+noun+state prompt floats beside
    // the NEAREST interactable in a short range+cone, not only the one
    // directly under the reticle — see
    // `crate::runtime_glue::draw_proximity_context`'s own doc. Supersedes the
    // facing-only `draw_faced_context` this HUD used to call. `proximity` is
    // `Option` for the same "focused harness" reason as the params above; the
    // real app always has it (`MenuRenderPlugin` inits it). `fov` comes from
    // the LIVE `re_render::Fov` resource (the player's own FOV/zoom setting),
    // not a fixed default, so the label projects at the same screen position
    // the raycaster itself is using this frame.
    if let (Ok(cam), Some(mut proximity)) = (cam_q.single(), proximity) {
        let level = level_data.as_ref().and_then(|d| d.0.as_ref());
        let fov_val = fov.as_deref().copied().unwrap_or_default();
        crate::runtime_glue::draw_proximity_context(
            &mut fb,
            cam,
            &puzzle,
            level,
            &mut proximity,
            &fov_val,
        );
    }

    // Crosshair: active only when the camera faces an interactable switch.
    let active = cam_q
        .single()
        .map(|cam| faced_switch(cam, &puzzle).3)
        .unwrap_or(false);
    let cx = fb.cols() / 2;
    let cy = fb.rows() / 2;
    let (glyph, fg) = if active {
        ('◆', CROSS_ACTIVE)
    } else {
        ('+', CROSS_IDLE)
    };
    if cx < fb.cols() && cy < fb.rows() {
        // Preserve the scene cell's background; only stamp the reticle glyph + fg.
        let bg = fb.get(cx, cy).map(|c| c.bg).unwrap_or(BG);
        fb.set(
            cx,
            cy,
            Cell {
                glyph: u32::from(glyph),
                fg,
                bg,
                ..Default::default()
            },
        );
    }
}

// ─────────────────────────── F3 perf overlay ────────────────────────────────

/// The F3 performance overlay (off by default; toggled via [`re_core::PerfOverlay`]).
///
/// A compact panel pinned to the TOP-RIGHT corner so it never collides with the
/// top-left objective HUD. Painted OVER the live/frozen 3D frame in `Last` (after
/// the PostUpdate composite, before re_present's present), gated by the caller's
/// `run_if` to (Playing|Pause) AND `PerfOverlay.visible`.
///
/// Pure compute — it only reads [`re_core::FrameTimingMetrics`] and writes the
/// [`Framebuffer`]. Every line is right-aligned by computing `x` from the panel
/// width; rows/cols are clamped so it never writes out of bounds on small grids
/// (`put_str` already clips, and we skip rows beyond the framebuffer).
fn draw_perf_overlay(mut fb: ResMut<Framebuffer>, m: Res<re_core::FrameTimingMetrics>) {
    // Each line is built to a fixed width; the panel width is the longest line.
    let lines = [
        format!("FPS {:5.1}  dt {:4.1}ms", m.fps_ema, m.frame_dt_us / 1000.0),
        format!("far  {:6.0}us", m.far_bake_us),
        format!("near {:6.0}us", m.near_cast_us),
        format!("sub  {:6.0}us", m.subcell_us),
        format!("emit {:6.0}us", m.emitter_us),
        format!("rslv {:6.0}us", m.resolve_us),
        // The render stages' own total is split from the terminal-write cost,
        // so a slow frame's cause (raycast/raymarch
        // stages vs. the crossterm/ratatui write in `re_present`) is legible
        // at a glance instead of hiding both inside one "pres" line.
        format!("rend {:6.0}us", m.render_total_us),
        format!("write{:6.0}us", m.present_us),
        format!("tot  {:6.0}us", m.render_total_us + m.present_us),
    ];

    let panel_w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    // Right-align: leave a 1-col right margin where the grid allows. `saturating_sub`
    // pins `x0` to 0 when the grid is narrower than the panel, so a tiny grid still
    // draws from column 0 (put_str clips the overflow).
    let cols = fb.cols();
    let x0 = cols.saturating_sub(panel_w + 1);

    for (i, line) in lines.iter().enumerate() {
        // Stamp each line with the panel BG so it stays legible over the scene.
        // Skip rows past the bottom of the grid (put_str also guards row bounds).
        if i >= fb.rows() {
            break;
        }
        // Pad to the panel width so the BG strip is a clean rectangle. The FPS
        // header line reads bright; the per-stage timings read dim.
        let padded = format!("{line:>panel_w$}");
        let fg = if i == 0 { FG } else { FG_DIM };
        put_str(&mut fb, x0, i, &padded, fg, BG);
    }
}

#[cfg(test)]
mod tests;
