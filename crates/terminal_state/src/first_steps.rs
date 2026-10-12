//! `first_steps` — a self-ticking onboarding checklist.
//!
//! Seven items, ticked by observation during ordinary play — nothing here ever
//! blocks input, forces a scripted action, or interrupts the player. Completion
//! persists in [`crate::save::Progress::first_steps`] (additive, serde-default,
//! so an old save loads with nothing ticked and never re-shows a completed
//! item after a restart) and is shown compactly in the play HUD
//! (`crate::menu::draw_play_hud`) until every item is done, then it stops
//! painting — permanently, since completion is persisted. A `FIRST STEPS`
//! settings row lets the player turn the HUD block off outright; observation
//! keeps running either way (it is free and it is what persists), only the
//! HUD block's visibility is gated.
//!
//! ## The six observed predicates + the one state-entry predicate
//!
//! | Item | Ticks when |
//! |---|---|
//! | [`Moved`](crate::first_steps::FirstStep::Moved) | cumulative camera-position drift since the last check exceeds [`MOVE_THRESHOLD_CELLS`](crate::first_steps::MOVE_THRESHOLD_CELLS) |
//! | [`Looked`](crate::first_steps::FirstStep::Looked) | cumulative yaw+pitch drift exceeds [`LOOK_THRESHOLD_DEG`](crate::first_steps::LOOK_THRESHOLD_DEG) |
//! | [`Jumped`](crate::first_steps::FirstStep::Jumped) | [`PlayerMotion`](re_world::PlayerMotion) transitions grounded→airborne with `vz > 0` (the up-kick of a real jump, not a fall) |
//! | [`FacedSwitch`](crate::first_steps::FirstStep::FacedSwitch) | [`crate::flow::faced_switch`] reports `lands == true` — the SAME probe that lights the crosshair |
//! | [`ToggledSwitch`](crate::first_steps::FirstStep::ToggledSwitch) | an [`InteractResolved`](re_puzzle::InteractResolved) message reports [`InteractOutcome::Toggled`](re_puzzle::InteractOutcome::Toggled) |
//! | [`WatchedPulse`](crate::first_steps::FirstStep::WatchedPulse) | at least one pipe reads ON ([`Pipe::is_on`](re_puzzle::Pipe::is_on)) while the camera has sat still for [`PULSE_WATCH_SECS`](crate::first_steps::PULSE_WATCH_SECS) |
//! | [`OpenedGuide`](crate::first_steps::FirstStep::OpenedGuide) | `crate::flow::enter_guide` runs (the guide state was entered) |
//!
//! The move/look/dwell thresholds are onboarding-checklist glue, not gameplay
//! numbers from a mechanics spec — chosen small enough that a few ordinary
//! steps/glances tick them, named as constants here so a later pass can retune
//! them in one place.

use bevy::prelude::*;
use re_core::Vec3;
use re_puzzle::{InteractOutcome, InteractResolved, PuzzleState};
use re_world::{CameraPose, PlayerMotion};

use crate::flow::{faced_switch, GameState, PlayCamera};
use crate::save::Progress;
use crate::strings;

/// One first-steps checklist item, in the order the HUD lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FirstStep {
    /// Ticks on cumulative camera-position drift past [`MOVE_THRESHOLD_CELLS`].
    Moved,
    /// Ticks on cumulative yaw+pitch drift past [`LOOK_THRESHOLD_DEG`].
    Looked,
    /// Ticks on the up-kick edge of a real jump (not a fall).
    Jumped,
    /// Ticks when the crosshair's own switch-facing probe reports a hit.
    FacedSwitch,
    /// Ticks on the first successful interact-toggle of a switch.
    ToggledSwitch,
    /// Ticks after dwelling still for [`PULSE_WATCH_SECS`] while a pipe is on.
    WatchedPulse,
    /// Ticks the moment the in-game guide is opened.
    OpenedGuide,
}

/// Every item, in display order.
pub const FIRST_STEPS: [FirstStep; 7] = [
    FirstStep::Moved,
    FirstStep::Looked,
    FirstStep::Jumped,
    FirstStep::FacedSwitch,
    FirstStep::ToggledSwitch,
    FirstStep::WatchedPulse,
    FirstStep::OpenedGuide,
];

