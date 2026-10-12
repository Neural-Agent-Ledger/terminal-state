//! Offline field-chain verification: for each placed campaign level, a
//! scripted walk through its authored fields, rendered through the REAL kira
//! graph (`re_audio::offline_backend`, no device ever opened) to a WAV file +
//! a metrics README. This is a `#[test]`-only harness, not a shipped feature —
//! it never compiles into `terminal-state`, only into this test binary, and it
//! does nothing unless explicitly asked (see `TS_FIELDS_OFFLINE_OUT` below) —
//! a normal `cargo test` run is a no-op here, the same gated shape as
//! `TS_BLESS_GOLDEN`.
//!
//! Lives in the game repo rather than the engine's `re_runtime` because this
//! harness needs real campaign `levels/*.lvl` files, and the engine repo does
//! not ship campaign content. Everything it touches is public API of
//! `re_runtime`/`re_level`/`re_puzzle`/`re_audio` — no game-repo-only shortcut
//! was needed except [`re_runtime::sound_fields::FieldRuntime::weight_of`],
//! which lets the harness read the converged per-field envelope weight for
//! its README without reaching into a private field.
//!
//! The walk itself: for every field authored on the level, sample the mix
//! with the listener AT that field's own position, at three real
//! puzzle-state snapshots along the level's own solve — the authored start,
//! the midpoint of its minimal toggle sequence
//! (`re_puzzle::solve_brute_force`'s own `winning_mask`, the same static
//! analysis `re_lvlcheck` uses), and the fully solved end — applied through
//! the real interaction seam (`re_puzzle::puzzle_interact`), exactly as
//! `campaign_solve_through.rs` solves a level for real. Each snapshot renders
//! `SECS_PER_WAYPOINT` seconds of decorrelated white noise through the
//! field's own resulting AMB-bus `BusFx`, via the actual
//! `playback::build_field_chain` graph (`render_field_chain_scenario_stereo`)
//! — so every render exercises the same kira effect chain the shipped game
//! would, not a hand-rolled approximation.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use re_audio::offline_backend::{
    has_nan_or_inf, hf_energy_rms, peak_dbfs, render_field_chain_scenario_stereo,
    stereo_correlation,
};
use re_audio::{offline, FieldMix, FieldSettings};
use re_level::fields::{FieldShape, ResolvedField};
use re_puzzle::{puzzle_interact, solve_brute_force, PuzzleState, Solvability};
use re_runtime::sound_fields::{evaluate_mix, FieldRuntime};

/// Four authored sound-field placements, each with its real campaign movement phase (the game's
/// `flow::Movement::P1..P4`, carried across the seam as a plain 0..=3 index —
/// see `re_runtime::sound_fields::FieldContext::phase`) — several
/// archetypes' `phase` table zeroes out a whole movement (e.g.
/// `recognition_narrow` is silent in P1/P2), so using the level's actual
/// phase instead of a fixed one matters for real numbers.
const PLACED_LEVELS: &[(&str, u8)] = &[
    ("L1-first-pulse.lvl", 0),    // P1
    ("L2-two-make-three.lvl", 0), // P1
    ("L7-hidden-depths.lvl", 2),  // P3
    ("L12-closing.lvl", 3),       // P4
];

const SECS_PER_WAYPOINT: f32 = 2.0;
/// Peak comfort ceiling the offline render must clear.
const PEAK_CEILING_DBFS: f32 = -0.3;
const HF_CUTOFF_HZ: f32 = 4_000.0;

/// Return the workspace root: `CARGO_MANIFEST_DIR` is `crates/terminal_state`, so two
/// levels up is the workspace root — same convention every other integration
/// test in this crate uses (`campaign_solve_through.rs`, `golden_campaign.rs`, …).
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// A representative world point for a field, for any of the three shapes the
/// four placed levels actually use (none of them authors a `room`-shaped
/// field — a `room`-shaped field attaches inline and has no single waypoint).
fn field_waypoint(field: &ResolvedField) -> [f32; 3] {
    match &field.shape {
        FieldShape::Sphere { at } => *at,
        FieldShape::BoxShape { min, max } => [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ],
        FieldShape::Capsule { points } => points
            .get(points.len() / 2)
            .or_else(|| points.first())
            .copied()
            .unwrap_or([0.0, 0.0, 0.0]),
        FieldShape::Room => [0.0, 0.0, 0.0], // unused by the 4 placed levels today
    }
}

