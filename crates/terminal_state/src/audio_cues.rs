//! `audio_cues` — the game side of the sound grammar: it watches play and tells
//! `re_audio` **what happened and where**, never which sample to play.
//!
//! `re_audio` owns the vocabulary ([`re_audio::Cue`] → voices, pitch, level,
//! harmonic tension) and the space (room reverb, listener, positional loops).
//! This module is the adapter from game state to that vocabulary:
//!
//! * **Room** — on every level load the level's box (`size`), its sky (an open
//!   sky has no ceiling to reflect from) and its floor/wall materials become a
//!   [`re_audio::Room`], sent as [`re_audio::SetRoom`]. The ambience is re-baked in that room, so a
//!   cramped server closet and a frozen hall no longer sound identical.
//! * **Listener + positional sources** — each play frame the camera becomes the
//!   [`re_audio::AudioListener`], and lamps, powered pipes, lit nodes and world emitters
//!   become [`re_audio::AudioSources`]. Pipe hum pitch and level follow the power load.
//! * **Harmony** — the lit fraction of the network is written to
//!   [`re_audio::HarmonyProgress`], so solving colours the ambience chord.
//! * **Cues** — interact outcomes, node lit/unlit edges, the circuit resolving or
//!   breaking, aiming at a switch, footsteps by surface, placards coming into
//!   reading range, portal crossings, storey changes, recovered colour shards and
//!   menu navigation are each sent as a [`re_audio::PlayCue`], positioned where they happen.
//!
//! Every decision is a pure function or a small state machine
//! ([`MotionTracker`](crate::audio_cues::MotionTracker), [`EdgeTracker`](crate::audio_cues::EdgeTracker))
//! so it is unit-tested with no App and no audio device.

use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;
use re_audio::{
    AudioListener, AudioSources, Cue, HarmonyProgress, Listener, LoopKind, MirrorFootstep,
    NoteVoice, PlayCue, PositionalSource, Room, SetRoom, SourceKey, Surface,
};
use re_level::{Level, NoteKind};
use re_present::MenuNav;
use re_puzzle::{InteractOutcome, InteractResolved, PuzzleState};
use re_world::emitter_spec::EmitterOrigin;
use re_world::CameraPose;

use crate::flow::{self, CurrentLevelData, GameState, PlayCamera};
use crate::mirror_ramp::shards_held;
use crate::save::Progress;
use re_runtime::sound_fields::false_echo_mirror;

// `audio_world` is `re_runtime`'s room-probe/occlusion module — brought into
// scope here so the existing `audio_world::probe_room` /
// `audio_world::occlusion_between` call sites below are unchanged.
use re_runtime::audio_world;

/// Horizontal distance (cells) between two footsteps.
pub const STRIDE_CELLS: f32 = 1.1;
/// A per-frame jump larger than this (cells) is a teleport, not a walk.
pub const TELEPORT_CELLS: f32 = 2.0;
/// A placard is "read" when the player is within this many cells of it…
pub const NOTE_RANGE: f32 = 2.2;
/// …and it lies within this cosine of straight ahead (≈ ±65°).
pub const NOTE_FACING_COS: f32 = 0.42;

/// Centre of a world cell.
#[inline]
fn cell_centre(x: i32, y: i32, z: i32) -> [f32; 3] {
    [x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5]
}

// ─────────────────────────────── level → room ───────────────────────────────

/// Mean absorption coefficient of one surface class.
fn absorption_of(s: Surface) -> f32 {
    match s {
        Surface::Metal => 0.08,
        Surface::Hard => 0.18,
        Surface::Wet => 0.26,
        Surface::Soft => 0.55,
    }
}

/// Classify a surface slot: the named material if the level defines one (its
/// name plus its texture recipe), else the slot's name itself.
fn surface_of_slot(level: &Level, slot: Option<&str>) -> Option<Surface> {
    let name = slot?;
    let mut key = name.to_string();
    if let Some(m) = level.materials.iter().find(|m| m.name == name) {
        if let Some(r) = m.recipe {
            use std::fmt::Write as _;
            let _ = write!(key, " {r:?}");
        }
    }
    Some(Surface::from_material(&key))
}

/// What the player's feet land on in this level. Snow on the ground wins
/// (whatever is under it, you walk on snow).
pub fn floor_surface(level: &Level) -> Surface {
    if level.effects.snow > 0.05 {
        return Surface::Soft;
    }
    surface_of_slot(level, level.surface_materials.floor.as_deref())
        .or_else(|| {
            (!level.room_floor.is_empty()).then(|| Surface::from_material(&level.room_floor))
        })
        .unwrap_or_default()
}

