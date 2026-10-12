//! `runtime_glue` — every place this game's own concrete types cross the
//! `re_runtime` seam.
//!
//! `re_runtime`'s plugins are generic: they know a `.lvl` play session exists,
//! but nothing about *this* game's state machine, its persisted settings, or
//! its campaign data — the runtime never names the game's state directly.
//! Something on the game side has to supply the concrete value each generic
//! plugin needs and translate this game's own resources into the plain-data
//! seam resources the runtime reads. This module is the ONE place that does
//! it, so nobody has to go hunting through `flow.rs`/`main.rs`/`menu.rs` to
//! find where a `Plugin<S>` became concrete.
//!
//! Every seam this module fills:
//!
//! | Runtime generic | Concrete value this module supplies | Wrapper |
//! |---|---|---|
//! | [`re_runtime::zoom::ZoomPlugin<S>`] | `live:` [`crate::flow::GameState::Playing`] | [`ZoomPlugin`](crate::runtime_glue::ZoomPlugin) |
//! | [`re_runtime::sound_fields::SoundFieldsPlugin<S>`] | `live:` [`crate::flow::GameState::Playing`] | [`SoundFieldsPlugin`](crate::runtime_glue::SoundFieldsPlugin) |
//! | [`re_runtime::sound_fields::FieldContext`] (resource) | derived from [`crate::flow::LevelCatalog`] + [`crate::flow::CurrentLevel`] + [`crate::flow::campaign_manifest`] + [`crate::flow::ambience_for`] | `fill_field_context_sys` |
//! | [`re_runtime::sound_fields::FieldSignals`] (resource) | derived from [`crate::audio_cues::CueTracker`] | `fill_field_signals_sys` |
//! | [`re_runtime::sound_fields::FieldSettingsRes`] (resource) | derived from `Settings::field_settings()` | `fill_field_settings_sys` |
//! | [`re_runtime::context_line`]'s HUD painters | this game's own `crate::menu::{FG, BG}` palette, as a [`re_runtime::context_line::HudPalette`] | [`draw_faced_context`](crate::runtime_glue::draw_faced_context), [`draw_proximity_context`](crate::runtime_glue::draw_proximity_context) |
//!
//! Every plain re-export with no concrete wrapper needed (`state_bridge`,
//! `emit_parts`, `prop_power`, `unstuck`, `asset_notice`, `color_drift_sources`,
//! `rng`, `dev`, `audio_world`) is called directly as `re_runtime::x::…` /
//! `re_asciifx::x::…` at each call site — there is nothing game-concrete to
//! glue, so gathering them here would just be one more layer of indirection.

use bevy::prelude::*;
use re_core::Framebuffer;
use re_level::Level;
use re_puzzle::PuzzleState;
use re_render::Fov;
use re_runtime::sound_fields::{FieldContext, FieldSettingsRes, FieldSignals};
use re_world::CameraPose;

use crate::audio_cues::CueTracker;
use crate::flow::{self, CurrentLevel, GameState, LevelCatalog, Movement};
use crate::settings::Settings;

// ─────────────────────────────── FOV-zoom ────────────────────────────────

/// TERMINAL STATE's zoom + post-fx plugin: live while
/// [`crate::flow::GameState::Playing`].
pub struct ZoomPlugin;

impl Plugin for ZoomPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(re_runtime::zoom::ZoomPlugin {
            live: GameState::Playing,
        });
    }
}

// ────────────────────────────── sound fields ─────────────────────────────

/// TERMINAL STATE's sound fields: the runtime's generic evaluation gated to
/// [`GameState::Playing`], fed by this module's three adapter systems.
pub struct SoundFieldsPlugin;

impl Plugin for SoundFieldsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FieldSignals>()
            .insert_resource(FieldSettingsRes(re_audio::FieldSettings::default()))
            .add_systems(
                PreUpdate,
                (
                    fill_field_context_sys,
                    fill_field_signals_sys,
                    fill_field_settings_sys,
                ),
            )
            .add_plugins(re_runtime::sound_fields::SoundFieldsPlugin {
                live: GameState::Playing,
            });
    }
}

/// `PreUpdate`: derive [`FieldContext`] from the campaign — the current level's
/// movement-phase index, its authored fog-escalation distance, and its base
/// ambience mood. Removes the resource entirely when there is no current
/// campaign level (matching the pre-split code's own "no `Res<CurrentLevel>`"
/// fallback, which `re_runtime::sound_fields::sound_fields_sys` reproduces for
/// an absent [`FieldContext`]).
#[allow(clippy::needless_pass_by_value)]
fn fill_field_context_sys(
    mut commands: Commands,
    catalog: Option<Res<LevelCatalog>>,
    current: Option<Res<CurrentLevel>>,
) {
    let Some(current) = current.as_deref() else {
        commands.remove_resource::<FieldContext>();
        return;
    };
    let (movement, fog_far) = current_movement_and_fog(catalog.as_deref(), Some(current));
    let mood = flow::ambience_for(current.0, false).mood;
    commands.insert_resource(FieldContext {
        phase: movement_index(movement),
        fog_far,
        mood,
    });
}