/// Applies `count` toggles from `solve_brute_force`'s minimal winning set (the
/// same bit-order `campaign_solve_through.rs` uses), in switchable-pipe
/// order, through the real `puzzle_interact` seam. `count` may be less than
/// the full toggle set (a genuine mid-solve snapshot).
fn apply_n_toggles(state: &mut PuzzleState, switchable: &[usize], winning_mask: u32, count: usize) {
    let mut applied = 0usize;
    for (bit, &pipe_idx) in switchable.iter().enumerate() {
        if applied >= count {
            break;
        }
        let want_on = (winning_mask >> bit) & 1 == 1;
        let is_on = state.pipes[pipe_idx].is_on();
        if want_on != is_on {
            let (x, y, z) = state.pipes[pipe_idx]
                .switch_coord()
                .unwrap_or_else(|| panic!("switchable pipe {pipe_idx} has no switch coordinate"));
            assert!(
                puzzle_interact(state, x, y, z),
                "toggle at ({x},{y},{z}) for pipe {pipe_idx} must take"
            );
            applied += 1;
        }
    }
}

/// One level's verification: walk every field at 3 puzzle snapshots, render
/// each through the real graph, concatenate, write one WAV, and return the
/// metrics lines for the README.
fn verify_level(level_name: &str, phase: u8, out_dir: &Path) -> String {
    let root = workspace_root();
    let path = root.join("levels").join(level_name);
    let lv = re_level::load(&path).unwrap_or_else(|e| panic!("load {level_name}: {e}"));
    let map = lv.build_map();
    let fields = lv.fields();
    assert!(
        !fields.is_empty(),
        "{level_name}: verification requires at least one placed field"
    );

    let mut state = PuzzleState::from_level(&lv)
        .unwrap_or_else(|e| panic!("{level_name}: puzzle graph must build (DAG): {e:?}"));
    let switchable: Vec<usize> = state
        .pipes
        .iter()
        .enumerate()
        .filter(|(_, p)| p.switch_cell.is_some())
        .map(|(i, _)| i)
        .collect();
    let (min_toggles, winning_mask) = match solve_brute_force(&mut state) {
        Solvability::Solvable {
            min_toggles,
            winning_mask,
            ..
        } => (min_toggles, winning_mask),
        other => {
            panic!("{level_name}: must be solvable by toggling switches; solver said {other:?}")
        }
    };
    let snapshots: [(&str, usize); 3] = [
        ("start", 0),
        ("midpoint", (min_toggles as usize) / 2),
        ("solved", min_toggles as usize),
    ];

    let field_settings = FieldSettings {
        intensity: 1.0,
        reduced: false,
    };
    let mut runtime = FieldRuntime::default();
    let mut samples: Vec<f32> = Vec::new();
    let mut weight_log = String::new();
    let mut seed_counter: u32 = 0x1000;

    for (label, toggles) in snapshots {
        // Reset to the authored start, then re-apply exactly `toggles` real
        // toggles through the real seam — a genuine intermediate puzzle
        // state, not an interpolated fake one.
        state = PuzzleState::from_level(&lv).expect("re-build from level");
        apply_n_toggles(&mut state, &switchable, winning_mask, toggles);

        for field in fields {
            let listener = field_waypoint(field);
            let mut mix = FieldMix::NEUTRAL;
            // Step ~1s at 60Hz so the attack/release envelope converges to
            // this snapshot's steady state before rendering/logging.
            for _ in 0..60 {
                mix = evaluate_mix(
                    fields,
                    listener,
                    Some(&map),
                    Some(&state),
                    0,
                    0,
                    1.0,
                    220.0,
                    phase,
                    None,
                    0.0,
                    1.0 / 60.0,
                    &mut runtime,
                    &field_settings,
                );
            }
            let w = runtime.weight_of(&field.name);
            let _ = writeln!(
                weight_log,
                "  - `{label}` (toggles={toggles}/{min_toggles}), listener at field `{}`: weight={w:.3}",
                field.name
            );

            seed_counter = seed_counter.wrapping_add(1);
            let target = mix.buses[re_audio::field::AMB];
            let rendered = render_field_chain_scenario_stereo(
                re_audio::SAMPLE_RATE,
                SECS_PER_WAYPOINT,
                seed_counter,
                seed_counter.wrapping_add(0x5EED),
                target,
            );
            samples.extend_from_slice(&rendered);
        }
    }

    assert!(
        !has_nan_or_inf(&samples),
        "{level_name}: offline render produced NaN/Inf"
    );
    let peak = peak_dbfs(&samples);
    assert!(
        peak <= PEAK_CEILING_DBFS,
        "{level_name}: peak {peak:.2} dBFS exceeds the {PEAK_CEILING_DBFS} dBFS ceiling"
    );
    let hf = hf_energy_rms(&samples, re_audio::SAMPLE_RATE as f32, HF_CUTOFF_HZ);
    let corr = stereo_correlation(&samples);

    let wav_path = out_dir.join(format!("{}.wav", level_name.trim_end_matches(".lvl")));
    let stereo = re_audio::offline_backend::interleaved_to_stereo(&samples);
    offline::write_wav(&wav_path, &stereo)
        .unwrap_or_else(|e| panic!("write {}: {e}", wav_path.display()));

    format!(
        "## {level_name}\n\n\
         - fields: {}\n\
         - minimal toggle count: {min_toggles}\n\
         - WAV: `{}` ({:.1}s, {} waypoints)\n\
         - peak: {peak:.2} dBFS (ceiling {PEAK_CEILING_DBFS} dBFS) — {}\n\
         - HF energy (>{HF_CUTOFF_HZ:.0}Hz) RMS: {hf:.5}\n\
         - stereo correlation: {corr:.3} (1.0 = mono-summed, 0 = fully decorrelated — lower reads WIDER)\n\
         - NaN/Inf: none\n\
         - field weight per snapshot:\n{weight_log}\n",
        fields.len(),
        wav_path.file_name().unwrap().to_string_lossy(),
        samples.len() as f32 / 2.0 / re_audio::SAMPLE_RATE as f32,
        fields.len() * 3,
        if peak <= PEAK_CEILING_DBFS { "PASS" } else { "FAIL" },
    )
}