/// The acoustic room a level describes: its voxel box, an open sky when it has
/// one, and an absorption averaged over floor and walls (snow deadens it).
pub fn room_for_level(level: &Level) -> Room {
    let (sx, sy, sz) = level.size;
    let floor = absorption_of(floor_surface(level));
    let wall = absorption_of(
        surface_of_slot(level, level.surface_materials.wall_solid.as_deref()).unwrap_or_default(),
    );
    let absorption = f32::midpoint(floor, wall) + 0.3 * level.effects.snow.clamp(0.0, 1.0);
    Room::from_cells(sx, sy, sz, !level.sky.is_empty(), absorption)
}

// ─────────────────────────────── sources ────────────────────────────────────

/// Power load in `[0, 1]`: ON pipes over the budget, or over all switchable
/// pipes when the level has no budget.
pub fn power_load(puzzle: &PuzzleState) -> f32 {
    let used = puzzle.power_used() as f32;
    let cap = if puzzle.power_budget > 0 {
        puzzle.power_budget as f32
    } else {
        puzzle
            .pipes
            .iter()
            .filter(|p| p.switch_cell.is_some())
            .count()
            .max(1) as f32
    };
    (used / cap).clamp(0.0, 1.0)
}

/// A node fires when its activation is 1 (the forward pass's own definition).
fn fired(puzzle: &PuzzleState) -> Vec<bool> {
    puzzle.nodes.iter().map(|n| n.activation == 1).collect()
}

/// `(lit, total)` over the non-input nodes (inputs are always lit, so they say
/// nothing about progress).
pub fn lit_count(puzzle: &PuzzleState) -> (u32, u32) {
    let mut lit = 0;
    let mut total = 0;
    for n in puzzle.nodes.iter().filter(|n| !n.is_input) {
        total += 1;
        if n.activation == 1 {
            lit += 1;
        }
    }
    (lit, total)
}

/// Every sounding thing in the level right now, heard from `ear`.
///
/// * lamps → [`LoopKind::LampHum`] at the light, level from its intensity;
/// * ON pipes → [`LoopKind::PipeHum`] at the path cell nearest the listener
///   (you hear the conduit you stand next to), pitch/level from the load;
/// * firing non-input nodes → [`LoopKind::NodeGlow`], each on its own ladder
///   pitch so a lit network is a chord;
/// * authored world emitters → [`LoopKind::EmitterHiss`].
pub fn sources_for(
    level: &Level,
    puzzle: Option<&PuzzleState>,
    ear: [f32; 3],
) -> Vec<PositionalSource> {
    let mut out = Vec::new();
    for (i, l) in level.lights.iter().enumerate() {
        out.push(PositionalSource {
            key: SourceKey {
                kind: LoopKind::LampHum,
                id: i as u32,
            },
            pos: [l.pos.x, l.pos.y, l.pos.z],
            level: (0.35 + 0.5 * l.intensity).clamp(0.2, 1.0),
            rate: 1.0,
            // Set by `play_frame_sys` from a grid ray test after this
            // function returns (this module is engine-agnostic and has no
            // grid to test against — see `audio_world::occlusion_between`).
            occlusion: 0.0,
        });
    }
    if let Some(p) = puzzle {
        let load = power_load(p);
        for (i, pipe) in p.pipes.iter().enumerate().filter(|(_, p)| p.is_on()) {
            let d2 = |c: &(i32, i32, i32)| {
                let q = cell_centre(c.0, c.1, c.2);
                (q[0] - ear[0]).powi(2) + (q[1] - ear[1]).powi(2) + (q[2] - ear[2]).powi(2)
            };
            let Some(c) = pipe.path.iter().min_by(|a, b| d2(a).total_cmp(&d2(b))) else {
                continue;
            };
            out.push(PositionalSource {
                key: SourceKey {
                    kind: LoopKind::PipeHum,
                    id: i as u32,
                },
                pos: cell_centre(c.0, c.1, c.2),
                level: re_audio::soundscape::pipe_hum_level(load),
                rate: re_audio::soundscape::pipe_hum_rate(load),
                occlusion: 0.0, // see the comment on the lamp push above
            });
        }
        let mut rung = 0usize;
        for (i, n) in p.nodes.iter().enumerate() {
            if n.is_input {
                continue;
            }
            if n.activation == 1 {
                if let Some(ln) = level.nodes.get(i) {
                    out.push(PositionalSource {
                        key: SourceKey {
                            kind: LoopKind::NodeGlow,
                            id: i as u32,
                        },
                        pos: cell_centre(ln.x, ln.y, ln.z),
                        level: 0.8,
                        rate: re_audio::soundscape::node_glow_rate(rung),
                        occlusion: 0.0, // see the comment on the lamp push above
                    });
                }
            }
            rung += 1;
        }
    }
    for (i, e) in level.emitters.iter().enumerate() {
        if e.synthesized {
            continue;
        }
        if let EmitterOrigin::World(v) = e.origin {
            out.push(PositionalSource {
                key: SourceKey {
                    kind: LoopKind::EmitterHiss,
                    id: i as u32,
                },
                pos: [v.x, v.y, v.z],
                level: 0.7,
                rate: 1.0,
                occlusion: 0.0, // see the comment on the lamp push above
            });
        }
    }
    out
}

