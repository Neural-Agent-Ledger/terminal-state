//! Integration test for `levels/L1-first-pulse.lvl` — the furnished two-storey
//! edition of the canon L1 puzzle.
//!
//! This level splits the canon L1 graph (`R(IN=1) → output(T=1)`, one pipe, one
//! switch, door target = `output`) across two floors connected by a walkable 45°
//! straight staircase. The INPUT node and the pipe SWITCH live on the ground
//! floor; the OUTPUT node and the EXIT DOOR live on a furnished second floor.
//!
//! The win mirrors the real game flow (`terminal_state::main`):
//!   solved == true  AND  re_world::at_door(cam.pos, door)
//! so a player MUST (1) flip the switch on the ground floor, (2) climb the
//! stairs to floor 2, and (3) walk into the door. None of those three is optional.
//!
//! What this test proves end-to-end:
//!
//! * **Loads + storey metadata** — the level parses and carries `storey_height = 4`.
//! * **Renders non-blank** — the spawn camera produces a non-trivial Framebuffer.
//! * **Puzzle is the canon L1 graph + solvable** — building `PuzzleState` from the
//!   level reports a single-toggle solution; toggling the switch at its real world
//!   cell drives `solved == true`.
//! * **The climb is real** — a scripted physics walk (`move_with_collision` +
//!   `step_gravity`) up the staircase raises the camera ~one storey and stays
//!   grounded.
//! * **The win requires the climb** — after the climb the camera reaches the
//!   floor-2 door cell (`at_door == true`) while the puzzle is solved, and the
//!   door cell is NOT reachable at ground-floor elevation (you cannot win without
//!   climbing).
//! * **Floor-0 vs floor-2 frames differ** — elevation changes the rendered frame.
//!
//! # How to run
//! ```sh
//! cargo test -p terminal_state --test l1_first_pulse_integration
//! ```

use re_core::{Framebuffer, Vec3};
use re_level::load;
use re_puzzle::{puzzle_interact, solve_brute_force, PuzzleState, Solvability};
use re_render::{composite::composite_dump_once, registry_for_map, Emitter, Fov};
use re_world::{
    at_door, CameraPose, PlayerMotion, PLAYER_EYE_HEIGHT, PLAYER_RADIUS, STANDING_EYE_Z,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Framebuffer size — matches DUMP_COLS / DUMP_ROWS in src/main.rs.
const COLS: usize = 100;
const ROWS: usize = 32;

const L1_LVL: &str = "levels/L1-first-pulse.lvl";

/// The pipe switch cell on the GROUND floor (authored `[pipe] switch_at`).
const SWITCH: (i32, i32, i32) = (5, 5, 2);
/// The exit door cell on the SECOND floor (authored `[door] pos`).
const DOOR: (i32, i32, i32) = (14, 4, 5);

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Hex-encode a digest byte array. sha2 0.11 (`digest` 0.11) returns its own
/// `hybrid_array::Array<u8, N>` from `finalize()` rather than the old
/// `generic_array::GenericArray`, and that type does not implement
/// `LowerHex` — so `format!("{:x}", ...)` no longer compiles directly.
fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{b:02x}").expect("write to String never fails");
    }
    s
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn render_hash(level_path: &PathBuf, cam: &CameraPose) -> String {
    let lv = load(level_path).unwrap_or_else(|e| panic!("load {}: {e}", level_path.display()));
    let map = lv.build_map();
    let registry = registry_for_map(&map);
    let emitters: Vec<Emitter> = lv.emitters.iter().map(Emitter::from_spec).collect();

    let mut fb = Framebuffer::new(COLS, ROWS);
    composite_dump_once(&map, cam, &registry, &Fov::default(), &emitters, &mut fb);

    let mut hasher = Sha256::new();
    for row in 0..ROWS {
        for col in 0..COLS {
            let cell = fb.get(col, row).expect("in-bounds cell");
            hasher.update(cell.glyph.to_le_bytes());
            hasher.update(cell.fg.to_le_bytes());
            hasher.update(cell.bg.to_le_bytes());
        }
    }
    to_hex(&hasher.finalize())
}

fn non_empty_glyph_count(level_path: &PathBuf, cam: &CameraPose) -> usize {
    let lv = load(level_path).unwrap_or_else(|e| panic!("load {}: {e}", level_path.display()));
    let map = lv.build_map();
    let registry = registry_for_map(&map);
    let emitters: Vec<Emitter> = lv.emitters.iter().map(Emitter::from_spec).collect();

    let mut fb = Framebuffer::new(COLS, ROWS);
    composite_dump_once(&map, cam, &registry, &Fov::default(), &emitters, &mut fb);

    let mut count = 0usize;
    for row in 0..ROWS {
        for col in 0..COLS {
            let g = fb.get(col, row).expect("in-bounds").glyph;
            let ch = char::from_u32(g).unwrap_or(' ');
            if ch != ' ' && ch != '.' {
                count += 1;
            }
        }
    }
    count
}

// ── Smoke-load + render ────────────────────────────────────────────────────────

