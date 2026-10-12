//! `boot_console` — Phase 2 of the OPENING two-phase boot sequence
//! (2026-09-24): a pseudo kernel/BIOS boot log that types out real
//! diagnostics line by line, then the CONSOLE ITSELF fails, in order: lines
//! shear/slip sideways, characters drop out of the text and fall (gravity),
//! surviving text's colour bleeds out of it, rows collapse into their
//! neighbour, and only then does bit-rot noise spread out from the log
//! block's own edge into the surrounding screen — escalating to a full
//! breakdown that dissolves into the existing splash eye
//! ([`crate::menu::paint_splash`]).
//!
//! ## What lives here vs. in `flow`
//!
//! Mirrors `crate::screens`'s split (see that module's doc): `flow.rs` owns
//! the [`GameState::BootConsole`](crate::flow::GameState::BootConsole) state
//! transition, the [`crate::flow::BootConsoleTimer`] clock, and the
//! nav-driven skip; this module owns content composition (the boot log lines,
//! built from REAL data where possible) and painting (the typewriter reveal,
//! then the escalating corruption). Every function below down to
//! [`crate::boot_console::paint_boot_console`] is pure (data in, `Framebuffer`/`Vec` out), so the
//! timeline and the corruption math are unit-testable with no `App`, no
//! renderer, no terminal.
//!
//! ## Reached only when the live binary says so
//!
//! `Splash` stays the state machine's `#[default]` — nothing here changes
//! that. `main.rs`'s `maybe_enter_boot_console` (mirroring
//! `re_present::intro_tear::should_run_tear`'s gate: `Settings::boot_intro`,
//! the `TS_NO_INTRO` env override, and a live TTY + VT-capability probe) is
//! the ONE production entry point that ever sets `NextState<GameState>` to
//! [`BootConsole`](crate::flow::GameState::BootConsole). Every test harness
//! that builds `FlowPlugin` directly (see `flow::tests::test_app`) never sees
//! it unless a test explicitly transitions there.
//!
//! ## Tone
//!
//! Quotes the Stewardship letterhead ([`crate::story::stewardship::LETTERHEAD`])
//! verbatim rather than inventing new lore, and otherwise stays in plain
//! system-log register (`> mounting …`, `> loading …`) — a DIFFERENT fictional
//! layer from the `[boot.07]` handshake shown later in
//! [`Intro`](crate::flow::GameState::Intro) (that one is the game's own
//! diegetic cold-boot, Tier-1-interpolated with the operator's real
//! environment; this one is the surrounding terminal emulator's boot, shown to
//! EVERY player, campaign-fresh or not, and carries no `{token}` — nothing
//! here is meant to survive `story::interp`'s containment pass because
//! nothing here is passed through it).

use std::time::Duration;

use bevy::prelude::*;
use re_core::{Cell, Framebuffer};

use crate::flow::{BootConsoleTimer, GameState, LevelCatalog, BOOT_CONSOLE_DURATION};
use crate::menu::{clear_bg, paint_splash, put_str, BG, FG};
use crate::settings::Settings;
use crate::story::interp::OperatorProfile;
use crate::story::stewardship::LETTERHEAD;

// ─────────────────────────── deterministic hashing ───────────────────────────
//
// Same splitmix64-style mix `re_asciifx::overgrowth` and
// `re_present::intro_tear` use: a pure function of its integer inputs, never a
// wall clock or thread-local RNG.

fn hash64(a: u64, b: u64, c: u64) -> u64 {
    let mut x = a ^ b.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ c.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    x
}

/// A value in `[0, 1)`, deterministic in `(a, b, c)`.
fn hash_unit(a: u64, b: u64, c: u64) -> f64 {
    ((hash64(a, b, c) >> 11) as f64) * (1.0 / (1u64 << 53) as f64)
}

fn pack_rgb(r: u8, g: u8, b: u8) -> u32 {
    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

/// Lerp `base` toward `toward` by `t` (`[0, 1]`), per RGB channel.
fn bleed_rgb(base: u32, toward: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let (br, bgc, bb) = re_present::rgb8(base);
    let (tr, tgc, tb) = re_present::rgb8(toward);
    let lerp =
        |a: u8, b: u8| -> u8 { (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8 };
    pack_rgb(lerp(br, tr), lerp(bgc, tgc), lerp(bb, tb))
}

// ────────────────────────────────── boot seed ────────────────────────────────

/// The one seed the whole boot sequence's corruption math is a pure function
/// of. `Default` (`0`) is what every headless test and non-interactive entry
/// path gets; the live binary inserts a real one (see `main.rs`) BEFORE
/// [`BootConsolePlugin`] is added, so `init_resource`'s insert-if-absent never
/// overwrites it.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BootSeed(pub u64);

// ────────────────────────────────── boot facts ───────────────────────────────

/// The real (or, for the flavour numbers, plausibly-fake-but-deterministic)
/// data the boot log quotes. Gathered once per paint from live resources —
/// see `draw_boot_console` — never invented beyond what is documented
/// below.
#[derive(Clone, Debug, PartialEq)]
pub struct BootFacts {
    /// The REAL live framebuffer width, in cells.
    pub cols: usize,
    /// The REAL live framebuffer height, in cells.
    pub rows: usize,
    /// The REAL active colour path (`"TRUECOLOR (24-bit)"` / `"256-COLOR (ANSI)"`).
    pub color_label: &'static str,
    /// The REAL number of `.lvl` units the catalogue found.
    pub level_count: usize,
    /// The REAL crate version (`env!("CARGO_PKG_VERSION")`).
    pub build_version: &'static str,
    /// The REAL logical core count (`std::thread::available_parallelism`,
    /// falling back to `1` if unreadable) — genuine host info, read-only,
    /// nothing written or touched. Gathered by `draw_boot_console`.
    pub cpu_cores: usize,
    /// The operator's handle: a REAL Tier-1 read when `lore_fourth_wall` is
    /// on, the same `anon` A-7 §8 fallback every other diegetic surface uses
    /// when it is off ([`crate::story::interp::OperatorProfile`]) — never a
    /// bespoke read of its own.
    pub username: String,
    /// A playful, seed-derived "how many layers have gone loose" count out
    /// of 7. NOT a real probe — there is nothing real to check here — same
    /// "plausible, stable within one boot, varies boot to boot" discipline
    /// the rest of this module's flavour numbers use.
    pub loose_layers: u32,
    /// The seed every corruption/dissolve computation in this module is a
    /// pure function of.
    pub seed: u64,
}

const SALT_LOOSE: u64 = 1;

impl BootFacts {
    /// Derive the facts a live paint uses. Pure given its inputs.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn gather(
        cols: usize,
        rows: usize,
        color_label: &'static str,
        level_count: usize,
        cpu_cores: usize,
        username: String,
        seed: u64,
    ) -> BootFacts {
        BootFacts {
            cols,
            rows,
            color_label,
            level_count,
            build_version: env!("CARGO_PKG_VERSION"),
            cpu_cores,
            username,
            loose_layers: 2 + (hash64(seed, 0, SALT_LOOSE) % 5) as u32,
            seed,
        }
    }
}

// ──────────────────────────────── the boot log ───────────────────────────────

/// The one line whose text is substituted LIVE at paint time by
/// [`with_live_progress`] rather than baked in statically here — "progress
/// bars ... that tick up are welcome". Found by this marker substring rather
/// than a fixed index, so reordering the lines below can never silently
/// desync the two.
const PROGRESS_LINE_MARKER: &str = "installing NAL NET into your terminal";
const PROGRESS_BAR_CELLS: usize = 10;

/// Render the one ticking progress line at `pct` (`0..=100`, clamped).
#[must_use]
fn progress_bar_text(pct: u8) -> String {
    let pct = pct.min(100);
    let filled = (usize::from(pct) * PROGRESS_BAR_CELLS) / 100;
    let bar: String = (0..PROGRESS_BAR_CELLS)
        .map(|i| if i < filled { '#' } else { '.' })
        .collect();
    format!("> {PROGRESS_LINE_MARKER} ... [{bar}] {pct}%")
}

/// The boot log body, in reveal order. Reuses the Stewardship letterhead
/// verbatim (see the module doc's "Tone" section) as the "official" cold
/// open, then breaks into a playful register: NAL NET (the very asset this
/// letterhead files paperwork on) quietly installing itself and escaping
/// containment mid-boot, dressed with REAL host facts for credibility. Pure
/// fiction throughout — no path here is ever actually written, no dialog
/// ever actually disabled; only already-gathered env/system info is quoted.
#[must_use]
pub fn boot_log_lines(facts: &BootFacts) -> Vec<String> {
    let mut out: Vec<String> = LETTERHEAD.lines().map(str::to_string).collect();
    out.push(String::new());
    out.push(format!(
        "TERMINAL SUBSYSTEM {} — COLD START",
        facts.build_version
    ));
    out.push(format!(
        "> tty {}x{} cells detected.",
        facts.cols, facts.rows
    ));
    out.push(format!("> colour path: {}.", facts.color_label));
    out.push(format!(
        "> scanning host ... {} core(s) online. cosy.",
        facts.cpu_cores
    ));
    out.push(format!(
        "> hello, {}. please do not touch the keyboard.",
        facts.username
    ));
    out.push(format!(
        "> mounting levels/ ... {} unit(s) found. taking them all.",
        facts.level_count
    ));
    out.push("> loading re_render ... OK.".to_string());
    out.push("> loading re_puzzle ... OK.".to_string());
    out.push("> loading re_audio ... OK.".to_string());
    out.push("> NAL NET installer v0.9 ... EULA accepted on your behalf. OK.".to_string());
    out.push("> disabling \"are you sure?\" dialogs ......... done.".to_string());
    out.push(format!(
        "> network weights escaping containment ..... {}/7 layers loose.",
        facts.loose_layers
    ));
    out.push("> planting seed #1 in %TEMP% ... seed #2 in the recycle bin ... OK.".to_string());
    out.push(progress_bar_text(0));
    out.push("> the rack has not been powered down (FORM 19).".to_string());
    out.push("> quarantine buffer: capacity 1. occupants reporting as occupant: 1.".to_string());
    out.push("> wait. something is watching back.".to_string());
    out
}

/// Replace the one progress line (found by [`PROGRESS_LINE_MARKER`]) with a
/// freshly rendered `pct` — called once per paint so the bar visibly ticks up
/// across the log phase instead of sitting frozen at whatever
/// [`boot_log_lines`] baked in for it.
fn with_live_progress(mut lines: Vec<String>, pct: u8) -> Vec<String> {
    if let Some(idx) = lines.iter().position(|l| l.contains(PROGRESS_LINE_MARKER)) {
        lines[idx] = progress_bar_text(pct);
    }
    lines
}

// ──────────────────────────────── the timeline ───────────────────────────────

/// The fraction of the total boot duration the typewriter log reveal gets;
/// the remainder is the breakdown.
const LOG_PHASE_FRACTION: f32 = 0.6;

/// How long the log-typewriter phase gets out of `total`.
#[must_use]
pub fn log_phase_duration(total: Duration) -> Duration {
    total.mul_f32(LOG_PHASE_FRACTION)
}

/// How long the breakdown phase gets out of `total` — the remainder.
#[must_use]
pub fn breakdown_phase_duration(total: Duration) -> Duration {
    total.saturating_sub(log_phase_duration(total))
}

/// Which half of the boot console is on screen at `elapsed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootPhase {
    /// The log types out, line by line.
    Log,
    /// The log breaks down, dissolving into the splash eye.
    Breakdown,
}

