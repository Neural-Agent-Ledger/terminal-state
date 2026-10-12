//! `entry_fx` — the level-**entry** "resolve" transition.
//!
//! A reference ASCII engine's level entry (and its fast-travel) plays a
//! per-cell threshold reveal — a thin frontier band of scrambled glyphs, a
//! green tint that fades out, `> …ONLINE` log lines typed over it — instead of
//! a hard cut. Ours WAS a hard cut: `OnEnter(Playing)` ran only
//! `enter_playing` / `open_camera_gate` (`flow.rs`). We already owned a reveal
//! compositor with feathering (`re_render::reveal`), but only the FOV-zoom
//! transition used it ([`re_runtime::zoom`]), and its orders were only
//! radial/spiral.
//!
//! This module adds the missing half on the game side: a
//! [`re_render::RevealMode::Hashed`] hashed/row-biased sweep from a solid-black OLD frame
//! into the freshly loaded level's live frame, a scramble-glyph frontier band
//! ([`re_render::apply_scramble_frontier`]), and 1-3 typed log lines in the
//! SAME boot-console voice ([`crate::boot_console::visible_line_count`]).
//! Study-only (`rules/references-are-study-only.md`): technique only — the
//! scramble alphabet, the log-line wording, and the tuning numbers below are
//! ours, not copied.
//!
//! Deliberately **calm, not chaotic** — the owner previously judged the intro
//! glitch sequence too chaotic. This is a single smooth sweep with a fading
//! tint, not an escalating failure narrative like `boot_console`'s breakdown:
//! the scramble alphabet here (SEE `SCRAMBLE_GLYPHS`) is a DIFFERENT,
//! gentler set from that module's `CORRUPTION_GLYPHS`, on purpose — this is
//! the scene *resolving into focus*, not a console *failing*.
//!
//! # Where it sits
//!
//! * [`crate::entry_fx::start_entry_fx_sys`] — `OnEnter(GameState::Playing)`: snapshots a
//!   black OLD frame and starts a fresh [`crate::entry_fx::Transition`], UNLESS this entry is
//!   a resume from Pause (mirrors `flow::enter_playing`'s own
//!   `pause_edge`-style check — a paused-then-resumed level must never
//!   replay the entry).
//! * [`crate::entry_fx::tick_entry_fx_sys`] — `Last`, `.before(re_present::PresentSet)`,
//!   gated to `Playing`: composites the hashed reveal + scramble frontier +
//!   typed lines over the LIVE frame each tick (mirrors
//!   `re_runtime::zoom::reveal_transition_sys`'s "new = whatever is live right
//!   now" pattern — no snapshot of NEW, no `RenderForceDirty` coupling, so
//!   this cannot race `re_runtime::zoom`'s own use of that resource), advances the
//!   clock, and clears the transition on completion.
//! * [`crate::entry_fx::skip_entry_fx_sys`] — `Update`, gated to `Playing`: any
//!   [`re_present::MenuNav`] or [`re_present::InteractPressed`] ends the
//!   transition immediately — the SAME "skip on any neutral input" contract
//!   `flow::tick_diving` already uses for the dive tunnel.
//!
//! # Gating (must not affect `--dump`, goldens, or tests that step Playing)
//!
//! [`crate::entry_fx::EntryFxPlugin`] IS registered live, in `main.rs`'s `build_core`, next to
//! `.add_plugins(runtime_glue::ZoomPlugin)` — every interactive launch runs it. Three
//! independent knobs decide whether a given `OnEnter(Playing)` actually starts
//! a transition, checked together in [`crate::entry_fx::start_entry_fx_sys`]:
//!
//! 1. **`Settings::entry_transition`** (default `true`, mirrors `boot_intro`) —
//!    the dedicated opt-out for this ONE flourish.
//! 2. **`Settings::boot_intro`** (default `true`) — turning off the opening
//!    boot sequence turns this one off too: both are "the game announcing
//!    itself with a flourish", and a player who disabled one almost certainly
//!    wants the other quiet as well, not a different flashy sweep left behind.
//! 3. **`TS_NO_INTRO`** (`EntryFxConfig::env_enabled`) — the SAME env override
//!    `boot_intro` honours, so one var turns off every opening/entry flourish
//!    at once (useful for a script or a CI capture).
//!
//! Any ONE of the three being off is enough to keep every `OnEnter(Playing)`
//! a hard cut, byte-identical to before this module existed.
//!
//! `--dump` never builds a Bevy `App` at all (it calls
//! `re_render::composite::composite_dump_once` directly), so it is
//! structurally unreachable regardless of any of the three gates. Every
//! existing headless test/harness that builds via `main.rs`'s `build_core`
//! only ever steps to `GameState::Splash` (see
//! `interactive_core_boots_to_splash_without_panic`), never `Playing`, so the
//! live registration does not perturb them either. The golden framebuffer
//! suite (`tests/golden/`) renders through `--dump`/`composite_dump_once`
//! directly and never constructs a Bevy `App`, so it is unaffected by
//! registration or by either default flipping.