impl FirstStep {
    /// The STABLE on-disk key ([`Progress::first_steps`]). Never renamed — an
    /// old save's ticked items must keep meaning the same thing.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Moved => "moved",
            Self::Looked => "looked",
            Self::Jumped => "jumped",
            Self::FacedSwitch => "faced_switch",
            Self::ToggledSwitch => "toggled_switch",
            Self::WatchedPulse => "watched_pulse",
            Self::OpenedGuide => "opened_guide",
        }
    }

    /// The HUD label (`crate::strings`, English only for now).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Moved => strings::FIRST_STEP_MOVED,
            Self::Looked => strings::FIRST_STEP_LOOKED,
            Self::Jumped => strings::FIRST_STEP_JUMPED,
            Self::FacedSwitch => strings::FIRST_STEP_FACED_SWITCH,
            Self::ToggledSwitch => strings::FIRST_STEP_TOGGLED_SWITCH,
            Self::WatchedPulse => strings::FIRST_STEP_WATCHED_PULSE,
            Self::OpenedGuide => strings::FIRST_STEP_OPENED_GUIDE,
        }
    }
}

// ─────────────────────────── persistence helpers ────────────────────────────

/// Mark one checklist item done. Returns `true` if this was newly set (so a
/// caller can decide whether a disk write is warranted, mirroring
/// [`Progress::mark_solved`]).
pub fn mark_done(progress: &mut Progress, step: FirstStep) -> bool {
    progress.first_steps.insert(step.key().to_string())
}

/// Is this item already ticked?
#[must_use]
pub fn is_done(progress: &Progress, step: FirstStep) -> bool {
    progress.first_steps.contains(step.key())
}

/// Are all items ticked? (The HUD hides once this is true.)
#[must_use]
pub fn all_done(progress: &Progress) -> bool {
    FIRST_STEPS.iter().all(|&s| is_done(progress, s))
}

/// The compact HUD lines for the checklist — empty (paints nothing) once every
/// item is done, or when `enabled` is false (the settings row).
#[must_use]
pub fn hud_lines(progress: &Progress, enabled: bool) -> Vec<String> {
    if !enabled || all_done(progress) {
        return Vec::new();
    }
    FIRST_STEPS
        .iter()
        .map(|&s| {
            let mark = if is_done(progress, s) {
                '\u{2713}'
            } else {
                '\u{00B7}'
            };
            format!("{mark} {}", s.label())
        })
        .collect()
}

// ───────────────────────────── observation glue ─────────────────────────────

/// Cumulative drift (world cells) since the last [`FirstStep::Moved`] check
/// that counts as "the player moved". Small on purpose — a couple of ordinary
/// steps.
pub const MOVE_THRESHOLD_CELLS: f32 = 0.3;
/// Cumulative yaw+pitch drift (degrees) that counts as "the player looked
/// around".
pub const LOOK_THRESHOLD_DEG: f32 = 8.0;
/// How long the camera must sit still (within the drift thresholds above)
/// while a pipe is carrying current for [`FirstStep::WatchedPulse`] to tick.
pub const PULSE_WATCH_SECS: f32 = 1.0;

/// Session-only (never persisted) bookkeeping the observer needs between
/// frames: the previous pose (to derive per-frame drift), the previous
/// grounded flag (to detect a jump's up-kick), and the still-dwell clock.
#[derive(Resource, Default)]
struct FirstStepsWatch {
    prev_pos: Option<Vec3>,
    prev_yaw_pitch: Option<(f32, f32)>,
    prev_grounded: Option<bool>,
    still_secs: f32,
}

