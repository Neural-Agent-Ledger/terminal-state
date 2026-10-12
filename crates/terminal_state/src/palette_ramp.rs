//! `palette_ramp` — campaign progress → the live [`re_render::palette_gate::PaletteGate`] override
//! (a colour-unlock progression driven by campaign progress). The twin of
//! [`crate::mirror_ramp`] on the colour axis:
//! [`re_render::palette_gate`] is pure engine data/math with no save/progress
//! knowledge, and this module is the GAME-side driver that turns campaign
//! progress into a [`re_render::palette_gate::PaletteGate`] value, exactly the split `mirror_ramp.rs`
//! already established for [`re_render::MirrorClarity`].
//!
//! ## The story (Design A "First Light", spec §5.1 — recommended design)
//!
//! Canon already supports this: STORY_BIBLE §5 has L1 "red is the one that
//! came first", L2 "R+G=Y", L3 "Full Spectrum", L7 "bloom". The seven
//! name-shards (L1–L7) are the SAME progress signal [`crate::mirror_ramp`]
//! already derives from the save via [`crate::story::NameAssembly`], so this
//! ramp can never drift out of sync with the mirror ramp or the name readout.
//!
//! ```text
//! 0 shards  → mono "red phosphor": every hue forced to a red tint, L kept
//! 1 shard   → R+G arc unlocked (yellow becomes reachable), rest stays mono
//! 2..6 shard→ full hue, chroma capped and ramping from "muted" toward "bloom"
//! 7 shards  → chroma cap lifted to the sRGB ceiling — full saturation
//! ```
//!
//! **Honest tuning note:** the spec gives the STORY beats but not exact hue-arc
//! widths or chroma fractions — those are a first-pass estimate here (named
//! constants below), left for `balance-reviewer` to retune in one place. `L4`
//! (the contour/underline channel), `L5` (colour drift), and `L6` (sky layers)
//! are separate systems (B13, B2) this module does not gate.
//!
//! ## Where it applies (mirrors `mirror_ramp`'s rule exactly)
//!
//! The gate is installed only while a CAMPAIGN level (`levels/campaign.toml`)
//! is live in `Playing`/`Pause` (pause included so the scene behind the pause
//! overlay does not pop). Every other state, and every non-campaign level (the
//! technique demos, archive rooms, test fixtures), gets [`re_render::palette_gate::PaletteGate::full`] —
//! a demo that exists to show colour off must not be gated by an unrelated
//! save file.
//!
//! ## Accessibility escape hatch
//!
//! `Settings::colour_unlock_enabled == false` forces [`re_render::palette_gate::PaletteGate::full`]
//! unconditionally — "off = full colour always", the Settings → "Colour
//! unlock: on/off" row (spec §5.3's accessibility clause; the three-way
//! Story/Full/Mono picker described there is not implemented, only the
//! on/off accessibility toggle this integration pass was asked for).
//!
//! `--dump`/goldens never install this module's plugin, so they stay
//! [`re_render::palette_gate::PaletteGate::full`] and byte-identical (spec §5.3 "Goldens: `--dump` and
//! the golden test use gate = Full").

use bevy::prelude::*;
use re_core::Framebuffer;
use re_render::palette_gate::{apply_gate, ChannelMask, GateLut, PaletteGate};

use crate::flow::{campaign_manifest, CurrentLevel, GameState, LevelCatalog};
use crate::mirror_ramp::{shards_held, SHARD_TOTAL};
use crate::save::Progress;
use crate::settings::Settings;

// ─── Design A tuning constants (first pass — see module docs) ──────────────

/// Warm red hue (Oklch degrees) used for the mono/locked stages — STORY_BIBLE
/// L1's red lamp, "the one that came first".
pub const RED_HUE_DEG: f32 = 25.0;

/// The "R+G, no blue" hue arc unlocked after the first shard: red through
/// amber/yellow into green, wrapping through 0°. Approximates Design A's
/// "colours reachable with B = 0" as a single Oklch hue arc (exact width is a
/// first-pass estimate, see module docs).
pub const RG_ARC_DEG: (f32, f32) = (-45.0, 150.0);

