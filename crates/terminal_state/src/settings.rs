//! `settings` — the player-tunable, on-disk settings store.
//!
//! A sibling of [`crate::save`] (`Progress`): a tiny TOML file persisted next to
//! the save, with the SAME robust path resolution. Where `Progress` tracks what
//! the player has *done*, [`crate::settings::Settings`] tracks how the game
//! *presents* — and every field is wired to the real system it controls, not
//! cosmetic:
//!
//! | Field | Wired to |
//! |-------|----------|
//! | [`color`](crate::settings::Settings::color) | the [`re_present::ColorMode`] resource both present backends read each frame |
//! | [`mouse_enabled`] / [`mouse_sens`] | the [`re_present::MouseSettings`] resource the mouse-look systems read |
//! | [`muted`](crate::settings::Settings::muted) / [`volume`](crate::settings::Settings::volume) | `re_audio` master volume (a [`re_audio::SetMasterVolume`] message) |
//! | [`glitch_enabled`](crate::settings::Settings::glitch_enabled) | the `re_glitch` `GlitchConfig` resource (accessibility / motion-sensitivity toggle) |
//! | [`colour_unlock_enabled`](crate::settings::Settings::colour_unlock_enabled) | [`crate::palette_ramp`]'s live `PaletteGate` (accessibility off-switch: `false` = full colour always) |
//!
//! ## On-disk shape
//!
//! ```toml
//! color = "truecolor"        # or "256"
//! mouse_enabled = true
//! mouse_sens = "medium"      # low | medium | high
//! muted = false
//! volume = 0.8               # 0.0 ..= 1.0
//! glitch_enabled = true      # visual-corruption layer (accessibility off-switch)
//! colour_unlock_enabled = true  # story colour-gate; false = full colour always
//! ```
//!
//! ## Robustness
//!
//! Identical contract to `save.rs`: a missing file loads defaults silently
//! (first run); an unreadable / corrupt file loads defaults with a warning;
//! saving is best-effort and never panics. The game ALWAYS comes up with a
//! valid `Settings`.
//!
//! [`mouse_enabled`]: crate::settings::Settings::mouse_enabled
//! [`mouse_sens`]: crate::settings::Settings::mouse_sens

use std::path::PathBuf;

use bevy::prelude::Resource;
use re_present::ColorMode;
use re_render::ShadowQuality;
use serde::{Deserialize, Serialize};

/// Mouse-look sensitivity presets, each mapping to a multiplier on the present
/// layer's base sensitivity (`MOUSE_YAW_SENS` / `MOUSE_PITCH_SENS`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseSens {
    /// Half the base feel.
    Low,
    /// The historical / base feel (the present-layer constants verbatim).
    #[default]
    Medium,
    /// Noticeably faster than base.
    High,
}

impl MouseSens {
    /// Cycle to the next preset (Low → Medium → High → Low). Drives the menu's
    /// Left/Right / cycle on the focused row.
    pub fn next(self) -> Self {
        match self {
            MouseSens::Low => MouseSens::Medium,
            MouseSens::Medium => MouseSens::High,
            MouseSens::High => MouseSens::Low,
        }
    }

    /// The previous preset (the reverse cycle).
    pub fn prev(self) -> Self {
        match self {
            MouseSens::Low => MouseSens::High,
            MouseSens::Medium => MouseSens::Low,
            MouseSens::High => MouseSens::Medium,
        }
    }

    /// The sensitivity multiplier fed into [`re_present::MouseSettings`].
    pub fn multiplier(self) -> f32 {
        match self {
            MouseSens::Low => 0.5,
            MouseSens::Medium => 1.0,
            MouseSens::High => 1.75,
        }
    }

    /// A short label for the settings row.
    pub fn label(self) -> &'static str {
        match self {
            MouseSens::Low => "LOW",
            MouseSens::Medium => "MEDIUM",
            MouseSens::High => "HIGH",
        }
    }
}

// ───────────────────────── GRAPHICS QUALITY ─────────────────────────────────
//
// A unified Low/Medium/High/Ultra preset PLUS individual knobs (draw distance, FPS
// cap, shadows, LOD, sub-cell, post-fx, colour). Selecting a preset writes the whole
// bundle; nudging any individual knob flips the preset to `Custom`. Every knob is
// wired to a real render lever (see `Settings::*` accessors + the `re_game` apply
// path). The DEFAULT quality is `High`, whose bundle reproduces TODAY's render exactly
// (shadows Soft, default LOD, sub-cell on, post-fx off, draw distance = the historical
// `FAR_FREEZE_DIST`), so a default build is byte-identical.

/// The four quality presets + `Custom`. A preset BUNDLES every quality knob; the menu
/// row cycles Low→Medium→High→Ultra (Custom is only entered by editing an individual
/// knob, and is shown but skipped by the cycler).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityPreset {
    /// Cheapest: short draw distance, shadows OFF, LOD aggressive, sub-cell OFF,
    /// post-fx OFF, 30 FPS. The "fog-death" mood.
    Low,
    /// Mid: medium draw distance, hard shadows, balanced LOD, sub-cell ON, post-fx OFF,
    /// 60 FPS.
    Medium,
    /// The shipping default — reproduces today's render exactly. Soft shadows, default
    /// LOD, sub-cell ON, post-fx OFF, default draw distance, 60 FPS.
    #[default]
    High,
    /// Grandest: long draw distance, soft shadows, LOD held near, sub-cell ON,
    /// cinematic post-fx, uncapped FPS.
    Ultra,
    /// Hand-tuned — at least one knob differs from every preset bundle.
    Custom,
}

impl QualityPreset {
    /// The presets in cycle order (Custom is NOT in the cycle — it is only reached by
    /// editing a knob). The menu row walks this with wrap.
    pub const CYCLE: [QualityPreset; 4] = [
        QualityPreset::Low,
        QualityPreset::Medium,
        QualityPreset::High,
        QualityPreset::Ultra,
    ];

    /// A short label for the settings row.
    pub fn label(self) -> &'static str {
        match self {
            QualityPreset::Low => "LOW",
            QualityPreset::Medium => "MEDIUM",
            QualityPreset::High => "HIGH",
            QualityPreset::Ultra => "ULTRA",
            QualityPreset::Custom => "CUSTOM",
        }
    }
}

/// The frame-rate cap. `Off` = uncapped (run-loop spins as fast as the schedule
/// allows); `Fps(n)` paces the loop to `n` frames/second. Persisted as a string
/// (`"off"`/`"30"`/`"60"`/`"120"`) so a hand-edited file reads cleanly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpsCap {
    /// Uncapped — the run-loop runs with a near-zero interval.
    Off,
    /// Capped to `n` frames per second.
    Fps(u32),
}

impl Default for FpsCap {
    fn default() -> Self {
        FpsCap::Fps(60)
    }
}

/// The allowed FPS-cap presets, in cycle order (the menu row + `--fps` accept these).
pub const FPS_PRESETS: [FpsCap; 4] = [
    FpsCap::Fps(30),
    FpsCap::Fps(60),
    FpsCap::Fps(120),
    FpsCap::Off,
];

impl FpsCap {
    /// The seconds-per-frame interval this cap maps to for
    /// `ScheduleRunnerPlugin::run_loop`. `Off` → a tiny interval (effectively
    /// uncapped); `Fps(n)` → `1/n`.
    pub fn loop_interval_secs(self) -> f64 {
        match self {
            FpsCap::Off => 0.0,
            FpsCap::Fps(n) => 1.0 / (n.max(1) as f64),
        }
    }

    /// A short label for the settings row / serde.
    pub fn label(self) -> String {
        match self {
            FpsCap::Off => "UNCAPPED".to_string(),
            FpsCap::Fps(n) => format!("{n}"),
        }
    }

    fn to_token(self) -> String {
        match self {
            FpsCap::Off => "off".to_string(),
            FpsCap::Fps(n) => n.to_string(),
        }
    }

    /// Parse a CLI / on-disk token (`off`/`30`/`60`/`120`/any `u32`). Returns `None`
    /// on an unrecognised value so a caller can usage-error.
    pub fn parse_token(raw: &str) -> Option<FpsCap> {
        let t = raw.trim().to_ascii_lowercase();
        if t == "off" || t == "uncapped" || t == "0" {
            return Some(FpsCap::Off);
        }
        t.parse::<u32>().ok().filter(|&n| n > 0).map(FpsCap::Fps)
    }
}

/// The LOD aggressiveness knob — how quickly distance LOD coarsens the picture. Maps
/// to the [`re_render::LodConfig`] near/far boundaries: `Aggressive` pulls them in
/// (cheap, detail drops sooner), `Near` pushes them out (detail held far), `Balanced`
/// is the engine default (byte-identical to today).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LodQuality {
    /// LOD coarsens early (boundaries pulled in ~40%). Cheapest.
    Aggressive,
    /// The engine default ([`re_render::LodConfig::default`]). Byte-identical to today.
    #[default]
    Balanced,
    /// LOD held near (boundaries pushed out ~60%). Detail farthest out.
    Near,
}

impl LodQuality {
    /// A short label for the settings row.
    pub fn label(self) -> &'static str {
        match self {
            LodQuality::Aggressive => "AGGRESSIVE",
            LodQuality::Balanced => "BALANCED",
            LodQuality::Near => "NEAR",
        }
    }

    /// The [`re_render::LodConfig`] this quality produces, scaling the DEFAULT
    /// boundaries. `Balanced` returns the default verbatim (byte-identical to today);
    /// `far_start` is additionally pushed to at least `draw_distance` so LOD never
    /// freezes geometry the draw-distance is still drawing.
    pub fn lod_config(self, draw_distance: u32) -> re_render::LodConfig {
        let base = re_render::LodConfig::default();
        let scale = match self {
            LodQuality::Aggressive => 0.6,
            LodQuality::Balanced => 1.0,
            LodQuality::Near => 1.6,
        };
        let mut cfg = base;
        cfg.near_end = base.near_end * scale;
        cfg.far_start = base.far_start * scale;
        // Keep the far LOD boundary inside the draw distance band; the cast freezes at
        // `draw_distance`, so holding LOD detail past it is wasted (and can pop at the
        // freeze edge). Cap at the draw distance.
        cfg.far_start = cfg
            .far_start
            .min(draw_distance as f32)
            .max(cfg.near_end + 1.0);
        cfg
    }
}