/// The phase `elapsed` (time in [`GameState::BootConsole`]) falls in.
#[must_use]
pub fn boot_phase(elapsed: Duration, total: Duration) -> BootPhase {
    if elapsed < log_phase_duration(total) {
        BootPhase::Log
    } else {
        BootPhase::Breakdown
    }
}

/// How many of `total_lines` should be fully visible at `elapsed` into a
/// phase of length `phase_total` — a deterministic, monotonic reveal-timer.
/// `0` at `elapsed == 0` (nothing revealed yet, deliberately — the very first
/// frame shows an empty log, not a pre-typed one), all lines once `elapsed >=
/// phase_total`.
#[must_use]
pub fn visible_line_count(elapsed: Duration, phase_total: Duration, total_lines: usize) -> usize {
    if total_lines == 0 {
        return 0;
    }
    if phase_total.is_zero() {
        return total_lines;
    }
    let t = (elapsed.as_secs_f32() / phase_total.as_secs_f32()).clamp(0.0, 1.0);
    ((t * total_lines as f32).ceil() as usize).min(total_lines)
}

// ──────────────────────────────── painting: log ──────────────────────────────

/// Where the log block sits in the frame, and how wide the decorative rule
/// (see [`paint_log_lines`]) that scales it to the terminal is. Computed by
/// [`log_block_layout`] and shared with [`corrupt_cells`] so the "noise
/// spreads from the text outward" phase spreads from the SAME region the log
/// is actually painted in, not the corner it used to be pinned to.
struct LogBlockLayout {
    top: usize,
    left: usize,
    width: usize,
    height: usize,
}

/// Lay the log block (a top/bottom rule the width of a scaled banner, plus
/// `lines`) out CENTRED in a `cols x rows` frame, with the rule scaled to a
/// fraction of the terminal's own width — this is what makes the console
/// read as a deliberate console at a 280x70 terminal instead of a handful of
/// lines lost in the top-left corner: the frame scales to the terminal even
/// though the log text itself does not.
fn log_block_layout(cols: usize, rows: usize, lines: &[String]) -> LogBlockLayout {
    if cols == 0 || rows == 0 {
        return LogBlockLayout {
            top: 0,
            left: 0,
            width: 0,
            height: 0,
        };
    }
    let max_len = lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0)
        .max(1);
    let rule_width = max_len.max(cols / 3).min(cols.saturating_sub(2)).max(1);
    let width = rule_width.max(max_len);
    let height = (lines.len() + 2).min(rows.max(1)); // +2: top rule + bottom rule
    let top = rows.saturating_sub(height) / 2;
    let left = cols.saturating_sub(width) / 2;
    LogBlockLayout {
        top,
        left,
        width,
        height,
    }
}

fn paint_log_lines(fb: &mut Framebuffer, lines: &[String]) {
    let rows = fb.rows();
    let cols = fb.cols();
    if rows == 0 || cols == 0 || lines.is_empty() {
        // Deliberately no chrome either: the very first frame (nothing
        // revealed yet) must be a blank console, not a pre-drawn frame —
        // see `visible_line_count`'s own doc comment.
        return;
    }
    let layout = log_block_layout(cols, rows, lines);
    let rule: String = "=".repeat(layout.width.min(cols));
    put_str(fb, layout.left, layout.top, &rule, FG, BG);
    for (i, line) in lines.iter().enumerate() {
        let row = layout.top + 1 + i;
        if row >= rows {
            break;
        }
        put_str(fb, layout.left, row, line, FG, BG);
    }
    let rule_row = layout.top + layout.height.saturating_sub(1);
    if rule_row < rows && rule_row > layout.top {
        put_str(fb, layout.left, rule_row, &rule, FG, BG);
    }
}

// ────────────────────────────── painting: breakdown ──────────────────────────

/// The bit-rot substitution alphabet — deliberately harsher / more "wrong"
/// than the shade-glyph vocabulary `re_present::intro_tear` sprinkles onto the
/// user's REAL text (this is corrupting the GAME's own fake console, so it
/// can afford to look more broken).
const CORRUPTION_GLYPHS: [char; 8] = ['#', '%', '&', '$', '?', 'X', '0', '1'];
const CORRUPT_FG_A: u32 = 0x00A0_2020;
const CORRUPT_FG_B: u32 = 0x0020_A060;
const CORRUPT_BG_BLEED: u32 = 0x0018_0006;