#[test]
fn l1_loads_and_renders_nonblank() {
    let path = workspace_root().join(L1_LVL);
    assert!(path.exists(), "level file missing: {}", path.display());

    let lv = load(&path).expect("L1-first-pulse.lvl must parse without error");

    assert_eq!(
        lv.storey_height(),
        4,
        "storey_height must be 4 (as authored in the [room] header)"
    );

    // Spawn is on the GROUND floor (eye z ≈ STANDING_EYE_Z).
    let cam = lv.spawn_camera();
    assert!(
        (cam.pos.z - STANDING_EYE_Z).abs() < 0.6,
        "spawn z ≈ STANDING_EYE_Z (ground floor): got {:.3}",
        cam.pos.z
    );

    let nonblank = non_empty_glyph_count(&path, &cam);
    assert!(
        nonblank > 200,
        "ground-floor frame must be non-blank: only {nonblank} non-empty glyphs"
    );
}

// ── The canon L1 graph + solvability ────────────────────────────────────────────

/// The level builds the canon L1 graph and is solvable in exactly ONE toggle, and
/// flipping the switch at its real world cell drives `solved == true`.
#[test]
fn l1_puzzle_is_canon_graph_and_solves_in_one_toggle() {
    let path = workspace_root().join(L1_LVL);
    let lv = load(&path).expect("load");

    let mut state = PuzzleState::from_level(&lv).expect("L1 graph must build (DAG)");

    // Canon L1: two nodes (R input, output), one switchable pipe, door target =
    // the output node.
    assert_eq!(state.nodes.len(), 2, "canon L1 has exactly 2 nodes");
    assert_eq!(state.pipes.len(), 1, "canon L1 has exactly 1 pipe");
    assert_eq!(
        state.door_targets.len(),
        1,
        "exactly one door target (output)"
    );

    // Solver: a winning configuration exists, reachable in a single toggle.
    match solve_brute_force(&mut state) {
        Solvability::Solvable { min_toggles, .. } => {
            assert_eq!(min_toggles, 1, "L1 must be a one-switch puzzle");
        }
        other => panic!("L1 must be solvable; solver said {other:?}"),
    }

    // Fresh state must start UNSOLVED (the pipe is authored OFF).
    let mut state = PuzzleState::from_level(&lv).expect("rebuild");
    assert!(
        !state.solved,
        "L1 must require player action — not pre-solved"
    );

    // Flip the switch at its real world cell → the pulse reaches the output.
    let toggled = puzzle_interact(&mut state, SWITCH.0, SWITCH.1, SWITCH.2);
    assert!(
        toggled,
        "switch at {SWITCH:?} must toggle the R->output pipe"
    );
    assert!(
        state.solved,
        "after the single toggle, the output must fire (WON)"
    );
}

// ── The full loop: toggle → climb → reach the door → WON ────────────────────────