use std::time::Duration;

use bevy::prelude::*;
use re_core::Framebuffer;
use re_present::{InteractPressed, MenuNav};
use re_render::{apply_scramble_frontier, composite_reveal_feathered, RevealMode};

use crate::boot_console::visible_line_count;
use crate::flow::{CurrentLevel, GameState, LevelCatalog};
use crate::menu::{put_center, BG};
use crate::settings::Settings;

/// Default transition duration — within the spec's ~1.5-1.8 s window.
const DEFAULT_ENTRY_FX_SECS: f32 = 1.7;

/// Feather band for the base hashed reveal (normalized-order units, the same
/// space [`re_render::DEFAULT_FEATHER`] lives in) — slightly wider than the
/// zoom transition's, since this sweep is slower and reads calmer with a
/// softer edge.
const FEATHER_BAND: f32 = 0.05;
/// Scramble-glyph frontier band width (same normalized-order units).
const SCRAMBLE_BAND: f32 = 0.05;
/// The scramble alphabet. Our own choice, deliberately DIFFERENT from
/// `boot_console::CORRUPTION_GLYPHS` (`#%&$?X01`) so this reads as the scene
/// resolving, not the console failing.
const SCRAMBLE_GLYPHS: [char; 8] = ['/', '\\', '|', '_', '.', ':', '+', '~'];
/// The green system-log tint the scramble band's colour shifts toward.
const TINT_GREEN: u32 = 0x0040_FF90;
/// Peak tint strength at `progress = 0`; linearly fades to `0` by
/// `progress = 1` — "a green tint that fades out" over the WHOLE sweep, on
/// top of the per-cell distance fade [`re_render::apply_scramble_frontier`]
/// already does across the band.
const TINT_PEAK: f32 = 0.7;

/// Typed-line colour — the same cold system-log green
/// [`crate::menu`]'s narrative-beat row uses, so this reads as the same
/// register (not the objective/HUD cyan, not an alarm colour).
const TYPED_FG: u32 = 0x0080_E0A8;
/// First row (from the top) the typed lines occupy — clear of nothing today
/// (the HUD itself is a SEPARATE system; see the module doc for why no
/// explicit ordering is asserted between them).
const TYPED_ROW_FROM_TOP: usize = 2;

/// Static, env-derived tuning, resolved once at plugin build (mirrors
/// `re_runtime::zoom::ZoomConfig::from_env`).
#[derive(Resource, Debug, Clone, Copy)]
pub struct EntryFxConfig {
    /// Total transition duration.
    pub duration: Duration,
    /// `false` when `TS_NO_INTRO` is set — the SAME env override
    /// `Settings::boot_intro` honours (`main.rs`'s `should_show_boot_intro`),
    /// reused here so one env var turns off every opening/entry flourish.
    pub env_enabled: bool,
}

impl Default for EntryFxConfig {
    fn default() -> Self {
        EntryFxConfig {
            duration: Duration::from_secs_f32(DEFAULT_ENTRY_FX_SECS),
            env_enabled: true,
        }
    }
}