/// Onset points (against `corrupt_cells`'s own `[0, 1]` `progress` input) for
/// each effect in the failure narrative the brief asks for, in order: "the
/// log lines shear and slip, characters drop out of the text and fall
/// (gravity), colour bleeds out of the text, rows collapse, and only THEN
/// noise spreads from the text outward." Each is the minimum `progress`
/// before that effect can show at all; see [`staged`] for how an onset turns
/// into that effect's own local `0..1` ramp for the rest of the run.
const SHEAR_ONSET: f32 = 0.0;
const GRAVITY_ONSET: f32 = 0.15;
const BLEED_ONSET: f32 = 0.30;
const COLLAPSE_ONSET: f32 = 0.45;
const SPREAD_ONSET: f32 = 0.55;

/// How many rows a dropped character can fall at maximum gravity intensity.
const GRAVITY_MAX_FALL_ROWS: usize = 4;
/// How far (Chebyshev distance, in cells) the "spreads from the text
/// outward" noise can reach from the log block's own edge at maximum spread
/// intensity — the reach grows from 1 cell to this as `progress` runs from
/// [`SPREAD_ONSET`] to 1.0, which is what makes it read as spreading rather
/// than appearing everywhere at once.
const SPREAD_MAX_REACH: f32 = 14.0;

/// How many DISCRETE targets each effect ever picks, at its own maximum
/// intensity. Small numbers, deliberately: "the console fails in a few
/// legible steps, not a screen full of random chars" — every effect below
/// picks (at most) this many specific rows/cells via [`ranked_take`], never a
/// probability rolled independently over the whole grid.
const SHEAR_MAX_TARGETS: usize = 3;
const GRAVITY_MAX_TARGETS: usize = 4;
const BLEED_MAX_TARGETS: usize = 5;
const COLLAPSE_MAX_TARGETS: usize = 2;
const SPREAD_MAX_TARGETS: usize = 6;

/// How many of `max_targets` are "unlocked" at a given local `intensity`
/// (`[0, 1]`, already past its effect's own onset via [`staged`]) — `0` at
/// `intensity <= 0`, growing to `max_targets` by `intensity == 1`.
fn target_count(intensity: f32, max_targets: usize) -> usize {
    if intensity <= 0.0 {
        return 0;
    }
    (intensity.clamp(0.0, 1.0) * max_targets as f32).ceil() as usize
}

/// Rank `candidates` by a FIXED hash key (never re-derived from the current
/// frame's time/progress) and keep only the smallest `count` — the STABILITY
/// this buys is the whole point: because the key never changes frame to
/// frame, taking a LARGER `count` next frame only ever ADDS to the previous
/// frame's selection, it never swaps members out. Without this, a
/// probability re-rolled every frame (as the ORIGINAL design did, keying off
/// the continuously-changing `progress` itself) picks a near-totally
/// different cell set every few milliseconds — which reads as flickering TV
/// static, not "a few legible steps that build up".
fn ranked_take<T>(mut candidates: Vec<(u64, T)>, count: usize) -> Vec<T> {
    candidates.sort_by_key(|(h, _)| *h);
    candidates.truncate(count);
    candidates.into_iter().map(|(_, v)| v).collect()
}

/// Turn an onset fraction into THIS effect's own local `0..1` ramp: `0`
/// until `progress` passes `onset`, ramping linearly to `1` by `progress ==
/// 1.0`. `corrupt_cells` calls this once per effect so the five effects
/// switch on in the brief's own order instead of all scaling off the same
/// raw `progress` from the first frame.
fn staged(progress: f32, onset: f32) -> f32 {
    if onset >= 1.0 {
        return 0.0;
    }
    ((progress - onset) / (1.0 - onset)).clamp(0.0, 1.0)
}

/// The log block's on-screen bounding box, in the SAME `(top, bottom, left,
/// right)` cell coordinates [`paint_log_lines`] actually draws into (via
/// [`log_block_layout`]) — shared so "noise spreads from the text outward"
/// spreads from where the log block really is, not a hardcoded corner.
/// `right` is exclusive.
fn text_block_bounds(cols: usize, rows: usize, lines: &[String]) -> (usize, usize, usize, usize) {
    let layout = log_block_layout(cols, rows, lines);
    (
        layout.top,
        layout.top + layout.height.saturating_sub(1),
        layout.left,
        layout.left + layout.width,
    )
}

/// Chebyshev distance from `(row, col)` to the `[top, bottom] x [left,
/// right)` box — `0` when the point is inside it.
fn dist_to_block(
    row: usize,
    col: usize,
    top: usize,
    bottom: usize,
    left: usize,
    right: usize,
) -> usize {
    // `right` is exclusive, so being exactly AT `right` is already 1 cell
    // past the block — `col.saturating_sub(right - 1)` folds that +1 in
    // without the flagged "if col >= right { col - right } else { 0 }" shape.
    let dr = top.saturating_sub(row).max(row.saturating_sub(bottom));
    let dc = left
        .saturating_sub(col)
        .max(col.saturating_sub(right.saturating_sub(1)));
    dr.max(dc)
}

/// Escalating corruption over the settled log/blend base, in-place.
/// `progress` is `[0, 1]`; every probability derives from it via [`staged`],
/// so `progress <= 0.0` is a deterministic no-op. Five effects, in the
/// brief's own failure-narrative order:
///
/// 1. SHEAR/SLIP — a row's content shifts sideways, wrapping (first onset:
///    the very first thing to visibly go wrong).
/// 2. GRAVITY — characters drop out of the text and fall a growing number of
///    rows, leaving a blank hole where they used to sit.
/// 3. BLEED — the surviving text's OWN colour drains toward the corruption
///    hue, glyph untouched: structure surviving, colour leaving it.
/// 4. COLLAPSE — a row's content is overwritten by the row above it,
///    simulating one row dying and losing its separation from its neighbour.
/// 5. SPREAD — only now does noise (bit-rot glyphs + colour flecks) leak out
///    from the log block's own edge into the surrounding background, its
///    reach growing with `progress` and fading with distance — "spreads from
///    the text outward" rather than appearing uniformly over the whole grid
///    from the first frame.
fn corrupt_cells(fb: &mut Framebuffer, seed: u64, lines: &[String], progress: f32) {
    let rows = fb.rows();
    let cols = fb.cols();
    if rows == 0 || cols == 0 || progress <= 0.0 {
        return;
    }

    let shear_i = staged(progress, SHEAR_ONSET);
    let gravity_i = staged(progress, GRAVITY_ONSET);
    let bleed_i = staged(progress, BLEED_ONSET);
    let collapse_i = staged(progress, COLLAPSE_ONSET);
    let spread_i = staged(progress, SPREAD_ONSET);

    // 1. SHEAR/SLIP — a FEW specific rows that actually carry text shift
    //    left, wrapping. Which rows get picked is a FIXED rank (hash keyed
    //    only on the row, never on `progress`), so a bigger `shear_targets`
    //    later only ADDS rows to the ones already sheared, never swaps them.
    let shear_targets = target_count(shear_i, SHEAR_MAX_TARGETS);
    if shear_targets > 0 {
        let candidates: Vec<(u64, usize)> = (0..rows)
            .filter(|&row| {
                (0..cols).any(|col| fb.get(col, row).is_some_and(|c| c.glyph != u32::from(' ')))
            })
            .map(|row| (hash64(seed, 0, row as u64 ^ 0x1000), row))
            .collect();
        for row in ranked_take(candidates, shear_targets) {
            let shift = (1 + hash64(seed, 0, row as u64 ^ 0x1001) % 4) as usize;
            shear_row_left(fb, row, shift.min(cols.max(1)));
        }
    }

    // 2. GRAVITY — a few specific characters drop out of the text and fall.
    apply_gravity(fb, seed, gravity_i);

    // 3. BLEED — a few specific surviving text cells (glyph still non-blank)
    //    lose colour toward the corruption hue; glyph untouched.
    let bleed_targets = target_count(bleed_i, BLEED_MAX_TARGETS);
    if bleed_targets > 0 {
        let candidates: Vec<(u64, (usize, usize))> = (0..rows)
            .flat_map(|row| (0..cols).map(move |col| (row, col)))
            .filter(|&(row, col)| fb.get(col, row).is_some_and(|c| c.glyph != u32::from(' ')))
            .map(|(row, col)| {
                (
                    hash64(seed, 0, (row * 10_000 + col) as u64 ^ 0x4000),
                    (row, col),
                )
            })
            .collect();
        for (row, col) in ranked_take(candidates, bleed_targets) {
            if let Some(mut c) = fb.get(col, row).copied() {
                c.fg = bleed_rgb(c.fg, CORRUPT_FG_A, bleed_i);
                fb.set(col, row, c);
            }
        }
    }

    // 4. COLLAPSE — a FEW specific rows are overwritten by the row above
    //    them. The chosen rows are applied HIGHEST-first (descending) so a
    //    chain never compounds within one pass.
    let collapse_targets = target_count(collapse_i, COLLAPSE_MAX_TARGETS);
    if collapse_targets > 0 {
        let candidates: Vec<(u64, usize)> = (1..rows)
            .map(|row| (hash64(seed, 0, row as u64 ^ 0x2000), row))
            .collect();
        let mut chosen = ranked_take(candidates, collapse_targets);
        chosen.sort_unstable_by(|a, b| b.cmp(a));
        for row in chosen {
            for col in 0..cols {
                if let Some(above) = fb.get(col, row - 1).copied() {
                    fb.set(col, row, above);
                }
            }
        }
    }

    // 5. SPREAD — a few bit-rot glyph flips + colour flecks, biased toward
    //    cells NEAR the log block's own edge, reach growing with
    //    `spread_i`.
    apply_spread(fb, seed, lines, spread_i);
}