/// The allowed DRAW-DISTANCE presets (world cells), in cycle order. Short = the
/// fog-death mood + a cheaper cast; long = grander vistas. The default (`High`) uses
/// the historical [`re_render::ascii_shader::FAR_FREEZE_DIST`] (32) so the render is
/// byte-identical.
pub const DRAW_DISTANCE_PRESETS: [u32; 4] = [16, 24, 32, 48];

/// A small serde shim for [`ColorMode`] (which lives in `re_present` and so
/// can't derive serde there without a dependency). We persist a string and map
/// it back, defaulting to truecolor on anything unexpected.
fn color_to_str(c: ColorMode) -> &'static str {
    match c {
        ColorMode::Truecolor => "truecolor",
        ColorMode::Ansi256 => "256",
    }
}

fn color_from_str(s: &str) -> ColorMode {
    ColorMode::parse_flag(s).unwrap_or(ColorMode::Truecolor)
}

/// A small serde shim for [`ShadowQuality`] (which lives in `re_render` and so
/// can't derive serde there without a dependency). We persist a string and map
/// it back, defaulting to the engine default (`Soft`) on anything unexpected.
fn shadows_to_str(q: ShadowQuality) -> &'static str {
    match q {
        ShadowQuality::Off => "off",
        ShadowQuality::Hard => "hard",
        ShadowQuality::Soft => "soft",
    }
}

fn shadows_from_str(s: &str) -> ShadowQuality {
    match s.trim().to_ascii_lowercase().as_str() {
        "off" => ShadowQuality::Off,
        "hard" => ShadowQuality::Hard,
        "soft" => ShadowQuality::Soft,
        _ => ShadowQuality::Soft,
    }
}

/// The persisted, player-tunable settings — a Bevy [`Resource`].
///
/// Loaded once at startup ([`Settings::load`]) and re-persisted whenever the
/// settings menu changes a value ([`Settings::save`]). Every field maps to a
/// real runtime system; see the module docs.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Color emission mode. Persisted as a string (`"truecolor"` / `"256"`). When the
    /// key is absent — a fresh install, or an old settings file saved before the
    /// player ever touched this row — the default comes from
    /// `default_color`, which defers to [`ColorMode::resolve`]'s env auto-detect
    /// (`COLORTERM` / the Windows VT probe) rather than hardcoding Truecolor. Once the
    /// player picks a value (or it round-trips through a save), that explicit choice is
    /// what gets persisted and always wins on the next load.
    #[serde(default = "default_color", with = "color_serde")]
    pub color: ColorMode,
    /// Master on/off for mouse-look.
    #[serde(default = "default_true")]
    pub mouse_enabled: bool,
    /// Mouse-look sensitivity preset.
    #[serde(default)]
    pub mouse_sens: MouseSens,
    /// Audio mute. When true, master volume is forced to 0 regardless of
    /// [`volume`](Self::volume) (so un-muting restores the prior level).
    #[serde(default)]
    pub muted: bool,
    /// Master volume in `[0, 1]`. Stepped by the menu in 0.1 increments.
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// Master on/off for the `re_glitch` visual-corruption layer. An
    /// accessibility toggle (motion-sensitivity / "no corruption"): when false,
    /// the `GlitchConfig` resource is forced [`off`](re_glitch::GlitchConfig::off)
    /// regardless of the level's authored `[effects] glitch`.
    #[serde(default = "default_true")]
    pub glitch_enabled: bool,
    /// Horizontal field of view preset in degrees (one of 60/65/70/75). Drives
    /// the raycaster's [`re_render::Fov::h_preset_deg`]. The in-game `[`/`]` keys
    /// (and gamepad Y) cycle it live; the settings FOV row cycles it too.
    #[serde(default = "default_fov_deg")]
    pub fov_deg: u32,
    /// Cell-aspect "stretch" override fed into [`re_render::Fov::stretch`]: a
    /// multiplier on the effective aspect (`1.0` = neutral, `>1` widens, `<1`
    /// narrows the horizontal FOV). Lets the player compensate for a terminal
    /// whose cells aren't the assumed ~2:1 tall:wide.
    #[serde(default = "default_stretch")]
    pub stretch: f32,
    /// Cast-shadow quality. Drives the live [`re_render::ShadowQuality`] resource
    /// the raycaster's shadow pass reads. Persisted as a string
    /// (`"off"`/`"hard"`/`"soft"`); serde-default = [`ShadowQuality::Soft`] so an
    /// old save (no `shadows` key) loads the engine default.
    #[serde(default = "default_shadows", with = "shadows_serde")]
    pub shadows: ShadowQuality,

    // ── GRAPHICS QUALITY block ───────────────────────────────────────────────
    /// The selected quality PRESET (Low/Medium/High/Ultra/Custom). Selecting a
    /// preset writes the whole bundle below; editing any knob flips this to `Custom`.
    /// Default `High` (reproduces today's render). serde-default so an old save loads.
    #[serde(default)]
    pub quality: QualityPreset,
    /// DRAW DISTANCE in world cells. Drives the runtime far-freeze
    /// ([`re_render::FarFreeze`]) + DDA step cap + the LOD `far_start`. Default 32
    /// (the historical [`re_render::ascii_shader::FAR_FREEZE_DIST`]) → byte-identical.
    #[serde(default = "default_draw_distance")]
    pub draw_distance: u32,
    /// FPS CAP — `Off` (uncapped) or `Fps(n)`. Drives the `ScheduleRunnerPlugin`
    /// run-loop interval at startup (+ a live in-menu frame-pacing throttle). Persisted
    /// as a token string. Default 60.
    #[serde(default, with = "fps_serde")]
    pub fps_cap: FpsCap,
    /// LOD aggressiveness — maps to [`re_render::LodConfig`]. Default `Balanced`
    /// (the engine default) → byte-identical.
    #[serde(default)]
    pub lod_quality: LodQuality,
    /// 2×2 sub-cell silhouette refinement (the [`re_render::composite::SubCellConfig`]
    /// blitter). Default `true` (on) → byte-identical.
    #[serde(default = "default_true")]
    pub subcell: bool,
    /// POST-FX preset NAME (`off`/`clean`/`subtle`/`cinematic`/`toon`/`crt`/`dream`).
    /// Resolved to a [`re_postfx::PostfxConfig`] by [`Self::postfx_config`]. Default
    /// `"off"` → byte-identical.
    #[serde(default = "default_postfx")]
    pub postfx: String,

    /// **Form A-7 fourth-wall opt-out** (`docs/lore/FORM_A7_OPERATOR_DIAGNOSTIC.md`
    /// §8). When `true` (the default), the level-load templating pass
    /// ([`crate::story::interp`]) resolves the Tier-1 `{username}` / `{hostname}` /
    /// `{local_time}` / `{tz}` / `{locale}` / `{sys_uptime}` tokens in `[note]`
    /// bodies from the operator's REAL local environment, so the Stewardship addresses
    /// the player by name. When `false`, those tokens substitute the A-7 §8 fallback
    /// constants (`anon` / `unknown` / `C`) instead — no real read is performed at
    /// all.
    ///
    /// This row is deliberately **file-only** (no settings-screen row): A-7 §8
    /// specifies it as a configuration-file setting, and the disclosure lives in the
    /// form + README, not in a menu toggle.
    #[serde(default = "default_true")]
    pub lore_fourth_wall: bool,

    /// Master on/off for the campaign colour-unlock gate ([`crate::palette_ramp`]).
    /// An accessibility toggle, same shape as
    /// [`glitch_enabled`](Self::glitch_enabled): `true` (default) = the story
    /// gate runs (red-phosphor early campaign, unlocking toward full colour);
    /// `false` = full colour always, regardless of progress. Persisted so the
    /// choice survives a restart; `#[serde(default = "default_true")]` so an old
    /// save (no `colour_unlock_enabled` key) loads the gate ON, unchanged from
    /// this feature's introduction.
    #[serde(default = "default_true")]
    pub colour_unlock_enabled: bool,

    /// Master on/off for the OPENING two-phase boot sequence (2026-09-24): the
    /// real-terminal screen tear (`re_present::intro_tear`, Phase 1) and the
    /// pseudo boot console ([`GameState::BootConsole`](crate::flow::GameState::BootConsole),
    /// Phase 2). `true` (default) = shown, subject ALSO to a live TTY +
    /// VT-capability probe and the `TS_NO_INTRO` env override (see
    /// `main.rs`'s `maybe_enter_boot_console` / the binary's startup path) —
    /// this flag alone does not guarantee the intro plays, but `false` alone
    /// guarantees it never does. `#[serde(default = "default_true")]` so an
    /// old save (no `boot_intro` key) loads the intro ON, unchanged from this
    /// feature's introduction.
    #[serde(default = "default_true")]
    pub boot_intro: bool,

    /// Master on/off for the level-**entry** "resolve" transition
    /// (study report Pass 2 item 4, `crate::entry_fx`): a short (~1.5-1.8s),
    /// skippable hashed-reveal sweep from black into the freshly loaded
    /// level, with 1-3 typed log lines in the boot-console voice. Also
    /// subject to the `TS_NO_INTRO` env override AND `Settings::boot_intro`
    /// — the SAME two knobs `boot_intro` itself is gated by
    /// (`EntryFxConfig::from_env`, `entry_fx::start_entry_fx_sys`) — so
    /// turning off the opening boot flourish turns off this one too, on the
    /// theory that a player who disabled one entry flourish wants both off,
    /// not just the one named identically to the setting.
    ///
    /// `EntryFxPlugin` is now registered in `main.rs`'s `build_core` (next to
    /// `ZoomPlugin`), so this default is the LIVE default for a fresh
    /// install: `#[serde(default = "default_true")]` (⇒ `true`) mirrors
    /// `boot_intro`'s own default-true shim, so a save from before this
    /// flag existed — same as a fresh install — plays the transition. Every
    /// headless test/harness that constructs `Settings::default()` and never
    /// drives `GameState::Playing` (nearly all of them — see `main.rs`'s
    /// `interactive_core_boots_to_splash_without_panic`) is unaffected: the
    /// transition only ever starts `OnEnter(Playing)`. `--dump` builds no
    /// `App` at all (`entry_fx`'s module doc), so it is structurally
    /// unreachable regardless of this default.
    #[serde(default = "default_true")]
    pub entry_transition: bool,

    /// `SOUND FIELDS` row: the global
    /// intensity multiplier `runtime_glue::fill_field_settings_sys` feeds into
    /// `re_audio::field::FieldSettings::intensity`. Quantized to four
    /// steps (Off/Low/Medium/Full = 0.0/0.33/0.66/1.0) by [`Self::cycle_sound_fields`];
    /// stored as the resolved `f32` (not an enum) so `FieldSettings` never needs a
    /// translation step. `#[serde(default = "default_sound_fields_intensity")]`
    /// so an old save (no `sound_fields_intensity` key) loads Full, unchanged
    /// from this feature's introduction.
    #[serde(default = "default_sound_fields_intensity")]
    pub sound_fields_intensity: f32,
    /// `FIELD EFFECTS` row: `sound_fields_reduced` (§3.4/§3.5). An INDEPENDENT
    /// accessibility setting — NOT tied to [`glitch_enabled`](Self::glitch_enabled)
    /// (owner decision 2026-09-27, recorded in the design doc). Default `false`
    /// (Full effects) so an old save (no `sound_fields_reduced` key) is unchanged.
    #[serde(default)]
    pub sound_fields_reduced: bool,

    /// `BRIGHTNESS` row (Swaggerfall study report item 4b): the index into
    /// [`BRIGHTNESS_STEPS`] driving [`Self::postfx_config`]'s lift/gain override.
    /// Default index `2` (`0.0`, neutral) → [`postfx_config`](Self::postfx_config)
    /// is byte-identical to the chosen POST-FX preset with no brightness applied.
    /// `#[serde(default = "default_brightness_index")]` so an old save (no
    /// `brightness` key) loads neutral.
    #[serde(default = "default_brightness_index")]
    pub brightness: usize,
    /// `COLOUR STEPS` row (Swaggerfall study report item 9): drives the live
    /// [`re_present::PresentConfig::color_deadband`] the present layer's
    /// temporal colour-deadband reads. Default [`ColorSteps::Light`] reproduces
    /// [`re_present::PresentConfig::DEFAULT_DEADBAND`] — TODAY's shipped
    /// behaviour — so an old save (no `color_steps` key) is unchanged.
    #[serde(default)]
    pub color_steps: ColorSteps,
    /// `FIRST STEPS` row (Swaggerfall study report item 2): shows/hides the
    /// self-ticking onboarding checklist in the play HUD
    /// ([`crate::first_steps::hud_lines`]). Observation itself always runs —
    /// this only gates the HUD block's visibility. `#[serde(default =
    /// "default_true")]` so an old save (no `first_steps_hud` key) loads with
    /// the checklist visible, unchanged from this feature's introduction.
    #[serde(default = "default_true")]
    pub first_steps_hud: bool,
}

