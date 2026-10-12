//! The game's LENIENT-BUT-LOUD policy for a missing asset file, proven end to end
//! by running the shipped binary.
//!
//! Two halves, and both matter:
//!
//! * It still LAUNCHES. Refusing to start because a decorative wall texture was
//!   moved is worse for a player than an untextured wall, so `build_map`'s
//!   best-effort skip stays the runtime default. The dump must exit 0 and still
//!   produce a frame.
//! * It no longer does it in SILENCE. Before this, deleting a texture produced an
//!   untextured level and zero output — the miss was resolved, formatted into a
//!   message naming every path probed, and then dropped, because nothing outside
//!   `re_level`'s own unit tests called the function that returned it.
//!
//! `re_runtime::asset_notice`'s unit tests pin the WORDING; this test exists to pin
//! the WIRING — that `main.rs`'s load path actually calls it. Deleting the call
//! leaves those unit tests green and this one red, which is the whole point.

use std::path::PathBuf;
use std::process::Command;

/// A minimal, dumpable level that binds an image material to a texture file
/// which is not on disk.
const LEVEL_WITH_ABSENT_TEXTURE: &str = "\
[meta]
name = assetnotice
[room]
size = 8 6 4
ambient = solid
[texture pic]
file = assets/texture/definitely_absent_texture.png
[material img_wall]
recipe = image
texture = pic
surface = wall_solid
[node a]
pos = 2 2 2
is_input = yes
[node b]
pos = 5 2 2
is_output = yes
[pipe a->b]
path = (2,2,2) (5,2,2)
[pedestal]
center = 5 1 1
volume = 1 1 1
[door]
pos = 6 2 1
[player]
spawn = 3 2 1
";

/// A unique scratch directory for this test's fixture (removed at the end).
fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ts-missing-asset-notice-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

#[test]
fn a_dump_of_a_level_with_an_absent_texture_still_renders_and_says_so() {
    let dir = scratch_dir();
    let lvl = dir.join("absent-texture.lvl");
    std::fs::write(&lvl, LEVEL_WITH_ABSENT_TEXTURE).expect("write fixture level");

    let out = Command::new(env!("CARGO_BIN_EXE_terminal-state"))
        .arg("--dump")
        .arg(&lvl)
        .output()
        .expect("run terminal-state --dump");

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_file(&lvl);
    let _ = std::fs::remove_dir(&dir);

    // 1. IT STILL RUNS — the fallback is kept, not traded for a hard failure.
    assert!(
        out.status.success(),
        "a missing texture must not stop the game: status={:?}\nstderr:\n{stderr}",
        out.status
    );
    assert!(
        stdout.contains("GLYPH GRID"),
        "the level still rendered a frame; stdout was:\n{stdout}"
    );

    // 2. IT SAYS SO — on stderr, naming the section, the authored path, and every
    //    candidate path that was probed.
    assert!(
        stderr.contains("is missing 1 asset file(s)"),
        "the notice is printed: stderr was:\n{stderr}"
    );
    assert!(stderr.contains("[texture pic]"), "{stderr}");
    assert!(
        stderr.contains("definitely_absent_texture.png"),
        "the authored path is named: {stderr}"
    );
    assert!(
        stderr.contains("looked in"),
        "every candidate path is named: {stderr}"
    );
    assert!(
        stderr.contains("untextured/procedural"),
        "the notice states the fallback, so it does not read as a crash: {stderr}"
    );
}

/// The counterweight: a SHIPPED level whose assets are all present prints no
/// notice at all. A diagnostic that fires on healthy content is noise, and noise
/// is how the next real one gets ignored.
///
/// Uses a CAMPAIGN level: `L8-cold-room.lvl` binds `[texture shelf_plate]` to
/// `assets/texture/genplate_tex.png`, so it exercises the same "asset present and
/// resolves" branch the engine's own TX5-genplate demo level used to, without this
/// game test depending on an engine-only level.
#[test]
fn a_dump_of_a_healthy_level_prints_no_asset_notice() {
    let lvl = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../levels/L8-cold-room.lvl");
    let out = Command::new(env!("CARGO_BIN_EXE_terminal-state"))
        .arg("--dump")
        .arg(&lvl)
        .output()
        .expect("run terminal-state --dump");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(out.status.success(), "the shipped level dumps: {stderr}");
    assert!(
        !stderr.contains("is missing"),
        "a level with all its assets is silent; stderr was:\n{stderr}"
    );
}
