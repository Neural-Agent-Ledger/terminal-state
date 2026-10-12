//! `mirror_ramp` — campaign progress → the live MIRROR-CLARITY override
//! (story-expansion W3 item 3.1).
//!
//! ## The beat
//!
//! A mirror surface carries its own authored `clarity` on its material;
//! [`re_render::MirrorClarity`] is the renderer's live override for every mirror
//! in the scene. This module is the GAME-side driver of that knob: the further
//! the player is along the campaign, the sharper every reflection resolves.
//!
//! The dread is deliberately *abstract*: the reflection sharpens **and you are
//! still not in it**. There is NO face, ever — `re_render::mirror` casts a second
//! scene bounce and there is no player geometry in the scene, so a mirror
//! physically cannot show one. The identity beat of L9 is carried elsewhere, by
//! the NAME read-back in the note/door readout (`PUZZLE_PROGRESSION` L9(d),
//! already shipped in [`crate::story::NameAssembly::door_line`]) — not by a
//! rendered figure. Nothing here should ever be described as a face.
//!
//! ## The curve
//!
//! ```text
//! clarity = min(shards, 7) / 7 · 0.85     (+ 0.15 while on the campaign's final level)
//! ```
//!
//! * `shards` is how many of the seven **name-shards** the player holds — derived
//!   from persisted progress via [`crate::story::NameAssembly`], never stored, so
//!   it can never drift from the progress it describes. Shards come from L1–L7
//!   only (the name completes at L7), which is exactly why the ramp is expressed
//!   in shards rather than in solved-level count: it saturates when the name does.
//! * `0.85` is the ceiling the shards alone can buy. The last `0.15` is spent by
//!   ENTERING the campaign's final level (L12, "The Closing" — the choice
//!   staging): only there does the reflection go fully crisp.
//! * Early campaign (0–1 shards) sits near `0.0` = a noisy / static reflection.
//!
//! ## Where it applies (and where it deliberately does not)
//!
//! The override is installed only while a **campaign** level (a level listed in
//! `levels/campaign.toml`) is live — `Playing`, plus `Pause` because the live
//! scene stays visible behind that overlay and the reflection must not pop when
//! the player pauses. In every other state, and for every non-campaign level
//! (the technique demos such as the `12-mirror` reflection fixture, the archive
//! rooms, test fixtures), the override is cleared to `None` so each mirror keeps the clarity
//! its material authored. A demo that exists to show a mirror off must not be
//! dimmed by an unrelated save file.
//!
//! Engine-agnostic and terminal-free: one pure mapping function plus one Bevy
//! system that reads three resources and writes one.

use bevy::prelude::*;
use re_render::MirrorClarity;

use crate::flow::{campaign_manifest, CurrentLevel, GameState, LevelCatalog};
use crate::save::Progress;
use crate::story::NameAssembly;

/// How many name-shards the true name is made of (mirrors
/// [`crate::story::NAME_SHARDS`]'s length — the divisor of the ramp).
pub const SHARD_TOTAL: usize = 7;

/// The clarity the seven shards can buy on their own. The remaining
/// [`FINAL_LEVEL_BONUS`] is reserved for the campaign's last level.
pub const SHARD_CEILING: f32 = 0.85;

/// The bonus added while the player is on the campaign's FINAL level (L12) — the
/// last step to a fully crisp reflection.
pub const FINAL_LEVEL_BONUS: f32 = 0.15;

/// **The mapping.** Campaign progress → mirror clarity in `[0, 1]`.
///
/// `shards` is the number of name-shards held (clamped to [`SHARD_TOTAL`], so a
/// nonsense count can never push past the ceiling); `on_final_level` is true only
/// while the campaign's last level is the live one.
///
/// Pure — no resources, no I/O. This is the function the unit tests pin.
pub fn clarity_for(shards: usize, on_final_level: bool) -> f32 {
    let shards = shards.min(SHARD_TOTAL);
    let base = (shards as f32 / SHARD_TOTAL as f32) * SHARD_CEILING;
    let bonus = if on_final_level {
        FINAL_LEVEL_BONUS
    } else {
        0.0
    };
    (base + bonus).clamp(0.0, 1.0)
}

/// How many of the seven name-shards `progress` holds. Derived from the solved
/// level ids through [`NameAssembly`], so it honours the same "which levels carry
/// a shard" rule the readouts use (L1–L7 / `01`–`07`; L8+ add none).
pub fn shards_held(progress: &Progress) -> usize {
    NameAssembly::from_solved_ids(progress.solved.iter().map(String::as_str)).count()
}

/// Is `level_id` the LAST level of the authored campaign (L12, "The Closing")?
///
/// Asked of the manifest rather than hardcoding the id, so re-ordering or
/// renaming the finale cannot silently strip the bonus. An empty / unparsed
/// manifest answers `false` (no campaign ⇒ no finale ⇒ no bonus).
pub fn is_final_campaign_level(level_id: &str) -> bool {
    let manifest = campaign_manifest();
    match manifest.len() {
        0 => false,
        n => manifest.position(level_id) == Some(n - 1),
    }
}

/// The clarity override the given state + progress + live level id should install:
/// `Some(clarity)` while a CAMPAIGN level is live, `None` otherwise (each mirror
/// keeps its authored clarity).
///
/// Pure, so the whole decision — including the "demos are untouched" rule — is
/// unit-testable without an `App`.
pub fn override_for(state: GameState, progress: &Progress, level_id: Option<&str>) -> Option<f32> {
    // Pause is included on purpose: the live scene is still on screen behind the
    // pause overlay (the flow keeps the level loaded), so clearing the override
    // there would visibly pop every mirror the instant the player pauses.
    if !matches!(state, GameState::Playing | GameState::Pause) {
        return None;
    }
    // Only the authored campaign ramps. Demos / archive / fixtures keep their
    // authored mirror clarity.
    let id = level_id?;
    if !campaign_manifest().contains(id) {
        return None;
    }
    Some(clarity_for(
        shards_held(progress),
        is_final_campaign_level(id),
    ))
}

/// `Update` (every state): install / clear the live [`MirrorClarity`] override
/// from campaign progress.
///
/// Writes ONLY on a real change, so an idle frame neither dirties the render nor
/// re-casts the reflection. Runs in every state (rather than on
/// `OnEnter`/`OnExit(Playing)`) because the Playing↔Pause edge must NOT clear it
/// and the Playing→Won edge must — one state read expresses both rules without
/// duplicating the flow's pause-edge bookkeeping.
fn drive_mirror_clarity(
    state: Res<State<GameState>>,
    progress: Res<Progress>,
    catalog: Res<LevelCatalog>,
    current: Res<CurrentLevel>,
    mut clarity: ResMut<MirrorClarity>,
) {
    let want = override_for(*state.get(), &progress, catalog.id_at(current.0));
    if clarity.override_value != want {
        clarity.override_value = want;
    }
}

/// Registers the mirror-clarity ramp. Added by
/// [`FlowPlugin`](crate::flow::FlowPlugin) so the ramp ships with the play flow;
/// a focused test can add it alone.
pub struct MirrorRampPlugin;

impl Plugin for MirrorRampPlugin {
    fn build(&self, app: &mut App) {
        // INSERT-IF-ABSENT: `re_render`'s plugin also inits this (same default),
        // and a headless test with no renderer needs it to exist for the system's
        // `ResMut` to resolve.
        app.init_resource::<MirrorClarity>()
            .add_systems(Update, drive_mirror_clarity);
    }
}

#[cfg(test)]
mod tests;