/// Drive the *whole* solve in first person: flip the switch on the ground floor,
/// climb the staircase to floor 2, and walk into the door. Assert the puzzle is
/// solved AND the camera is `at_door` — exactly the win condition the game uses.
#[test]
fn l1_scripted_solve_climb_and_reach_door_wins() {
    let path = workspace_root().join(L1_LVL);
    let lv = load(&path).expect("load");
    let map = lv.build_map();
    let storey = lv.storey_height() as f32;

    // ── Puzzle: flip the ground-floor switch ────────────────────────────────
    let mut state = PuzzleState::from_level(&lv).expect("build puzzle");
    assert!(!state.solved, "must start unsolved");
    assert!(
        puzzle_interact(&mut state, SWITCH.0, SWITCH.1, SWITCH.2),
        "switch toggle must take"
    );
    assert!(state.solved, "puzzle must be solved after the one toggle");

    // ── Physics: walk the canon path to the door ────────────────────────────
    // Start near the switch / node R on the ground floor, facing +x.
    let mut cam = CameraPose {
        pos: Vec3::new(2.5, 5.5, STANDING_EYE_Z),
        yaw: std::f32::consts::FRAC_PI_2, // facing +x
        pitch: 0.0,
    };
    let mut motion = PlayerMotion::default();

    // Settle onto the ground floor.
    for _ in 0..20 {
        cam.step_gravity(&mut motion, 1.0 / 60.0, &map);
    }
    assert!(
        motion.grounded,
        "must be grounded at spawn (z={:.3})",
        cam.pos.z
    );
    let ground_z = cam.pos.z;

    // 1) Walk north from the switch row (y≈5.5) to the stair corridor (y≈3.5).
    for _ in 0..400 {
        if cam.pos.y <= 3.5 {
            break;
        }
        cam.move_with_collision(0.0, -0.06, &map, PLAYER_RADIUS);
        for _ in 0..3 {
            cam.step_gravity(&mut motion, 1.0 / 60.0, &map);
        }
    }
    assert!(
        cam.pos.y <= 3.7,
        "must reach the stair corridor (y≈3.5); stuck at y={:.3}",
        cam.pos.y
    );

    // 2) Reposition west of the first tread (tread 1 at x=4) so we approach the
    //    staircase from its base.
    for _ in 0..400 {
        if cam.pos.x <= 3.5 {
            break;
        }
        cam.move_with_collision(-0.06, 0.0, &map, PLAYER_RADIUS);
        for _ in 0..3 {
            cam.step_gravity(&mut motion, 1.0 / 60.0, &map);
        }
    }

    // 3) Climb east up the staircase and emerge onto floor 2 (x > 9).
    let mut seam_failures: Vec<f32> = Vec::new();
    for _ in 0..500 {
        if cam.pos.x >= 9.5 {
            break;
        }
        cam.move_with_collision(0.06, 0.0, &map, PLAYER_RADIUS);
        for _ in 0..3 {
            cam.step_gravity(&mut motion, 1.0 / 60.0, &map);
        }
        let local_x = cam.pos.x - cam.pos.x.floor();
        let near_seam = !(0.15..=0.85).contains(&local_x);
        if near_seam && !motion.grounded {
            seam_failures.push(cam.pos.x);
        }
    }
    assert!(
        seam_failures.is_empty(),
        "grounded flickered near ramp seams at x={seam_failures:?}"
    );
    assert!(
        motion.grounded,
        "must be grounded on floor 2 after the climb (x={:.3}, z={:.3})",
        cam.pos.x, cam.pos.z
    );

    let rise = cam.pos.z - ground_z;
    assert!(
        rise >= storey - 1.0 && rise <= storey + 0.6,
        "climb must raise z ≈ one storey ({storey}); got {rise:.3} \
         (ground_z={ground_z:.3}, floor2_z={:.3})",
        cam.pos.z
    );

    // 4) Walk east along the floor-2 hall toward the door alcove, hugging the
    //    y=4 passage between the partition stubs.
    for _ in 0..600 {
        // Nudge toward the door's y-row.
        if cam.pos.y > 4.6 {
            cam.move_with_collision(0.0, -0.05, &map, PLAYER_RADIUS);
        } else if cam.pos.y < 4.4 {
            cam.move_with_collision(0.0, 0.05, &map, PLAYER_RADIUS);
        }
        cam.move_with_collision(0.06, 0.0, &map, PLAYER_RADIUS);
        for _ in 0..3 {
            cam.step_gravity(&mut motion, 1.0 / 60.0, &map);
        }
        if at_door(cam.pos, DOOR.0, DOOR.1, DOOR.2) {
            break;
        }
    }

    // ── Win condition: solved AND at_door (the exact game predicate). ────────
    assert!(
        at_door(cam.pos, DOOR.0, DOOR.1, DOOR.2),
        "player must reach the floor-2 door cell {DOOR:?}; ended at pos={:?}",
        cam.pos
    );
    assert!(
        state.solved && at_door(cam.pos, DOOR.0, DOOR.1, DOOR.2),
        "WIN: puzzle solved AND camera at the floor-2 door"
    );
}

/// The win REQUIRES the climb: the door cell is on floor 2, so a camera standing
/// at ground-floor elevation directly under the door is NOT `at_door`. You cannot
/// trigger the win without ascending.
#[test]
fn l1_door_is_unreachable_from_ground_floor() {
    // A camera at the door's x/y but at GROUND-floor eye height.
    let ground_under_door = Vec3::new(
        DOOR.0 as f32 + 0.5,
        DOOR.1 as f32 + 0.5,
        STANDING_EYE_Z, // ground-floor eye z ≈ 1.5
    );
    assert!(
        !at_door(ground_under_door, DOOR.0, DOOR.1, DOOR.2),
        "the door must NOT be reachable from the ground floor (z too low) — \
         the climb is mandatory"
    );

    // The SAME x/y at the floor-2 eye height IS at the door (sanity: the door is
    // reachable once you've climbed).
    let on_floor2 = Vec3::new(
        DOOR.0 as f32 + 0.5,
        DOOR.1 as f32 + 0.5,
        DOOR.2 as f32 + PLAYER_EYE_HEIGHT, // floor-2 surface (5.0) + eye height
    );
    assert!(
        at_door(on_floor2, DOOR.0, DOOR.1, DOOR.2),
        "the door must be reachable at the floor-2 elevation"
    );
}

// ── Floor-0 vs floor-2 frames differ ────────────────────────────────────────────

#[test]
fn l1_floor0_and_floor2_frames_differ() {
    let path = workspace_root().join(L1_LVL);
    let lv = load(&path).expect("load");
    let storey = lv.storey_height() as f32;

    let cam_floor0 = lv.spawn_camera();
    let cam_floor2 = CameraPose {
        pos: Vec3::new(
            cam_floor0.pos.x,
            cam_floor0.pos.y,
            cam_floor0.pos.z + storey,
        ),
        yaw: cam_floor0.yaw,
        pitch: cam_floor0.pitch,
    };

    let hash0 = render_hash(&path, &cam_floor0);
    let hash2 = render_hash(&path, &cam_floor2);

    assert_ne!(
        hash0, hash2,
        "ground-floor and second-floor frames must differ.\n\
         floor-0 digest: {hash0}\n\
         floor-2 digest: {hash2}"
    );
}