/// The color mode a fresh install (or an old save missing the `color` key) boots
/// with: [`ColorMode::resolve`]'s env auto-detect (`COLORTERM` / the Windows VT
/// probe), NOT a hardcoded [`ColorMode::Truecolor`] — a player on a real 256-color
/// terminal deserves auto-detect on their very first run, same as every later one.
/// `explicit = None` here because this IS the "no explicit choice yet" case; once the
/// player has a saved value (or picks one in the settings menu), it round-trips as
/// that explicit value and this function is never consulted again for them.
fn default_color() -> ColorMode {
    ColorMode::resolve(None)
}
fn default_true() -> bool {
    true
}
fn default_volume() -> f32 {
    0.8
}
fn default_fov_deg() -> u32 {
    65
}
fn default_stretch() -> f32 {
    1.0
}
fn default_shadows() -> ShadowQuality {
    ShadowQuality::Soft
}
fn default_draw_distance() -> u32 {
    // The historical FAR_FREEZE_DIST (32) — the High preset / byte-identical reach.
    re_render::ascii_shader::FAR_FREEZE_DIST as u32
}
fn default_postfx() -> String {
    "off".to_string()
}
fn default_sound_fields_intensity() -> f32 {
    SOUND_FIELDS_STEPS[SOUND_FIELDS_STEPS.len() - 1] // Full
}
fn default_brightness_index() -> usize {
    BRIGHTNESS_STEPS.len() / 2 // the neutral (0.0) middle step
}

/// `sound_fields` (§3.4): Off/Low/Medium/Full = 0.0/0.33/0.66/1.0, in cycle order.
pub const SOUND_FIELDS_STEPS: [f32; 4] = [0.0, 0.33, 0.66, 1.0];
/// Display labels matching [`SOUND_FIELDS_STEPS`] 1:1.
pub const SOUND_FIELDS_LABELS: [&str; 4] = ["OFF", "LOW", "MEDIUM", "FULL"];

// ───────────────────────────── BRIGHTNESS ───────────────────────────────────
//
// Swaggerfall study report item 4b: a BRIGHTNESS row driving the existing
// `re_postfx` lift/gamma/gain grade knobs. Neutral (`0.0`, index 2) leaves
// `postfx_config()`'s output byte-for-byte whatever the chosen POST-FX preset
// already produces — see `Settings::postfx_config`'s early-return.

/// Discrete brightness steps: an additive lift into the shadows plus a
/// multiplicative gain, in cycle order. `0.0` (index 2, the default) is
/// neutral — the row does not exist as far as the render is concerned.
pub const BRIGHTNESS_STEPS: [f32; 5] = [-0.2, -0.1, 0.0, 0.1, 0.2];
/// Display labels matching [`BRIGHTNESS_STEPS`] 1:1.
pub const BRIGHTNESS_LABELS: [&str; 5] = ["-2", "-1", "0", "+1", "+2"];
/// How much of a brightness step goes into `GradeFx::lift` (shadows) versus
/// `GradeFx::gain` (overall): lift moves by `step * LIFT_SCALE`, gain moves by
/// `1.0 + step`. Lift is the one that matters most for this game — dark
/// terminal palettes need their SHADOWS raised, not just their highlights.
const BRIGHTNESS_LIFT_SCALE: f32 = 0.15;

// ───────────────────────────── COLOUR STEPS ─────────────────────────────────
//
// Swaggerfall study report item 9: expose `re_present::PresentCache`'s colour
// deadband as a player-facing setting. `Light` reproduces today's shipped
// default (`PresentConfig::DEFAULT_DEADBAND` = 6) byte-for-byte.

/// The COLOUR STEPS row: how aggressively the present layer holds a cell's
/// last-shown colour against small drift, trading colour precision for far
/// fewer terminal writes on a slow link/terminal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorSteps {
    /// Deadband `0` — every colour delta is emitted (legacy / maximum fidelity).
    Off,
    /// Deadband = [`re_present::PresentConfig::DEFAULT_DEADBAND`] (6) — TODAY's
    /// shipped default.
    #[default]
    Light,
    /// Deadband = `2×` the default (12) — coarser steps, fewer writes still.
    Strong,
}

impl ColorSteps {
    /// Cycle Off → Light → Strong → Off (`forward`), or the reverse.
    pub fn next(self, forward: bool) -> ColorSteps {
        let order = [ColorSteps::Off, ColorSteps::Light, ColorSteps::Strong];
        let i = order.iter().position(|&c| c == self).unwrap_or(1);
        let n = order.len();
        let j = if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        order[j]
    }

    /// A short label for the settings row.
    pub fn label(self) -> &'static str {
        match self {
            ColorSteps::Off => "OFF",
            ColorSteps::Light => "LIGHT",
            ColorSteps::Strong => "STRONG",
        }
    }

    /// The live [`re_present::PresentConfig::color_deadband`] this step produces.
    pub fn deadband(self) -> u8 {
        match self {
            ColorSteps::Off => 0,
            ColorSteps::Light => re_present::PresentConfig::DEFAULT_DEADBAND,
            ColorSteps::Strong => re_present::PresentConfig::DEFAULT_DEADBAND.saturating_mul(2),
        }
    }
}

/// The allowed FOV presets, in cycle order. The settings row and the live
/// `[`/`]` hotkey both walk this list with wrap.
pub const FOV_PRESETS: [u32; 4] = [60, 65, 70, 75];
/// The inclusive `stretch` range the menu may dial to.
pub const STRETCH_MIN: f32 = 0.6;
/// The inclusive `stretch` range the menu may dial to.
pub const STRETCH_MAX: f32 = 1.5;

/// Index into [`FOV_PRESETS`] of the preset closest to `deg` (used to snap an
/// off-list value before cycling / on load).
fn nearest_fov_index(deg: u32) -> usize {
    FOV_PRESETS
        .iter()
        .enumerate()
        .min_by_key(|(_, &p)| p.abs_diff(deg))
        .map(|(i, _)| i)
        .unwrap_or(1)
}