/// Power-reactive prop lamps → [`LoopKind::LampHum`].
///
/// `atmo_lights` is the built map's light list: the level's `authored` `[light]`s
/// first (already voiced by [`sources_for`]), then the prop lamps, which
/// `prop_power` holds at `intensity = 0` while unpowered — those stay silent, so a
/// lamp coming on is heard where it hangs. Ids continue the authored lamps' ids, so
/// none collide.
#[must_use]
pub fn prop_lamp_sources(
    authored: usize,
    atmo_lights: &[re_world::PointLight],
) -> Vec<PositionalSource> {
    atmo_lights
        .iter()
        .enumerate()
        .skip(authored)
        .filter(|(_, l)| l.intensity > 0.0)
        .map(|(i, l)| PositionalSource {
            key: SourceKey {
                kind: LoopKind::LampHum,
                id: i as u32,
            },
            pos: [l.pos.x, l.pos.y, l.pos.z],
            level: (0.35 + 0.5 * l.intensity).clamp(0.2, 1.0),
            rate: 1.0,
            occlusion: 0.0, // set by `play_frame_sys` after both lists merge
        })
        .collect()
}

// ─────────────────────────────── cue mapping ────────────────────────────────

/// The cue a resolved interaction earned. `used`/`budget` is the power readout
/// AFTER the press. ON and OFF are different cues (rising vs falling), a press
/// into empty air is its own dull tick rather than a "confirm", and the two
/// refusals are two different dissonances.
pub fn outcome_cue(outcome: InteractOutcome, used: u32, budget: u32) -> Cue {
    match outcome {
        InteractOutcome::Toggled { now_on: true, .. } => Cue::SwitchOn { used, budget },
        InteractOutcome::Toggled { now_on: false, .. } => Cue::SwitchOff,
        InteractOutcome::NoTarget => Cue::InteractEmpty,
        InteractOutcome::RefusedNoPower { .. } => Cue::RefusedNoPower,
        InteractOutcome::RefusedCycle { .. } => Cue::RefusedCycle,
    }
}

/// Menu navigation → cue. Up/Down/Left/Right move the highlight, Confirm
/// commits (rising), Back cancels (falling).
pub fn menu_cue(nav: MenuNav) -> Cue {
    match nav {
        MenuNav::Up | MenuNav::Down | MenuNav::Left | MenuNav::Right => Cue::MenuMove,
        MenuNav::Confirm => Cue::MenuConfirm,
        MenuNav::Back => Cue::MenuBack,
    }
}

/// Placard kind → the voice that reads it.
pub fn note_voice(kind: NoteKind) -> NoteVoice {
    match kind {
        NoteKind::Researcher => NoteVoice::Researcher,
        NoteKind::SystemLog => NoteVoice::SystemLog,
        NoteKind::ModelThought => NoteVoice::ModelThought,
        NoteKind::Stencil => NoteVoice::Stencil,
    }
}

/// Node / circuit edge detector. The first observation after a reset is the
/// baseline and is silent (a level that loads with lit nodes, or vacuously
/// solved, must not announce it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdgeTracker {
    fired: Option<Vec<bool>>,
    solved: Option<bool>,
}

impl EdgeTracker {
    /// Forget the baseline (new level).
    pub fn reset(&mut self) {
        *self = EdgeTracker::default();
    }

    /// Observe the network; returns `(node index or None, cue)` for each edge,
    /// nodes first, then the circuit. `lit`/`total` in the node cues count the
    /// non-input nodes AFTER the change.
    pub fn observe(&mut self, puzzle: &PuzzleState) -> Vec<(Option<usize>, Cue)> {
        let now = fired(puzzle);
        let (lit, total) = lit_count(puzzle);
        let mut out = Vec::new();
        if let Some(prev) = &self.fired {
            if prev.len() == now.len() {
                for (i, (&was, &is)) in prev.iter().zip(&now).enumerate() {
                    if puzzle.nodes[i].is_input || was == is {
                        continue;
                    }
                    let cue = if is {
                        Cue::NodeLit { lit, total }
                    } else {
                        Cue::NodeUnlit { lit, total }
                    };
                    out.push((Some(i), cue));
                }
            }
        }
        if let Some(was) = self.solved {
            if was != puzzle.solved {
                out.push((
                    None,
                    if puzzle.solved {
                        Cue::CircuitComplete
                    } else {
                        Cue::CircuitBroken
                    },
                ));
            }
        }
        self.fired = Some(now);
        self.solved = Some(puzzle.solved);
        out
    }
}