impl EntryFxConfig {
    /// `TS_ENTRY_FX_MS` overrides the duration (floored at 1 ms, so it is
    /// never zero); `TS_NO_INTRO` (any value, same as `boot_intro`'s own
    /// override) disables the effect outright. A malformed value is ignored
    /// (keeps the default), never panics — same discipline as
    /// `ZoomConfig::from_env`.
    pub fn from_env() -> Self {
        let mut cfg = EntryFxConfig::default();
        if let Ok(v) = std::env::var("TS_ENTRY_FX_MS") {
            if let Ok(ms) = v.trim().parse::<f32>() {
                if ms.is_finite() && ms >= 0.0 {
                    cfg.duration = Duration::from_secs_f32(ms.max(1.0) / 1000.0);
                }
            }
        }
        if std::env::var_os("TS_NO_INTRO").is_some() {
            cfg.env_enabled = false;
        }
        cfg
    }
}

/// The live entry-transition state. `None` = idle (the ordinary case — every
/// non-Playing state, and every Playing frame once the transition ends).
#[derive(Resource, Debug, Default)]
pub struct EntryFxState {
    /// The in-flight transition, or `None` when idle.
    pub transition: Option<Transition>,
}

/// One in-flight entry transition.
#[derive(Debug)]
pub struct Transition {
    /// The solid-black frame the reveal sweeps away from.
    old: Framebuffer,
    /// Time elapsed since the transition started.
    elapsed: Duration,
    /// Seed for the scramble-glyph pick — varies transition to transition,
    /// stable for the WHOLE transition (never re-derived mid-sweep).
    seed: u64,
    /// The 1-3 typed log lines, in reveal order.
    lines: Vec<String>,
}

/// `>`-prefixed, boot-console-voice log lines for a fresh level entry. Real
/// data where available (the level's own catalogue label) — same discipline
/// `boot_console::boot_log_lines` uses, never invented lore beyond the label.
fn entry_log_lines(level_label: &str) -> Vec<String> {
    vec![
        format!("> sector {level_label} ... mounted."),
        "> puzzle network ... online.".to_string(),
        "> awaiting input.".to_string(),
    ]
}

/// A fresh, non-reproducible seed for the scramble glyph pick — purely
/// cosmetic (unlike `boot_console::BootSeed`, nothing here needs to be
/// reproduced for a golden/test comparison, since the whole effect is gated
/// off in every test path — see the module doc).
fn derive_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// `OnEnter(GameState::Playing)`: start a fresh transition, UNLESS this entry
/// is a RESUME from Pause (a paused-then-resumed level must never replay the
/// entry — mirrors `flow::enter_playing`'s own `pause_edge` check). A no-op
/// when `Settings::entry_transition` is off, `Settings::boot_intro` is off, or
/// `EntryFxConfig::env_enabled` is false (see the module doc's "Gating"
/// section for why all three share one check).
pub fn start_entry_fx_sys(
    mut transitions: MessageReader<StateTransitionEvent<GameState>>,
    settings: Res<Settings>,
    cfg: Res<EntryFxConfig>,
    fb: Res<Framebuffer>,
    catalog: Res<LevelCatalog>,
    current: Res<CurrentLevel>,
    mut state: ResMut<EntryFxState>,
) {
    if !settings.entry_transition || !settings.boot_intro || !cfg.env_enabled {
        return;
    }
    let Some(ev) = transitions.read().last() else {
        return;
    };
    if ev.entered != Some(GameState::Playing) || ev.exited == Some(GameState::Pause) {
        return;
    }
    let label = catalog
        .paths
        .get(current.0)
        .map(|p| LevelCatalog::label(p).to_string())
        .unwrap_or_else(|| "unknown".to_string());
    state.transition = Some(Transition {
        old: Framebuffer::new(fb.cols(), fb.rows()),
        elapsed: Duration::ZERO,
        seed: derive_seed(),
        lines: entry_log_lines(&label),
    });
}

/// `Update` (Playing): skip on any neutral input — the SAME events
/// `flow::tick_diving` treats as "skip" for the dive tunnel
/// (`MenuNav`/`InteractPressed`), so the two skippable set-pieces feel
/// consistent.
pub fn skip_entry_fx_sys(
    mut nav: MessageReader<MenuNav>,
    mut presses: MessageReader<InteractPressed>,
    mut state: ResMut<EntryFxState>,
) {
    if state.transition.is_none() {
        return;
    }
    let skipped = nav.read().count() > 0 || presses.read().count() > 0;
    if skipped {
        state.transition = None;
    }
}