/// serde adaptor module so `color` round-trips through a string.
mod color_serde {
    use super::{color_from_str, color_to_str};
    use re_present::ColorMode;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(c: &ColorMode, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(color_to_str(*c))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<ColorMode, D::Error> {
        let raw = String::deserialize(d)?;
        Ok(color_from_str(&raw))
    }
}

/// serde adaptor module so `fps_cap` round-trips through a token string
/// (`"off"`/`"30"`/`"60"`/`"120"`).
mod fps_serde {
    use super::FpsCap;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(c: &FpsCap, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&c.to_token())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<FpsCap, D::Error> {
        let raw = String::deserialize(d)?;
        Ok(FpsCap::parse_token(&raw).unwrap_or_default())
    }
}

/// serde adaptor module so `shadows` round-trips through a string.
mod shadows_serde {
    use super::{shadows_from_str, shadows_to_str};
    use re_render::ShadowQuality;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(q: &ShadowQuality, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(shadows_to_str(*q))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<ShadowQuality, D::Error> {
        let raw = String::deserialize(d)?;
        Ok(shadows_from_str(&raw))
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            color: default_color(),
            mouse_enabled: true,
            mouse_sens: MouseSens::Medium,
            muted: false,
            volume: 0.8,
            glitch_enabled: true,
            fov_deg: 65,
            stretch: 1.0,
            shadows: ShadowQuality::Soft,
            quality: QualityPreset::High,
            draw_distance: default_draw_distance(),
            fps_cap: FpsCap::Fps(60),
            lod_quality: LodQuality::Balanced,
            subcell: true,
            postfx: default_postfx(),
            lore_fourth_wall: true,
            colour_unlock_enabled: true,
            boot_intro: true,
            entry_transition: true,
            sound_fields_intensity: default_sound_fields_intensity(),
            sound_fields_reduced: false,
            brightness: default_brightness_index(),
            color_steps: ColorSteps::default(),
            first_steps_hud: true,
        }
    }
}

impl Settings {
    /// The effective [`re_present::MouseSettings`] this config produces.
    pub fn mouse_settings(&self) -> re_present::MouseSettings {
        re_present::MouseSettings {
            enabled: self.mouse_enabled,
            sensitivity: self.mouse_sens.multiplier(),
        }
    }