/// Walking, teleporting and climbing, from successive camera positions.
#[derive(Clone, Debug, PartialEq)]
pub struct MotionTracker {
    /// Footstep surface for this level.
    pub surface: Surface,
    /// Cells per storey.
    pub storey_h: f32,
    /// Whether the level has portals (a teleport there is a portal crossing;
    /// elsewhere it is a respawn and stays silent).
    pub has_portals: bool,
    last: Option<[f32; 3]>,
    acc: f32,
    stride: u32,
    storey: Option<i32>,
}

impl Default for MotionTracker {
    fn default() -> Self {
        MotionTracker::new(Surface::Hard, 4.0, false)
    }
}

impl MotionTracker {
    /// Start a tracker with no prior position: the first `step` call establishes
    /// the baseline instead of producing a cue.
    pub fn new(surface: Surface, storey_h: f32, has_portals: bool) -> MotionTracker {
        MotionTracker {
            surface,
            storey_h: storey_h.max(1.0),
            has_portals,
            last: None,
            acc: 0.0,
            stride: 0,
            storey: None,
        }
    }

    /// Forget the previous position (the next step is a silent baseline).
    pub fn rebase(&mut self) {
        self.last = None;
        self.storey = None;
        self.acc = 0.0;
    }

    fn storey_of(&self, z: f32) -> i32 {
        (z / self.storey_h).floor() as i32
    }

    /// Advance to `pos`; returns the cues this move produced.
    pub fn step(&mut self, pos: [f32; 3]) -> Vec<Cue> {
        let mut out = Vec::new();
        let storey = self.storey_of(pos[2]);
        if let Some(last) = self.last {
            let dh = (pos[0] - last[0]).hypot(pos[1] - last[1]);
            let jump = dh.hypot(pos[2] - last[2]);
            if jump > TELEPORT_CELLS {
                if self.has_portals {
                    out.push(Cue::Portal);
                }
                self.acc = 0.0;
            } else {
                self.acc += dh;
                while self.acc >= STRIDE_CELLS {
                    self.acc -= STRIDE_CELLS;
                    out.push(Cue::Footstep {
                        surface: self.surface,
                        stride: self.stride,
                    });
                    self.stride = self.stride.wrapping_add(1);
                }
            }
            if let Some(prev) = self.storey {
                if storey != prev {
                    out.push(Cue::Storey { up: storey > prev });
                }
            }
        }
        self.last = Some(pos);
        self.storey = Some(storey);
        out
    }
}

/// Is a placard at `note` readable from `cam`: close, on the same storey, and in
/// front?
pub fn note_in_view(cam: &CameraPose, note: [f32; 3]) -> bool {
    let dx = note[0] - cam.pos.x;
    let dy = note[1] - cam.pos.y;
    let d = dx.hypot(dy);
    if d > NOTE_RANGE || (note[2] - cam.pos.z).abs() > 1.5 {
        return false;
    }
    if d < 0.35 {
        return true;
    }
    let (fx, fy) = cam.dir();
    (fx * dx + fy * dy) / d >= NOTE_FACING_COS
}

// ─────────────────────────────── Bevy wiring ────────────────────────────────

/// Per-level cue state.
#[derive(Resource, Debug, Default)]
pub struct CueTracker {
    /// Footstep/teleport/storey-change tracking for the current level.
    pub motion: MotionTracker,
    /// Node-lit-edge → cue tracking for the current level's puzzle.
    pub edges: EdgeTracker,
    /// Whether the crosshair was on a switch last frame.
    pub aimed: bool,
    /// One flag per level note: already read this visit.
    pub notes_read: Vec<bool>,
    /// Shards held when last observed (`None` = no baseline yet).
    pub shards: Option<usize>,
    /// The level's material absorption (`room_for_level`'s floor/wall/snow
    /// blend), cached on load so the per-position room probe
    /// (`play_frame_sys`) does not re-walk the level's materials every
    /// frame — only the ray-probe geometry changes as the player moves; the
    /// absorption stays whatever the level's overall material mix implies
    /// (per-position material sampling is a LATER editor item).
    pub room_absorption: f32,
    /// Per-position local-room estimator, seeded on level load
    /// (`level_room_sys`) and advanced every play frame
    /// (`play_frame_sys`) — `None` before a level has ever loaded.
    pub room_tracker: Option<re_audio::local_room::RoomTracker>,
}

/// Registers the cue systems. Add after `re_audio::AudioPlugin`; it also
/// registers the messages/resources itself so a focused test app works.
pub struct AudioCuesPlugin;

impl Plugin for AudioCuesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayCue>()
            .add_message::<SetRoom>()
            .add_message::<InteractResolved>()
            .add_message::<MenuNav>()
            .init_resource::<AudioListener>()
            .init_resource::<AudioSources>()
            .init_resource::<HarmonyProgress>()
            .init_resource::<CueTracker>()
            .add_systems(
                Update,
                (
                    level_room_sys,
                    interact_cue_sys.after(re_puzzle::apply_interactions),
                    play_frame_sys
                        .after(level_room_sys)
                        .after(re_puzzle::apply_interactions)
                        .run_if(in_state(GameState::Playing)),
                    shard_cue_sys,
                    menu_cue_sys.run_if(
                        in_state(GameState::Menu)
                            .or_else(in_state(GameState::CategorySelect))
                            .or_else(in_state(GameState::LevelSelect))
                            .or_else(in_state(GameState::Settings))
                            .or_else(in_state(GameState::Pause)),
                    ),
                ),
            )
            .add_systems(OnEnter(GameState::Playing), rebase_motion_sys)
            .add_systems(OnExit(GameState::Playing), clear_listener_sys);
    }
}