/// `PreUpdate`: derive [`FieldSignals`] from [`CueTracker`] — pack
/// `notes_read: Vec<bool>` into a bitmask (bit `N` = note `N` read; 32 bits is
/// well over any authored level's note count) and read the shard tally.
/// Absent tracker (no level loaded yet) reads as "nothing read, no shards".
#[allow(clippy::needless_pass_by_value)]
fn fill_field_signals_sys(mut signals: ResMut<FieldSignals>, tracker: Option<Res<CueTracker>>) {
    let (notes_read, shards) = tracker.as_deref().map_or((0u32, 0u32), |t| {
        let mask = t
            .notes_read
            .iter()
            .enumerate()
            .fold(0u32, |mask, (i, &read)| {
                if read && i < 32 {
                    mask | (1 << i)
                } else {
                    mask
                }
            });
        (mask, t.shards.unwrap_or(0) as u32)
    });
    let next = FieldSignals { notes_read, shards };
    if *signals != next {
        *signals = next;
    }
}

/// `PreUpdate`: derive [`FieldSettingsRes`] from the player's live
/// [`Settings`] — the sound-fields intensity + reduced-effects toggle.
#[allow(clippy::needless_pass_by_value)]
fn fill_field_settings_sys(mut settings_res: ResMut<FieldSettingsRes>, settings: Res<Settings>) {
    let next = settings.field_settings();
    if settings_res.0 != next {
        settings_res.0 = next;
    }
}

/// The current level's [`Movement`] and authored `fog_far`, from the campaign
/// manifest — `None`/`P1` for a level outside the campaign (demo/archive/test
/// fixtures), matching [`Movement::from_tag`]'s own "unrecognised reads as P1"
/// fallback.
fn current_movement_and_fog(
    catalog: Option<&LevelCatalog>,
    current: Option<&CurrentLevel>,
) -> (Movement, Option<f32>) {
    let (Some(catalog), Some(current)) = (catalog, current) else {
        return (Movement::P1, None);
    };
    let Some(path) = catalog.paths.get(current.0) else {
        return (Movement::P1, None);
    };
    let id = LevelCatalog::label(path);
    flow::campaign_manifest()
        .get(id)
        .map_or((Movement::P1, None), |entry| {
            (entry.movement, Some(entry.fog_far))
        })
}

/// §2.0 phase scaling: `Movement` P1..P4 → [`FieldContext::phase`]'s 0..=3.
const fn movement_index(m: Movement) -> u8 {
    match m {
        Movement::P1 => 0,
        Movement::P2 => 1,
        Movement::P3 => 2,
        Movement::P4 => 3,
    }
}

// ────────────────────────────── HUD context line ─────────────────────────

/// TERMINAL STATE's own palette: the same [`crate::menu::FG`]/[`crate::menu::BG`]
/// every other HUD row is stamped with.
const fn hud_palette() -> re_runtime::context_line::HudPalette {
    re_runtime::context_line::HudPalette {
        fg: crate::menu::FG,
        bg: crate::menu::BG,
    }
}

/// The one call `crate::menu::draw_play_hud` used to make directly: resolve the
/// faced context (if any) and paint it with this game's own HUD palette.
pub fn draw_faced_context(
    fb: &mut Framebuffer,
    cam: &CameraPose,
    puzzle: &PuzzleState,
    level: Option<&Level>,
) {
    re_runtime::context_line::draw_faced_context(fb, cam, puzzle, level, hud_palette());
}

/// The call `crate::menu::draw_play_hud` makes every frame: resolve this frame's
/// proximity label through `tracker` and paint it with this game's own HUD palette.
pub fn draw_proximity_context(
    fb: &mut Framebuffer,
    cam: &CameraPose,
    puzzle: &PuzzleState,
    level: Option<&Level>,
    tracker: &mut re_runtime::context_line::ProximityTracker,
    fov: &Fov,
) {
    re_runtime::context_line::draw_proximity_context(
        fb,
        cam,
        puzzle,
        level,
        tracker,
        fov,
        hud_palette(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_index_matches_the_p1_p4_order() {
        assert_eq!(movement_index(Movement::P1), 0);
        assert_eq!(movement_index(Movement::P2), 1);
        assert_eq!(movement_index(Movement::P3), 2);
        assert_eq!(movement_index(Movement::P4), 3);
    }

    #[test]
    fn no_catalog_or_current_level_falls_back_to_p1_with_no_fog() {
        assert_eq!(current_movement_and_fog(None, None), (Movement::P1, None));
    }
}