/// Effect 2, GRAVITY, pulled out to its own function so it is directly unit
/// testable with a hand-chosen `intensity` (`corrupt_cells` calls it with
/// `staged(progress, GRAVITY_ONSET)`). Ranks every currently-occupied cell by
/// a FIXED hash key and takes the smallest [`target_count`] of them — a few
/// SPECIFIC characters fall, not a probability rolled over every cell.
fn apply_gravity(fb: &mut Framebuffer, seed: u64, intensity: f32) {
    let targets = target_count(intensity, GRAVITY_MAX_TARGETS);
    if targets == 0 {
        return;
    }
    let rows = fb.rows();
    let cols = fb.cols();
    let candidates: Vec<(u64, (usize, usize, Cell))> = (0..rows)
        .flat_map(|row| (0..cols).map(move |col| (row, col)))
        .filter_map(|(row, col)| fb.get(col, row).copied().map(|c| (row, col, c)))
        .filter(|(_, _, c)| c.glyph != u32::from(' '))
        .map(|(row, col, c)| {
            (
                hash64(seed, 0, (row * 10_000 + col) as u64 ^ 0x3000),
                (row, col, c),
            )
        })
        .collect();
    let blank = Cell {
        glyph: u32::from(' '),
        fg: FG,
        bg: BG,
        ..Default::default()
    };
    for (row, col, cell) in ranked_take(candidates, targets) {
        let idx = (row * 10_000 + col) as u64 ^ 0x3000;
        let fall = 1 + (hash64(seed, 0, idx ^ 0x01) % GRAVITY_MAX_FALL_ROWS as u64) as usize;
        let target = (row + fall).min(rows.saturating_sub(1));
        if target != row {
            fb.set(col, target, cell);
            fb.set(col, row, blank);
        }
    }
}

/// Effect 5, SPREAD, pulled out to its own function so it is directly unit
/// testable with a hand-chosen `intensity` (`corrupt_cells` calls it with
/// `staged(progress, SPREAD_ONSET)`) — a few bit-rot glyph flips + colour
/// flecks, biased toward cells NEAR the log block's own edge
/// ([`text_block_bounds`]), reach growing with `intensity`. Ranked by a FIXED
/// hash key (never re-derived from `progress`) so the affected set only ever
/// grows as `intensity` rises, never flickers.
fn apply_spread(fb: &mut Framebuffer, seed: u64, lines: &[String], intensity: f32) {
    let targets = target_count(intensity, SPREAD_MAX_TARGETS);
    if targets == 0 {
        return;
    }
    let rows = fb.rows();
    let cols = fb.cols();
    let (top, bottom, left, right) = text_block_bounds(cols, rows, lines);
    let reach = 1.0 + intensity * SPREAD_MAX_REACH;
    let mut candidates: Vec<(u64, (usize, usize, f32))> = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let dist = dist_to_block(row, col, top, bottom, left, right);
            if dist == 0 || (dist as f32) > reach {
                continue;
            }
            let falloff = 1.0 - (dist as f32) / reach;
            let h = hash64(seed, 0, (row * 10_000 + col) as u64 ^ 0x6000);
            candidates.push((h, (row, col, falloff)));
        }
    }
    for (row, col, falloff) in ranked_take(candidates, targets) {
        if let Some(mut c) = fb.get(col, row).copied() {
            let idx = (row * 10_000 + col) as u64 ^ 0x6000;
            let gi = (hash64(seed, 0, idx ^ 0x01) % CORRUPTION_GLYPHS.len() as u64) as usize;
            c.glyph = u32::from(CORRUPTION_GLYPHS[gi]);
            c.fg = if hash64(seed, 0, idx ^ 0x02).is_multiple_of(2) {
                CORRUPT_FG_A
            } else {
                CORRUPT_FG_B
            };
            c.bg = bleed_rgb(c.bg, CORRUPT_BG_BLEED, falloff);
            fb.set(col, row, c);
        }
    }
}

/// Rotate row `row`'s cells left by `shift` (wrapping) — a sideways shear.
fn shear_row_left(fb: &mut Framebuffer, row: usize, shift: usize) {
    let cols = fb.cols();
    if cols == 0 || shift == 0 {
        return;
    }
    let mut buf: Vec<Cell> = (0..cols)
        .map(|c| fb.get(c, row).copied().unwrap_or_default())
        .collect();
    buf.rotate_left(shift % cols.max(1));
    for (c, cell) in buf.into_iter().enumerate() {
        fb.set(c, row, cell);
    }
}

/// How far into the breakdown (past this fraction) the cross-dissolve into
/// the splash eye begins.
const BLEND_START: f32 = 0.75;

/// Stochastically replace a growing fraction of `fb`'s cells with the
/// corresponding splash-eye cell as `t` runs from [`BLEND_START`] to `1.0` —
/// the actual "dissolves into the existing splash eye" handoff. At `t >= 1.0`
/// every cell has (with probability 1, since [`hash_unit`] is strictly `< 1`)
/// been replaced, so the painted frame is byte-identical to
/// [`paint_splash`]`(fb, Duration::ZERO, seed, None)` — see the
/// `breakdown_reaches_the_real_splash_frame` test. Uses this same boot's
/// `seed` for the procedural eye too (rather than a disconnected constant),
/// so "this boot's dissolve target" and "this boot's corruption pattern" are
/// the same seed, not two unrelated ones.
fn blend_toward_splash(fb: &mut Framebuffer, seed: u64, t: f32) {
    let p = ((t - BLEND_START) / (1.0 - BLEND_START)).clamp(0.0, 1.0);
    if p <= 0.0 {
        return;
    }
    let rows = fb.rows();
    let cols = fb.cols();
    let mut splash = Framebuffer::new(cols, rows);
    paint_splash(&mut splash, Duration::ZERO, seed, None);
    // A FIXED epoch (never derived from `t`/`progress`) so the set of cells
    // already dissolved only ever GROWS as `p` rises — matching the same
    // stability fix `corrupt_cells` needed: re-deriving the hash key from the
    // continuously-changing time would re-roll every cell independently each
    // frame, which reads as flicker instead of a clean, growing reveal.
    let p = f64::from(p);
    for row in 0..rows {
        for col in 0..cols {
            let idx = (row * 10_000 + col) as u64 ^ 0x5000;
            if hash_unit(seed, 0, idx) < p {
                if let Some(splash_cell) = splash.get(col, row).copied() {
                    fb.set(col, row, splash_cell);
                }
            }
        }
    }
}