impl FirstStepsWatch {
    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// `OnEnter(Playing)`: forget the previous level's pose so a level's spawn
/// point is never mistaken for drift from the last one.
fn reset_watch_on_level_enter(mut watch: ResMut<FirstStepsWatch>) {
    watch.reset();
}

/// `Update` (Playing only): the six play-observed predicates.
#[allow(clippy::too_many_arguments)]
fn observe_first_steps(
    cam_q: Query<(&CameraPose, &PlayerMotion), With<PlayCamera>>,
    puzzle: Res<PuzzleState>,
    mut resolved: MessageReader<InteractResolved>,
    mut watch: ResMut<FirstStepsWatch>,
    mut progress: ResMut<Progress>,
    time: Res<Time>,
) {
    let mut dirty = false;

    // TOGGLED — independent of camera pose, so it's checked first and always
    // drains the reader even if no camera exists yet.
    for msg in resolved.read() {
        if matches!(msg.outcome, InteractOutcome::Toggled { .. }) {
            dirty |= mark_done(&mut progress, FirstStep::ToggledSwitch);
        }
    }

    let Ok((cam, motion)) = cam_q.single() else {
        return;
    };

    // MOVED / LOOKED — cumulative drift since the last tick of either.
    let moved_now = !is_done(&progress, FirstStep::Moved);
    let looked_now = !is_done(&progress, FirstStep::Looked);
    if moved_now || looked_now {
        if let Some(prev) = watch.prev_pos {
            if moved_now && prev.distance(cam.pos) >= MOVE_THRESHOLD_CELLS {
                dirty |= mark_done(&mut progress, FirstStep::Moved);
            }
        }
        if let Some((py, pp)) = watch.prev_yaw_pitch {
            let drift = (cam.yaw - py).abs() + (cam.pitch - pp).abs();
            if looked_now && drift.to_degrees() >= LOOK_THRESHOLD_DEG {
                dirty |= mark_done(&mut progress, FirstStep::Looked);
            }
        }
    }

    // JUMPED — the up-kick edge: grounded was true, now false, rising.
    if !is_done(&progress, FirstStep::Jumped)
        && watch.prev_grounded == Some(true)
        && !motion.grounded
        && motion.vz > 0.0
    {
        dirty |= mark_done(&mut progress, FirstStep::Jumped);
    }

    // FACED SWITCH — the same probe that lights the crosshair.
    if !is_done(&progress, FirstStep::FacedSwitch) && faced_switch(cam, &puzzle).3 {
        dirty |= mark_done(&mut progress, FirstStep::FacedSwitch);
    }

    // WATCHED PULSE — dwell while a pipe is live.
    if !is_done(&progress, FirstStep::WatchedPulse) {
        let still = watch
            .prev_pos
            .is_some_and(|p| p.distance(cam.pos) < MOVE_THRESHOLD_CELLS)
            && watch.prev_yaw_pitch.is_some_and(|(py, pp)| {
                ((cam.yaw - py).abs() + (cam.pitch - pp).abs()).to_degrees() < LOOK_THRESHOLD_DEG
            });
        let pipe_live = puzzle.pipes.iter().any(re_puzzle::Pipe::is_on);
        if still && pipe_live {
            watch.still_secs += time.delta_secs();
            if watch.still_secs >= PULSE_WATCH_SECS {
                dirty |= mark_done(&mut progress, FirstStep::WatchedPulse);
            }
        } else {
            watch.still_secs = 0.0;
        }
    }

    watch.prev_pos = Some(cam.pos);
    watch.prev_yaw_pitch = Some((cam.yaw, cam.pitch));
    watch.prev_grounded = Some(motion.grounded);

    if dirty {
        progress.save();
    }
}

/// Registers the checklist's observation systems + its session-only watch resource.
///
/// Painting is the play HUD's job (`crate::menu::draw_play_hud` reads
/// [`hud_lines`] directly) — this plugin owns only the ticking.
pub struct FirstStepsPlugin;

impl Plugin for FirstStepsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FirstStepsWatch>()
            // Idempotent (mirrors the established pattern in `FlowPlugin`): a
            // headless test that adds only this plugin still gets a working
            // `MessageReader<InteractResolved>`, and the real app's
            // `PuzzlePlugin` registering it too is harmless.
            .add_message::<InteractResolved>()
            .add_systems(OnEnter(GameState::Playing), reset_watch_on_level_enter)
            .add_systems(
                Update,
                observe_first_steps.run_if(in_state(GameState::Playing)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam(pos: Vec3, yaw: f32, pitch: f32) -> CameraPose {
        CameraPose { pos, yaw, pitch }
    }

    #[test]
    fn every_step_has_a_stable_key_and_a_label() {
        let mut seen = std::collections::HashSet::new();
        for &s in &FIRST_STEPS {
            assert!(!s.key().is_empty());
            assert!(!s.label().is_empty());
            assert!(seen.insert(s.key()), "duplicate key {}", s.key());
        }
    }

    #[test]
    fn mark_done_reports_newness_and_persists_in_progress() {
        let mut progress = Progress::default();
        assert!(!is_done(&progress, FirstStep::Moved));
        assert!(mark_done(&mut progress, FirstStep::Moved));
        assert!(is_done(&progress, FirstStep::Moved));
        assert!(
            !mark_done(&mut progress, FirstStep::Moved),
            "second mark is not new"
        );
    }

    #[test]
    fn all_done_is_false_until_every_item_is_marked() {
        let mut progress = Progress::default();
        assert!(!all_done(&progress));
        for &s in &FIRST_STEPS {
            mark_done(&mut progress, s);
        }
        assert!(all_done(&progress));
    }

    #[test]
    fn hud_lines_empty_when_disabled_or_all_done() {
        let mut progress = Progress::default();
        assert!(
            hud_lines(&progress, false).is_empty(),
            "disabled -> empty regardless"
        );
        assert_eq!(hud_lines(&progress, true).len(), FIRST_STEPS.len());
        for &s in &FIRST_STEPS {
            mark_done(&mut progress, s);
        }
        assert!(
            hud_lines(&progress, true).is_empty(),
            "hides once every item is done"
        );
    }

    #[test]
    fn hud_lines_tick_marks_reflect_progress() {
        let mut progress = Progress::default();
        mark_done(&mut progress, FirstStep::Moved);
        let lines = hud_lines(&progress, true);
        assert!(
            lines[0].starts_with('\u{2713}'),
            "moved is ticked: {}",
            lines[0]
        );
        assert!(
            lines[1].starts_with('\u{00B7}'),
            "looked is not yet ticked: {}",
            lines[1]
        );
    }

    #[test]
    fn old_save_with_no_first_steps_key_loads_with_nothing_ticked() {
        // `Progress::first_steps` is additive + serde(default) — proven properly
        // in `save.rs`'s own tests; this asserts the CONSUMER side (`all_done`/
        // `hud_lines`) degrades correctly on a genuinely empty set, which is
        // exactly what an old save's default produces.
        let progress = Progress::default();
        assert!(!all_done(&progress));
        assert_eq!(hud_lines(&progress, true).len(), FIRST_STEPS.len());
    }

    fn test_app() -> App {
        let mut app = App::new();
        // `MinimalPlugins` (not just `StatesPlugin`) so `Time` — read by
        // `observe_first_steps` for the `WatchedPulse` dwell clock — actually
        // exists; mirrors the established harness shape in `flow::tests::test_app`.
        app.add_plugins(bevy::MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<GameState>();
        app.insert_resource(Progress::default());
        app.insert_resource(PuzzleState::default());
        app.add_plugins(FirstStepsPlugin);
        app
    }

    fn spawn_camera(app: &mut App, cam: CameraPose, motion: PlayerMotion) -> Entity {
        app.world_mut().spawn((cam, motion, PlayCamera)).id()
    }

    fn set_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
    }

    #[test]
    fn moved_ticks_after_real_drift() {
        let mut app = test_app();
        set_playing(&mut app);
        let e = spawn_camera(
            &mut app,
            cam(Vec3::ZERO, 0.0, 0.0),
            PlayerMotion {
                vz: 0.0,
                grounded: true,
                ..Default::default()
            },
        );
        app.update();
        assert!(!is_done(
            app.world().resource::<Progress>(),
            FirstStep::Moved
        ));

        // Move the camera well past the threshold and tick again.
        app.world_mut()
            .entity_mut(e)
            .get_mut::<CameraPose>()
            .unwrap()
            .pos = Vec3::new(2.0, 0.0, 0.0);
        app.update();
        assert!(is_done(
            app.world().resource::<Progress>(),
            FirstStep::Moved
        ));
    }

    #[test]
    fn looked_ticks_after_real_yaw_drift() {
        let mut app = test_app();
        set_playing(&mut app);
        let e = spawn_camera(
            &mut app,
            cam(Vec3::ZERO, 0.0, 0.0),
            PlayerMotion {
                vz: 0.0,
                grounded: true,
                ..Default::default()
            },
        );
        app.update();
        app.world_mut()
            .entity_mut(e)
            .get_mut::<CameraPose>()
            .unwrap()
            .yaw = 0.5;
        app.update();
        assert!(is_done(
            app.world().resource::<Progress>(),
            FirstStep::Looked
        ));
    }

    #[test]
    fn jumped_ticks_on_the_up_kick_not_on_a_fall() {
        let mut app = test_app();
        set_playing(&mut app);
        let e = spawn_camera(
            &mut app,
            cam(Vec3::ZERO, 0.0, 0.0),
            PlayerMotion {
                vz: 0.0,
                grounded: true,
                ..Default::default()
            },
        );
        app.update();
        assert!(!is_done(
            app.world().resource::<Progress>(),
            FirstStep::Jumped
        ));

        // Falling (vz < 0, airborne) must NOT tick it.
        {
            let mut entity = app.world_mut().entity_mut(e);
            let mut m = entity.get_mut::<PlayerMotion>().unwrap();
            m.grounded = false;
            m.vz = -1.0;
        }
        app.update();
        assert!(!is_done(
            app.world().resource::<Progress>(),
            FirstStep::Jumped
        ));

        // Re-ground, then a real jump up-kick.
        {
            let mut entity = app.world_mut().entity_mut(e);
            let mut m = entity.get_mut::<PlayerMotion>().unwrap();
            m.grounded = true;
            m.vz = 0.0;
        }
        app.update();
        {
            let mut entity = app.world_mut().entity_mut(e);
            let mut m = entity.get_mut::<PlayerMotion>().unwrap();
            m.grounded = false;
            m.vz = 3.0;
        }
        app.update();
        assert!(is_done(
            app.world().resource::<Progress>(),
            FirstStep::Jumped
        ));
    }

    #[test]
    fn toggled_switch_ticks_from_interact_resolved() {
        let mut app = test_app();
        set_playing(&mut app);
        app.update();
        app.world_mut().write_message(InteractResolved {
            at: re_puzzle::InteractAt { x: 0, y: 0, z: 0 },
            outcome: InteractOutcome::Toggled {
                pipe: 0,
                now_on: true,
            },
        });
        app.update();
        assert!(is_done(
            app.world().resource::<Progress>(),
            FirstStep::ToggledSwitch
        ));
    }

    #[test]
    fn refused_interact_does_not_tick_toggled() {
        let mut app = test_app();
        set_playing(&mut app);
        app.update();
        app.world_mut().write_message(InteractResolved {
            at: re_puzzle::InteractAt { x: 0, y: 0, z: 0 },
            outcome: InteractOutcome::NoTarget,
        });
        app.update();
        assert!(!is_done(
            app.world().resource::<Progress>(),
            FirstStep::ToggledSwitch
        ));
    }

    #[test]
    fn level_reentry_resets_the_watch_but_not_progress() {
        let mut app = test_app();
        set_playing(&mut app);
        let e = spawn_camera(
            &mut app,
            cam(Vec3::ZERO, 0.0, 0.0),
            PlayerMotion {
                vz: 0.0,
                grounded: true,
                ..Default::default()
            },
        );
        app.update();
        app.world_mut()
            .entity_mut(e)
            .get_mut::<CameraPose>()
            .unwrap()
            .pos = Vec3::new(2.0, 0.0, 0.0);
        app.update();
        assert!(is_done(
            app.world().resource::<Progress>(),
            FirstStep::Moved
        ));

        // Re-entering Playing (a new level) resets the session watch, but the
        // persisted Progress tick must survive — first-steps are per-save, not
        // per-level.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Menu);
        app.update();
        set_playing(&mut app);
        app.update();
        assert!(is_done(
            app.world().resource::<Progress>(),
            FirstStep::Moved
        ));
    }
}