    /// The effective master volume `re_audio` should apply: 0 when muted, else
    /// the clamped `volume`. Driving the [`re_audio::SetMasterVolume`] message
    /// off this keeps mute + volume consistent.
    pub fn effective_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume.clamp(0.0, 1.0)
        }
    }

    // ── value cycling helpers, used by the settings menu ─────────────────────

    /// Toggle truecolor ⇄ 256.
    pub fn cycle_color(&mut self) {
        self.color = match self.color {
            ColorMode::Truecolor => ColorMode::Ansi256,
            ColorMode::Ansi256 => ColorMode::Truecolor,
        };
    }

    /// Step the volume by `delta` (e.g. ±0.1), clamped to `[0, 1]` and rounded
    /// to one decimal so the persisted value stays tidy.
    pub fn step_volume(&mut self, delta: f32) {
        let v = (self.volume + delta).clamp(0.0, 1.0);
        // Round to the nearest 0.1 to avoid float drift accumulating.
        self.volume = (v * 10.0).round() / 10.0;
    }

    /// Cycle the FOV preset through [`FOV_PRESETS`] with wrap. `forward` walks
    /// 60→65→70→75→60; `false` reverses. An off-list stored value snaps to the
    /// nearest entry on the first cycle.
    pub fn cycle_fov(&mut self, forward: bool) {
        let i = FOV_PRESETS
            .iter()
            .position(|&p| p == self.fov_deg)
            .unwrap_or_else(|| nearest_fov_index(self.fov_deg));
        let n = FOV_PRESETS.len();
        let j = if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        self.fov_deg = FOV_PRESETS[j];
    }

    /// Snap `fov_deg` to the nearest allowed preset (used on load to repair a
    /// hand-edited / off-list value).
    pub fn set_fov_clamped(&mut self, deg: u32) {
        self.fov_deg = FOV_PRESETS
            .iter()
            .copied()
            .find(|&p| p == deg)
            .unwrap_or_else(|| FOV_PRESETS[nearest_fov_index(deg)]);
    }

    /// Step the stretch override by `delta` (e.g. ±0.05), clamped to
    /// `[STRETCH_MIN, STRETCH_MAX]` and rounded to two decimals so the persisted
    /// value stays tidy (mirrors [`step_volume`](Self::step_volume)).
    pub fn step_stretch(&mut self, delta: f32) {
        let v = (self.stretch + delta).clamp(STRETCH_MIN, STRETCH_MAX);
        // Round to the nearest 0.01 to avoid float drift accumulating.
        self.stretch = (v * 100.0).round() / 100.0;
    }

    /// Cycle the cast-shadow quality through Off → Hard → Soft → Off (`forward`),
    /// or the reverse. Mirrors the other preset cyclers; drives the live
    /// [`re_render::ShadowQuality`] resource through [`Self::shadow_quality`].
    pub fn cycle_shadows(&mut self, forward: bool) {
        let order = [ShadowQuality::Off, ShadowQuality::Hard, ShadowQuality::Soft];
        let i = order.iter().position(|&q| q == self.shadows).unwrap_or(2);
        let n = order.len();
        let j = if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        self.shadows = order[j];
    }

    /// The live [`re_render::ShadowQuality`] this config produces. Inserting it
    /// before `CompositeRenderPlugin` (whose `init_resource` is insert-if-absent)
    /// is what makes the saved value win; re-applying it on a change updates the
    /// live shadow pass (mirrors [`Self::fov`]).
    pub fn shadow_quality(&self) -> ShadowQuality {
        self.shadows
    }

    // ── GRAPHICS QUALITY — preset bundles + per-knob levers ──────────────────

    /// Apply a quality PRESET: write the whole bundle of knobs (draw distance,
    /// shadows, LOD, sub-cell, post-fx, FPS) and set `quality`. `Custom` is a no-op
    /// (it has no canonical bundle — it IS the hand-tuned state). The `High` bundle
    /// reproduces today's render exactly, so selecting High on a fresh install is a
    /// no-op against the defaults.
    pub fn apply_preset(&mut self, preset: QualityPreset) {
        match preset {
            QualityPreset::Low => {
                self.draw_distance = 16;
                self.shadows = ShadowQuality::Off;
                self.lod_quality = LodQuality::Aggressive;
                self.subcell = false;
                self.postfx = "off".to_string();
                self.fps_cap = FpsCap::Fps(30);
            }
            QualityPreset::Medium => {
                self.draw_distance = 24;
                self.shadows = ShadowQuality::Hard;
                self.lod_quality = LodQuality::Balanced;
                self.subcell = true;
                self.postfx = "off".to_string();
                self.fps_cap = FpsCap::Fps(60);
            }
            QualityPreset::High => {
                // The shipping default — byte-identical to today's render.
                self.draw_distance = default_draw_distance();
                self.shadows = ShadowQuality::Soft;
                self.lod_quality = LodQuality::Balanced;
                self.subcell = true;
                self.postfx = "off".to_string();
                self.fps_cap = FpsCap::Fps(60);
            }
            QualityPreset::Ultra => {
                self.draw_distance = 48;
                self.shadows = ShadowQuality::Soft;
                self.lod_quality = LodQuality::Near;
                self.subcell = true;
                self.postfx = "cinematic".to_string();
                self.fps_cap = FpsCap::Off;
            }
            QualityPreset::Custom => return,
        }
        self.quality = preset;
    }

    /// Re-derive `quality` from the current knobs: if they exactly match one preset's
    /// bundle, snap `quality` to it; otherwise it is `Custom`. Called after any
    /// individual-knob edit so the preset label tracks reality (the "editing a knob
    /// flips to Custom" rule, but also re-recognises a hand-set bundle).
    pub fn resync_preset(&mut self) {
        for preset in QualityPreset::CYCLE {
            let mut probe = self.clone();
            probe.apply_preset(preset);
            if probe.quality_bundle_eq(self) {
                self.quality = preset;
                return;
            }
        }
        self.quality = QualityPreset::Custom;
    }

    /// Whether the quality-knob bundle (the fields a preset sets) matches `other`.
    /// Ignores the non-quality fields (colour, mouse, audio, fov, …).
    fn quality_bundle_eq(&self, other: &Settings) -> bool {
        self.draw_distance == other.draw_distance
            && self.shadows == other.shadows
            && self.lod_quality == other.lod_quality
            && self.subcell == other.subcell
            && self.postfx == other.postfx
            && self.fps_cap == other.fps_cap
    }

    /// The live [`re_render::FarFreeze`] this config produces: the draw distance is the
    /// freeze cutoff, and a shorter draw distance also lowers the DDA step cap (cheaper
    /// cast). The default draw distance (32) yields the historical
    /// [`re_render::FarFreeze::default`] (32 / 256) → byte-identical.
    pub fn far_freeze(&self) -> re_render::FarFreeze {
        let dd = self.draw_distance.max(1);
        // Scale the DDA cap with the draw distance, but never below the historical 256
        // for the default reach (so the High default is byte-identical), and never
        // above it (a shorter reach only ever REDUCES cost). Roughly ~8 steps/cell with
        // a generous margin, clamped to [64, 256].
        let default_dd = default_draw_distance();
        let max_dda_steps = if dd >= default_dd {
            re_render::draw_dist::DEFAULT_MAX_DDA_STEPS
        } else {
            (dd * 8).clamp(64, re_render::draw_dist::DEFAULT_MAX_DDA_STEPS)
        };
        re_render::FarFreeze {
            distance: dd as f32,
            max_dda_steps,
        }
    }

    /// The live [`re_render::LodConfig`] this config produces (LOD aggressiveness +
    /// the draw-distance cap on `far_start`). `Balanced` + default draw distance =
    /// the engine default → byte-identical.
    pub fn lod_config(&self) -> re_render::LodConfig {
        self.lod_quality.lod_config(self.draw_distance)
    }

    /// The live [`re_render::composite::SubCellConfig`] this config produces.
    pub fn subcell_config(&self) -> re_render::composite::SubCellConfig {
        re_render::composite::SubCellConfig {
            enabled: self.subcell,
        }
    }

    /// The live [`re_postfx::PostfxConfig`] this config's post-fx preset name produces,
    /// with the `BRIGHTNESS` row (study report item 4b) folded onto the grade's
    /// `lift`/`gain`. An unknown preset name falls back to `off` (the byte-identical
    /// identity), so a hand-edited bad value never crashes.
    ///
    /// **Neutral brightness is a byte-identical no-op**: at the default index
    /// (`0.0`, `default_brightness_index`) this returns EXACTLY the preset's own
    /// config, untouched — the risk note in the study report ("None for goldens —
    /// the default brightness is neutral") is enforced structurally by the early
    /// return, not by a coincidentally-zero multiplier.
    pub fn postfx_config(&self) -> re_postfx::PostfxConfig {
        let mut cfg = re_postfx::PostfxConfig::from_preset_name(self.postfx.trim())
            .unwrap_or_else(re_postfx::PostfxConfig::off);
        let step = self.brightness_value();
        if step != 0.0 {
            cfg.grade.enabled = true;
            cfg.grade.lift += step * BRIGHTNESS_LIFT_SCALE;
            cfg.grade.gain *= 1.0 + step;
        }
        cfg
    }

    /// The current `BRIGHTNESS` step value ([`BRIGHTNESS_STEPS`]), snapping an
    /// out-of-range index (a hand-edited file) to the neutral middle step.
    fn brightness_value(&self) -> f32 {
        BRIGHTNESS_STEPS
            .get(self.brightness)
            .copied()
            .unwrap_or(0.0)
    }

    /// Cycle the `BRIGHTNESS` row through [`BRIGHTNESS_STEPS`] (clamped, not
    /// wrapped — there is no reason to wrap from brightest to darkest).
    pub fn cycle_brightness(&mut self, forward: bool) {
        let n = BRIGHTNESS_STEPS.len();
        self.brightness = if forward {
            (self.brightness + 1).min(n - 1)
        } else {
            self.brightness.saturating_sub(1)
        };
    }

    /// The display label for the current `BRIGHTNESS` row value.
    #[must_use]
    pub fn brightness_label(&self) -> &'static str {
        BRIGHTNESS_LABELS
            .get(self.brightness)
            .copied()
            .unwrap_or("0")
    }

    /// Cycle the `COLOUR STEPS` row (Off → Light → Strong → Off).
    pub fn cycle_color_steps(&mut self, forward: bool) {
        self.color_steps = self.color_steps.next(forward);
    }

    /// The live [`re_present::PresentConfig::color_deadband`] the current
    /// `COLOUR STEPS` row produces.
    #[must_use]
    pub fn color_steps_deadband(&self) -> u8 {
        self.color_steps.deadband()
    }

    /// Cycle the `SOUND FIELDS` row through [`SOUND_FIELDS_STEPS`], with wrap.
    /// Snaps a hand-edited off-step value to its nearest step first, so the row
    /// always lands on exactly one of the four labelled positions.
    pub fn cycle_sound_fields(&mut self, forward: bool) {
        let cur = SOUND_FIELDS_STEPS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (**a - self.sound_fields_intensity)
                    .abs()
                    .total_cmp(&(**b - self.sound_fields_intensity).abs())
            })
            .map_or(0, |(i, _)| i);
        let n = SOUND_FIELDS_STEPS.len();
        let next = if forward {
            (cur + 1) % n
        } else {
            (cur + n - 1) % n
        };
        self.sound_fields_intensity = SOUND_FIELDS_STEPS[next];
    }

    /// The display label for the current `SOUND FIELDS` row value.
    #[must_use]
    pub fn sound_fields_label(&self) -> &'static str {
        SOUND_FIELDS_STEPS
            .iter()
            .zip(SOUND_FIELDS_LABELS)
            .min_by(|(a, _), (b, _)| {
                (**a - self.sound_fields_intensity)
                    .abs()
                    .total_cmp(&(**b - self.sound_fields_intensity).abs())
            })
            .map_or("FULL", |(_, l)| l)
    }

    /// The live [`re_audio::field::FieldSettings`] this config produces —
    /// `crate::runtime_glue::fill_field_settings_sys`'s only read of the settings resource.
    #[must_use]
    pub fn field_settings(&self) -> re_audio::FieldSettings {
        re_audio::FieldSettings {
            intensity: self.sound_fields_intensity.clamp(0.0, 1.0),
            reduced: self.sound_fields_reduced,
        }
    }

    /// True while the player has never picked a POST-FX preset of their own — the
    /// row still holds the shipped default. A level's `[atmosphere] postfx =`
    /// (lighting spec §1.3) is applied ONLY in that case: an explicit player
    /// choice always outranks the level's request.
    pub fn postfx_is_default(&self) -> bool {
        self.postfx.trim() == default_postfx()
    }

    /// The run-loop seconds-per-frame interval the FPS cap maps to. Read once at
    /// startup to size `ScheduleRunnerPlugin::run_loop` (+ the live throttle reads it).
    pub fn fps_loop_interval_secs(&self) -> f64 {
        self.fps_cap.loop_interval_secs()
    }

    // ── quality cyclers (menu rows) ──────────────────────────────────────────

    /// Cycle the quality PRESET through Low→Medium→High→Ultra (wrap), applying the new
    /// bundle. Skips `Custom` (which is only entered by editing a knob).
    pub fn cycle_quality(&mut self, forward: bool) {
        let cur = QualityPreset::CYCLE
            .iter()
            .position(|&p| p == self.quality)
            // From Custom, forward starts at Low, back starts at Ultra.
            .unwrap_or(if forward {
                QualityPreset::CYCLE.len() - 1
            } else {
                0
            });
        let n = QualityPreset::CYCLE.len();
        let j = if forward {
            (cur + 1) % n
        } else {
            (cur + n - 1) % n
        };
        self.apply_preset(QualityPreset::CYCLE[j]);
    }

    /// Cycle the DRAW DISTANCE through [`DRAW_DISTANCE_PRESETS`] (wrap), then resync
    /// the preset label (→ Custom unless the new bundle matches a preset).
    pub fn cycle_draw_distance(&mut self, forward: bool) {
        let i = DRAW_DISTANCE_PRESETS
            .iter()
            .position(|&d| d == self.draw_distance)
            .unwrap_or(2); // default 32 is index 2
        let n = DRAW_DISTANCE_PRESETS.len();
        let j = if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        self.draw_distance = DRAW_DISTANCE_PRESETS[j];
        self.resync_preset();
    }

    /// Cycle the FPS cap through [`FPS_PRESETS`] (30→60→120→Off→wrap), then resync.
    pub fn cycle_fps(&mut self, forward: bool) {
        let i = FPS_PRESETS
            .iter()
            .position(|&f| f == self.fps_cap)
            .unwrap_or(1);
        let n = FPS_PRESETS.len();
        let j = if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        self.fps_cap = FPS_PRESETS[j];
        self.resync_preset();
    }

    /// Cycle the LOD quality (Aggressive→Balanced→Near→wrap), then resync.
    pub fn cycle_lod_quality(&mut self, forward: bool) {
        let order = [
            LodQuality::Aggressive,
            LodQuality::Balanced,
            LodQuality::Near,
        ];
        let i = order
            .iter()
            .position(|&q| q == self.lod_quality)
            .unwrap_or(1);
        let n = order.len();
        let j = if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        self.lod_quality = order[j];
        self.resync_preset();
    }

    /// Toggle the sub-cell refinement, then resync.
    pub fn toggle_subcell(&mut self) {
        self.subcell = !self.subcell;
        self.resync_preset();
    }

    /// Cycle the POST-FX preset through the named list, then resync.
    pub fn cycle_postfx(&mut self, forward: bool) {
        let names: Vec<&'static str> = re_postfx::PostfxConfig::all_presets()
            .iter()
            .map(|(n, _)| *n)
            .collect();
        let i = names.iter().position(|&n| n == self.postfx).unwrap_or(0);
        let n = names.len();
        let j = if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        self.postfx = names[j].to_string();
        self.resync_preset();
    }

    /// Cycle SHADOWS as a quality knob (Off→Hard→Soft), then resync the preset. (The
    /// existing [`Self::cycle_shadows`] does NOT resync; this is the quality-row
    /// wrapper that does.)
    pub fn cycle_shadows_quality(&mut self, forward: bool) {
        self.cycle_shadows(forward);
        self.resync_preset();
    }

    /// The live [`re_render::Fov`] this config produces: the chosen preset +
    /// player stretch, with the standard terminal cell aspect (`0.5`). Inserting
    /// this before `CompositeRenderPlugin` (whose `init_resource` is
    /// insert-if-absent) is what makes the saved FOV win, and re-applying it on a
    /// change updates the live near cast.
    pub fn fov(&self) -> re_render::Fov {
        re_render::Fov {
            h_preset_deg: self.fov_deg as f32,
            cell_aspect: 0.5,
            stretch: self.stretch,
        }
    }

    // ── persistence (mirrors save.rs) ────────────────────────────────────────

    /// Load from the resolved [`settings_path`]; any failure → defaults.
    pub fn load() -> Self {
        match settings_path() {
            Some(path) => Self::load_from(&path),
            None => {
                eprintln!("terminal-state: no settings location — using defaults this session.");
                Self::default()
            }
        }
    }

    /// Load from an explicit path. Missing → defaults (silent, first-run);
    /// unreadable / corrupt → defaults + a warning. Never panics.
    pub fn load_from(path: &std::path::Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                eprintln!(
                    "terminal-state: could not read settings '{}': {e} — using defaults.",
                    path.display()
                );
                return Self::default();
            }
        };
        match toml::from_str::<Settings>(&text) {
            Ok(s) => s.normalized(),
            Err(e) => {
                eprintln!(
                    "terminal-state: settings '{}' is corrupt ({e}) — using defaults.",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Clamp / snap any loaded field to a valid value (a hand-edited file may
    /// carry an out-of-range volume or an off-list preset).
    fn normalized(mut self) -> Self {
        self.volume = self.volume.clamp(0.0, 1.0);
        // FOV must be one of the allowed presets; snap an off-list value.
        if !FOV_PRESETS.contains(&self.fov_deg) {
            self.set_fov_clamped(self.fov_deg);
        }
        // Stretch clamped into its allowed band (and tidied to 2 decimals).
        let s = self.stretch.clamp(STRETCH_MIN, STRETCH_MAX);
        self.stretch = (s * 100.0).round() / 100.0;
        // Draw distance: snap an off-list / out-of-range value to the nearest preset.
        if !DRAW_DISTANCE_PRESETS.contains(&self.draw_distance) {
            self.draw_distance = DRAW_DISTANCE_PRESETS
                .iter()
                .copied()
                .min_by_key(|&p| p.abs_diff(self.draw_distance))
                .unwrap_or_else(default_draw_distance);
        }
        // Post-fx: an unknown preset name falls back to "off" (the identity).
        if re_postfx::PostfxConfig::from_preset_name(self.postfx.trim()).is_none() {
            self.postfx = default_postfx();
        }
        // Re-derive the preset label from the (now-valid) knobs, so a hand-edited file
        // whose knobs match a preset shows that preset, and a mismatched one shows
        // Custom (never a stale label).
        self.resync_preset();
        self
    }

    /// Persist to the resolved [`settings_path`]; best-effort, never panics.
    pub fn save(&self) {
        let Some(path) = settings_path() else {
            eprintln!("terminal-state: no settings location — settings not persisted.");
            return;
        };
        self.save_to(&path);
    }

    /// Persist to an explicit path, creating parent dirs. Failures logged + swallowed.
    pub fn save_to(&self, path: &std::path::Path) {
        let text = match toml::to_string(self) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("terminal-state: could not serialize settings ({e}) — not saved.");
                return;
            }
        };
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!(
                        "terminal-state: could not create settings dir '{}': {e} — not saved.",
                        parent.display()
                    );
                    return;
                }
            }
        }
        if let Err(e) = std::fs::write(path, text) {
            eprintln!(
                "terminal-state: could not write settings '{}': {e} — not saved.",
                path.display()
            );
        }
    }
}