/// Soft-edge width on either side of an arc boundary (spec §5.2: "10°: soft
/// edge so hues don't pop at the arc boundary").
pub const ARC_SOFT_DEG: f32 = 10.0;

/// Oklch chroma ceiling for in-gamut sRGB colours (`palette_gate.rs`'s own
/// doc: "0..0.37").
pub const CHROMA_MAX: f32 = 0.37;

/// Chroma fraction (of [`CHROMA_MAX`]) for the two locked/mono stages (0 and 1
/// shards) — visibly saturated but not full, so the red lamp still reads as
/// "the only saturated thing" without looking washed out.
pub const MONO_CHROMA_FRAC: f32 = 0.55;

/// Chroma fraction at the START of the "full hue, muted" band (2 shards) —
/// spec §5.1: "Full spectrum, but muted".
pub const FULL_CHROMA_FRAC_LO: f32 = 0.6;

/// Chroma fraction at "bloom" (7 shards) — spec §5.1: "full saturation for the
/// first time".
pub const FULL_CHROMA_FRAC_HI: f32 = 1.0;

/// **The mapping.** Campaign progress (name-shards held, 0..=[`SHARD_TOTAL`])
/// → the engine [`PaletteGate`] for Design A "First Light" (spec §5.1).
///
/// Pure — no resources, no I/O. This is the function the unit tests pin.
#[must_use]
pub fn gate_for_shards(shards: usize) -> PaletteGate {
    let shards = shards.min(SHARD_TOTAL);
    let mono_tint = Some((RED_HUE_DEG, MONO_CHROMA_FRAC * CHROMA_MAX));
    match shards {
        // Stage 0 (before L1): mono red-phosphor. Every hue is locked to the
        // red tint; `hue_arcs` empty means "always locked" (see
        // `PaletteGate::gate_oklch`'s `arc_weight`).
        0 => PaletteGate {
            hue_arcs: Vec::new(),
            arc_soft_deg: ARC_SOFT_DEG,
            chroma_cap: CHROMA_MAX,
            mono_tint,
            channels: ChannelMask::COLOR_AND_SKY,
            leak: 0.0,
        },
        // Stage 1 (after L1): R+G unlocked (yellow becomes reachable);
        // everything outside the arc (blue/violet) stays the red mono tint.
        1 => PaletteGate {
            hue_arcs: vec![RG_ARC_DEG],
            arc_soft_deg: ARC_SOFT_DEG,
            chroma_cap: CHROMA_MAX,
            mono_tint,
            channels: ChannelMask::COLOR_AND_SKY,
            leak: 0.0,
        },
        // Stages 2..=7 (after L2 onward): full hue, chroma ramps linearly from
        // "muted" (2 shards) to "bloom" (7 shards).
        n => {
            let span = (SHARD_TOTAL - 2) as f32; // 5
            let t = ((n - 2) as f32 / span).clamp(0.0, 1.0);
            let frac = FULL_CHROMA_FRAC_LO + (FULL_CHROMA_FRAC_HI - FULL_CHROMA_FRAC_LO) * t;
            PaletteGate {
                hue_arcs: vec![(0.0, 360.0)],
                arc_soft_deg: ARC_SOFT_DEG,
                chroma_cap: frac * CHROMA_MAX,
                mono_tint: None,
                channels: ChannelMask::COLOR_AND_SKY,
                leak: 0.0,
            }
        }
    }
}

/// Is `level_id` a CAMPAIGN level (the only levels this ramp gates)? Demos,
/// archive rooms and test fixtures answer `false` and keep their authored
/// colour untouched.
fn is_campaign_level(level_id: &str) -> bool {
    campaign_manifest().contains(level_id)
}