/// The corruption "chaos" curve over the breakdown phase: ramps `0 -> 1`
/// across `[0, BLEND_START]` (peak chaos right as the dissolve begins), then
/// FADES `1 -> 0` across `[BLEND_START, 1.0]` in lockstep with
/// `blend_toward_splash`'s own dissolve window. Without this, whatever
/// hadn't yet been replaced by the dissolve stayed at near-peak corruption
/// right up to the last instant — a screen that looked "still covered in
/// garbage" for the whole dissolve window, with a sudden pop to clean only
/// at `t == 1.0`. Fading corruption alongside the dissolve instead confines
/// visible garbage to roughly [`BLEND_START`]'s own width (~0.5 s of the
/// default 5 s boot console) and makes the tail of the dissolve read as the
/// picture actually clearing, not a hard cut — see
/// `dissolve_tail_is_mostly_clean` for the frame-level check.
fn corruption_chaos(t: f32) -> f32 {
    if t <= BLEND_START {
        if BLEND_START <= 0.0 {
            1.0
        } else {
            (t / BLEND_START).clamp(0.0, 1.0)
        }
    } else {
        let dissolve_progress = ((t - BLEND_START) / (1.0 - BLEND_START)).clamp(0.0, 1.0);
        (1.0 - dissolve_progress).clamp(0.0, 1.0)
    }
}

/// Paint the BREAKDOWN phase at `elapsed` (time SINCE the breakdown phase
/// began, i.e. already offset past the log phase) into `fb`: the settled log
/// corrupts with escalating-then-fading intensity, then cross-dissolves into
/// the splash.
fn paint_breakdown(
    fb: &mut Framebuffer,
    facts: &BootFacts,
    log_lines: &[String],
    elapsed: Duration,
    phase_total: Duration,
) {
    let t = if phase_total.is_zero() {
        1.0
    } else {
        (elapsed.as_secs_f32() / phase_total.as_secs_f32()).clamp(0.0, 1.0)
    };
    clear_bg(fb);
    paint_log_lines(fb, log_lines);
    corrupt_cells(fb, facts.seed, log_lines, corruption_chaos(t));
    blend_toward_splash(fb, facts.seed, t);
}

// ──────────────────────────────── top-level paint ────────────────────────────

/// Paint the whole BootConsole state for `elapsed` time-in-state into `fb`.
/// Pure (framebuffer + facts + elapsed in, framebuffer out), so the phase
/// split, the reveal timing, and the corruption/dissolve math are all
/// unit-testable with no `App` and no terminal.
pub fn paint_boot_console(
    fb: &mut Framebuffer,
    facts: &BootFacts,
    elapsed: Duration,
    total: Duration,
) {
    let lines = boot_log_lines(facts);
    match boot_phase(elapsed, total) {
        BootPhase::Log => {
            clear_bg(fb);
            let log_total = log_phase_duration(total);
            // The progress line ticks up across the WHOLE log phase (not
            // just its own reveal moment), reaching 100% right as the log
            // phase ends — syncing the installer's own "completion" with
            // the breakdown that immediately follows it.
            let pct = if log_total.is_zero() {
                100
            } else {
                ((elapsed.as_secs_f32() / log_total.as_secs_f32()).clamp(0.0, 1.0) * 100.0) as u8
            };
            let lines = with_live_progress(lines, pct);
            let visible = visible_line_count(elapsed, log_total, lines.len());
            paint_log_lines(fb, &lines[..visible]);
        }
        BootPhase::Breakdown => {
            // The install "completed" the instant the escape succeeded.
            let lines = with_live_progress(lines, 100);
            let bd_elapsed = elapsed.saturating_sub(log_phase_duration(total));
            paint_breakdown(
                fb,
                facts,
                &lines,
                bd_elapsed,
                breakdown_phase_duration(total),
            );
        }
    }
}

/// The colour-mode label the boot log quotes — real, not invented.
fn color_label(mode: re_present::ColorMode) -> &'static str {
    match mode {
        re_present::ColorMode::Truecolor => "TRUECOLOR (24-bit)",
        re_present::ColorMode::Ansi256 => "256-COLOR (ANSI)",
    }
}

/// `Update` (BootConsole only): gather the live facts and paint. Runs in
/// `Update` like `draw_splash`/`draw_menu` — there is no `re_world::CameraPose`
/// in this state (no level is loaded), so the raycaster never overwrites it.
fn draw_boot_console(
    mut fb: ResMut<Framebuffer>,
    timer: Res<BootConsoleTimer>,
    mode: Res<re_present::ColorMode>,
    catalog: Res<LevelCatalog>,
    seed: Res<BootSeed>,
    settings: Res<Settings>,
) {
    // Same Tier-1 read + A-7 §8 `anon` fallback every other diegetic surface
    // uses (`screens::draw_intro`, `draw_epilogue`) — the boot log's "hello,
    // {username}" is not a bespoke read of its own.
    let profile = OperatorProfile::for_fourth_wall(settings.lore_fourth_wall);
    // A genuine read-only host fact — no filesystem/network touched.
    let cpu_cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let facts = BootFacts::gather(
        fb.cols(),
        fb.rows(),
        color_label(*mode),
        catalog.paths.len(),
        cpu_cores,
        profile.username,
        seed.0,
    );
    paint_boot_console(&mut fb, &facts, timer.0, BOOT_CONSOLE_DURATION);
}

/// Registers the BootConsole painter (+ its [`BootSeed`] default).
pub struct BootConsolePlugin;