/// On a level load: send the room, re-arm every per-level tracker.
fn level_room_sys(
    level: Option<Res<CurrentLevelData>>,
    mut tracker: ResMut<CueTracker>,
    mut harmony: ResMut<HarmonyProgress>,
    mut room: MessageWriter<SetRoom>,
) {
    let Some(level) = level else { return };
    if !level.is_changed() {
        return;
    }
    let Some(lvl) = level.0.as_ref() else { return };
    let whole_level_room = room_for_level(lvl);
    room.write(SetRoom(whole_level_room));
    tracker.motion = MotionTracker::new(
        floor_surface(lvl),
        lvl.storey_height() as f32,
        !lvl.portals.is_empty(),
    );
    tracker.edges.reset();
    tracker.aimed = false;
    tracker.notes_read = vec![false; lvl.notes.len()];
    harmony.0 = 0.0;
    // Seed the per-position estimator at the whole-level room (a sane guess
    // before the player's first probe lands) and cache the level's material
    // absorption for it (see `CueTracker::room_absorption`).
    tracker.room_absorption = whole_level_room.absorption;
    tracker.room_tracker = Some(re_audio::local_room::RoomTracker::new(whole_level_room));
}

/// A resolved interaction → its cue, at the cell that was pressed.
fn interact_cue_sys(
    mut resolved: MessageReader<InteractResolved>,
    puzzle: Option<Res<PuzzleState>>,
    mut cues: MessageWriter<PlayCue>,
) {
    for r in resolved.read() {
        let (used, budget) = puzzle
            .as_ref()
            .map_or((0, 0), |p| (p.power_used(), p.power_budget));
        let cue = outcome_cue(r.outcome, used, budget);
        cues.write(match r.outcome {
            InteractOutcome::NoTarget => PlayCue::new(cue),
            _ => PlayCue::at(cue, cell_centre(r.at.x, r.at.y, r.at.z)),
        });
    }
}

/// Every play frame: listener, sources, harmony, and the edge-driven cues.
// Bevy system params are by value by construction (`needless_pass_by_value` is a
// false positive on `Res`).
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
fn play_frame_sys(
    cam_q: Query<&CameraPose, With<PlayCamera>>,
    level: Option<Res<CurrentLevelData>>,
    puzzle: Option<Res<PuzzleState>>,
    mut tracker: ResMut<CueTracker>,
    mut listener: ResMut<AudioListener>,
    mut sources: ResMut<AudioSources>,
    mut harmony: ResMut<HarmonyProgress>,
    map: Option<Res<re_world::Map>>,
    time: Res<Time>,
    mut cues: MessageWriter<PlayCue>,
    mut room_msgs: MessageWriter<SetRoom>,
    mut mirror_msgs: MessageWriter<MirrorFootstep>,
) {
    let Ok(cam) = cam_q.single() else { return };
    let ear = [cam.pos.x, cam.pos.y, cam.pos.z];
    listener.0 = Some(Listener {
        pos: ear,
        yaw: cam.yaw,
    });
    let lvl = level.as_ref().and_then(|l| l.0.as_ref());
    let puzzle = puzzle.as_deref();

    sources.0 = lvl.map(|l| sources_for(l, puzzle, ear)).unwrap_or_default();
    // Power-reactive prop lamps live only in the built map's atmosphere (after the
    // authored `[light]`s); a lit one hums like any lamp, a dark one is silent.
    if let (Some(l), Some(m)) = (lvl, map.as_deref()) {
        sources
            .0
            .extend(prop_lamp_sources(l.lights.len(), &m.atmosphere().lights));
    }
    if let Some(m) = map.as_deref() {
        // Occlusion: how much solid wall sits between the listener and each
        // source right now (a grid ray test — the one thing `re_audio`'s
        // pure `spatial` module cannot compute itself).
        for s in &mut sources.0 {
            s.occlusion = audio_world::occlusion_between(m, ear, s.pos);
        }
        // Local room: re-probe from the player's own position every frame
        // (`RoomTracker` internally throttles how often that actually turns
        // into a `SetRoom` — see its `MIN_INTERVAL_S`), so the reverb tracks
        // corridor → hall → outdoors → closet as the player walks, not just
        // the level as a whole.
        let probe = audio_world::probe_room(m, ear, tracker.room_absorption);
        let dt = time.delta_secs();
        // `level_room_sys` (which runs first, see the plugin's system order)
        // always seeds this on load; a `None` here means no level has ever
        // loaded, so there is nothing to probe relative to yet.
        if let Some(room) = tracker
            .room_tracker
            .as_mut()
            .and_then(|rt| rt.update(&probe, dt))
        {
            room_msgs.write(SetRoom(room));
        }
    }

    let tracker = &mut *tracker;
    if let Some(p) = puzzle {
        let (lit, total) = lit_count(p);
        harmony.0 = re_audio::soundscape::harmony_progress(lit, total, p.solved);
        for (node, cue) in tracker.edges.observe(p) {
            let at = node.and_then(|i| lvl.and_then(|l| l.nodes.get(i)));
            cues.write(match at {
                Some(n) => PlayCue::at(cue, cell_centre(n.x, n.y, n.z)),
                None => PlayCue::new(cue),
            });
        }
        let (x, y, z, lands) = flow::faced_switch(cam, p);
        if lands && !tracker.aimed {
            cues.write(PlayCue::at(Cue::AimSwitch, cell_centre(x, y, z)));
        }
        tracker.aimed = lands;
    }

    for cue in tracker.motion.step(ear) {
        // §2 F4 "Mirror voice": a footstep landing inside a `false_echo`
        // field's cone is re-emitted as a one-shot at the listener's
        // reflection across the mirror plane (`re_audio::plugin::
        // play_mirror_footsteps` forwards this to `Player::
        // schedule_mirror_footstep`, which spatialises it on the clock).
        if matches!(cue, Cue::Footstep { .. }) {
            if let Some(l) = lvl {
                if let Some((pos, delay_s)) = false_echo_mirror(l.fields(), ear) {
                    mirror_msgs.write(MirrorFootstep { pos, delay_s });
                }
            }
        }
        cues.write(PlayCue::new(cue));
    }

    if let Some(l) = lvl {
        if tracker.notes_read.len() != l.notes.len() {
            tracker.notes_read = vec![false; l.notes.len()];
        }
        for (i, n) in l.notes.iter().enumerate() {
            let at = cell_centre(n.x, n.y, n.z);
            if !tracker.notes_read[i] && note_in_view(cam, at) {
                tracker.notes_read[i] = true;
                cues.write(PlayCue::at(Cue::NoteRead(note_voice(n.kind)), at));
            }
        }
    }
}