/// `Last` (Playing, before `re_present::PresentSet`): if a transition is in
/// flight, composite the hashed reveal + scramble frontier + typed lines over
/// the LIVE [`Framebuffer`] into a scratch buffer, copy it back, advance the
/// clock, and clear the transition on completion.
///
/// Reads `fb` as NEW every tick (never a one-time snapshot) — the SAME
/// pattern `re_runtime::zoom::reveal_transition_sys` uses for its own `new` —
/// so a moving camera or an animated material during the transition simply
/// shows through correctly; no `re_render::RenderForceDirty` coupling is
/// needed (unlike zoom's instant-FOV-snap case, nothing here creates a
/// static-camera staleness bug, so this cannot race `re_runtime::zoom`'s own use
/// of that shared resource).
pub fn tick_entry_fx_sys(
    time: Res<Time>,
    cfg: Res<EntryFxConfig>,
    mut state: ResMut<EntryFxState>,
    mut fb: ResMut<Framebuffer>,
) {
    let Some(t) = state.transition.as_mut() else {
        return;
    };

    // Dimension guard: a resize mid-transition invalidates the OLD snapshot —
    // abandon cleanly (show the live frame) rather than composite garbage.
    if t.old.cols() != fb.cols() || t.old.rows() != fb.rows() {
        state.transition = None;
        return;
    }

    let duration_secs = cfg.duration.as_secs_f32().max(f32::EPSILON);
    let progress = (t.elapsed.as_secs_f32() / duration_secs).clamp(0.0, 1.0);

    let mut scratch = Framebuffer::new(fb.cols(), fb.rows());
    composite_reveal_feathered(
        &t.old,
        &fb,
        &mut scratch,
        progress,
        RevealMode::Hashed,
        FEATHER_BAND,
    );
    let tint_strength = (TINT_PEAK * (1.0 - progress)).max(0.0);
    apply_scramble_frontier(
        &mut scratch,
        RevealMode::Hashed,
        progress,
        SCRAMBLE_BAND,
        &SCRAMBLE_GLYPHS,
        TINT_GREEN,
        tint_strength,
        t.seed,
    );
    let visible = visible_line_count(t.elapsed, cfg.duration, t.lines.len());
    paint_entry_lines(&mut scratch, &t.lines[..visible]);
    *fb = scratch;

    t.elapsed += time.delta();
    if t.elapsed >= cfg.duration {
        state.transition = None;
    }
}

/// Paint `lines`, one per row, centred, starting at [`TYPED_ROW_FROM_TOP`].
/// Rows past the bottom of the frame are simply skipped (never panics on a
/// tiny grid).
fn paint_entry_lines(fb: &mut Framebuffer, lines: &[String]) {
    for (i, line) in lines.iter().enumerate() {
        let row = TYPED_ROW_FROM_TOP + i;
        if row >= fb.rows() {
            break;
        }
        put_center(fb, row, line, TYPED_FG, BG);
    }
}

/// Plugin: registers the entry-fx resources and the three systems above.
///
/// Registered live in `main.rs`'s `build_core`, next to
/// `.add_plugins(runtime_glue::ZoomPlugin)` — see the module doc's "Gating" section
/// for the three independent knobs that still gate any given transition.
pub struct EntryFxPlugin;