/// The on-disk settings file name.
const SETTINGS_FILE: &str = "settings.toml";

/// Resolve where the settings file lives. SAME search order as
/// [`crate::save::progress_path`] (sharing `TS_SAVE_DIR` and the OS config
/// dir), differing only in the file name — so settings sit beside the save.
pub fn settings_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("TS_SAVE_DIR") {
        return Some(PathBuf::from(dir).join(SETTINGS_FILE));
    }
    if let Some(dir) = config_dir() {
        return Some(dir.join("TerminalState").join(SETTINGS_FILE));
    }
    Some(PathBuf::from("save").join(SETTINGS_FILE))
}

/// The OS config directory, from environment variables only (mirrors `save.rs`).
fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg));
            }
        }
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        p.push(format!("ts_settings_{tag}_{nonce}"));
        p.push(SETTINGS_FILE);
        p
    }

    #[test]
    fn round_trips_through_save_and_load() {
        let path = temp_path("roundtrip");
        let s = Settings {
            color: ColorMode::Ansi256,
            mouse_enabled: false,
            mouse_sens: MouseSens::High,
            muted: true,
            volume: 0.3,
            glitch_enabled: false,
            fov_deg: 70,
            stretch: 1.15,
            shadows: ShadowQuality::Hard,
            quality: QualityPreset::Custom,
            draw_distance: 48,
            fps_cap: FpsCap::Off,
            lod_quality: LodQuality::Near,
            subcell: false,
            postfx: "cinematic".to_string(),
            lore_fourth_wall: false,
            colour_unlock_enabled: false,
            boot_intro: false,
            entry_transition: true,
            sound_fields_intensity: 0.33,
            sound_fields_reduced: true,
            brightness: 0,
            color_steps: ColorSteps::Strong,
            first_steps_hud: false,
        };
        s.save_to(&path);
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded, s);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_loads_defaults_without_panic() {
        let path = temp_path("missing");
        let _ = std::fs::remove_file(&path);
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn corrupt_file_loads_defaults_without_panic() {
        let path = temp_path("corrupt");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "this is not toml ][ = =").unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded, Settings::default());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn out_of_range_values_are_normalized_on_load() {
        let path = temp_path("normalize");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        // volume above 1.0 must be snapped on load.
        std::fs::write(&path, "volume = 9.0\n").unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.volume, 1.0, "volume clamped to 1.0");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn old_partial_save_fills_defaults() {
        // A file with only some keys still loads — serde `default`s fill the rest.
        let path = temp_path("partial");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "color = \"256\"\n").unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.color, ColorMode::Ansi256);
        assert!(loaded.mouse_enabled, "missing mouse_enabled defaults true");
        assert_eq!(loaded.mouse_sens, MouseSens::Medium);
        assert!(
            loaded.glitch_enabled,
            "missing glitch_enabled defaults true"
        );
        assert!(
            loaded.boot_intro,
            "missing boot_intro defaults true — an old save must still boot into the intro"
        );
        assert!(loaded.entry_transition, "missing entry_transition defaults true — an old save must still play the level-entry sweep");
        let _ = std::fs::remove_file(&path);
    }

    // ── level-entry "resolve" transition opt-in ──────────────────────────────

    #[test]
    fn entry_transition_defaults_on_and_round_trips_off() {
        assert!(
            Settings::default().entry_transition,
            "the entry transition is ON by default, mirroring boot_intro"
        );
        let path = temp_path("entry_transition");
        let s = Settings {
            entry_transition: false,
            ..Default::default()
        };
        s.save_to(&path);
        let loaded = Settings::load_from(&path);
        assert!(!loaded.entry_transition, "the opt-out persists");
        let _ = std::fs::remove_file(&path);
    }

    // ── colour auto-detect wiring ─────────────────────────────────────────────
    //
    // One test function, not two, deliberately: both scenarios mutate the SAME
    // process-global `COLORTERM` env var, and `cargo test` runs unit tests on
    // separate threads within one binary by default — two independent `#[test]`
    // functions racing on that var would be flaky. Running both scenarios
    // sequentially inside one test (and restoring/removing the var at the end)
    // keeps this deterministic without reaching for a cross-crate env-lock.

    #[test]
    fn color_default_resolves_from_env_but_an_explicit_choice_always_wins() {
        // Scenario 1 — no `color` key in the file at all (a pre-color-row save, or a
        // fresh file the player never touched): `default_color()` must track
        // `ColorMode::resolve(None)`'s env auto-detect, not hardcode Truecolor. Force a
        // COLORTERM value that classifies to Ansi256 so the assertion cannot pass by
        // coincidence with the old hardcoded-Truecolor behavior.
        let path_absent = temp_path("color_auto_absent");
        if let Some(parent) = path_absent.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::env::set_var("COLORTERM", "not-truecolor-value");
        std::fs::write(&path_absent, "volume = 0.5\n").unwrap(); // no `color` key
        let loaded_absent = Settings::load_from(&path_absent);
        assert_eq!(
            loaded_absent.color,
            ColorMode::Ansi256,
            "missing color key must resolve from the env (COLORTERM), not hardcode Truecolor"
        );
        assert_eq!(
            default_color(),
            ColorMode::resolve(None),
            "default_color() must be exactly ColorMode::resolve(None)"
        );
        let _ = std::fs::remove_file(&path_absent);

        // Scenario 2 — an explicit SAVED choice must be kept regardless of what the
        // environment would auto-detect. COLORTERM here classifies to Truecolor, but
        // the file explicitly saved "256" — the explicit choice must win.
        let path_explicit = temp_path("color_explicit_kept");
        if let Some(parent) = path_explicit.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::env::set_var("COLORTERM", "truecolor");
        std::fs::write(&path_explicit, "color = \"256\"\n").unwrap();
        let loaded_explicit = Settings::load_from(&path_explicit);
        assert_eq!(
            loaded_explicit.color,
            ColorMode::Ansi256,
            "an explicit saved color must not be overridden by env auto-detect"
        );
        let _ = std::fs::remove_file(&path_explicit);

        std::env::remove_var("COLORTERM");
    }

    #[test]
    fn mouse_settings_reflect_preset() {
        let mut s = Settings::default();
        assert_eq!(s.mouse_settings().sensitivity, 1.0);
        s.mouse_sens = MouseSens::Low;
        assert_eq!(s.mouse_settings().sensitivity, 0.5);
        s.mouse_sens = MouseSens::High;
        assert_eq!(s.mouse_settings().sensitivity, 1.75);
        s.mouse_enabled = false;
        assert!(!s.mouse_settings().enabled);
    }

    #[test]
    fn effective_volume_honors_mute() {
        let mut s = Settings {
            volume: 0.6,
            muted: false,
            ..Default::default()
        };
        assert_eq!(s.effective_volume(), 0.6);
        s.muted = true;
        assert_eq!(s.effective_volume(), 0.0, "mute forces 0");
        // Un-mute restores the stored level (it was never zeroed).
        s.muted = false;
        assert_eq!(s.effective_volume(), 0.6);
    }

    #[test]
    fn cyclers_walk_their_value_sets() {
        // `color` on a fresh `Settings::default()` now comes from `ColorMode::resolve`'s
        // env auto-detect (not a hardcoded constant), so this test — which is about
        // `cycle_color()` toggling, not about what the default happens to be in
        // whatever environment `cargo test` runs under — pins a known starting value.
        let mut s = Settings {
            color: ColorMode::Truecolor,
            ..Settings::default()
        };
        s.cycle_color();
        assert_eq!(s.color, ColorMode::Ansi256);
        s.cycle_color();
        assert_eq!(s.color, ColorMode::Truecolor);

        assert_eq!(MouseSens::Low.next(), MouseSens::Medium);
        assert_eq!(MouseSens::High.next(), MouseSens::Low);
        assert_eq!(MouseSens::Low.prev(), MouseSens::High);

        s.volume = 0.95;
        s.step_volume(0.1);
        assert_eq!(s.volume, 1.0, "clamps at 1.0");
        s.volume = 0.05;
        s.step_volume(-0.1);
        assert_eq!(s.volume, 0.0, "clamps at 0.0");
    }

    #[test]
    fn fov_cycles_presets_with_wrap() {
        let mut s = Settings::default();
        assert_eq!(s.fov_deg, 65);
        s.cycle_fov(true);
        assert_eq!(s.fov_deg, 70);
        s.cycle_fov(true);
        assert_eq!(s.fov_deg, 75);
        s.cycle_fov(true);
        assert_eq!(s.fov_deg, 60, "wraps past the top preset");
        s.cycle_fov(false);
        assert_eq!(s.fov_deg, 75, "reverse wraps");
    }

    #[test]
    fn fov_off_list_value_snaps_then_cycles() {
        let mut s = Settings {
            fov_deg: 63,
            ..Default::default()
        }; // hand-edited off-list value
        s.set_fov_clamped(63);
        assert_eq!(s.fov_deg, 65, "63 snaps to the nearest preset 65");
        // cycle from an off-list value snaps to nearest (75) first, then steps
        // forward — which wraps past the top preset back to 60.
        s.fov_deg = 99;
        s.cycle_fov(true);
        assert_eq!(
            s.fov_deg, 60,
            "99 snaps to 75 (nearest), forward wraps to 60"
        );
        // And reverse from a snapped value steps down (75 → 70).
        s.fov_deg = 99;
        s.cycle_fov(false);
        assert_eq!(s.fov_deg, 70, "99 snaps to 75, reverse steps to 70");
    }

    #[test]
    fn fov_drives_render_fov_resource() {
        let s = Settings {
            fov_deg: 70,
            stretch: 1.2,
            ..Default::default()
        };
        let f = s.fov();
        assert_eq!(f.h_preset_deg, 70.0);
        assert_eq!(f.cell_aspect, 0.5);
        assert!((f.stretch - 1.2).abs() < 1e-6);
    }

    #[test]
    fn stretch_steps_and_clamps() {
        let mut s = Settings::default();
        assert!((s.stretch - 1.0).abs() < 1e-6);
        s.step_stretch(0.05);
        assert!((s.stretch - 1.05).abs() < 1e-6);
        // clamp at the top.
        s.stretch = 1.48;
        s.step_stretch(0.05);
        assert_eq!(s.stretch, STRETCH_MAX, "clamps at the max");
        // clamp at the bottom.
        s.stretch = 0.62;
        s.step_stretch(-0.05);
        assert_eq!(s.stretch, STRETCH_MIN, "clamps at the min");
    }

    #[test]
    fn fov_and_stretch_round_trip_through_save_and_load() {
        let path = temp_path("fov_roundtrip");
        let s = Settings {
            fov_deg: 75,
            stretch: 1.25,
            ..Default::default()
        };
        s.save_to(&path);
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.fov_deg, 75);
        assert!((loaded.stretch - 1.25).abs() < 1e-6);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn out_of_range_fov_and_stretch_normalized_on_load() {
        let path = temp_path("fov_normalize");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "fov_deg = 200\nstretch = 9.0\n").unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.fov_deg, 75, "off-list fov snaps to nearest preset");
        assert_eq!(loaded.stretch, STRETCH_MAX, "out-of-range stretch clamps");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn old_save_without_fov_keys_defaults() {
        // A pre-FOV save must still load (serde defaults fill fov/stretch).
        let path = temp_path("fov_old_save");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "color = \"256\"\nvolume = 0.5\n").unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.fov_deg, 65, "missing fov_deg defaults to 65");
        assert!(
            (loaded.stretch - 1.0).abs() < 1e-6,
            "missing stretch defaults to 1.0"
        );
        let _ = std::fs::remove_file(&path);
    }

    // ── GRAPHICS QUALITY ─────────────────────────────────────────────────────

    #[test]
    fn default_quality_is_high_and_byte_identical_to_today() {
        // The DEFAULT quality must reproduce TODAY's render exactly: the FarFreeze /
        // LodConfig / SubCellConfig / PostfxConfig it produces are the engine defaults.
        let s = Settings::default();
        assert_eq!(s.quality, QualityPreset::High);
        assert_eq!(
            s.far_freeze(),
            re_render::FarFreeze::default(),
            "default draw distance yields the historical FarFreeze (32 / 256)"
        );
        assert_eq!(
            s.lod_config(),
            re_render::LodConfig::default(),
            "Balanced LOD + default draw distance = the engine-default LodConfig"
        );
        assert!(s.subcell_config().enabled, "sub-cell on by default");
        assert_eq!(
            s.postfx_config(),
            re_postfx::PostfxConfig::off(),
            "post-fx off by default (the identity)"
        );
        assert_eq!(s.fps_cap, FpsCap::Fps(60));
    }

    #[test]
    fn presets_set_the_expected_bundle() {
        let mut s = Settings::default();

        s.apply_preset(QualityPreset::Low);
        assert_eq!(s.draw_distance, 16);
        assert_eq!(s.shadows, ShadowQuality::Off);
        assert_eq!(s.lod_quality, LodQuality::Aggressive);
        assert!(!s.subcell);
        assert_eq!(s.postfx, "off");
        assert_eq!(s.fps_cap, FpsCap::Fps(30));
        assert_eq!(s.quality, QualityPreset::Low);

        s.apply_preset(QualityPreset::Ultra);
        assert_eq!(s.draw_distance, 48);
        assert_eq!(s.shadows, ShadowQuality::Soft);
        assert_eq!(s.lod_quality, LodQuality::Near);
        assert!(s.subcell);
        assert_eq!(s.postfx, "cinematic");
        assert_eq!(s.fps_cap, FpsCap::Off);
        assert_eq!(s.quality, QualityPreset::Ultra);
    }

    #[test]
    fn editing_a_knob_flips_the_preset_to_custom() {
        let mut s = Settings::default(); // High
        assert_eq!(s.quality, QualityPreset::High);
        // Cycle the draw distance off the High value (32 → 48) → no preset matches.
        s.cycle_draw_distance(true);
        assert_eq!(s.draw_distance, 48);
        assert_eq!(
            s.quality,
            QualityPreset::Custom,
            "an off-bundle knob → Custom"
        );
    }

    #[test]
    fn cycling_back_onto_a_bundle_resyncs_the_preset() {
        let mut s = Settings::default();
        s.apply_preset(QualityPreset::Low);
        assert_eq!(s.quality, QualityPreset::Low);
        // Toggle sub-cell ON then OFF — back onto the Low bundle → re-recognised as Low.
        s.toggle_subcell();
        assert_eq!(s.quality, QualityPreset::Custom);
        s.toggle_subcell();
        assert_eq!(
            s.quality,
            QualityPreset::Low,
            "back onto the bundle → Low again"
        );
    }

    #[test]
    fn shorter_draw_distance_reduces_the_resolved_far_cap() {
        // Draw distance drives the FarFreeze cutoff, the DDA step cap, AND the LOD
        // far_start — a shorter distance reduces ALL THREE resolved far caps.
        let long = Settings {
            draw_distance: 48,
            ..Settings::default()
        }; // Ultra-ish reach
        let short = Settings {
            draw_distance: 16,
            lod_quality: LodQuality::Aggressive,
            ..Settings::default()
        }; // Low reach

        assert!(
            short.far_freeze().distance < long.far_freeze().distance,
            "shorter draw distance → smaller far-freeze cutoff"
        );
        assert!(
            short.far_freeze().max_dda_steps <= long.far_freeze().max_dda_steps,
            "shorter draw distance never costs MORE DDA steps"
        );
        assert!(
            short.lod_config().far_start <= long.lod_config().far_start,
            "shorter draw distance → LOD far_start no larger"
        );
        // And the short reach is strictly below the historical default cap.
        assert!(
            short.far_freeze().distance < re_render::FarFreeze::default().distance,
            "a 16-cell draw distance is below the historical 32-cell freeze"
        );
    }

    #[test]
    fn fps_cap_maps_to_the_run_loop_interval() {
        assert!((FpsCap::Fps(60).loop_interval_secs() - 1.0 / 60.0).abs() < 1e-9);
        assert!((FpsCap::Fps(30).loop_interval_secs() - 1.0 / 30.0).abs() < 1e-9);
        assert!((FpsCap::Fps(120).loop_interval_secs() - 1.0 / 120.0).abs() < 1e-9);
        assert_eq!(
            FpsCap::Off.loop_interval_secs(),
            0.0,
            "uncapped → 0 interval"
        );
        let s = Settings {
            fps_cap: FpsCap::Fps(30),
            ..Default::default()
        };
        assert!((s.fps_loop_interval_secs() - 1.0 / 30.0).abs() < 1e-9);
    }

    #[test]
    fn fps_cap_token_round_trips() {
        assert_eq!(FpsCap::parse_token("30"), Some(FpsCap::Fps(30)));
        assert_eq!(FpsCap::parse_token("OFF"), Some(FpsCap::Off));
        assert_eq!(FpsCap::parse_token("uncapped"), Some(FpsCap::Off));
        assert_eq!(FpsCap::parse_token("0"), Some(FpsCap::Off));
        assert_eq!(FpsCap::parse_token("nonsense"), None);
        assert_eq!(FpsCap::Fps(120).to_token(), "120");
        assert_eq!(FpsCap::Off.to_token(), "off");
    }

    #[test]
    fn quality_block_round_trips_through_save_and_load() {
        let path = temp_path("quality_roundtrip");
        let mut s = Settings::default();
        s.apply_preset(QualityPreset::Ultra);
        s.save_to(&path);
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.quality, QualityPreset::Ultra);
        assert_eq!(loaded.draw_distance, 48);
        assert_eq!(loaded.fps_cap, FpsCap::Off);
        assert_eq!(loaded.lod_quality, LodQuality::Near);
        assert!(loaded.subcell);
        assert_eq!(loaded.postfx, "cinematic");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn old_save_without_quality_keys_defaults_to_high_bundle() {
        // A pre-quality save must still load: serde-defaults fill the quality block to
        // the High (byte-identical) bundle.
        let path = temp_path("quality_old_save");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "color = \"256\"\nvolume = 0.5\n").unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(
            loaded.quality,
            QualityPreset::High,
            "missing quality → High"
        );
        assert_eq!(loaded.draw_distance, 32);
        assert_eq!(loaded.fps_cap, FpsCap::Fps(60));
        assert_eq!(loaded.lod_quality, LodQuality::Balanced);
        assert!(loaded.subcell);
        assert_eq!(loaded.postfx, "off");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn off_list_draw_distance_and_bad_postfx_normalize_on_load() {
        let path = temp_path("quality_normalize");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "draw_distance = 1000\npostfx = \"bogus\"\n").unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(
            loaded.draw_distance, 48,
            "off-list draw distance snaps to nearest preset"
        );
        assert_eq!(
            loaded.postfx, "off",
            "unknown post-fx preset falls back to off"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn quality_cycler_walks_low_medium_high_ultra_with_wrap() {
        let mut s = Settings::default(); // High
        s.cycle_quality(true);
        assert_eq!(s.quality, QualityPreset::Ultra);
        s.cycle_quality(true);
        assert_eq!(s.quality, QualityPreset::Low, "wraps past Ultra → Low");
        s.cycle_quality(false);
        assert_eq!(s.quality, QualityPreset::Ultra, "reverse wraps");
    }

    #[test]
    fn shadows_default_is_soft() {
        assert_eq!(Settings::default().shadows, ShadowQuality::Soft);
        assert_eq!(Settings::default().shadow_quality(), ShadowQuality::Soft);
    }

    #[test]
    fn shadows_cycle_off_hard_soft_with_wrap() {
        let mut s = Settings::default();
        assert_eq!(s.shadows, ShadowQuality::Soft);
        // Forward: Soft → Off → Hard → Soft.
        s.cycle_shadows(true);
        assert_eq!(s.shadows, ShadowQuality::Off);
        s.cycle_shadows(true);
        assert_eq!(s.shadows, ShadowQuality::Hard);
        s.cycle_shadows(true);
        assert_eq!(s.shadows, ShadowQuality::Soft, "wraps back to Soft");
        // Reverse: Soft → Hard.
        s.cycle_shadows(false);
        assert_eq!(s.shadows, ShadowQuality::Hard, "reverse steps Soft → Hard");
    }

    #[test]
    fn shadows_round_trip_through_save_and_load() {
        let path = temp_path("shadows_roundtrip");
        let s = Settings {
            shadows: ShadowQuality::Off,
            ..Default::default()
        };
        s.save_to(&path);
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.shadows, ShadowQuality::Off);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn old_save_without_shadows_key_defaults_to_soft() {
        // A pre-shadows save must still load (serde default fills `shadows`).
        let path = temp_path("shadows_old_save");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "color = \"256\"\nvolume = 0.5\n").unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(
            loaded.shadows,
            ShadowQuality::Soft,
            "missing shadows defaults to Soft"
        );
        let _ = std::fs::remove_file(&path);
    }

    // ── Sound fields ──────────────────────────────────────────────────────────

    #[test]
    fn sound_fields_default_full_and_independent_of_glitch_and_reduced_default_off() {
        let s = Settings::default();
        assert!(
            (s.sound_fields_intensity - 1.0).abs() < 1e-6,
            "default is Full"
        );
        assert_eq!(s.sound_fields_label(), "FULL");
        assert!(
            !s.sound_fields_reduced,
            "reduced-effects mode defaults OFF (owner decision)"
        );
        // Independent of glitch_enabled: flipping one must not move the other.
        let s2 = Settings {
            glitch_enabled: false,
            ..Settings::default()
        };
        assert!(
            !s2.sound_fields_reduced,
            "sound_fields_reduced is not derived from glitch_enabled"
        );
    }

    #[test]
    fn cycle_sound_fields_walks_the_four_steps_and_wraps() {
        let mut s = Settings::default();
        assert_eq!(s.sound_fields_label(), "FULL");
        s.cycle_sound_fields(true);
        assert_eq!(
            s.sound_fields_label(),
            "OFF",
            "wraps Full -> Off going forward"
        );
        s.cycle_sound_fields(true);
        assert_eq!(s.sound_fields_label(), "LOW");
        s.cycle_sound_fields(true);
        assert_eq!(s.sound_fields_label(), "MEDIUM");
        s.cycle_sound_fields(true);
        assert_eq!(s.sound_fields_label(), "FULL");
        s.cycle_sound_fields(false);
        assert_eq!(
            s.sound_fields_label(),
            "MEDIUM",
            "reverse steps Full -> Medium"
        );
    }

    #[test]
    fn field_settings_off_yields_zero_intensity_and_reduced_passes_through() {
        let s = Settings {
            sound_fields_intensity: 0.0,
            sound_fields_reduced: true,
            ..Settings::default()
        };
        let fs = s.field_settings();
        assert_eq!(fs.intensity, 0.0);
        assert!(fs.reduced);
    }

    #[test]
    fn sound_fields_round_trip_through_save_and_load() {
        let path = temp_path("sound_fields_roundtrip");
        let s = Settings {
            sound_fields_intensity: 0.66,
            sound_fields_reduced: true,
            ..Default::default()
        };
        s.save_to(&path);
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded, s);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn old_save_without_sound_fields_keys_defaults_to_full_and_not_reduced() {
        let path = temp_path("sound_fields_old_save");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "color = \"256\"\nvolume = 0.5\n").unwrap();
        let loaded = Settings::load_from(&path);
        assert!((loaded.sound_fields_intensity - 1.0).abs() < 1e-6);
        assert!(!loaded.sound_fields_reduced);
        let _ = std::fs::remove_file(&path);
    }

    // ── Form A-7 fourth-wall opt-out ─────────────────────────────────────────

    #[test]
    fn lore_fourth_wall_defaults_on_and_round_trips_off() {
        assert!(
            Settings::default().lore_fourth_wall,
            "the fourth wall is ON by default (A-7 Tier-1 reads active)"
        );
        let path = temp_path("lore_fourth_wall");
        let s = Settings {
            lore_fourth_wall: false,
            ..Default::default()
        };
        s.save_to(&path);
        let loaded = Settings::load_from(&path);
        assert!(!loaded.lore_fourth_wall, "the opt-out persists");
        let _ = std::fs::remove_file(&path);
    }

    /// A save written BEFORE the flag existed must load with the fourth wall ON
    /// (the shipped default) — additive `#[serde(default)]`, no migration.
    #[test]
    fn old_save_without_lore_key_defaults_on() {
        let path = temp_path("lore_old_save");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "color = \"256\"\nvolume = 0.5\n").unwrap();
        let loaded = Settings::load_from(&path);
        assert!(
            loaded.lore_fourth_wall,
            "missing lore_fourth_wall defaults to true"
        );
        let _ = std::fs::remove_file(&path);
    }

    // ── BRIGHTNESS (Swaggerfall study report item 4b) ────────────────────────

    #[test]
    fn brightness_defaults_neutral_and_postfx_is_byte_identical() {
        let s = Settings::default();
        assert_eq!(s.brightness, default_brightness_index());
        assert_eq!(s.brightness_label(), "0");
        // The default postfx (off) with neutral brightness must be EXACTLY the
        // off identity — no grade stage silently enabled.
        assert_eq!(s.postfx_config(), re_postfx::PostfxConfig::off());
    }

    #[test]
    fn brightness_step_enables_the_grade_and_moves_lift_and_gain() {
        let mut s = Settings::default();
        s.cycle_brightness(true); // one step above neutral: +1
        assert_eq!(s.brightness_label(), "+1");
        let cfg = s.postfx_config();
        assert!(
            cfg.grade.enabled,
            "a non-neutral brightness must enable the grade stage"
        );
        assert!(cfg.grade.lift > 0.0);
        assert!(cfg.grade.gain > 1.0);
    }

    #[test]
    fn brightness_clamps_instead_of_wrapping() {
        let mut s = Settings::default();
        for _ in 0..10 {
            s.cycle_brightness(true);
        }
        assert_eq!(s.brightness, BRIGHTNESS_STEPS.len() - 1);
        for _ in 0..10 {
            s.cycle_brightness(false);
        }
        assert_eq!(s.brightness, 0);
    }

    #[test]
    fn brightness_round_trips_and_old_save_defaults_neutral() {
        let path = temp_path("brightness_roundtrip");
        let mut s = Settings::default();
        s.cycle_brightness(false);
        s.save_to(&path);
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.brightness, s.brightness);
        let _ = std::fs::remove_file(&path);

        let path_old = temp_path("brightness_old_save");
        if let Some(parent) = path_old.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path_old, "color = \"256\"\n").unwrap();
        let old = Settings::load_from(&path_old);
        assert_eq!(
            old.brightness,
            default_brightness_index(),
            "missing key defaults neutral"
        );
        let _ = std::fs::remove_file(&path_old);
    }

    // ── COLOUR STEPS (Swaggerfall study report item 9) ───────────────────────

    #[test]
    fn color_steps_defaults_to_light_and_matches_todays_deadband() {
        let s = Settings::default();
        assert_eq!(s.color_steps, ColorSteps::Light);
        assert_eq!(
            s.color_steps_deadband(),
            re_present::PresentConfig::DEFAULT_DEADBAND
        );
    }

    #[test]
    fn color_steps_cycle_and_deadband_mapping() {
        let mut s = Settings::default();
        assert_eq!(s.color_steps_deadband(), 6);
        s.cycle_color_steps(true);
        assert_eq!(s.color_steps, ColorSteps::Strong);
        assert_eq!(s.color_steps_deadband(), 12);
        s.cycle_color_steps(true);
        assert_eq!(s.color_steps, ColorSteps::Off);
        assert_eq!(s.color_steps_deadband(), 0);
        s.cycle_color_steps(true);
        assert_eq!(s.color_steps, ColorSteps::Light);
    }

    #[test]
    fn color_steps_round_trips_and_old_save_defaults_light() {
        let path = temp_path("color_steps_roundtrip");
        let s = Settings {
            color_steps: ColorSteps::Off,
            ..Default::default()
        };
        s.save_to(&path);
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.color_steps, ColorSteps::Off);
        let _ = std::fs::remove_file(&path);

        let path_old = temp_path("color_steps_old_save");
        if let Some(parent) = path_old.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path_old, "color = \"256\"\n").unwrap();
        let old = Settings::load_from(&path_old);
        assert_eq!(
            old.color_steps,
            ColorSteps::Light,
            "missing key defaults to today's behaviour"
        );
        let _ = std::fs::remove_file(&path_old);
    }

    // ── FIRST STEPS HUD toggle ────────────────────────────────────────────────

    #[test]
    fn first_steps_hud_defaults_on_and_old_save_defaults_on() {
        assert!(Settings::default().first_steps_hud);
        let path = temp_path("first_steps_hud_old_save");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "color = \"256\"\n").unwrap();
        let loaded = Settings::load_from(&path);
        assert!(loaded.first_steps_hud, "missing key defaults to visible");
        let _ = std::fs::remove_file(&path);
    }
}