/// The [`PaletteGate`] the given state/settings/progress/level should install
/// RIGHT NOW: [`PaletteGate::full`] unless colour-unlock is on, the game is
/// actually playing (or paused over play), and the live level is a campaign
/// level — mirrors [`crate::mirror_ramp::override_for`]'s exact rule.
///
/// Pure, so the whole decision is unit-testable without an `App`.
#[must_use]
pub fn gate_for(
    state: GameState,
    settings: &Settings,
    progress: &Progress,
    level_id: Option<&str>,
) -> PaletteGate {
    if !settings.colour_unlock_enabled {
        return PaletteGate::full();
    }
    if !matches!(state, GameState::Playing | GameState::Pause) {
        return PaletteGate::full();
    }
    let Some(id) = level_id else {
        return PaletteGate::full();
    };
    if !is_campaign_level(id) {
        return PaletteGate::full();
    }
    gate_for_shards(shards_held(progress))
}

/// The live gate + its precomputed LUT (spec §5.2: "rebuilt only when the gate
/// itself changes"). Defaults to the permanent no-op so a headless app with no
/// progress yet renders untouched.
#[derive(Resource)]
pub struct PaletteGateState {
    /// The currently-active gate, derived from progress/settings/state.
    pub gate: PaletteGate,
    /// The precomputed colour LUT for `gate`, rebuilt only when `gate` changes.
    pub lut: GateLut,
}

impl Default for PaletteGateState {
    fn default() -> Self {
        let gate = PaletteGate::full();
        let lut = GateLut::build(&gate);
        PaletteGateState { gate, lut }
    }
}

/// `Update` (every state): recompute the wanted gate from progress/settings/
/// state and, ONLY on a real change, rebuild the LUT (the 33³ Oklab sweep —
/// cheap-ish but not meant to run every frame).
fn drive_palette_gate(
    state: Res<State<GameState>>,
    settings: Res<Settings>,
    progress: Res<Progress>,
    catalog: Res<LevelCatalog>,
    current: Res<CurrentLevel>,
    mut gate_state: ResMut<PaletteGateState>,
) {
    let want = gate_for(*state.get(), &settings, &progress, catalog.id_at(current.0));
    if gate_state.gate != want {
        gate_state.lut = GateLut::build(&want);
        gate_state.gate = want;
    }
}

/// Registers the progress → gate computation (`Update`, schedule-order
/// independent). Added by [`crate::flow::FlowPlugin`] so every consumer —
/// including the headless flow tests — gets the ramp, exactly like
/// [`crate::mirror_ramp::MirrorRampPlugin`].
pub struct PaletteRampPlugin;

impl Plugin for PaletteRampPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaletteGateState>()
            .add_systems(Update, drive_palette_gate);
    }
}

/// Apply the current [`PaletteGateState`] to the live frame. A no-op gate
/// (`Full`, the common case) returns without touching a cell.
///
/// **Pipeline placement is the caller's responsibility**: this must be
/// registered in `PostUpdate` AFTER `PostfxPlugin`'s system and BEFORE
/// `GlitchPlugin`'s (spec §5.2 "after postfx, before glitch" — glitch can
/// then show a locked colour for a few frames via `PaletteGate::leak`, "a
/// memory intrusion from the future"). `re_game`'s binary is the one place
/// that can see both `re_postfx`/`re_glitch`'s plugin add-order AND this
/// module, so [`PaletteGateFramePlugin`] is added directly there rather than
/// folded into [`PaletteRampPlugin`] (which only computes the resource, in
/// `Update`, where relative PostUpdate ordering does not apply).
pub fn apply_palette_gate_to_frame(state: Res<PaletteGateState>, mut fb: ResMut<Framebuffer>) {
    if state.gate.is_noop() {
        return;
    }
    apply_gate(&mut fb, &state.gate, &state.lut);
}

/// Registers ONLY the `PostUpdate` frame-application system, so the binary can
/// place it precisely between `PostfxPlugin` and `GlitchPlugin` in its
/// plugin-add chain (see [`apply_palette_gate_to_frame`]'s docs). Defensive
/// `init_resource` in case this is ever added without [`PaletteRampPlugin`]
/// (e.g. a focused render test).
pub struct PaletteGateFramePlugin;