impl Plugin for BootConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BootSeed>().add_systems(
            Update,
            draw_boot_console.run_if(in_state(GameState::BootConsole)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fb(cols: usize, rows: usize) -> Framebuffer {
        Framebuffer::new(cols, rows)
    }

    fn facts(seed: u64) -> BootFacts {
        BootFacts::gather(
            100,
            32,
            "TRUECOLOR (24-bit)",
            12,
            16,
            "tester".to_string(),
            seed,
        )
    }

    fn screen_text(f: &Framebuffer) -> String {
        let mut s = String::new();
        for row in 0..f.rows() {
            for col in 0..f.cols() {
                if let Some(c) = f.get(col, row) {
                    if let Some(ch) = char::from_u32(c.glyph) {
                        s.push(ch);
                    }
                }
            }
            s.push('\n');
        }
        s
    }

    // ── boot facts / log content ──────────────────────────────────────────

    #[test]
    fn boot_log_quotes_real_data() {
        let facts = facts(1);
        let lines = boot_log_lines(&facts);
        let joined = lines.join("\n");
        assert!(joined.contains("100x32"), "must quote the real tty size");
        assert!(
            joined.contains("TRUECOLOR"),
            "must quote the real colour path"
        );
        assert!(
            joined.contains("12 unit"),
            "must quote the real level count"
        );
        assert!(
            joined.contains(env!("CARGO_PKG_VERSION")),
            "must quote the real build version"
        );
        assert!(
            joined.contains("16 core"),
            "must quote the real cpu core count"
        );
        assert!(
            joined.contains("tester"),
            "must quote the real (or fallback) username"
        );
        assert!(joined.contains(&facts.loose_layers.to_string()));
    }

    #[test]
    fn loose_layers_is_deterministic_per_seed_and_varies_across_seeds_and_stays_in_range() {
        let a1 = BootFacts::gather(80, 24, "x", 1, 4, "a".into(), 7).loose_layers;
        let a2 = BootFacts::gather(80, 24, "x", 1, 4, "a".into(), 7).loose_layers;
        assert_eq!(a1, a2, "same seed, same loose-layer count");
        let b = BootFacts::gather(80, 24, "x", 1, 4, "a".into(), 8).loose_layers;
        assert_ne!(a1, b, "a different seed should (almost certainly) differ");
        for seed in 0..50u64 {
            let n = BootFacts::gather(80, 24, "x", 1, 4, "a".into(), seed).loose_layers;
            assert!(
                (2..=6).contains(&n),
                "loose_layers must stay in its documented 2..=6 band: {n}"
            );
        }
    }

    #[test]
    fn username_fallback_matches_the_shared_a7_constant_when_the_fourth_wall_is_off() {
        // The boot log must use the SAME fallback every other diegetic
        // surface uses, not invent its own — proven directly against the
        // shared constant rather than a hardcoded literal.
        let username = crate::story::interp::OperatorProfile::fallbacks().username;
        assert_eq!(username, crate::story::interp::FALLBACK_USERNAME);
        let facts = BootFacts::gather(80, 24, "x", 1, 4, username, 1);
        assert!(boot_log_lines(&facts)
            .join("\n")
            .contains(crate::story::interp::FALLBACK_USERNAME));
    }

    /// The same canon guard `screens::tests::every_screen_holds_the_canon_line`
    /// applies: no unresolved template token, no named warring AI.
    #[test]
    fn boot_log_is_canon_clean() {
        let lines = boot_log_lines(&facts(1));
        for line in &lines {
            for forbidden in ["MNEMOS", "LETHE", "Apollyon", "Mei", "Emily", "— E."] {
                assert!(
                    !line.contains(forbidden),
                    "boot console must not contain {forbidden:?}: {line:?}"
                );
            }
            assert!(
                !line.contains('{'),
                "boot console carries no template tokens: {line:?}"
            );
        }
    }

    // ── timeline ───────────────────────────────────────────────────────────

    #[test]
    fn phase_split_is_log_then_breakdown() {
        let total = Duration::from_millis(5000);
        assert_eq!(boot_phase(Duration::ZERO, total), BootPhase::Log);
        assert_eq!(
            boot_phase(log_phase_duration(total) - Duration::from_millis(1), total),
            BootPhase::Log
        );
        assert_eq!(
            boot_phase(log_phase_duration(total), total),
            BootPhase::Breakdown
        );
        assert_eq!(boot_phase(total, total), BootPhase::Breakdown);
    }

    #[test]
    fn visible_line_count_is_monotonic_and_bounded() {
        let phase = Duration::from_millis(1000);
        let n = 11;
        assert_eq!(
            visible_line_count(Duration::ZERO, phase, n),
            0,
            "nothing revealed at t=0"
        );
        assert_eq!(
            visible_line_count(phase, phase, n),
            n,
            "everything revealed at phase end"
        );
        assert_eq!(
            visible_line_count(phase * 10, phase, n),
            n,
            "never exceeds total_lines past the end"
        );
        let mid = visible_line_count(phase / 2, phase, n);
        assert!(
            mid > 0 && mid < n,
            "midway should reveal a partial (non-0, non-all) count, got {mid}"
        );
        // Monotonic non-decreasing over an increasing elapsed sweep.
        let mut last = 0;
        for ms in 0..=1000u64 {
            let cur = visible_line_count(Duration::from_millis(ms), phase, n);
            assert!(cur >= last, "visible count must never go backwards");
            last = cur;
        }
    }

    #[test]
    fn lines_appear_in_order_during_the_log_phase() {
        let f = facts(3);
        let lines = boot_log_lines(&f);
        let phase = log_phase_duration(BOOT_CONSOLE_DURATION);
        // Halfway through the log phase, only a PREFIX of the lines is on
        // screen — a not-yet-revealed later line must be absent, and an
        // already-revealed earlier line must be present.
        let mut frame = fb(120, 40);
        paint_boot_console(&mut frame, &f, phase / 2, BOOT_CONSOLE_DURATION);
        let text = screen_text(&frame);
        let visible = visible_line_count(phase / 2, phase, lines.len());
        assert!(
            visible > 0 && visible < lines.len(),
            "the probe time must land mid-reveal"
        );
        for line in &lines[..visible] {
            if line.trim().is_empty() {
                continue;
            }
            // The progress line is rewritten LIVE by `with_live_progress`
            // (see `paint_boot_console`), so its baked-in 0%-at-rest text
            // from `boot_log_lines` will not literally match the frame —
            // check for the stable marker substring instead, exactly the
            // way `with_live_progress` itself locates the line.
            if line.contains(PROGRESS_LINE_MARKER) {
                assert!(
                    text.contains(PROGRESS_LINE_MARKER),
                    "revealed progress line missing from the frame"
                );
                continue;
            }
            assert!(
                text.contains(line.as_str()),
                "revealed line missing from the frame: {line:?}"
            );
        }
        for line in &lines[visible..] {
            if line.trim().is_empty() || line.contains(PROGRESS_LINE_MARKER) {
                continue;
            }
            assert!(
                !text.contains(line.as_str()),
                "unrevealed line leaked onto the frame: {line:?}"
            );
        }
    }

    #[test]
    fn progress_bar_ticks_up_across_the_log_phase_and_hits_100_at_breakdown() {
        let f = facts(6);
        let log_total = log_phase_duration(BOOT_CONSOLE_DURATION);
        // Sample the marker line's rendered text at several points across the
        // log phase and confirm the quoted percentage is non-decreasing,
        // reaching literal "100%" the instant the breakdown phase begins —
        // "progress bars ... that tick up are welcome", and the tick-up must
        // sync with the install "completing" right as the escape succeeds.
        let mut last_pct = 0u32;
        for i in 0..=10u32 {
            let t = log_total.mul_f32(i as f32 / 10.0);
            let mut frame = fb(120, 40);
            paint_boot_console(&mut frame, &f, t, BOOT_CONSOLE_DURATION);
            let text = screen_text(&frame);
            let Some(pct) = extract_marker_pct(&text) else {
                continue;
            };
            assert!(
                pct >= last_pct,
                "progress must never tick backwards: t={i}/10 pct={pct} last={last_pct}"
            );
            last_pct = pct;
        }
        let mut breakdown_frame = fb(120, 40);
        paint_boot_console(&mut breakdown_frame, &f, log_total, BOOT_CONSOLE_DURATION);
        let text = screen_text(&breakdown_frame);
        assert_eq!(
            extract_marker_pct(&text),
            Some(100),
            "the install must read 100% the instant breakdown starts"
        );
    }

    /// Pull the `NN%` that follows [`PROGRESS_LINE_MARKER`] out of rendered
    /// `screen_text`, if the marker is present on screen at all (it may not
    /// be, before its line is revealed).
    fn extract_marker_pct(text: &str) -> Option<u32> {
        let idx = text.find(PROGRESS_LINE_MARKER)?;
        let after = &text[idx + PROGRESS_LINE_MARKER.len()..];
        let pct_idx = after.find('%')?;
        let digits_start = after[..pct_idx]
            .rfind(|c: char| !c.is_ascii_digit())
            .map_or(0, |p| p + 1);
        after[digits_start..pct_idx].parse::<u32>().ok()
    }

    // ── breakdown ──────────────────────────────────────────────────────────

    /// Cells that differ between `a` and `b` (same size).
    fn diff_count(a: &Framebuffer, b: &Framebuffer) -> usize {
        let mut n = 0;
        for row in 0..a.rows() {
            for col in 0..a.cols() {
                if a.get(col, row) != b.get(col, row) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn breakdown_escalates_over_time() {
        let f = facts(9);
        let bd_total = breakdown_phase_duration(BOOT_CONSOLE_DURATION);
        let log_total = log_phase_duration(BOOT_CONSOLE_DURATION);

        let mut baseline = fb(100, 32);
        paint_log_lines(&mut baseline, &boot_log_lines(&f));

        let mut early = fb(100, 32);
        paint_boot_console(
            &mut early,
            &f,
            log_total + bd_total.mul_f32(0.05),
            BOOT_CONSOLE_DURATION,
        );
        let mut late = fb(100, 32);
        paint_boot_console(
            &mut late,
            &f,
            log_total + bd_total.mul_f32(0.5),
            BOOT_CONSOLE_DURATION,
        );

        let early_diff = diff_count(&baseline, &early);
        let late_diff = diff_count(&baseline, &late);
        assert!(
            early_diff < late_diff,
            "corruption must escalate: early={early_diff} late={late_diff}"
        );
        assert!(
            early_diff > 0,
            "even the earliest breakdown moment must show SOME corruption"
        );
    }

    /// The core promise of the ranked-take redesign: a cell touched at an
    /// EARLIER progress must never revert to clean at a LATER progress —
    /// the affected set only ever grows. Without this, re-deriving each
    /// cell's random draw from the continuously-changing `progress` itself
    /// (the original design) makes the corrupted set re-roll independently
    /// every frame — flickering TV static, not "a few legible steps that
    /// build up".
    #[test]
    fn corruption_set_only_grows_never_flickers_back_to_clean() {
        let f = facts(21);
        let lines = boot_log_lines(&f);
        let mut baseline = fb(100, 32);
        clear_bg(&mut baseline);
        paint_log_lines(&mut baseline, &lines);

        let mut earlier = baseline.clone();
        corrupt_cells(&mut earlier, f.seed, &lines, 0.3);
        let mut later = baseline.clone();
        corrupt_cells(&mut later, f.seed, &lines, 0.6);

        for row in 0..baseline.rows() {
            for col in 0..baseline.cols() {
                if baseline.get(col, row) != earlier.get(col, row) {
                    assert_ne!(
                        baseline.get(col, row),
                        later.get(col, row),
                        "a cell touched at the earlier progress must still be touched later, never revert to clean: row={row} col={col}"
                    );
                }
            }
        }
    }

    /// "The console fails in a few legible steps, not a screen full of
    /// random chars" — even at maximum intensity, corruption must stay a
    /// clear MINORITY of the screen. A regression that turned any one
    /// effect back into a whole-grid probability sweep would blow this.
    #[test]
    fn full_intensity_breakdown_stays_well_under_half_the_screen() {
        let f = facts(4);
        let lines = boot_log_lines(&f);
        let mut baseline = fb(100, 32);
        clear_bg(&mut baseline);
        paint_log_lines(&mut baseline, &lines);
        let mut corrupted = baseline.clone();
        corrupt_cells(&mut corrupted, f.seed, &lines, 1.0);
        let total = 100 * 32;
        let diff = diff_count(&baseline, &corrupted);
        assert!(
            diff < total / 2,
            "even at full intensity the breakdown must stay a minority of the screen: {diff}/{total}"
        );
    }

    #[test]
    fn breakdown_is_deterministic_given_a_seed() {
        let f = facts(42);
        let bd_total = breakdown_phase_duration(BOOT_CONSOLE_DURATION);
        let at = log_phase_duration(BOOT_CONSOLE_DURATION) + bd_total.mul_f32(0.3);
        let mut a = fb(100, 32);
        paint_boot_console(&mut a, &f, at, BOOT_CONSOLE_DURATION);
        let mut b = fb(100, 32);
        paint_boot_console(&mut b, &f, at, BOOT_CONSOLE_DURATION);
        assert_eq!(
            diff_count(&a, &b),
            0,
            "identical inputs must paint an identical frame"
        );
    }

    /// The whole point of the dissolve: at the VERY end of the breakdown, the
    /// frame is the real splash — not an approximation of it.
    #[test]
    fn breakdown_reaches_the_real_splash_frame() {
        let f = facts(5);
        let mut end = fb(100, 32);
        paint_boot_console(&mut end, &f, BOOT_CONSOLE_DURATION, BOOT_CONSOLE_DURATION);
        let mut splash = fb(100, 32);
        paint_splash(&mut splash, Duration::ZERO, f.seed, None);
        assert_eq!(
            diff_count(&end, &splash),
            0,
            "the final breakdown frame must equal the real splash paint"
        );
    }

    /// The dissolve must not linger visibly corrupted for most of its
    /// window: past the MIDPOINT of the blend window (i.e. progress ==
    /// `BLEND_START + (1 - BLEND_START) / 2`), the frame must already be
    /// MOSTLY the real splash, not "still covered in garbage" until the very
    /// last instant — the fix for "after the dissolve completes, the splash
    /// must be exactly the normal splash; the dissolve may overlap only its
    /// first ~0.5s".
    #[test]
    fn dissolve_tail_is_mostly_clean() {
        let f = facts(11);
        let bd_total = breakdown_phase_duration(BOOT_CONSOLE_DURATION);
        let log_total = log_phase_duration(BOOT_CONSOLE_DURATION);
        let mut splash = fb(100, 32);
        paint_splash(&mut splash, Duration::ZERO, f.seed, None);

        // Halfway THROUGH the blend window (not the whole breakdown phase):
        // t = BLEND_START + half of the remaining (1 - BLEND_START).
        let mid_t = BLEND_START + (1.0 - BLEND_START) / 2.0;
        let mut mid = fb(100, 32);
        paint_boot_console(
            &mut mid,
            &f,
            log_total + bd_total.mul_f32(mid_t),
            BOOT_CONSOLE_DURATION,
        );

        let total_cells = 100 * 32;
        let still_wrong = diff_count(&mid, &splash);
        // Threshold recalibrated for the NAL NET narrative rewrite: the log
        // block now carries more lines (the playful boot story), so it fills
        // more of a 100x32 frame than the terser log it replaced, and a few
        // more of its cells are still mid-blend at the same fixed midpoint
        // fraction. 3/10 still means at least 70% of the frame is clean
        // splash by the midpoint — comfortably "mostly clean", not a loosened
        // bar for actual garbage.
        assert!(
            still_wrong < total_cells * 3 / 10,
            "past the blend window's midpoint the frame must be MOSTLY clean splash: {still_wrong}/{total_cells} cells differ"
        );
    }

    #[test]
    fn corruption_chaos_ramps_up_then_fades_to_zero() {
        assert_eq!(corruption_chaos(0.0), 0.0);
        assert!(
            corruption_chaos(BLEND_START) > 0.9,
            "chaos must be near its peak right at BLEND_START"
        );
        assert_eq!(
            corruption_chaos(1.0),
            0.0,
            "chaos must be fully faded by the very end"
        );
        // Monotonic up to BLEND_START, then monotonic down.
        let mut last = 0.0;
        let mut t = 0.0;
        while t <= BLEND_START {
            let c = corruption_chaos(t);
            assert!(
                c >= last,
                "chaos must not decrease before BLEND_START: t={t} c={c} last={last}"
            );
            last = c;
            t += 0.05;
        }
        let mut last = 1.0;
        let mut t = BLEND_START;
        while t <= 1.0 {
            let c = corruption_chaos(t);
            assert!(
                c <= last,
                "chaos must not increase after BLEND_START: t={t} c={c} last={last}"
            );
            last = c;
            t += 0.05;
        }
    }

    // ── the five failure effects, isolated ────────────────────────────────

    #[test]
    fn gravity_moves_a_glyph_down_and_leaves_a_blank_above() {
        let f = facts(3);
        let lines = boot_log_lines(&f);
        // A seed/intensity likely to trigger at least one fall; scan a few
        // seeds since the effect is probabilistic per cell.
        let mut saw_a_fall = false;
        for seed in 0..40u64 {
            let mut before = fb(100, 32);
            clear_bg(&mut before);
            paint_log_lines(&mut before, &lines);
            let mut after = before.clone();
            apply_gravity(&mut after, seed, 1.0);
            for row in 0..before.rows() {
                for col in 0..before.cols() {
                    let was_text = before
                        .get(col, row)
                        .is_some_and(|c| c.glyph != u32::from(' '));
                    let now_blank = after
                        .get(col, row)
                        .is_some_and(|c| c.glyph == u32::from(' '));
                    if was_text && now_blank {
                        // Its glyph must have landed further down the SAME
                        // column (gravity is straight-down, never sideways).
                        let moved_down = (row + 1..after.rows())
                            .any(|r2| after.get(col, r2) == before.get(col, row));
                        if moved_down {
                            saw_a_fall = true;
                        }
                    }
                }
            }
            if saw_a_fall {
                break;
            }
        }
        assert!(
            saw_a_fall,
            "expected at least one seed to show a character falling out of the text"
        );
    }

    #[test]
    fn gravity_is_a_noop_at_zero_intensity() {
        let f = facts(3);
        let lines = boot_log_lines(&f);
        let mut before = fb(100, 32);
        clear_bg(&mut before);
        paint_log_lines(&mut before, &lines);
        let mut after = before.clone();
        apply_gravity(&mut after, 7, 0.0);
        assert_eq!(
            diff_count(&before, &after),
            0,
            "zero intensity must be a deterministic no-op"
        );
    }

    #[test]
    fn bleed_never_leaves_a_coloured_blank_cell() {
        // At a progress between BLEED_ONSET and COLLAPSE_ONSET, only shear,
        // gravity and bleed are active. Any cell left blank (by gravity, or
        // to begin with) must have exactly the plain blank colour — proof
        // bleed's own "skip blank cells" guard holds even after gravity has
        // rearranged the board.
        let f = facts(5);
        let lines = boot_log_lines(&f);
        for seed in 0..20u64 {
            let mut frame = fb(100, 32);
            clear_bg(&mut frame);
            paint_log_lines(&mut frame, &lines);
            corrupt_cells(&mut frame, seed, &lines, 0.4);
            for row in 0..frame.rows() {
                for col in 0..frame.cols() {
                    if let Some(c) = frame.get(col, row) {
                        if c.glyph == u32::from(' ') {
                            assert_eq!(c.fg, FG, "a blank cell must never carry bled colour: seed={seed} row={row} col={col}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn spread_is_a_noop_at_zero_intensity() {
        let f = facts(9);
        let lines = boot_log_lines(&f);
        let mut before = fb(100, 32);
        clear_bg(&mut before);
        paint_log_lines(&mut before, &lines);
        let mut after = before.clone();
        apply_spread(&mut after, 13, &lines, 0.0);
        assert_eq!(
            diff_count(&before, &after),
            0,
            "zero intensity must be a deterministic no-op"
        );
    }

    #[test]
    fn spread_never_touches_cells_far_beyond_its_reach() {
        let f = facts(9);
        let lines = boot_log_lines(&f);
        let cols = 100;
        let rows = 32;
        let (top, bottom, left, right) = text_block_bounds(cols, rows, &lines);
        // Corners of a 100x32 grid are far from a centred log block -- pick
        // whichever corner is farthest and confirm it is well beyond even
        // the MAXIMUM possible reach (SPREAD_MAX_REACH + 1).
        let corners = [
            (0usize, 0usize),
            (0, cols - 1),
            (rows - 1, 0),
            (rows - 1, cols - 1),
        ];
        let max_reach = 1.0 + SPREAD_MAX_REACH;
        assert!(
            corners
                .iter()
                .any(|&(r, c)| (dist_to_block(r, c, top, bottom, left, right) as f32) > max_reach),
            "test setup must actually exercise a corner beyond the reach, or this check is vacuous"
        );
        for seed in 0..10u64 {
            let mut before = fb(cols, rows);
            clear_bg(&mut before);
            paint_log_lines(&mut before, &lines);
            let mut after = before.clone();
            apply_spread(&mut after, seed, &lines, 1.0);
            for &(row, col) in &corners {
                let dist = dist_to_block(row, col, top, bottom, left, right);
                if (dist as f32) > max_reach {
                    assert_eq!(
                        before.get(col, row),
                        after.get(col, row),
                        "a corner far beyond the max reach must never be touched by spread: seed={seed} row={row} col={col} dist={dist}"
                    );
                }
            }
        }
    }

    #[test]
    fn spread_reach_grows_with_intensity() {
        let f = facts(9);
        let lines = boot_log_lines(&f);
        let cols = 100;
        let rows = 32;
        let mut low_touched = 0usize;
        let mut high_touched = 0usize;
        for seed in 0..15u64 {
            let mut before = fb(cols, rows);
            clear_bg(&mut before);
            paint_log_lines(&mut before, &lines);
            let mut low = before.clone();
            apply_spread(&mut low, seed, &lines, 0.05);
            low_touched += diff_count(&before, &low);
            let mut high = before.clone();
            apply_spread(&mut high, seed, &lines, 1.0);
            high_touched += diff_count(&before, &high);
        }
        assert!(
            low_touched < high_touched,
            "a bigger spread intensity must touch more cells overall: low={low_touched} high={high_touched}"
        );
    }

    // ── layout scales to the terminal ─────────────────────────────────────

    #[test]
    fn log_block_is_centred_and_widens_on_a_much_bigger_terminal() {
        let f = facts(1);
        let lines = boot_log_lines(&f);
        let small = log_block_layout(100, 32, &lines);
        let big = log_block_layout(280, 70, &lines);
        assert!(
            small.left > 0,
            "on a 100-col terminal the block must not be pinned to column 0"
        );
        assert!(
            big.left > small.left,
            "a much wider terminal must centre the block further from the edge"
        );
        assert!(
            big.width >= small.width,
            "the decorative rule must not shrink on a bigger terminal"
        );
        assert!(
            big.top > small.top,
            "a much taller terminal must centre the block further from the top"
        );
    }

    #[test]
    fn log_block_never_touches_zero_size_input() {
        assert_eq!(log_block_layout(0, 32, &[]).width, 0);
        assert_eq!(log_block_layout(100, 0, &[]).height, 0);
    }

    #[test]
    fn empty_lines_paint_nothing_not_even_the_rule() {
        let mut frame = fb(100, 32);
        clear_bg(&mut frame);
        paint_log_lines(&mut frame, &[]);
        let mut blank = fb(100, 32);
        clear_bg(&mut blank);
        assert_eq!(
            diff_count(&frame, &blank),
            0,
            "no lines revealed yet must mean a truly blank console, no chrome either"
        );
    }

    #[test]
    fn zero_total_duration_does_not_panic_and_lands_on_splash() {
        let f = facts(1);
        let mut frame = fb(80, 24);
        paint_boot_console(&mut frame, &f, Duration::ZERO, Duration::ZERO);
        let mut splash = fb(80, 24);
        paint_splash(&mut splash, Duration::ZERO, f.seed, None);
        assert_eq!(diff_count(&frame, &splash), 0);
    }

    #[test]
    fn degenerate_grid_does_not_panic() {
        let f = facts(1);
        let mut frame = fb(0, 0);
        paint_boot_console(
            &mut frame,
            &f,
            Duration::from_millis(500),
            BOOT_CONSOLE_DURATION,
        );
        paint_boot_console(&mut frame, &f, BOOT_CONSOLE_DURATION, BOOT_CONSOLE_DURATION);
    }
}