impl Plugin for EntryFxPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EntryFxConfig::from_env())
            .init_resource::<EntryFxState>()
            .add_systems(OnEnter(GameState::Playing), start_entry_fx_sys)
            .add_systems(
                Update,
                skip_entry_fx_sys.run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Last,
                tick_entry_fx_sys
                    .run_if(in_state(GameState::Playing))
                    .before(re_present::PresentSet),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── pure helpers: no App needed ─────────────────────────────────────────

    #[test]
    fn entry_log_lines_names_the_level_and_stays_short() {
        let lines = entry_log_lines("07-full-spectrum");
        assert!(
            !lines.is_empty() && lines.len() <= 3,
            "spec: 1-3 short lines"
        );
        assert!(lines[0].starts_with('>'), "system-log register");
        assert!(
            lines[0].contains("07-full-spectrum"),
            "names the real level"
        );
    }

    #[test]
    fn paint_entry_lines_writes_something_and_skips_offscreen_rows() {
        let mut fb = Framebuffer::new(40, 20);
        let before = fb.cells().to_vec();
        paint_entry_lines(&mut fb, &["> hello".to_string()]);
        assert_ne!(fb.cells(), before.as_slice());

        // A frame too short for TYPED_ROW_FROM_TOP must not panic and must
        // leave the frame untouched (every line falls off the bottom).
        let mut tiny = Framebuffer::new(40, TYPED_ROW_FROM_TOP);
        let tiny_before = tiny.cells().to_vec();
        paint_entry_lines(&mut tiny, &["> hello".to_string()]);
        assert_eq!(tiny.cells(), tiny_before.as_slice());
    }

    #[test]
    fn config_env_overrides_are_isolated_in_one_test() {
        // Same discipline `settings.rs`'s colour-auto-detect test uses: both
        // env vars are process-global, so scenarios run sequentially inside
        // ONE test rather than racing across parallel `#[test]` threads.
        let prev_ms = std::env::var("TS_ENTRY_FX_MS").ok();
        let prev_no_intro = std::env::var("TS_NO_INTRO").ok();
        std::env::remove_var("TS_ENTRY_FX_MS");
        std::env::remove_var("TS_NO_INTRO");

        let default_cfg = EntryFxConfig::from_env();
        assert!(default_cfg.env_enabled);
        assert!((default_cfg.duration.as_secs_f32() - DEFAULT_ENTRY_FX_SECS).abs() < 1e-6);

        std::env::set_var("TS_ENTRY_FX_MS", "900");
        let overridden = EntryFxConfig::from_env();
        assert!((overridden.duration.as_secs_f32() - 0.9).abs() < 1e-6);

        std::env::set_var("TS_NO_INTRO", "1");
        let disabled = EntryFxConfig::from_env();
        assert!(
            !disabled.env_enabled,
            "TS_NO_INTRO must disable entry_fx too"
        );

        // Restore.
        match prev_ms {
            Some(v) => std::env::set_var("TS_ENTRY_FX_MS", v),
            None => std::env::remove_var("TS_ENTRY_FX_MS"),
        }
        match prev_no_intro {
            Some(v) => std::env::set_var("TS_NO_INTRO", v),
            None => std::env::remove_var("TS_NO_INTRO"),
        }
    }

    // ── end-to-end: a LOCAL app, mirroring `flow::tests::test_app` ─────────
    //
    // Deliberately NOT `flow::tests::test_app` itself (that fixture is shared
    // by ~every other flow test and must stay entry-fx-free) — a small local
    // builder that adds `EntryFxPlugin` on top of the same minimal stack.

    use bevy::state::app::StatesPlugin;
    use bevy::MinimalPlugins;
    use re_puzzle::PuzzlePlugin;

    fn levels_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .unwrap()
            .join("levels")
    }

    fn abs_catalog() -> LevelCatalog {
        let root = levels_root();
        let paths = (1..=7)
            .map(|n| {
                root.join(format!("{n:02}.lvl"))
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        LevelCatalog { paths }
    }

    fn entry_fx_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_message::<InteractPressed>()
            // The entry transition needs Settings PRESENT and set BEFORE
            // `FlowPlugin`'s `init_resource` (insert-if-absent) runs, so it
            // is on for THIS app without touching the shared default.
            .insert_resource(Settings {
                entry_transition: true,
                ..Settings::default()
            })
            .add_plugins(PuzzlePlugin)
            .add_plugins(crate::flow::FlowPlugin { cli_level: None })
            .add_plugins(EntryFxPlugin)
            .insert_resource(abs_catalog())
            // `EntryFxPlugin::build` does an UNCONDITIONAL `insert_resource`
            // for `EntryFxConfig` (mirroring `ZoomPlugin`'s own
            // `insert_resource(ZoomConfig::from_env())`), so the test's
            // short duration MUST be applied AFTER the plugin, not before —
            // otherwise the plugin's `from_env()` default silently wins.
            .insert_resource(EntryFxConfig {
                duration: Duration::from_millis(50),
                env_enabled: true,
            });
        app
    }

    #[test]
    fn a_fresh_level_load_starts_a_transition_that_paints_and_completes() {
        let mut app = entry_fx_test_app();
        app.update(); // Splash

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update(); // OnEnter(Playing) runs: enter_playing + start_entry_fx_sys

        assert!(
            app.world().resource::<EntryFxState>().transition.is_some(),
            "entering Playing fresh must start a transition"
        );
        let before = app.world().resource::<Framebuffer>().cells().to_vec();

        app.update(); // Last: tick_entry_fx_sys composites + advances
        let after = app.world().resource::<Framebuffer>().cells().to_vec();
        assert_ne!(
            before, after,
            "the transition must actually paint the frame"
        );

        // Run past the configured 50ms duration (real wall-clock ticks are
        // tiny in a test loop, so give it a generous number of frames).
        for _ in 0..200 {
            app.update();
            if app.world().resource::<EntryFxState>().transition.is_none() {
                break;
            }
        }
        assert!(
            app.world().resource::<EntryFxState>().transition.is_none(),
            "the transition must complete on its own within a bounded number of frames"
        );
    }

    #[test]
    fn interact_pressed_skips_the_transition_immediately() {
        let mut app = entry_fx_test_app();
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        assert!(app.world().resource::<EntryFxState>().transition.is_some());

        app.world_mut().write_message(InteractPressed);
        app.update();
        assert!(
            app.world().resource::<EntryFxState>().transition.is_none(),
            "any neutral input (InteractPressed) must skip the transition"
        );
    }

    /// Build the same minimal app [`entry_fx_test_app`] does, but with an
    /// explicit `Settings` override so each disable-path test controls only
    /// the ONE knob it is checking (the other two stay at their live-default
    /// `true`).
    fn entry_fx_test_app_with(settings: Settings) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_message::<InteractPressed>()
            .insert_resource(settings)
            .add_plugins(PuzzlePlugin)
            .add_plugins(crate::flow::FlowPlugin { cli_level: None })
            .add_plugins(EntryFxPlugin)
            .insert_resource(abs_catalog())
            .insert_resource(EntryFxConfig {
                duration: Duration::from_millis(50),
                env_enabled: true,
            });
        app
    }

    #[test]
    fn entry_transition_setting_off_never_starts_a_transition() {
        let mut app = entry_fx_test_app_with(Settings {
            entry_transition: false,
            boot_intro: true,
            ..Settings::default()
        });
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        assert!(
            app.world().resource::<EntryFxState>().transition.is_none(),
            "entry_transition = false must disable the transition even though boot_intro is on"
        );
    }

    #[test]
    fn boot_intro_off_also_disables_the_entry_transition() {
        let mut app = entry_fx_test_app_with(Settings {
            entry_transition: true,
            boot_intro: false,
            ..Settings::default()
        });
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        assert!(
            app.world().resource::<EntryFxState>().transition.is_none(),
            "boot_intro = false must disable the entry transition too, even though entry_transition is on"
        );
    }

    #[test]
    fn both_flags_on_by_default_starts_a_transition() {
        // No explicit Settings override: exercises the LIVE default (both
        // entry_transition and boot_intro default true).
        let mut app = entry_fx_test_app_with(Settings::default());
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        assert!(
            app.world().resource::<EntryFxState>().transition.is_some(),
            "the live default (both flags true) must auto-start the transition on a fresh level load"
        );
    }

    #[test]
    fn resuming_from_pause_does_not_restart_the_transition() {
        let mut app = entry_fx_test_app();
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        // Let the transition finish so we can tell a RESTART apart from it
        // simply still running.
        for _ in 0..200 {
            app.update();
            if app.world().resource::<EntryFxState>().transition.is_none() {
                break;
            }
        }
        assert!(app.world().resource::<EntryFxState>().transition.is_none());

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Pause);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        assert!(
            app.world().resource::<EntryFxState>().transition.is_none(),
            "Pause -> Playing (a resume) must NOT restart the entry transition"
        );
    }
}