impl Plugin for PaletteGateFramePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaletteGateState>()
            .add_systems(PostUpdate, apply_palette_gate_to_frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::Progress;

    fn progress_with_solved(ids: &[&str]) -> Progress {
        let mut p = Progress::default();
        for id in ids {
            p.solved.insert((*id).to_string());
        }
        p
    }

    #[test]
    fn zero_shards_is_mono_and_full_arcs_are_not_present() {
        let g = gate_for_shards(0);
        assert!(
            g.hue_arcs.is_empty(),
            "0 shards must be mono (no allowed arcs)"
        );
        assert!(g.mono_tint.is_some());
        assert_eq!(g.channels, ChannelMask::COLOR_AND_SKY);
    }

    #[test]
    fn one_shard_unlocks_the_rg_arc_only() {
        let g = gate_for_shards(1);
        assert_eq!(g.hue_arcs, vec![RG_ARC_DEG]);
        assert!(
            g.mono_tint.is_some(),
            "colours outside the arc still lock to red"
        );
    }

    #[test]
    fn two_shards_is_full_hue_muted() {
        let g = gate_for_shards(2);
        assert_eq!(g.hue_arcs, vec![(0.0, 360.0)]);
        assert!(g.mono_tint.is_none());
        assert!((g.chroma_cap - FULL_CHROMA_FRAC_LO * CHROMA_MAX).abs() < 1e-5);
    }

    #[test]
    fn seven_shards_reaches_the_chroma_ceiling() {
        let g = gate_for_shards(SHARD_TOTAL);
        assert!((g.chroma_cap - CHROMA_MAX).abs() < 1e-5);
    }

    #[test]
    fn shard_count_beyond_total_is_clamped_not_out_of_range() {
        let a = gate_for_shards(SHARD_TOTAL);
        let b = gate_for_shards(SHARD_TOTAL + 5);
        assert_eq!(
            a, b,
            "an over-count must clamp to the ceiling stage, not panic/extrapolate"
        );
    }

    #[test]
    fn chroma_cap_is_monotonically_non_decreasing_across_the_whole_ramp() {
        let mut prev = 0.0_f32;
        for shards in 0..=SHARD_TOTAL {
            let g = gate_for_shards(shards);
            // Locked stages: compare the mono tint's chroma, since
            // `chroma_cap` alone is not yet the visible ceiling there.
            let cap = g.mono_tint.map_or(g.chroma_cap, |(_, tint_c)| tint_c);
            assert!(
                cap + 1e-4 >= prev,
                "colour must never visibly regress as shards increase (shards={shards})"
            );
            prev = cap;
        }
    }

    #[test]
    fn colour_unlock_disabled_forces_full_regardless_of_progress_or_level() {
        let settings = Settings {
            colour_unlock_enabled: false,
            ..Settings::default()
        };
        let progress = progress_with_solved(&[]);
        let g = gate_for(
            GameState::Playing,
            &settings,
            &progress,
            Some("L1-first-pulse"),
        );
        assert!(
            g.is_noop(),
            "the accessibility off-switch must always yield Full"
        );
    }

    #[test]
    fn non_campaign_level_is_never_gated() {
        let settings = Settings::default();
        let progress = progress_with_solved(&[]);
        let g = gate_for(
            GameState::Playing,
            &settings,
            &progress,
            Some("08-material-demo"),
        );
        assert!(
            g.is_noop(),
            "a technique-demo level must keep its authored colour"
        );
    }

    #[test]
    fn menu_state_is_never_gated_even_with_progress() {
        let settings = Settings::default();
        let progress = progress_with_solved(&["L1-first-pulse", "L2-yellow"]);
        let g = gate_for(GameState::Menu, &settings, &progress, None);
        assert!(g.is_noop());
    }

    #[test]
    fn pause_over_a_campaign_level_keeps_the_same_gate_as_playing() {
        let settings = Settings::default();
        let progress = progress_with_solved(&["L1-first-pulse"]);
        let id = campaign_manifest().levels().first().map(|l| l.id.clone());
        let Some(id) = id else { return }; // manifest empty in this build config
        let playing = gate_for(GameState::Playing, &settings, &progress, Some(&id));
        let paused = gate_for(GameState::Pause, &settings, &progress, Some(&id));
        assert_eq!(playing, paused, "pausing over play must not pop the gate");
    }
}