/// A recovered colour shard (First Light) → the bloom cue, one step higher per
/// shard. The first observation is a silent baseline (loading a save is not a
/// recovery).
fn shard_cue_sys(
    progress: Option<Res<Progress>>,
    mut tracker: ResMut<CueTracker>,
    mut cues: MessageWriter<PlayCue>,
) {
    let Some(progress) = progress else { return };
    let held = shards_held(&progress);
    if let Some(prev) = tracker.shards {
        if held > prev {
            cues.write(PlayCue::new(Cue::FirstLight {
                shards: held as u32,
            }));
        }
    }
    tracker.shards = Some(held);
}

/// Menu navigation (menu states only — the same `MenuNav` also fires in play).
fn menu_cue_sys(mut nav: MessageReader<MenuNav>, mut cues: MessageWriter<PlayCue>) {
    for n in nav.read() {
        cues.write(PlayCue::new(menu_cue(*n)));
    }
}

/// Entering play (a new level or un-pausing): the next camera position is a
/// baseline, never a step or a teleport.
fn rebase_motion_sys(mut tracker: ResMut<CueTracker>) {
    tracker.motion.rebase();
}

/// Leaving play: the ears leave the world, the positional loops fall silent.
fn clear_listener_sys(mut listener: ResMut<AudioListener>, mut sources: ResMut<AudioSources>) {
    listener.0 = None;
    sources.0.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use re_puzzle::InteractOutcome as O;

    fn level(name: &str) -> Level {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../levels")
            .join(format!("{name}.lvl"));
        re_level::load(p).expect("level loads")
    }

    #[test]
    fn every_interact_outcome_has_its_own_cue_and_on_differs_from_off() {
        let on = outcome_cue(
            O::Toggled {
                pipe: 0,
                now_on: true,
            },
            1,
            2,
        );
        let off = outcome_cue(
            O::Toggled {
                pipe: 0,
                now_on: false,
            },
            0,
            2,
        );
        let empty = outcome_cue(O::NoTarget, 0, 2);
        let power = outcome_cue(
            O::RefusedNoPower {
                pipe: 0,
                used: 2,
                budget: 2,
            },
            2,
            2,
        );
        let cycle = outcome_cue(O::RefusedCycle { pipe: 0 }, 1, 2);
        assert_eq!(on, Cue::SwitchOn { used: 1, budget: 2 });
        assert_eq!(off, Cue::SwitchOff);
        let all = [on, off, empty, power, cycle];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.voices()[0].sound, b.voices()[0].sound, "{a:?} vs {b:?}");
            }
        }
        // Refusals raise tension; acceptances do not.
        assert!(power.tension() > 0.0 && cycle.tension() > 0.0);
        assert_eq!(on.tension(), 0.0);
        // Player feedback is never beat-quantized: a denial that lags a beat
        // behind the key press reads as a dropped input, not as a rule.
        for c in all {
            for v in c.voices() {
                assert_eq!(
                    re_audio::quantize_of(v.sound),
                    re_audio::Quantize::Immediate,
                    "{c:?}"
                );
            }
        }
    }

    #[test]
    fn menu_moves_confirms_and_backs_are_distinct() {
        assert_eq!(menu_cue(MenuNav::Up), Cue::MenuMove);
        assert_eq!(menu_cue(MenuNav::Right), Cue::MenuMove);
        assert_eq!(menu_cue(MenuNav::Confirm), Cue::MenuConfirm);
        assert_eq!(menu_cue(MenuNav::Back), Cue::MenuBack);
    }

    #[test]
    fn walking_steps_by_stride_and_alternates_feet() {
        let mut m = MotionTracker::new(Surface::Metal, 4.0, false);
        assert!(
            m.step([1.0, 1.0, 1.5]).is_empty(),
            "first sample is a baseline"
        );
        let mut steps = Vec::new();
        for i in 1..=40 {
            steps.extend(m.step([1.0 + i as f32 * 0.1, 1.0, 1.5]));
        }
        // 4.0 cells walked → 3 strides of 1.1.
        assert_eq!(steps.len(), 3);
        assert_eq!(
            steps[0],
            Cue::Footstep {
                surface: Surface::Metal,
                stride: 0
            }
        );
        assert_eq!(
            steps[1],
            Cue::Footstep {
                surface: Surface::Metal,
                stride: 1
            }
        );
        // Turning on the spot makes no steps.
        assert!(m.step([5.0, 1.0, 1.5]).is_empty());
    }

    #[test]
    fn teleports_are_portals_only_where_portals_exist_and_storeys_have_direction() {
        let mut m = MotionTracker::new(Surface::Hard, 4.0, true);
        m.step([1.0, 1.0, 1.5]);
        assert_eq!(m.step([9.0, 9.0, 1.5]), vec![Cue::Portal]);
        let mut r = MotionTracker::new(Surface::Hard, 4.0, false);
        r.step([1.0, 1.0, 1.5]);
        assert!(r.step([9.0, 9.0, 1.5]).is_empty(), "a respawn is silent");

        let mut s = MotionTracker::new(Surface::Hard, 4.0, false);
        s.step([1.0, 1.0, 3.5]);
        assert_eq!(s.step([1.0, 1.0, 4.5]), vec![Cue::Storey { up: true }]);
        assert_eq!(s.step([1.0, 1.0, 3.5]), vec![Cue::Storey { up: false }]);
        s.rebase();
        assert!(
            s.step([1.0, 1.0, 9.5]).is_empty(),
            "after a rebase: baseline"
        );
    }

    #[test]
    fn node_and_circuit_edges_are_announced_once_after_a_silent_baseline() {
        let lvl = level("02");
        let mut p = PuzzleState::from_level(&lvl).expect("02 builds");
        let mut e = EdgeTracker::default();
        assert!(e.observe(&p).is_empty(), "baseline is silent");
        assert!(e.observe(&p).is_empty(), "no change, no cue");
        // Light every non-input node by hand and mark solved.
        for n in p.nodes.iter_mut().filter(|n| !n.is_input) {
            n.activation = 1;
        }
        let was_solved = p.solved;
        p.solved = true;
        let cues = e.observe(&p);
        assert!(cues
            .iter()
            .any(|(i, c)| i.is_some() && matches!(c, Cue::NodeLit { .. })));
        if !was_solved {
            assert_eq!(cues.last().unwrap().1, Cue::CircuitComplete);
        }
        // Dark again: unlit + broken.
        for n in p.nodes.iter_mut().filter(|n| !n.is_input) {
            n.activation = 0;
        }
        p.solved = false;
        let cues = e.observe(&p);
        assert!(cues
            .iter()
            .any(|(_, c)| matches!(c, Cue::NodeUnlit { lit: 0, .. })));
        assert_eq!(cues.last().unwrap().1, Cue::CircuitBroken);
    }

    #[test]
    fn a_level_becomes_a_room_with_its_own_size_and_sky() {
        // "11-cold-storage", "21-emitter-showcase", "22-lit-cellar" were engine
        // example levels, used here only as a convenient size/shape sample —
        // not a campaign or archive fact. They live under `terminal3d/levels/`,
        // which this crate's own `levels/` (campaign + archive only) does not
        // reach. Swapped for three more archive rooms that stay in this repo
        // and still vary in size/shape.
        for n in ["01", "02", "05", "03", "04", "06"] {
            let lvl = level(n);
            let room = room_for_level(&lvl);
            let (sx, sy, sz) = lvl.size;
            assert_eq!(
                room,
                Room::from_cells(sx, sy, sz, !lvl.sky.is_empty(), room.absorption)
            );
            assert!((0.02..=0.95).contains(&room.absorption));
            assert!(room.rt60() > 0.1);
        }
    }

    #[test]
    fn snow_makes_the_floor_soft_and_the_room_deader() {
        let mut lvl = level("02");
        lvl.effects.snow = 0.0;
        let dry = room_for_level(&lvl);
        lvl.effects.snow = 0.8;
        assert_eq!(floor_surface(&lvl), Surface::Soft);
        let snowy = room_for_level(&lvl);
        assert!(snowy.absorption > dry.absorption);
        assert!(snowy.rt60() < dry.rt60());
    }

    #[test]
    fn sources_follow_the_level_and_the_power_load() {
        let lvl = level("02");
        let mut p = PuzzleState::from_level(&lvl).expect("02 builds");
        for pipe in &mut p.pipes {
            pipe.state = re_level::PipeState::Off;
        }
        let ear = [1.5, 1.5, 1.5];
        let quiet = sources_for(&lvl, Some(&p), ear);
        assert!(
            quiet.iter().all(|s| s.key.kind != LoopKind::PipeHum),
            "no power, no hum"
        );
        assert_eq!(
            quiet
                .iter()
                .filter(|s| s.key.kind == LoopKind::LampHum)
                .count(),
            lvl.lights.len()
        );
        // Power one switchable pipe, then all of them: the hum rises in pitch.
        let sw: Vec<usize> = (0..p.pipes.len())
            .filter(|&i| p.pipes[i].switch_cell.is_some())
            .collect();
        assert!(!sw.is_empty());
        p.power_budget = 0;
        p.pipes[sw[0]].state = re_level::PipeState::On;
        let one = sources_for(&lvl, Some(&p), ear);
        let hum = |v: &[PositionalSource]| {
            v.iter()
                .find(|s| s.key.kind == LoopKind::PipeHum)
                .map(|s| s.rate)
        };
        let low = hum(&one).expect("a powered pipe hums");
        for &i in &sw {
            p.pipes[i].state = re_level::PipeState::On;
        }
        let high = hum(&sources_for(&lvl, Some(&p), ear)).unwrap();
        assert!(high >= low, "more load, higher hum: {low} → {high}");
        if sw.len() > 1 {
            assert!(high > low);
        }
    }

    #[test]
    fn a_note_is_read_only_when_close_and_in_front() {
        let cam = CameraPose {
            pos: re_core::Vec3::new(5.0, 5.0, 1.5),
            yaw: 0.0,
            pitch: 0.0,
        };
        // yaw 0 faces +y.
        assert!(note_in_view(&cam, [5.0, 6.5, 1.5]));
        assert!(!note_in_view(&cam, [5.0, 3.5, 1.5]), "behind");
        assert!(!note_in_view(&cam, [5.0, 9.0, 1.5]), "too far");
        assert!(!note_in_view(&cam, [5.0, 6.5, 5.5]), "another storey");
    }

    #[test]
    fn the_plugin_turns_a_refused_press_into_a_positioned_refusal_cue() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<GameState>()
            .add_plugins(re_puzzle::PuzzlePlugin)
            .add_plugins(AudioCuesPlugin);
        let lvl = level("02");
        let mut state = PuzzleState::from_level(&lvl).expect("02 builds");
        state.power_budget = 1;
        let switches: Vec<(i32, i32, i32)> = state
            .pipes
            .iter()
            .filter_map(re_puzzle::Pipe::switch_coord)
            .collect();
        for pipe in &mut state.pipes {
            pipe.state = re_level::PipeState::Off;
        }
        app.insert_resource(state);
        let drain = |app: &mut App| -> Vec<PlayCue> {
            app.world_mut()
                .resource_mut::<Messages<PlayCue>>()
                .drain()
                .collect()
        };
        let (x, y, z) = switches[0];
        app.world_mut()
            .write_message(re_puzzle::InteractAt { x, y, z });
        app.update();
        let first = drain(&mut app);
        assert_eq!(first.len(), 1);
        assert!(matches!(first[0].cue, Cue::SwitchOn { used: 1, budget: 1 }));
        assert_eq!(first[0].pos, Some(cell_centre(x, y, z)));

        let (x, y, z) = switches[1];
        app.world_mut()
            .write_message(re_puzzle::InteractAt { x, y, z });
        app.update();
        let second = drain(&mut app);
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].cue, Cue::RefusedNoPower);

        app.world_mut().write_message(re_puzzle::InteractAt {
            x: -99,
            y: -99,
            z: -99,
        });
        app.update();
        let third = drain(&mut app);
        assert_eq!(third, vec![PlayCue::new(Cue::InteractEmpty)]);
    }
}
