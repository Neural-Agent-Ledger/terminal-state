//! Restore of a test dropped when its fixture moved into the engine's
//! `re_runtime` crate: `a_lit_prop_lamp_hums_and_a_dark_one_is_silent` asserts
//! through `audio_cues::prop_lamp_sources`, which is a game-only module that
//! `re_runtime` must not depend on. It belongs here instead, driving the real
//! engine pieces (`re_level::load_string`, `re_runtime::prop_power`,
//! `re_puzzle::puzzle_interact`) plus the game's own audio-cue adapter.
//!
//! See `git show 75d39ae:crates/re_game/tests/prop_power_live.rs` for the
//! original this is restored from.

use re_level::Level;
use re_puzzle::{puzzle_interact, PuzzleState};
use re_runtime::prop_power::{bind_prop_power, sync_prop_power, PropPower};
use re_world::Map;

/// A small room: one lamp prop powered by pipe `G->output`, dark until that
/// pipe's switch is thrown.
const LEVEL: &str = "[meta]
name = prop-lamp-audio
title = prop lamp audio

[room]
size = 12 12 6
ambient = solid
storey_height = 4

[node G]
pos = 2,10,2
glyph = G
color = #40FF60
is_input = yes

[node output]
pos = 9,10,2
glyph = O
color = #FFFFFF
is_output = yes
threshold = 1

[pipe G->output]
state = OFF
path = (2,10,2) (2,8,2) (9,8,2) (9,10,2)
switch_at = (4,8,2)

[inputs]
G = 1

[pedestal]
center = 6 6 1
volume = 1 1 1

[door]
pos = 9 1 1
target_parts = output

[player]
spawn = 6 5 1
facing = -y

[prop lamp]
kind = lamp_pendant
at = 4.5 4.5 3.0
light = yes
power = pipe:G->output
";

const SWITCH_G: (i32, i32, i32) = (4, 8, 2);

fn load() -> (Level, Map, PuzzleState, PropPower) {
    let level = re_level::load_string(LEVEL, "prop-lamp-audio.lvl")
        .unwrap_or_else(|e| panic!("fixture loads: {e}"));
    let map = level.build_map();
    let puzzle = PuzzleState::from_level(&level).unwrap_or_else(|e| panic!("puzzle builds: {e:?}"));
    let pp = bind_prop_power(&level, &map);
    (level, map, puzzle, pp)
}

/// The audio hook: a prop lamp hums (the existing `LampHum` loop) only while lit.
#[test]
fn a_lit_prop_lamp_hums_and_a_dark_one_is_silent() {
    let (level, mut map, mut puzzle, mut pp) = load();
    let authored = level.lights.len();
    assert!(
        terminal_state::audio_cues::prop_lamp_sources(authored, &map.atmosphere().lights)
            .is_empty()
    );
    assert!(puzzle_interact(
        &mut puzzle,
        SWITCH_G.0,
        SWITCH_G.1,
        SWITCH_G.2
    ));
    let _ = sync_prop_power(&mut pp, &puzzle, &mut map, None);
    let hum = terminal_state::audio_cues::prop_lamp_sources(authored, &map.atmosphere().lights);
    assert_eq!(hum.len(), 1);
    assert_eq!(hum[0].key.kind, re_audio::LoopKind::LampHum);
}