/// No-op unless `TS_FIELDS_OFFLINE_OUT` is set — MACHINE-SILENT compliant by
/// construction (`OfflineBackend` never opens a device regardless, and this
/// test writes nothing outside the directory the caller names).
#[test]
fn offline_field_verification() {
    let Ok(out_dir) = std::env::var("TS_FIELDS_OFFLINE_OUT") else {
        eprintln!(
            "[fields-offline] TS_FIELDS_OFFLINE_OUT not set; skipping \
             (set it to a writable directory to render the WAVs + README)"
        );
        return;
    };
    let out_dir = PathBuf::from(out_dir);
    std::fs::create_dir_all(&out_dir)
        .unwrap_or_else(|e| panic!("create {}: {e}", out_dir.display()));

    let mut readme = String::new();
    readme.push_str(
        "# Sound-fields offline verification\n\n\
         Generated by `terminal_state`'s `tests/sound_fields_offline.rs`\n\
         (`cargo test -p terminal_state --test sound_fields_offline -- --nocapture`\n\
         with `TS_FIELDS_OFFLINE_OUT` set). Every WAV is rendered through the REAL kira field-chain\n\
         graph (`re_audio::offline_backend`) — never a device, never a hand-rolled approximation —\n\
         at 3 genuine puzzle-state snapshots (start / midpoint / solved) per authored field, with the\n\
         listener at that field's own position each time.\n\n",
    );
    for (name, phase) in PLACED_LEVELS {
        readme.push_str(&verify_level(name, *phase, &out_dir));
    }
    let readme_path = out_dir.join("README.md");
    std::fs::write(&readme_path, &readme)
        .unwrap_or_else(|e| panic!("write {}: {e}", readme_path.display()));
    eprintln!("[fields-offline] wrote {}", readme_path.display());
}
