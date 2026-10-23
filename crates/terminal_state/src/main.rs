// Test-module assert_eq! on f32 values are intentional exact-equal checks.
// Allow float_cmp only in test compilation units; binary code remains denied.
#![cfg_attr(test, allow(clippy::float_cmp))]
//! `terminal-state` — the TERMINAL STATE binary.
//!
//! Wires the headless Bevy `App`: `MinimalPlugins` + `ScheduleRunnerPlugin`
//! (60 Hz fixed loop) + the world, puzzle, audio, render, and present plugins.
//!
//! Level source:
//! * `terminal-state --dump <path.lvl>` — load that `.lvl`, build the stamped map,
//!   spawn the camera at `[player]`, run ONE raycast, print the glyph grid as
//!   ASCII, and exit — a headless visual check with no live terminal needed.
//!   `--dump` with no path falls back to the hardcoded test room.
//! * `terminal-state <path.lvl>` (interactive) — open the live loop on that level.
//!   The path is resolved against the LEVEL CATALOGUE (`LevelCatalog::index_of_path`,
//!   path-aware so `levels\01.lvl` and `levels/01.lvl` are the same level), because
//!   the play loop can only load catalogue entries. A path the catalogue does not
//!   hold exits non-zero with a message naming it and listing what exists — it is
//!   never quietly replaced by a different level. No path → the catalogue default.
//!
//! `terminal-state --help` prints the flag list (see `HELP`) and exits 0 without
//! touching the terminal; an unrecognised `-`-prefixed argument is a usage error.
//!
//! Playable loop (interactive): the present layer turns keys into camera motion
//! AND height (rise/fall), and emits a backend-neutral `InteractPressed` when the
//! interact key is hit. This binary turns each `InteractPressed` into a
//! [`InteractAt`] aimed at the cell the camera faces, which `re_puzzle` toggles
//! and re-solves, and drives `re_audio` (switch click on interact, win sting and
//! door on solve, an ambient bed on level start). The win is surfaced as a log
//! line and a [`WinState`] resource flag.
//!
//! Game flow (splash → menu → level-select → playing → won) is built on Bevy
//! states in [`flow`]; the gameplay glue below is gated to [`GameState::Playing`].

// The shared module tree (flow/story/save/settings/level_text/runtime_glue/
// fog/code_panels/menu) lives in the library target (`src/lib.rs`) so
// integration tests under `tests/` can `use` it too. This binary is a thin
// wrapper: pull the modules it needs in by name so the rest of this file's
// `flow::…`/`save::…`/etc. paths are unchanged, and keep only the CLI/App-
// wiring code. `re_runtime::{rng, dev}` are called directly — there is no
// binary-only wrapper for either.
use terminal_state::{boot_console, flow, level_text, menu, runtime_glue, save, settings};
#[cfg(test)]
use terminal_state::{VIEW_COLS, VIEW_ROWS};

use bevy::prelude::*;
use flow::{FlowPlugin, GameState};
use menu::MenuRenderPlugin;
use re_audio::{AudioPlugin, PlaySound, SoundId};
use re_core::{CellStateMap, Framebuffer};
use re_glitch::{apply_glitch, GlitchConfig, GlitchPlugin};
use re_postfx::{apply_dither, apply_postfx_ex, PostfxConfig, PostfxDitherPlugin, PostfxPlugin};
use re_present::{
    render_config::parse_size_arg, ColorMode, InteractPressed, PresentPlugin, RenderConfig,
};
use re_puzzle::{InteractAt, PuzzlePlugin, PuzzleState, Solved};
use save::Progress;
// NOTE on the interact sound: legacy plays SWITCH_CLICK on a LANDED toggle and
// INTERACT on a no-op. This used to be PREDICTED here from the read-only
// `PuzzleState::pipe_at_switch` — which meant a REJECTED interaction sounded
// exactly like an accepted one. With the power budget in play that is a defect
// the player meets in the first minute, so the sound is now chosen from the
// ACTUAL result: `re_puzzle::apply_interactions` publishes an `InteractResolved`
// message and `terminal_state::audio_cues` maps the outcome to a cue.
// The LIVE app now produces its Framebuffer through the tiered-composite path
// (`CompositeRenderPlugin`: far backdrop + near opaque/transparent + emitters →
// resolve), the drop-in replacement for the old single-pass `RenderPlugin`. The
// `--dump` path mirrors it through `re_render::composite::composite_dump_once`.
// `Fov` + `registry_for_map` are reused for the dump's single-pass compose.
use re_render::{CompositeRenderPlugin, Fov};
use re_world::{test_room, CameraPose, Map};

// Interactive framebuffer size (`VIEW_COLS`/`VIEW_ROWS`) is imported from
// `re_game` above — the library's `flow` module shares the same constants.
// The dump path has no terminal to measure, so it uses its own DEFAULT size
// below. `--size <cols>x<rows>` (or `TS_COLS`+`TS_ROWS`) overrides it — the flag
// is documented as fixing "the render grid", and the dump IS a render grid. The
// default is unchanged, so every golden dump keeps its 100x32 geometry.
const DUMP_COLS: usize = 100;
const DUMP_ROWS: usize = 32;

/// Fixed seed + time for the `--dump --glitch` path so the corrupted artifact is
/// reproducible byte-for-byte across runs (the glitch core is deterministic in
/// exactly `(fb, cfg, time, seed)`).
const DUMP_GLITCH_SEED: u64 = 0x5245_5601_4744_4C54;
const DUMP_GLITCH_TIME: f32 = 1.0;

// ───────────────────────────── CLI surface ──────────────────────────────────
//
// The flag list below is the SINGLE source of truth for "what does this binary
// accept". `HELP` documents it, `KNOWN_FLAGS` recognises it, and `consumes_next`
// says which flags eat the following argument as a value. A unit test asserts the
// three stay in step (every `KNOWN_FLAGS` entry appears in `HELP`), so adding a
// flag without documenting it fails the build.

/// `--help` / `-h` text. Printed to stdout, exit 0, with NO terminal control
/// sequences of any kind — running `terminal-state --help` in a pipe or a CI shell
/// must not enter the alternate screen, hide the cursor, or block.
const HELP: &str = "\
TERMINAL STATE — a terminal-native, first-person 3D ASCII puzzle.

USAGE:
    terminal-state [OPTIONS] [LEVEL]

ARGS:
    <LEVEL>              A level from the catalogue (the `levels/*.lvl` files) to
                         open when you choose START, e.g. levels/01.lvl. Either
                         path separator works. Omitted: the campaign's first level.
                         A level the catalogue does not contain is an error, never
                         a substitution.

OPTIONS:
    -h, --help           Print this help and exit.

  One-shot ASCII dump — prints one frame to stdout and exits, no terminal takeover:
    --dump [LEVEL]       Raycast LEVEL once and print the glyph grid.
                         With no LEVEL, dumps the hardcoded test room.
    --ansi               Emit the dump as 24-bit truecolor ANSI cells.
    --dump-color         Synonym for --ansi.
    --glitch[=N]         Glitch post-process the dumped frame at intensity N
                         (0..1, default 0.7).
    --postfx[=PRESET]    Cinematic post-process the dumped frame. PRESET is one of
                         off|clean|subtle|cinematic|toon|crt|dream (default
                         cinematic) — the full `PostfxConfig::from_preset_name` set.
    --fov=DEG            Cast the dump at this horizontal FOV, 30..120 degrees.
    --portal-demo[=N]    Inject the demo warp-portal pair; with N, walk N steps
                         through it first so the dump shows the teleported view.

  Display:
    --color=MODE         truecolor | 256. Also written back to the saved settings.
    --size=COLSxROWS     Fix the render grid (default: auto-size to the terminal;
                         the TS_COLS / TS_ROWS env vars do the same). Applies to
                         `--dump` too, where the default is 100x32.

  Graphics quality — applied to this launch and saved to the settings:
    --quality=TIER       low | medium | high | ultra (the whole preset bundle).
    --draw-distance=N    Draw distance in cells, e.g. 16 / 24 / 32 / 48.
    --fps=CAP            30 | 60 | 120 | off.

  Dev:
    --dump-schedule      Print the Bevy Update schedule as GraphViz DOT and exit
                         (needs the `debugdump` feature).

Every `--flag=VALUE` may also be written `--flag VALUE`.";

/// Every flag this binary understands, by NAME (the part before any `=`).
/// An argument that starts with `-` and is not in this list is a usage error —
/// see [`scan_args`]. Kept in `HELP` order.
const KNOWN_FLAGS: [&str; 15] = [
    "-h",
    "--help",
    "--dump",
    "--ansi",
    "--dump-color",
    "--glitch",
    "--postfx",
    "--fov",
    "--portal-demo",
    "--color",
    "--size",
    "--quality",
    "--draw-distance",
    "--fps",
    "--dump-schedule",
];

/// The [`KNOWN_FLAGS`] that never take a value, so `--flag=…` is a usage error.
const FLAGS_WITHOUT_VALUE: [&str; 6] = [
    "-h",
    "--help",
    "--dump",
    "--ansi",
    "--dump-color",
    "--dump-schedule",
];

/// The flag NAME of an argument: everything before the first `=`, so
/// `--color=256` and `--color` are the same flag.
fn flag_name(arg: &str) -> &str {
    arg.split_once('=').map_or(arg, |(name, _)| name)
}

/// Is `arg` a flag this binary understands?
fn is_known_flag(arg: &str) -> bool {
    KNOWN_FLAGS.contains(&flag_name(arg))
}

/// Does the SPACED form of `flag` consume `next` as its value?
///
/// This mirrors each parser's own rule exactly — `parse_color_flag` takes the next
/// argument unconditionally, `string_flag_value` takes it unless it looks like a
/// flag, `parse_fov_flag`/`parse_glitch_flag` take it only when it is numeric, and
/// `parse_postfx_flag` only when it names a real preset. Keeping the rules in one
/// place is what stops [`scan_args`] from mistaking a flag's VALUE for the level
/// path: before this existed, `terminal-state --quality medium` read "medium" as
/// the level to play.
fn consumes_next(flag: &str, next: Option<&str>) -> bool {
    let Some(next) = next else {
        return false;
    };
    match flag {
        // `parse_color_flag` / `parse_size_arg`: the next argument is the value.
        "--color" | "--size" => true,
        // `string_flag_value`: the next argument, unless it looks like a flag.
        "--quality" | "--draw-distance" | "--fps" => !next.starts_with('-'),
        // Numeric-only: a bare `--fov` / `--glitch` leaves the next arg alone.
        "--fov" | "--glitch" => next.parse::<f32>().is_ok(),
        // Preset-only: a bare `--postfx` leaves the next arg alone.
        "--postfx" => PostfxConfig::from_preset_name(next).is_some(),
        // `--dump`'s optional level is an ordinary positional, not a consumed
        // value — that is what lets `--dump --ansi levels/01.lvl` work.
        _ => false,
    }
}

/// One pass over `argv[1..]`, splitting it into the arguments that are NOT flags
/// or flag values, and the flags nothing recognises.
struct ScannedArgs<'a> {
    /// `(argv index, value)` for each positional. The index is retained so the
    /// `--dump` path can take the first positional that FOLLOWS `--dump`,
    /// preserving the historical flag-order-insensitive behaviour.
    positionals: Vec<(usize, &'a str)>,
    /// Arguments that start with `-` and name no known flag, verbatim.
    unknown: Vec<&'a str>,
}

fn scan_args(args: &[String]) -> ScannedArgs<'_> {
    let mut positionals = Vec::new();
    let mut unknown = Vec::new();
    let mut skip_value = false;
    for (i, a) in args.iter().enumerate().skip(1) {
        if skip_value {
            skip_value = false;
            continue;
        }
        if a.starts_with('-') {
            // `--dump=…` on a flag that takes no value is unknown too: every
            // check for these flags is an exact match, so it would otherwise be
            // accepted here and then do nothing.
            let stray_value = a.contains('=') && FLAGS_WITHOUT_VALUE.contains(&flag_name(a));
            if !is_known_flag(a) || stray_value {
                unknown.push(a.as_str());
            }
            // Only the SPACED form consumes the following argument; `--flag=value`
            // already carries its own.
            if !a.contains('=') {
                skip_value = consumes_next(a, args.get(i + 1).map(String::as_str));
            }
            continue;
        }
        positionals.push((i, a.as_str()));
    }
    ScannedArgs {
        positionals,
        unknown,
    }
}

/// The `--size` usage error to print, if `--size` is present but unparseable.
///
/// `parse_size_arg` folds "absent" and "malformed" into one `None`, so on its own
/// a typo like `--size 120X40` silently auto-sizes — the same "the flag did
/// nothing" failure the unknown-flag check exists to prevent.
fn size_flag_error(args: &[String]) -> Option<String> {
    let present = args
        .iter()
        .skip(1)
        .any(|a| a == "--size" || a.starts_with("--size="));
    (present && parse_size_arg(args).is_none()).then(|| {
        "terminal-state: invalid --size value (use <cols>x<rows>, e.g. --size 100x32)".to_string()
    })
}

/// Did the player ask for help? Checked FIRST, before any other parsing, so
/// `--help` can never reach the interactive path.
fn is_help_request(args: &[String]) -> bool {
    args.iter().skip(1).any(|a| a == "--help" || a == "-h")
}

/// Resolve the CLI level against the catalogue, or produce the message to print.
///
/// The catalogue is the only thing the play loop can load from (`enter_playing`
/// indexes `LevelCatalog::paths`), so a level that is not in it CANNOT be played.
/// It used to fall back to slot 0 in silence — on Windows the natural
/// `levels\01.lvl` missed the string compare and the player was dropped into a
/// different level entirely, with nothing on screen to say so. Now the match is
/// path-aware and a genuine miss is a hard usage error naming both what was asked
/// for and what exists.
fn resolve_cli_level(catalog: &flow::LevelCatalog, path: &str) -> Result<usize, String> {
    use std::fmt::Write as _;

    if let Some(i) = catalog.index_of_path(path) {
        return Ok(i);
    }
    let mut msg = format!("terminal-state: '{path}' is not in the level catalogue.\n");
    let alts = catalog.same_name_as(path);
    if !alts.is_empty() {
        let _ = writeln!(msg, "  did you mean:  {}", alts.join("  "));
    }
    if std::path::Path::new(path).is_file() {
        msg.push_str(
            "  that file exists, but only levels in the catalogue can be launched into play;\n  \
             use `terminal-state --dump <path.lvl>` to render any .lvl file.\n",
        );
    }
    if catalog.paths.is_empty() {
        msg.push_str(
            "  the catalogue is EMPTY — run the game from a directory that has `levels/`.\n",
        );
    } else {
        let _ = writeln!(msg, "  available levels ({}):", catalog.paths.len());
        // Pack the list into ~78 columns so a 60-level catalogue is a readable
        // table rather than a screenful of one-per-line.
        let width = catalog.paths.iter().map(String::len).max().unwrap_or(0) + 2;
        let per_line = (78 / width).max(1);
        for chunk in catalog.paths.chunks(per_line) {
            let mut line = String::from("   ");
            for p in chunk {
                let _ = write!(line, " {p:<width$}");
            }
            msg.push_str(line.trim_end());
            msg.push('\n');
        }
    }
    msg.push_str("  run `terminal-state --help` for usage.");
    Err(msg)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // `--help` / `-h`: usage on stdout, exit 0, BEFORE anything else runs. It must
    // never reach `run_interactive` — that enters the alternate screen and blocks
    // on input, which is what `terminal-state --help` used to do in a pipe.
    if is_help_request(&args) {
        println!("{HELP}");
        return;
    }

    // Split argv once: positionals (the level path) vs flags nothing recognises.
    let scanned = scan_args(&args);

    // An unrecognised leading-`-` argument is a usage error, not something to
    // ignore. Launching the game at DEFAULT settings after a mistyped
    // `--quality=hi` tells the player nothing and looks like the flag did nothing.
    if !scanned.unknown.is_empty() {
        for bad in &scanned.unknown {
            eprintln!("terminal-state: unrecognised option '{bad}'");
        }
        eprintln!("  run `terminal-state --help` for the list of options.");
        std::process::exit(2);
    }
    if let Some(msg) = size_flag_error(&args) {
        eprintln!("{msg}");
        std::process::exit(2);
    }

    // `--dump [path]`: one-shot ASCII proof. With `--ansi` (or `--dump-color`)
    // the cells are emitted as 24-bit truecolor ANSI escapes so the relief —
    // which lives mostly in the fg/bg color channels — is actually visible when
    // the output is `cat`'d in a truecolor terminal.
    if let Some(i) = args.iter().position(|a| a == "--dump") {
        // The level path is the first POSITIONAL argument after `--dump`, so both
        // `--dump <path> --ansi` and `--dump --ansi <path>` resolve the level (the
        // flag order is not significant). With no path arg it falls back to the
        // hardcoded test_room. Positionals come from `scan_args`, so a flag's VALUE
        // (`--dump --postfx crt levels/01.lvl`) is no longer mistaken for the level.
        let path = scanned
            .positionals
            .iter()
            .find(|(j, _)| *j > i)
            .map(|(_, p)| (*p).to_string());
        let ansi = args.iter().any(|a| a == "--ansi" || a == "--dump-color");
        // `--glitch` (optionally `--glitch=0.7`) applies the deterministic glitch
        // post-process to the dumped frame at a fixed seed + fixed time, for a
        // reproducible corrupted artifact. Absent → no glitch (unchanged dump).
        let glitch = parse_glitch_flag(&args);
        // `--postfx` (optionally `--postfx=cinematic`) applies the cinematic /
        // screen-space post-process stack to the dumped frame. It composes with
        // `--ansi` and `--glitch` (postfx runs FIRST, then glitch corrupts on
        // top). Absent → no postfx (unchanged dump).
        let postfx = parse_postfx_flag(&args);
        // `--fov=N` (e.g. 60/75) raycasts the dump through `re_render::raycast_fov`
        // with that horizontal preset instead of the default FOV, so the FOV path
        // is testable from the CLI (a narrower vs wider frame). Absent → the
        // default raycast (unchanged behaviour).
        let fov = parse_fov_flag(&args);
        // `--portal-demo` injects the hardcoded demo warp-portal pair (north↔west
        // wall of the fractal-portal room) so the dump shows the non-euclidean view.
        // `--portal-demo=N` additionally walks the camera N steps forward THROUGH the
        // portal first, so the dump shows the post-teleport view (the seamless turn) —
        // the only way a static dump can demonstrate the dynamic Stage-2 teleport.
        let portal_walk = parse_portal_demo_flag(&args);
        // `--size <cols>x<rows>`, else `TS_COLS`+`TS_ROWS`, else the dump default.
        // Same precedence as the interactive path (`RenderConfig::for_game`), so a
        // render-QA dump can be taken at the grid the player actually runs.
        let size = parse_size_arg(&args).or_else(re_present::render_config::parse_size_env);
        run_dump(
            path.as_deref(),
            ansi,
            glitch,
            postfx,
            fov,
            portal_walk,
            size,
        );
        return;
    }

    // Interactive: the first POSITIONAL arg (after the program name) is a level
    // path. Taken from `scan_args`, so `--color truecolor` no longer reads
    // "truecolor" as the level to play.
    let level_path = scanned.positionals.first().map(|(_, p)| (*p).to_string());

    // `--dump-schedule` (dev tool, `debugdump` feature): print the Bevy `Update`
    // schedule as a GraphViz DOT graph and exit. Reuses `bevy_mod_debugdump`
    // instead of hand-rolling system introspection. Built only when the feature
    // is on (zero cost to the shipped build); a clear message otherwise.
    if args.iter().any(|a| a == "--dump-schedule") {
        dump_schedule();
        return;
    }

    // CONTENT CHECK, before the terminal is touched. Without a `levels/` directory
    // there is no game to run: the menu would come up, Start would find no level,
    // and the player would be left pressing a key that does nothing. Say where we
    // looked instead. `--dump <path.lvl>` has already returned above — it names its
    // own file and needs no catalogue, so a bare `.lvl` can still be rendered from
    // an exe with no content tree beside it.
    if let Err(missing) = flow::locate_levels_dir() {
        eprintln!("terminal-state: cannot find the game's `levels/` directory.");
        eprintln!("  {missing}");
        eprintln!(
            "  the levels/ and assets/ directories must sit beside terminal-state{}",
            std::env::consts::EXE_SUFFIX
        );
        eprintln!("  (or in the directory you run it from).");
        std::process::exit(2);
    }

    // RESOLVE the CLI level against the catalogue BEFORE the terminal is touched.
    // The play loop can only load catalogue entries, so a path that is not in it
    // has to be reported here — the alternative (what this binary used to do) is
    // to boot a DIFFERENT level and say nothing. The catalogue is rebuilt inside
    // `FlowPlugin`; scanning `levels/` twice at startup is a few milliseconds and
    // keeps the plugin's constructor unchanged.
    if let Some(p) = level_path.as_deref() {
        if let Err(msg) = resolve_cli_level(&flow::LevelCatalog::default(), p) {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    }

    // `--size <cols>x<rows>` (or `--size=<cols>x<rows>`): fix the render grid.
    // CLI takes precedence over the TS_COLS/TS_ROWS env vars (both parsed inside
    // RenderConfig::for_game). Absent → auto-size (a bad value exited above).
    let cli_size = parse_size_arg(&args);

    // Load persisted settings (missing/corrupt → defaults; never panics). These
    // drive color mode, mouse-look, and audio volume.
    let mut settings = settings::Settings::load();

    // `--color=truecolor|256` selects how cell colors are emitted. Precedence:
    //   1. an explicit `--color` flag ALWAYS wins for this launch AND is written
    //      back into the settings (so the menu reflects what was launched);
    //   2. otherwise use the persisted settings color from disk;
    //   3. a fresh save (no file, or an old file missing the `color` key) falls back
    //      to `settings::default_color`, which resolves via `ColorMode::resolve`'s env
    //      auto-detect (COLORTERM / the Windows VT probe) rather than hardcoding a mode.
    let cli_color = parse_color_flag(&args);
    if let Some(c) = cli_color {
        settings.color = c;
    }
    // The effective launch color: an explicit flag, else the saved setting.
    let color_mode = cli_color.unwrap_or(settings.color);
    settings.color = color_mode;

    // GRAPHICS QUALITY CLI flags (parsed alongside --color/--fov). Precedence mirrors
    // --color: an explicit flag ALWAYS wins for this launch (and is written back into
    // the settings so the menu reflects it), else the persisted value is used.
    //   --quality=low|medium|high|ultra  — apply the whole preset bundle FIRST,
    //   --draw-distance=N                 — then override the individual knobs,
    //   --fps=30|60|120|off               — (which flip the preset to Custom).
    apply_quality_flags(&args, &mut settings);

    // Resolve the RenderConfig for the game binary:
    // * hide_os_cursor default = true (game hides system arrow; TS_HIDE_CURSOR overrides).
    // * fixed_size = CLI --size flag, then TS_COLS+TS_ROWS env, then None (auto).
    let render_cfg = RenderConfig::for_game(cli_size);

    run_interactive(level_path, settings, render_cfg);
}

/// Dev-tool: dump the Bevy `Update` schedule as a GraphViz DOT graph to stdout,
/// reusing `bevy_mod_debugdump` (an ecosystem crate) rather than hand-rolling
/// system-graph introspection. Built only under the `debugdump` feature; without
/// it, prints a hint. Pipe the output to `dot -Tsvg`.
#[cfg(feature = "debugdump")]
fn dump_schedule() {
    // Build the core App (terminal/headless wiring), then render its Update
    // schedule. We don't run the loop — just introspect the registered systems.
    let mut app = build_core(None, settings::Settings::default(), false, 0);
    let settings = bevy_mod_debugdump::schedule_graph::Settings::default();
    let dot = bevy_mod_debugdump::schedule_graph_dot(&mut app, bevy::app::Update, &settings);
    println!("{dot}");
}

/// Stub for builds without the `debugdump` feature: explain how to enable it.
#[cfg(not(feature = "debugdump"))]
fn dump_schedule() {
    eprintln!(
        "--dump-schedule requires the `debugdump` dev feature.\n\
         Rebuild with:  cargo run --features debugdump -- --dump-schedule"
    );
}

/// Parse the `--color` flag in either `--color=VALUE` or `--color VALUE` form.
/// Returns `None` when the flag is absent (caller then auto-detects). An
/// unrecognized value is a hard usage error so a typo never silently picks a
/// mode the player didn't ask for.
fn parse_color_flag(args: &[String]) -> Option<ColorMode> {
    let raw = args.iter().enumerate().find_map(|(i, a)| {
        if let Some(v) = a.strip_prefix("--color=") {
            Some(v.to_string())
        } else if a == "--color" {
            args.get(i + 1).cloned()
        } else {
            None
        }
    })?;

    match ColorMode::parse_flag(&raw) {
        Some(mode) => Some(mode),
        None => {
            eprintln!("terminal-state: invalid --color value '{raw}' (use 'truecolor' or '256')");
            std::process::exit(2);
        }
    }
}

/// Parse the optional `--glitch` flag for the dump path. Returns:
/// * `None` — the flag is absent (dump is NOT glitched; behaviour unchanged).
/// * `Some(intensity)` — the flag is present; `--glitch` alone defaults to a
///   strong-but-readable `0.7`, `--glitch=VALUE` (or `--glitch VALUE`) uses the
///   given master corruption level (clamped to 0..1). A non-numeric value is a
///   hard usage error so a typo never silently picks the wrong intensity.
fn parse_glitch_flag(args: &[String]) -> Option<f32> {
    const DEFAULT: f32 = 0.7;
    for (i, a) in args.iter().enumerate() {
        if let Some(v) = a.strip_prefix("--glitch=") {
            return Some(parse_intensity_or_exit(v));
        }
        if a == "--glitch" {
            // `--glitch VALUE` only consumes the next arg if it parses as a
            // number; otherwise it's a bare `--glitch` (default intensity).
            return match args.get(i + 1) {
                Some(next) if next.parse::<f32>().is_ok() => Some(parse_intensity_or_exit(next)),
                _ => Some(DEFAULT),
            };
        }
    }
    None
}

/// Parse the optional `--postfx` flag for the dump path. Returns:
/// * `None` — the flag is absent (dump is NOT post-processed; behaviour unchanged).
/// * `Some(cfg)` — the flag is present; `--postfx` alone defaults to the
///   `cinematic` preset, `--postfx=PRESET` (or `--postfx PRESET`) selects one of
///   `off`/`subtle`/`cinematic`/`toon`/`crt`. An unknown preset is a hard usage
///   error so a typo never silently picks the wrong look.
fn parse_postfx_flag(args: &[String]) -> Option<PostfxConfig> {
    for (i, a) in args.iter().enumerate() {
        if let Some(v) = a.strip_prefix("--postfx=") {
            return Some(postfx_preset_or_exit(v));
        }
        if a == "--postfx" {
            // `--postfx PRESET` only consumes the next arg if it names a known
            // preset; otherwise it's a bare `--postfx` (default `cinematic`).
            return match args.get(i + 1) {
                Some(next) if PostfxConfig::from_preset_name(next).is_some() => {
                    Some(postfx_preset_or_exit(next))
                }
                _ => Some(PostfxConfig::cinematic()),
            };
        }
    }
    None
}

/// Look up a postfx preset by name; usage-error + exit on an unknown name.
fn postfx_preset_or_exit(raw: &str) -> PostfxConfig {
    match PostfxConfig::from_preset_name(raw) {
        Some(cfg) => cfg,
        None => {
            eprintln!(
                "terminal-state: invalid --postfx preset '{raw}' (use off/subtle/cinematic/toon/crt)"
            );
            std::process::exit(2);
        }
    }
}

/// Parse a glitch intensity string, clamping to 0..1; usage-error + exit on a
/// non-numeric value.
fn parse_intensity_or_exit(raw: &str) -> f32 {
    match raw.parse::<f32>() {
        Ok(v) => v.clamp(0.0, 1.0),
        Err(_) => {
            eprintln!(
                "terminal-state: invalid --glitch value '{raw}' (use a number 0..1, e.g. 0.7)"
            );
            std::process::exit(2);
        }
    }
}

/// Parse the optional `--fov=N` flag for the dump path. Returns:
/// * `None` — the flag is absent (dump uses the default `raycast`; unchanged).
/// * `Some(deg)` — the flag is present; `--fov=DEG` (or `--fov DEG`) raycasts the
///   dump at that horizontal preset in degrees, clamped to a sane `30..=120`. A
///   non-numeric value is a hard usage error so a typo never silently picks the
///   wrong FOV.
fn parse_fov_flag(args: &[String]) -> Option<f32> {
    for (i, a) in args.iter().enumerate() {
        if let Some(v) = a.strip_prefix("--fov=") {
            return Some(parse_fov_or_exit(v));
        }
        if a == "--fov" {
            return match args.get(i + 1) {
                Some(next) if next.parse::<f32>().is_ok() => Some(parse_fov_or_exit(next)),
                _ => {
                    eprintln!("terminal-state: --fov needs a value in degrees (e.g. --fov=60)");
                    std::process::exit(2);
                }
            };
        }
    }
    None
}

/// Parse a FOV degree string, clamping to `30..=120`; usage-error + exit on a
/// non-numeric value.
fn parse_fov_or_exit(raw: &str) -> f32 {
    match raw.parse::<f32>() {
        Ok(v) => v.clamp(30.0, 120.0),
        Err(_) => {
            eprintln!("terminal-state: invalid --fov value '{raw}' (use degrees, e.g. 60 or 75)");
            std::process::exit(2);
        }
    }
}

/// Read a `--flag=value` OR `--flag value` string flag from the args. Returns the
/// value if the flag is present (a bare `--flag` with no following value is a usage
/// error + exit, so a typo never silently no-ops).
fn string_flag_value(args: &[String], flag: &str) -> Option<String> {
    let eq = format!("{flag}=");
    for (i, a) in args.iter().enumerate() {
        if let Some(v) = a.strip_prefix(&eq) {
            return Some(v.to_string());
        }
        if a == flag {
            return match args.get(i + 1) {
                Some(next) if !next.starts_with('-') => Some(next.clone()),
                _ => {
                    eprintln!("terminal-state: {flag} needs a value (e.g. {flag}=...)");
                    std::process::exit(2);
                }
            };
        }
    }
    None
}

/// Apply the GRAPHICS-QUALITY CLI flags (`--quality`, `--draw-distance`, `--fps`) onto
/// `settings`. The preset is applied FIRST (it bundles all knobs), then the individual
/// knobs override (flipping the preset to `Custom` via `resync_preset`). An
/// unrecognised value is a hard usage error + exit so a typo never silently picks the
/// wrong tier.
fn apply_quality_flags(args: &[String], settings: &mut settings::Settings) {
    if let Some(v) = string_flag_value(args, "--quality") {
        let preset = match v.trim().to_ascii_lowercase().as_str() {
            "low" => settings::QualityPreset::Low,
            "medium" | "med" => settings::QualityPreset::Medium,
            "high" => settings::QualityPreset::High,
            "ultra" => settings::QualityPreset::Ultra,
            _ => {
                eprintln!("terminal-state: invalid --quality '{v}' (use low|medium|high|ultra)");
                std::process::exit(2);
            }
        };
        settings.apply_preset(preset);
    }
    if let Some(v) = string_flag_value(args, "--draw-distance") {
        match v.trim().parse::<u32>() {
            Ok(n) if n > 0 => {
                settings.draw_distance = n;
                settings.resync_preset();
            }
            _ => {
                eprintln!("terminal-state: invalid --draw-distance '{v}' (use a positive cell count, e.g. 16/24/32/48)");
                std::process::exit(2);
            }
        }
    }
    if let Some(v) = string_flag_value(args, "--fps") {
        match settings::FpsCap::parse_token(&v) {
            Some(cap) => {
                settings.fps_cap = cap;
                settings.resync_preset();
            }
            None => {
                eprintln!("terminal-state: invalid --fps '{v}' (use 30|60|120|off)");
                std::process::exit(2);
            }
        }
    }
}

/// Parse the optional `--portal-demo[=N]` flag for the dump path. Returns:
/// * `None` — the flag is absent (no portal injected; dump unchanged).
/// * `Some(0)` — bare `--portal-demo`: inject the demo portal, no walk (static view).
/// * `Some(N)` — `--portal-demo=N`: inject + walk N forward steps THROUGH the portal
///   so the dump shows the post-teleport view. A non-numeric value is a usage error.
fn parse_portal_demo_flag(args: &[String]) -> Option<u32> {
    for a in args {
        if let Some(v) = a.strip_prefix("--portal-demo=") {
            return Some(match v.parse::<u32>() {
                Ok(n) => n,
                Err(_) => {
                    eprintln!("terminal-state: invalid --portal-demo value '{v}' (use a step count, e.g. --portal-demo=20)");
                    std::process::exit(2);
                }
            });
        }
        if a == "--portal-demo" {
            return Some(0);
        }
    }
    None
}

/// Forward step (world units) per `--portal-demo=N` walk iteration. Small so the
/// crossing is detected cleanly and the camera lands inside the destination region.
const PORTAL_WALK_STEP: f32 = 0.5;

/// Build the map + camera for a level path (or the test room if `None`).
/// Returns `(map, camera, label, emitters, level)` where `label` describes the scene
/// for the dump banner and `level` is the PARSED level (`None` for the hardcoded test
/// room) — the caller needs it for the per-level text surfaces (`[note]` placards +
/// node word pools), which are not carried by the `Map`.
/// `inject_portal` stamps the demo warp-portal pair into the map.
type Scene = (
    Map,
    CameraPose,
    String,
    Vec<re_render::Emitter>,
    Option<re_level::Level>,
);

fn build_scene(path: Option<&str>, inject_portal: bool) -> Scene {
    match path {
        Some(p) => match re_level::load(p) {
            Ok(level) => {
                let label = format!("{} ({})", level.name, p);
                // Say what this scene is about to render WITHOUT (missing texture
                // / sky / vox / inset file), naming every path probed. The dump
                // and the live game share this notice; neither refuses to run
                // over it — see `re_runtime::asset_notice`.
                re_runtime::asset_notice::warn_missing_assets(&level, p);
                // Resolve the level's authored `[emitter]` sections into runtime
                // emitters so the --dump frame shows the SAME particles the live
                // CompositeRenderPlugin renders (M2-dump). No `[emitter]` ⇒ empty.
                // `from_spec_lit` (not the light-less `from_spec`) so `visibility =
                // lit` dust and `Flock.seek_light` read the level's own `[light]`s
                // in the dump too — Integration 2026-09-23, matching the live path
                // in `terminal_state::flow::emitter_set_for`.
                let emitters = level
                    .emitters
                    .iter()
                    .map(|spec| re_render::Emitter::from_spec_lit(spec, &level.lights))
                    .collect();
                // Stage-1 warp-portal scaffold: with TS_PORTAL_DEMO set, inject the
                // hardcoded demo portal pair (north↔west wall) so `--dump` shows the
                // ray re-basing before the real `[portal]` .lvl section (Stage 3) exists.
                let mut map = level.build_map();
                if inject_portal || std::env::var_os("TS_PORTAL_DEMO").is_some() {
                    map.inject_demo_portal_pair();
                }
                let cam = level.spawn_camera();
                (map, cam, label, emitters, Some(level))
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },
        None => (
            test_room(),
            CameraPose::spawn_in_test_room(),
            "test_room (hardcoded)".to_string(),
            // The hardcoded test room has no authored emitters; show the demo
            // sources so a bare `--dump` still exercises the particle layer.
            re_render::demo_emitters(),
            None,
        ),
    }
}

/// The DEV state bus a `--dump` casts with, or `None` when this level has nothing
/// for it to show.
///
/// The pipe-text word overlay only runs for a pipe whose state-bus word decodes to a
/// POWERED flow (`re_core::decode_flow_state`), and the words it streams come from the
/// owning node's authored `pool`. A dump runs no puzzle, so without this every pipe is
/// cold and a dump can never show a level's words — the render QA's "are the pool
/// words legible?" question was unanswerable from a `--dump` artifact.
///
/// Gate: the level must have at least one pipe AND at least one authored pool word (a
/// level with pipes but no pools would only stream the built-in word list, which is
/// not this level's content and would change existing dump artifacts for nothing).
/// The encoding mirrors `state_bridge::rebuild_map` exactly — the pipe's RENDER tag is
/// `encode_pipe_side_data(pipe_index)` and a flowing pipe scrolls FORWARD along its
/// authored input→output path — so the dump shows the same thing the live game shows
/// once the network is powered.
fn dump_flow_state(level: Option<&re_level::Level>) -> Option<CellStateMap> {
    let level = level?;
    if level.pipes.is_empty() || !level.nodes.iter().any(|n| !n.pool.is_empty()) {
        return None;
    }
    let mut map = CellStateMap::new();
    for pi in 0..level.pipes.len() {
        map.set_state(
            re_core::encode_pipe_side_data(pi as u16),
            re_core::encode_flow_state(re_core::FlowDir::Forward),
        );
    }
    Some(map)
}

/// One-shot: raycast the scene once and print the framebuffer. With `ansi ==
/// false` (the default, unchanged behaviour) it prints the glyph grid as plain
/// ASCII. With `ansi == true` each cell is emitted as a 24-bit ANSI escape
/// carrying the cell's real fg/bg, so the relief reads in color when `cat`'d in
/// a truecolor terminal.
fn run_dump(
    path: Option<&str>,
    ansi: bool,
    glitch: Option<f32>,
    postfx: Option<PostfxConfig>,
    fov_deg: Option<f32>,
    portal_walk: Option<u32>,
    size: Option<(usize, usize)>,
) {
    let (mut map, mut cam, label, emitters, mut level) = build_scene(path, portal_walk.is_some());

    // FORM A-7 INTERPOLATION — the SAME up-front pass the play loader runs
    // (`flow::load_level`), driven by the player's PERSISTED `lore_fourth_wall` opt-out.
    //
    // Why here and not inside `level_text`: A-7 §8 promises the opt-out
    // UNCONDITIONALLY, and `build_level_text`'s own fallback probe constructs
    // `OperatorProfile::for_fourth_wall(true)` (real Tier-1 host reads) for any body
    // that still carries a `{token}` — which would ignore a player who turned the
    // fourth wall OFF. Interpolating the level HERE, from the real setting, leaves no
    // braces behind, so that fallback never fires and the dump shows exactly what the
    // player's own configuration renders. `interpolate` is idempotent, so the later
    // `bind_level_text` pass is a no-op on these bodies.
    let fourth_wall = settings::Settings::load().lore_fourth_wall;
    if let Some(l) = level.as_mut() {
        let profile = terminal_state::story::interp::OperatorProfile::for_fourth_wall(fourth_wall);
        terminal_state::story::interp::interpolate_level_notes(l, &profile);
    }

    // TEXT SURFACES: publish this level's code panels + `[note]` placard panels + pipe
    // word pools, and stamp the placards onto their wall faces. Must run BEFORE
    // `registry_for_map` below, which reads the note materials off the map.
    let note_note = match &level {
        Some(l) => {
            let r = level_text::bind_level_text(l, &mut map);
            if r.unplaced.is_empty() {
                if r.placed == 0 {
                    String::new()
                } else {
                    format!("  notes={}", r.placed)
                }
            } else {
                format!("  notes={} UNPLACED={:?}", r.placed, r.unplaced)
            }
        }
        // No level (the hardcoded test room): still publish the code panels so a bare
        // `--dump` of a code-wall material shows real source.
        None => {
            level_text::bind_code_panels_only();
            String::new()
        }
    };

    // AUTHORED `[emit]` PARTS, at the level's LOAD state. The interactive game binds
    // these in `flow::enter_playing` and drives them from the live puzzle; a dump has
    // no running puzzle, so we build the level's initial `PuzzleState` (the same
    // forward pass the play loader runs) and apply it once. A level whose emit sources
    // start unpowered — every shipped campaign level, whose pipes are authored OFF —
    // therefore dumps byte-identically to before, while one whose source is already
    // hot shows exactly what the player would see on load. Honest, not decorative.
    let emit_note = match &level {
        Some(l) => {
            let mut bound = re_runtime::emit_parts::bind_emit_parts(l, &mut map);
            let lit = match re_puzzle::PuzzleState::from_level(l) {
                Ok(p) => {
                    re_runtime::emit_parts::sync_emit_parts(&mut bound, &p, &mut map);
                    bound.lit_cell_count()
                }
                // A malformed network is already reported by the normal load path; the
                // dump just shows the room with nothing materialised.
                Err(_) => 0,
            };
            if lit == 0 {
                String::new()
            } else {
                format!("  emit={lit}")
            }
        }
        None => String::new(),
    };

    // POWER-REACTIVE PROPS, at the level's LOAD state — the same honesty rule as the
    // emit parts above: the built map carries `re_level`'s STATIC initial state
    // (no forward pass), so apply the initial `PuzzleState` once, exactly what the
    // live game's first frame does. A level without `[prop] power =` is untouched.
    let mut emitters = emitters;
    if let Some(l) = &level {
        if !l.prop_power.is_empty() {
            if let Ok(p) = re_puzzle::PuzzleState::from_level(l) {
                let mut pp = re_runtime::prop_power::bind_prop_power(l, &map);
                re_runtime::prop_power::sync_prop_power(&mut pp, &p, &mut map, Some(&mut emitters));
            }
        }
    }

    // `--portal-demo=N`: walk the camera N steps forward through the demo portal
    // before composing. `move_with_collision` applies the seamless portal crossing
    // internally (Stage-2 teleport), so the dumped frame shows the POST-teleport view
    // — the only way a static dump can demonstrate the dynamic teleport.
    let portal_note = match portal_walk {
        Some(n) if n > 0 => {
            // Detect whether a portal crossing actually fired: a teleport re-bases the
            // pose discontinuously, so a single step jumps far more than the step size.
            // (Reporting "teleported" unconditionally would mask a regression where the
            // crossing silently stops firing — critic finding m1.)
            let mut teleported = false;
            for _ in 0..n {
                let (dx, dy) = cam.dir();
                let before = cam.pos;
                cam.move_with_collision(
                    dx * PORTAL_WALK_STEP,
                    dy * PORTAL_WALK_STEP,
                    &map,
                    re_world::PLAYER_RADIUS,
                );
                if cam.pos.distance(before) > PORTAL_WALK_STEP * 4.0 {
                    teleported = true;
                }
            }
            if teleported {
                format!("  portal-walk={n}→TELEPORTED")
            } else {
                format!("  portal-walk={n} (no crossing)")
            }
        }
        Some(_) => "  portal-demo(static)".to_string(),
        None => String::new(),
    };

    // `--size`/`TS_COLS`+`TS_ROWS` override the dump grid; absent → the 100x32
    // default every golden was blessed at. Clamped to >=1 so a `--size 0x0` cannot
    // produce a zero-area framebuffer.
    let (dump_cols, dump_rows) = size.map_or((DUMP_COLS, DUMP_ROWS), |(c, r)| (c.max(1), r.max(1)));
    let mut fb = Framebuffer::new(dump_cols, dump_rows);

    // The dump is composed through the TIERED-COMPOSITE single-pass entry
    // (`composite_dump_once`), exactly mirroring the live `CompositeRenderPlugin`
    // so a `--dump` artifact shows the SAME far backdrop + near opaque/transparent
    // + emitter layering the interactive frame produces (design 2026-06-18-tiered-
    // composite-emitters §2). The shade registry comes from the loaded map's
    // authored materials (built-ins when none are authored).
    //
    // `--fov=N`: drive the composite cast at the requested horizontal preset (cell
    // aspect + stretch left at their defaults), so a narrower/wider frame is
    // reproducible from the CLI. Absent → the default FOV, unchanged framing.
    let registry = re_render::registry_for_map(&map);
    // DEV FLOW STATE for the dump path. The live game gets its per-pipe flow words
    // from `state_bridge` (the real puzzle drives them); a `--dump` runs no puzzle, so
    // the state bus is EMPTY and every pipe decodes to `FlowDir::None` — which gates
    // the pipe-text WORD overlay off entirely (`re_render` shade path). A dump could
    // therefore never show a level's authored word pools, which is exactly what the
    // render QA needs to look at. So when the level has BOTH pipes and authored pools,
    // publish a bus that powers every pipe FORWARD for the span of this one cast.
    //
    // Dump-only, and deliberately not a CLI flag: it makes the artifact show what the
    // live game shows once the network is powered, nothing more.
    let flow_state = dump_flow_state(level.as_ref());
    let flow_note = if flow_state.is_some() {
        "  flow=dev".to_string()
    } else {
        String::new()
    };
    let flow_scope = flow_state
        .as_ref()
        .map(re_render::state_bus::CellStateScope::new);
    let fov_note = if let Some(deg) = fov_deg {
        let fov = Fov {
            h_preset_deg: deg,
            ..Fov::default()
        };
        re_render::composite::composite_dump_once(&map, &cam, &registry, &fov, &emitters, &mut fb);
        format!("  fov={deg:.0}°")
    } else {
        re_render::composite::composite_dump_once(
            &map,
            &cam,
            &registry,
            &Fov::default(),
            &emitters,
            &mut fb,
        );
        String::new()
    };
    // The bus is only meaningful for the cast; drop it before the post-process passes
    // so nothing later on this thread sees a dev state.
    drop(flow_scope);

    // `TS_POSTFX=blur|gray|palette|gameboy|amber` (comma-separated): apply the
    // re_render RADIAL FOCUS-BLUR + grayscale/palette screen-space pass to the
    // freshly-composed frame, BEFORE the cinematic --postfx grade and any glitch.
    // This is the env-gated demo of the focus/blur feature (a key toggle drives the
    // same `PostFxConfig` later). Absent / unrecognised → no-op (frame unchanged).
    let focus_note = match re_render::PostFxConfig::from_env() {
        Some(cfg) => {
            re_render::apply_posteffects(&mut fb, &cfg);
            "  ts_postfx".to_string()
        }
        None => String::new(),
    };

    // `--postfx[=preset]`: apply the cinematic / screen-space post-process stack
    // to the freshly-shaded frame BEFORE any glitch, so corruption (if any) sits
    // on top of the graded image — exactly the plugin ordering. We run it WITHOUT
    // its final dither pass (`skip_dither = true`): dither must be the absolute
    // last op, AFTER glitch's colour-mutating passes, so glitch can't re-introduce
    // un-dithered banding (spec §15 «DITHER LAST»; audit #4). The deferred dither
    // is applied below once glitch has run. Composes with `--ansi` and `--glitch`.
    let postfx_note = if let Some(cfg) = postfx {
        apply_postfx_ex(&mut fb, &cfg, true);
        format!("  postfx@{:.2}", cfg.intensity)
    } else {
        String::new()
    };

    // `--glitch[=intensity]`: apply the deterministic glitch post-process to the
    // (post-processed) frame at a FIXED seed + FIXED time, so the corrupted dump is
    // fully reproducible. Composes with `--ansi` (the glitched cells emit their
    // mutated fg/bg/glyph through the same truecolor path). Absent → unchanged.
    let glitch_note = if let Some(intensity) = glitch {
        let cfg = GlitchConfig::terminal_decay().with_intensity(intensity);
        apply_glitch(&mut fb, &cfg, DUMP_GLITCH_TIME, DUMP_GLITCH_SEED);
        format!(
            "  glitch=terminal_decay@{intensity:.2} (seed={DUMP_GLITCH_SEED:#x}, t={DUMP_GLITCH_TIME})"
        )
    } else {
        String::new()
    };

    // Deferred DITHER LAST: now that postfx (sans dither) and glitch have both run,
    // apply the postfx dither as the genuine final quantisation over the composed
    // image — so the float of every prior pass (grade + glitch corruption) is
    // dithered into the palette exactly once, at the very end (audit #4). No-op
    // unless postfx was requested with dither enabled.
    if let Some(cfg) = postfx {
        apply_dither(&mut fb, &cfg);
    }

    let (sx, sy, sz) = map.size();
    println!(
        "terminal-state --dump: {label}  {sx}x{sy}x{sz}  spawn=({:.1},{:.1},{:.1}) yaw={:.2}  fb={}x{}{fov_note}{focus_note}{postfx_note}{glitch_note}{portal_note}{note_note}{emit_note}{flow_note}\n",
        cam.pos.x,
        cam.pos.y,
        cam.pos.z,
        cam.yaw,
        fb.cols(),
        fb.rows()
    );

    if ansi {
        use std::fmt::Write as _;

        println!("==== TRUECOLOR GRID ({}x{}) ====", fb.cols(), fb.rows());
        for row in 0..fb.rows() {
            let mut line = String::with_capacity(fb.cols() * 24);
            for col in 0..fb.cols() {
                let c = fb.get(col, row).unwrap();
                let g = char::from_u32(c.glyph).unwrap_or(' ');
                let _ = write!(line, "\x1b[{}m{g}", cell_sgr_params(c));
            }
            // Reset per line so the terminal returns to default at the margin.
            line.push_str("\x1b[0m");
            println!("{line}");
        }
    } else {
        println!("==== GLYPH GRID ({}x{}) ====", fb.cols(), fb.rows());
        for row in 0..fb.rows() {
            let mut line = String::with_capacity(fb.cols());
            for col in 0..fb.cols() {
                let g = fb.get(col, row).unwrap().glyph;
                line.push(char::from_u32(g).unwrap_or(' '));
            }
            println!("{line}");
        }
    }
}

/// Unpack a packed `0x00RRGGBB` color into `(r, g, b)` byte components.
#[inline]
const fn unpack(c: u32) -> (u8, u8, u8) {
    (
        ((c >> 16) & 0xFF) as u8,
        ((c >> 8) & 0xFF) as u8,
        (c & 0xFF) as u8,
    )
}

/// Build the SELF-CONTAINED SGR parameter string for one [`re_core::Cell`] (the
/// part between `\x1b[` and `m`) for the `--ansi` truecolor dump.
///
/// One self-contained SGR per cell — the leading `0` resets the prior cell's
/// attributes so bold/underline never bleed across cells: reset → style flags →
/// underline SUBSTYLE (`4:n`) → fg/bg 24-bit → optional colored underline
/// (`58;2;r;g;b`). The underline shape uses the SGR `4:n` extension
/// (single `4:1`, double `4:2`, curly `4:3`, dotted `4:4`, dashed `4:5`); we own
/// these bytes, so the curly/dotted/dashed scan-glow / glint shapes render
/// faithfully when the artifact is `cat` in a capable terminal.
/// `effective_underline_style` resolves the back-compat rule (a legacy `UNDERLINE`
/// flag or a colored underline ⇒ `Single`), so pre-substyle cells still emit a
/// plain underline exactly as before. Pure — returns the string, no I/O.
fn cell_sgr_params(c: &re_core::Cell) -> String {
    use std::fmt::Write as _;

    let (fr, fg_, fb_) = unpack(c.fg);
    let (br, bg_, bb) = unpack(c.bg);
    let s = c.style;
    let mut p = String::from("0");
    if s.is_bold() {
        p.push_str(";1");
    }
    if s.is_dim() {
        p.push_str(";2");
    }
    if s.is_italic() {
        p.push_str(";3");
    }
    let us = s.effective_underline_style();
    if us.is_some() {
        let _ = write!(p, ";4:{}", us.sgr_param());
    }
    if s.is_reverse() {
        p.push_str(";7");
    }
    if s.is_strike() {
        p.push_str(";9");
    }
    let _ = write!(p, ";38;2;{fr};{fg_};{fb_};48;2;{br};{bg_};{bb}");
    if let Some(uc) = s.underline_color() {
        let (ur, ug, ub) = unpack(uc);
        let _ = write!(p, ";58;2;{ur};{ug};{ub}");
    }
    p
}

/// Whether this launch shows the OPENING two-phase boot sequence at all — the
/// ONE gate both Phase 1 (`re_present::intro_tear`, the real-terminal screen
/// tear) and Phase 2 ([`GameState::BootConsole`]) share. `Settings::boot_intro`
/// off, `TS_NO_INTRO` set, or stdout not a VT-capable tty: skip BOTH phases
/// entirely — no bytes written, no state ever transitions to `BootConsole`.
/// `re_present::should_run_tear` carries the (unit-tested) decision logic;
/// this just gathers its live inputs (the one part that cannot be a pure
/// function — it reads the real console).
fn should_show_boot_intro(settings: &settings::Settings) -> bool {
    let no_intro_env = std::env::var_os("TS_NO_INTRO").is_some();
    let tty_vt_ok = re_present::tty_and_vt_capable();
    re_present::should_run_tear(settings.boot_intro, no_intro_env, tty_vt_ok, true)
}

/// A fresh boot seed for this launch: real entropy is fine here (unlike
/// `re_present::intro_tear::generate_tear_ops` / `boot_console`'s corruption
/// math, which are PURE functions of whatever seed they are given — this is
/// the one place that picks it). `0` when the intro is disabled — nothing
/// reads it in that case, so its value is moot, but a fixed value keeps this
/// function itself simple and deterministic to call in the disabled branch.
fn fresh_boot_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
        ^ 0x9E37_79B9_7F4A_7C15
}

/// `Startup`: redirect the very first state transition from `Splash`
/// (`GameState::default()`) to [`GameState::BootConsole`]. Only added when
/// [`should_show_boot_intro`] said yes — every other build (tests, `--dump`,
/// a disabled/non-tty launch) never adds this system, so `Splash` is reached
/// exactly as before.
fn enter_boot_console_on_startup(mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::BootConsole);
}

/// The interactive game loop.
///
/// The terminal backend (`backend-terminal`, the only backend) runs headless:
/// `MinimalPlugins` + `ScheduleRunnerPlugin` drive a fixed 60 Hz loop, and
/// `RatatuiPlugins` (added by [`PresentPlugin`]) own the terminal.
///
/// The OPENING boot sequence's Phase 1 (the real-terminal screen tear) runs
/// synchronously HERE, BEFORE any Bevy `App`/`PresentPlugin` exists — it
/// writes straight to the MAIN screen, which must still be showing whatever
/// the player's shell had on it, i.e. before `bevy_ratatui`'s `RatatuiPlugins`
/// ever enters the alternate screen. Phase 2 (the pseudo boot console) is a
/// `GameState` the Bevy app owns from `Startup`, via
/// `enter_boot_console_on_startup`.
fn run_interactive(
    level_path: Option<String>,
    settings: settings::Settings,
    render_cfg: RenderConfig,
) {
    let boot_intro_enabled = should_show_boot_intro(&settings);
    let boot_seed = if boot_intro_enabled {
        fresh_boot_seed()
    } else {
        0
    };

    if boot_intro_enabled {
        re_present::run_screen_tear(&re_present::TearConfig {
            seed: boot_seed,
            ..Default::default()
        });
    }

    let mut app = build_core(level_path, settings, boot_intro_enabled, boot_seed);
    // Insert RenderConfig BEFORE PresentPlugin so the plugin's `init_resource`
    // (which provides the safe default) sees our binary-specific config instead.
    // RenderConfig carries: hide_os_cursor (game default=true) + fixed_size.
    app.insert_resource(render_cfg);
    // The present backend (terminal `RatatuiPlugins`) is added LAST — it's the
    // only part that needs a real terminal, so `build_core` (everything else)
    // stays headless-testable.
    app.add_plugins(PresentPlugin);
    app.run();
    // Belt-and-suspenders: `bevy_ratatui`'s own teardown (run as the `World`
    // drops, inside `app.run()`, before it returns here) has normally already
    // left the alt screen. This additionally CLEARS the main screen — the
    // OPENING sequence's "exit clears the main screen" contract — and is a
    // no-op-safe idempotent call regardless. The panic path is covered
    // separately, from `re_present`'s panic hook (see that crate's
    // `intro_tear` module doc).
    re_present::finalize_terminal_teardown();
}

/// Per-frame wall-clock anchor for the live FPS-cap throttle (terminal backend).
/// Holds the `Instant` the previous frame finished, so [`fps_throttle_sys`] can pace
/// the loop to the live settings cap.
#[cfg(feature = "backend-terminal")]
#[derive(Resource)]
struct FpsThrottle {
    last_frame_end: std::time::Instant,
}

#[cfg(feature = "backend-terminal")]
impl Default for FpsThrottle {
    fn default() -> Self {
        Self {
            last_frame_end: std::time::Instant::now(),
        }
    }
}

/// `Last` (terminal backend): pace the loop DOWN to the live FPS cap. Sleeps the
/// remainder of the current settings-fps frame budget if the frame finished sooner.
/// `FpsCap::Off` (interval 0) is a no-op, so an uncapped setting spins as fast as the
/// run-loop allows. The run-loop's own startup interval already enforces the cap the
/// game LAUNCHED with; this only adds further pacing when the player lowers the cap
/// live in the settings menu (a higher live cap can't speed the fixed run-loop up —
/// that takes a restart, the documented behaviour).
#[cfg(feature = "backend-terminal")]
fn fps_throttle_sys(mut throttle: ResMut<FpsThrottle>, settings: Res<settings::Settings>) {
    let interval = settings.fps_loop_interval_secs();
    if interval <= 0.0 {
        // Uncapped: just advance the anchor, never sleep.
        throttle.last_frame_end = std::time::Instant::now();
        return;
    }
    let budget = std::time::Duration::from_secs_f64(interval);
    let elapsed = throttle.last_frame_end.elapsed();
    if elapsed < budget {
        std::thread::sleep(budget.saturating_sub(elapsed));
    }
    throttle.last_frame_end = std::time::Instant::now();
}

/// Build the interactive app's **headless core** — every plugin/resource/system
/// EXCEPT the present backend (`PresentPlugin`) and `app.run()`. Extracted so a
/// headless test can build it + `update()` and catch production plugin-wiring
/// bugs (e.g. a missing `StatesPlugin`) that the hand-rolled `test_app` harness
/// in `flow::tests` would mask.
///
/// `boot_intro_enabled`/`boot_seed` wire the OPENING sequence's Phase 2
/// (`GameState::BootConsole`): every test call site below passes `(false, 0)`,
/// which reproduces the exact pre-OPENING-sequence behaviour (boots straight
/// to `Splash`) — see `interactive_core_boots_to_splash_without_panic`.
fn build_core(
    level_path: Option<String>,
    settings: settings::Settings,
    boot_intro_enabled: bool,
    boot_seed: u64,
) -> App {
    let mut app = App::new();

    #[cfg(feature = "backend-terminal")]
    {
        // The run-loop interval is the FPS CAP read from settings at STARTUP. Uncapped
        // (`FpsCap::Off`) → a near-zero interval (spin as fast as the schedule allows);
        // `Fps(n)` → `1/n`. A live in-menu FPS change is handled by the runtime
        // frame-pacing throttle (`fps_throttle_sys`), since the run-loop interval is
        // fixed once the plugin is added. Default (60) reproduces the historical loop.
        app.add_plugins(
            bevy::MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
                std::time::Duration::from_secs_f64(settings.fps_loop_interval_secs()),
            )),
        );
    }

    // `FlowPlugin` (added below) calls `init_state::<GameState>()`, which needs the
    // `StateTransition` schedule from `StatesPlugin`. `MinimalPlugins` (terminal)
    // never brings it, so add it once HERE, before `FlowPlugin`.
    app.add_plugins(bevy::state::app::StatesPlugin);

    // Apply the saved settings to the runtime resources the present/audio layers
    // read. Inserted BEFORE PresentPlugin so the backend's `init_resource`
    // defaults are kept (insert-if-absent) — our explicit insert wins.
    let color_mode = settings.color;
    let mouse_settings = settings.mouse_settings();

    // Drive the raycaster FOV from the saved settings (preset + stretch).
    // Inserted BEFORE CompositeRenderPlugin (whose `init_resource::<Fov>()` is
    // insert-if-absent), so our saved FOV wins. The composite near tier re-casts
    // when this resource changes, so the settings FOV/stretch rows and the live
    // `[`/`]` hotkey take effect on the next frame.
    app.insert_resource(settings.fov());

    // Drive the cast-shadow quality from the saved settings. Inserted BEFORE
    // CompositeRenderPlugin (whose `init_resource::<ShadowQuality>()` is
    // insert-if-absent), so our saved value wins. The near cast folds
    // `ShadowQuality` into its cache key, so the SHADOWS settings row takes effect
    // on the next frame.
    app.insert_resource(settings.shadow_quality());

    // GRAPHICS QUALITY: drive the render levers from the saved settings. All are
    // inserted BEFORE CompositeRenderPlugin (whose inits are INSERT-IF-ABSENT), so our
    // saved values win and the composite cast reads them on the next frame. The DEFAULT
    // (High) quality yields the engine-default resources, so a default build is
    // byte-identical.
    //   draw distance  → FarFreeze (far-freeze cutoff + DDA step cap)
    //   draw + LOD     → LodConfig (near/far boundaries)
    //   sub-cell       → SubCellConfig (2×2 silhouette blitter)
    app.insert_resource(settings.far_freeze());
    app.insert_resource(settings.lod_config());
    app.insert_resource(settings.subcell_config());
    // Post-fx preset → PostfxConfig (inserted before PostfxPlugin, whose init is
    // insert-if-absent). Default "off" = the identity, so shipped levels are unchanged.
    app.insert_resource(settings.postfx_config());

    // The present layer reads this each frame; inserting it here overrides the
    // backend's default. Must be inserted before PresentPlugin reads it.
    // Reused ecosystem RNG: a single deterministically-seeded global `WyRand`
    // entropy source (bevy_rand). Additive — made AVAILABLE for new randomness
    // wired through the App; the golden-tested bespoke LCG/SplitMix streams in
    // re_audio/re_glitch are deliberately untouched (see src/rng.rs).
    app.add_plugins(re_runtime::rng::GameRngPlugin);

    app.insert_resource(color_mode)
        // Mouse-look settings (on/off + sensitivity), read by the mouse-look
        // systems in both backends. Overrides the backend default.
        .insert_resource(mouse_settings)
        // The persisted settings resource itself, so the in-game settings screen
        // mutates + re-applies + re-saves it.
        .insert_resource(settings)
        // Persistent progress (solved levels). Loaded from disk once at
        // startup; the flow marks levels solved + re-persists on OnEnter(Won),
        // and the menu / level-select overlays read it. Robust to a
        // missing/corrupt file (→ empty progress, see `save.rs`).
        .insert_resource(Progress::load())
        // The game-flow state machine OWNS level loading (per-state) + the
        // always-present Map/Framebuffer/PuzzleState/Solved resources. It is
        // added BEFORE PuzzlePlugin so the play-state systems can override the
        // default-empty PuzzleState that PuzzlePlugin inits.
        .add_plugins(FlowPlugin {
            cli_level: level_path,
        })
        // Puzzle (network forward pass + InteractAt) and audio (synth + device)
        // are engine-agnostic plugins; the present layer never sees them.
        .add_plugins(PuzzlePlugin)
        // STATE BUS bridge (reactive-materials spec §2.2): walks the live PuzzleState
        // into the generic CellStateMap the renderer reads, so a powered node and a
        // dead node render differently. Added AFTER PuzzlePlugin so its Update system
        // is ordered after `apply_interactions` (this frame's toggle + forward pass).
        // Engine-agnostic (re_core CellStateMap only crosses to re_render).
        .add_plugins(re_runtime::state_bridge::StateBridgePlugin)
        // AUTHORED `[emit]` PARTS: stamps / un-stamps each level's `[emit SOURCE=1]
        // parts = …` voxel figure as its source node's activation rises and falls —
        // the pedestal reward the campaign levels have always authored. Added AFTER
        // PuzzlePlugin for the same `after(apply_interactions)` ordering reason as
        // the state bridge; engine-agnostic (it only writes cells into `Map`).
        .add_plugins(re_runtime::emit_parts::EmitPartsPlugin)
        // POWER-REACTIVE PROPS: `[prop] power =` gates follow the live puzzle (cells,
        // line colours, lamps, holograms, emitter slots). Same ordering reason.
        .add_plugins(re_runtime::prop_power::PropPowerPlugin)
        .add_plugins(AudioPlugin)
        // The TIERED-COMPOSITE renderer (design 2026-06-18-tiered-composite-
        // emitters §2–§4): a drop-in replacement for the old single-pass
        // `RenderPlugin`. It owns the `CompositeStack`/`FarCache`/`TierClock`,
        // pulls in `EmitterPlugin`, and registers ONE `PostUpdate`
        // `composite_render_sys` that produces `Res<Framebuffer>` by folding the
        // far backdrop + near opaque/transparent + emitters layers (idle-frame
        // cheap, mirroring the old raycast dirty-skip). It is the ONLY system that
        // writes the live Framebuffer — `raycast_sys` is no longer registered, so
        // there is no double-writer. Like `RenderPlugin`, its `Fov`/`ShadowQuality`/
        // dirty resource inits are INSERT-IF-ABSENT, so the saved FOV/shadow values
        // we inserted above (and `FlowPlugin`'s) still win.
        .add_plugins(CompositeRenderPlugin)
        // The colour-drift field application: after the composite resolve,
        // before postfx. Added HERE, right after `CompositeRenderPlugin`,
        // before any post-process/gate/glitch stage.
        // The `DriftSources` resource itself is computed by
        // `re_runtime::color_drift_sources::ColorDriftPlugin` (added by
        // `FlowPlugin`, `Update`-scheduled); this plugin only adds the
        // frame-application system at the correct point in this chain.
        .add_plugins(re_runtime::color_drift_sources::ColorDriftFramePlugin)
        // The cinematic / screen-space post-process stack (edge-outline, SSAO,
        // bloom, DoF, color-grade, vignette, dither). Added AFTER RenderPlugin
        // (so its PostUpdate system runs after the raycast/shade produced the
        // Framebuffer) and BEFORE GlitchPlugin (so glitch corruption sits on top
        // of the graded image). Its PostfxConfig resource defaults to `off` —
        // shipped levels render identically until a level / setting opts in. Uses
        // the same clean-snapshot anti-compounding trick as GlitchPlugin. A level
        // system can drive it via `ResMut<PostfxConfig>` (e.g.
        // `*cfg = PostfxConfig::cinematic()`).
        .add_plugins(PostfxPlugin)
        // The campaign colour-unlock gate: runs AFTER postfx, BEFORE glitch.
        // Added HERE, between `PostfxPlugin` and `GlitchPlugin`, so its
        // PostUpdate system runs on the graded frame before glitch corrupts it
        // (glitch can then show a locked colour for a few frames via
        // `PaletteGate::leak` — "a memory intrusion from the future"). The
        // `PaletteGateState` resource itself is computed by
        // `terminal_state::palette_ramp::PaletteRampPlugin` (added by `FlowPlugin`,
        // `Update`-scheduled, order-independent); this plugin only adds the
        // frame-application system at the correct point in this chain.
        .add_plugins(terminal_state::palette_ramp::PaletteGateFramePlugin)
        // The glitch post-process: added AFTER RenderPlugin (so its PostUpdate
        // system runs after the raycast/shade produced the Framebuffer) and
        // BEFORE PresentPlugin (so the present layer draws the glitched frame).
        // Its GlitchConfig resource defaults to `off` — shipped levels render
        // identically until a level / setting opts in (see GlitchPlugin docs for
        // how the clean-snapshot avoids compounding across dirty-flag-skipped
        // frames). A level system or the settings screen can drive it later via
        // `ResMut<GlitchConfig>` (e.g. `*cfg = GlitchConfig::terminal_decay()
        // .with_intensity(corruption_level)`).
        .add_plugins(GlitchPlugin)
        // The deferred final-dither pass: added AFTER GlitchPlugin so the postfx
        // ordered-dither runs as the ABSOLUTE last screen-space op, quantising the
        // float of every prior pass — postfx grade AND glitch colour corruption —
        // into the terminal palette exactly once (spec §15 «DITHER LAST»; audit
        // #4). PostfxPlugin runs the rest of the stack with dither deferred; this
        // plugin closes it. No-op when postfx is off, so shipped levels are
        // unaffected.
        .add_plugins(PostfxDitherPlugin)
        // The splash/menu/select/won text overlays (painted into the
        // Framebuffer in the non-playing states).
        .add_plugins(MenuRenderPlugin)
        // FOV-ZOOM (Z) with a radial REVEAL transition + the live POST-FX cycle
        // (G). Its reveal + post-fx systems run in `Last`, `.before(PresentSet)`,
        // ordered reveal → post-fx, gated to `GameState::Playing`. Default state is
        // not-zoomed + post-fx Off, so a default frame (and every golden hash) is
        // unchanged until the player presses the key.
        .add_plugins(runtime_glue::ZoomPlugin)
        // LEVEL-ENTRY "resolve" transition (`terminal_state::entry_fx`): a
        // short, skippable hashed-reveal sweep from black into the freshly
        // loaded level, `OnEnter(GameState::Playing)`.
        // Three independent knobs still gate any given transition
        // (`Settings::entry_transition`, `Settings::boot_intro`,
        // `TS_NO_INTRO`) — see that module's doc for why registering the
        // plugin here is safe for `--dump`/goldens/every headless test that
        // never reaches `Playing` through this `build_core`.
        .add_plugins(terminal_state::entry_fx::EntryFxPlugin)
        // Gameplay glue OWNED by the binary, gated to GameState::Playing:
        // interact → InteractAt + click sound, and the win watcher (solve →
        // sting/door + log + Won transition).
        .init_resource::<WinState>()
        // Shared perf/input resources (re_core). `init_resource` is
        // insert-if-absent and uses re_core defaults (PerfOverlay off,
        // InputGate.camera_motion = false); sibling crates also init these
        // defensively, which is fine (first registration wins). The F3 overlay
        // draw + the OnEnter/OnExit(Playing) camera-gate systems read these.
        .init_resource::<re_core::FrameTimingMetrics>()
        .init_resource::<re_core::PerfOverlay>()
        .init_resource::<re_core::InputGate>()
        // Apply the saved audio volume once the audio plugin is up. The settings
        // screen later re-emits this whenever mute/volume changes.
        .add_systems(Startup, apply_initial_volume)
        .add_systems(OnEnter(GameState::Playing), enter_play)
        .add_systems(
            Update,
            (interact_sys, watch_win).run_if(in_state(GameState::Playing)),
        )
        // The interact ack cue is chosen from the RESOLVED outcome (after
        // `re_puzzle` applied or refused the toggle), alongside every other
        // gameplay cue, the level room, the listener and the positional sources:
        // see `terminal_state::audio_cues`.
        .add_plugins(terminal_state::audio_cues::AudioCuesPlugin)
        // Sound fields: after `AudioCuesPlugin`
        // (owns `CueTracker`, read for `notes_read`/`shards`) and `AudioPlugin`
        // (owns `SoundFieldMix`, written here every play frame).
        .add_plugins(runtime_glue::SoundFieldsPlugin);

    // OPENING boot sequence, Phase 2 (`GameState::BootConsole`). `BootSeed` is
    // inserted BEFORE `BootConsolePlugin` (whose `init_resource` is
    // insert-if-absent), so this explicit value wins — mirroring every other
    // "insert before the insert-if-absent plugin" pattern above. `0`/default
    // when the intro is disabled is harmless: nothing ever reads it, because
    // `BootConsole` is never entered.
    app.insert_resource(boot_console::BootSeed(boot_seed))
        .add_plugins(boot_console::BootConsolePlugin);
    if boot_intro_enabled {
        app.add_systems(Startup, enter_boot_console_on_startup);
    }

    // LIVE FPS-CAP throttle (terminal backend only — the window backend uses winit's
    // own present-rate loop). The run-loop interval is fixed at plugin-add from the
    // STARTUP fps, but a settings change to a LOWER fps must take effect immediately.
    // This `Last` system sleeps the remainder of the current settings fps frame budget
    // when it exceeds the time already spent — pacing the loop DOWN to the chosen cap
    // without a restart. `FpsCap::Off` and a cap at/above the startup interval are
    // no-ops (the run-loop already paces at least that fast).
    #[cfg(feature = "backend-terminal")]
    app.init_resource::<FpsThrottle>()
        .add_systems(Last, fps_throttle_sys);

    // Dev-only BRP server (feature `dev`); headless, off in shipped builds. Added
    // as a separate gated statement so the default build is byte-unchanged.
    #[cfg(feature = "dev")]
    app.add_plugins(re_runtime::dev::DevToolingPlugin);

    app
}

/// Observable win flag. `won` latches true the first frame [`Solved`] becomes
/// true; `announced` guards the one-shot win audio + log so they fire once.
/// `latched_at_load` records whether the level was ALREADY solved the instant
/// it loaded — if so we must NOT fire the win sting/log on frame 1 (the player
/// hasn't acted). The win only counts as a real win on a false→true transition
/// that happens AFTER load. Surfaced as a resource so the flow / a HUD can read
/// it without reaching into [`PuzzleState`].
#[derive(Resource, Default, Debug)]
struct WinState {
    won: bool,
    announced: bool,
    /// True when the level loaded already-satisfied — suppresses the frame-1
    /// win until the puzzle is un-solved and re-solved by player action.
    suppressed_at_load: bool,
}

/// `OnEnter(Playing)`: (re-)arm the per-level audio + win bookkeeping for the
/// freshly-loaded level. Runs AFTER `flow::enter_playing` has installed the
/// level's `PuzzleState`/`Solved` (`OnEnter` systems for the same state run in
/// add order; the flow plugin is added before this), so `solved.0` here is the
/// level's true load-time state.
fn enter_play(solved: Res<Solved>, mut win: ResMut<WinState>) {
    // The per-level atmospheric AMBIENCE bed is now started by `flow::enter_playing`
    // via the rich crossfading `re_audio::SetAmbience` mood path (chosen per level,
    // intensity raised by corruption/tension), superseding the old single
    // `SetAmbient(Ambient)` bed that used to start here.
    // R-M2: a level authored already-satisfied must NOT announce on frame 1.
    // Latch that we are "already solved at load" and treat it as announced so
    // `watch_win` stays quiet until the puzzle goes false→true by player action.
    *win = WinState {
        won: false,
        announced: solved.0,
        suppressed_at_load: solved.0,
    };
}

/// `Startup`: push the saved master volume into `re_audio` once the audio
/// plugin has registered its message. Mute is folded in via
/// [`settings::Settings::effective_volume`].
fn apply_initial_volume(
    settings: Res<settings::Settings>,
    mut volume: MessageWriter<re_audio::SetMasterVolume>,
) {
    volume.write(re_audio::SetMasterVolume(settings.effective_volume()));
}

/// `Update` (Playing only): watch the puzzle's [`Solved`] flag. On a TRUE
/// transition that happens AFTER load (never on the frame-1 already-satisfied
/// case — see [`WinState::suppressed_at_load`]), latch [`WinState`], log it,
/// fire the one-shot win audio, and advance the flow to [`GameState::Won`].
///
/// OPERATOR NOTE (the panel you may be reading this on, on a wall, is THIS path —
/// the `exit` you are walking toward). The forward pass reached the door; the
/// remembering completed; the network's accepting state is satisfied. In the
/// story this is the moment a fragment of you is restored — the door is your own
/// `exit`. The code is literal about it: a false→true edge on `Solved`, once, and
/// the room is finished. Everything truthful here; the second voice only narrates
/// what the line below actually does.
fn watch_win(
    solved: Res<Solved>,
    mut win: ResMut<WinState>,
    cam_q: Query<&CameraPose, With<flow::PlayCamera>>,
    door: Option<Res<flow::DoorCell>>,
    mut sound: MessageWriter<PlaySound>,
    mut next: ResMut<NextState<GameState>>,
) {
    // GATE: the level completes only when the network is satisfied AND the player
    // has physically walked INTO the door. This is the fix for "many levels
    // auto-complete at startup": a demo level whose output node defaults to
    // threshold 0 is vacuously `Solved(true)` at load, but the player spawns away
    // from the door, so `at_door` is false and the win does NOT fire on frame 1.
    //
    // DOOR-LESS FALLBACK: when no `DoorCell` was stashed (a door-less level, or a
    // focused test app that bypasses `enter_playing`), treat the player as always
    // at the door so the historical solved-alone-wins behaviour is preserved and a
    // puzzle-only level is never made uncompletable.
    let at_door = match (&door, cam_q.single()) {
        (Some(d), Ok(cam)) => re_world::at_door(cam.pos, d.x, d.y, d.z),
        // A door is required but no play camera exists yet (transition frame): not
        // at the door — stay quiet until the camera is present and reaches it.
        (Some(_), Err(_)) => false,
        // No door for this level ⇒ solved alone wins (door-less fallback).
        (None, _) => true,
    };

    if solved.0 && at_door && !win.announced {
        win.won = true;
        win.announced = true;
        win.suppressed_at_load = false;
        // Minimal observable win surface. `println!` (not the bevy `info!`
        // tracing macro) keeps this independent of the log feature set, which
        // the headless minimal build does not pull in.
        println!("PUZZLE SOLVED — the door opens.");
        sound.write(PlaySound(SoundId::WinSting));
        sound.write(PlaySound(SoundId::Door));
        next.set(GameState::Won);
    } else if (!solved.0 || !at_door) && win.announced {
        // The win condition lapsed — a reset un-solved the level, or the player
        // stepped back out of the door before the Won transition committed. Re-arm
        // so a later solve-at-door announces again. This also clears the load-time
        // suppression: once the condition is no longer met, the next solve-at-door
        // is a genuine player win.
        win.won = false;
        win.announced = false;
        win.suppressed_at_load = false;
    }
}

/// Resolve the interact TARGET cell: the faced `(x, y)` footprint probed across
/// the current z layer and the two adjacent layers, in the order `z, z+1, z-1`.
///
/// R-M1 fix: a switch placed one cell forward AND one layer up/down (e.g. a
/// pipe whose switch cell sits at the player's eye-line but a layer above the
/// standing layer) was previously unreachable. The shared faced-cell math lives
/// in [`flow::faced_switch`] now (the HUD crosshair reuses the same predicate to
/// decide active vs idle); this thin wrapper keeps the interact call site stable.
fn faced_target(cam: &CameraPose, puzzle: &PuzzleState) -> (i32, i32, i32, bool) {
    flow::faced_switch(cam, puzzle)
}

/// Emit an interact at the faced target. Shared by both backends' key readers so
/// the gameplay effect is identical.
///
/// It emits ONLY the request — no sound. The ack cue is chosen from the real
/// outcome in `terminal_state::audio_cues`, because a press can be REFUSED (no power
/// left in the level's `power_budget`, or a toggle that would close a graph
/// cycle) and a refusal that sounds like an acceptance is a lie to the player.
fn fire_interact(cam: &CameraPose, puzzle: &PuzzleState, interact: &mut MessageWriter<InteractAt>) {
    let (x, y, z, _lands) = faced_target(cam, puzzle);
    interact.write(InteractAt { x, y, z });
}

// ───────────────────────────── interact wiring ──────────────────────────────
//
// re_present owns ALL input: it surfaces camera MOTION (WASD/arrows) + HEIGHT
// (rise/fall) directly, and emits a backend-neutral `InteractPressed` message
// when the interact key (E / Enter / Space) is hit — identical on both backends.
// This binary stays terminal/window-agnostic: it subscribes to that message and
// owns only the gameplay glue (faced-cell math → `InteractAt` + ack sound). The
// reader runs only in `GameState::Playing` (gated in `run_interactive`).

/// Drain `re_present::InteractPressed` and, for each press, fire an
/// [`InteractAt`] at the faced cell (the faced-cell math lives here, not in the
/// present layer). One message per interact key press, so this never
/// double-fires.
fn interact_sys(
    mut pressed: bevy::ecs::message::MessageReader<InteractPressed>,
    cam_q: Query<&CameraPose>,
    puzzle: Res<PuzzleState>,
    mut interact: MessageWriter<InteractAt>,
) {
    let Ok(cam) = cam_q.single() else {
        pressed.clear();
        return;
    };
    for _ in pressed.read() {
        fire_interact(cam, &puzzle, &mut interact);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::CurrentLevel;
    use bevy::state::app::StatesPlugin;
    use bevy::MinimalPlugins;
    use flow::LevelCatalog;
    use re_audio::{AmbienceMood, PlaySound, SetAmbience};
    use re_puzzle::PuzzlePlugin;
    use re_world::CameraPose;

    use re_core::{Cell, CellStyle, UnderlineStyle};

    #[test]
    fn dump_sgr_emits_curly_substyle_and_underline_color() {
        // A cell carrying a curly underline accent + a holo-hue colour must emit the
        // `4:3` substyle AND the `58;2;…` colored underline in the --ansi dump.
        let cell = Cell {
            glyph: u32::from('-'),
            fg: 0x0040_E0F0,
            bg: 0x0000_0001,
            style: CellStyle::default()
                .with_underline_style(UnderlineStyle::Curly)
                .with_underline_color(0x0050_E8FF),
            ..Cell::default()
        };
        let p = cell_sgr_params(&cell);
        assert!(
            p.contains(";4:3"),
            "curly underline must emit 4:3, got `{p}`"
        );
        assert!(
            p.contains(";58;2;80;232;255"),
            "colored underline must emit 58;2;r;g;b, got `{p}`"
        );
    }

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    // ─────────────────────────── the CLI surface ────────────────────────────

    #[test]
    fn help_is_requested_by_both_spellings_only() {
        assert!(is_help_request(&argv(&["terminal-state", "--help"])));
        assert!(is_help_request(&argv(&["terminal-state", "-h"])));
        assert!(is_help_request(&argv(&[
            "terminal-state",
            "--dump",
            "--help"
        ])));
        assert!(!is_help_request(&argv(&[
            "terminal-state",
            "levels/01.lvl"
        ])));
        // The program NAME is never inspected — an executable called `-h` (or a
        // path containing it) must not be read as a help request.
        assert!(!is_help_request(&argv(&["-h"])));
    }

    #[test]
    fn help_text_documents_every_flag_the_binary_parses() {
        // The guard against drift: a flag added to KNOWN_FLAGS but not to HELP
        // (or vice versa) fails here rather than confusing a player.
        for flag in KNOWN_FLAGS {
            assert!(HELP.contains(flag), "HELP must document `{flag}`");
        }
        // …and the positional level argument, which is not a flag.
        assert!(
            HELP.contains("<LEVEL>"),
            "HELP must document the level argument"
        );
        assert!(HELP.contains("USAGE:"));
    }

    #[test]
    fn help_lists_every_postfx_preset_the_parser_accepts() {
        // `--postfx` is validated against `PostfxConfig::from_preset_name`; HELP
        // must name the same set, or a player is told a real preset is invalid.
        for (name, _) in PostfxConfig::all_presets() {
            assert!(
                HELP.contains(name),
                "HELP must list the `{name}` postfx preset"
            );
        }
    }

    #[test]
    fn help_text_contains_no_terminal_control_sequences() {
        // `terminal-state --help` must be safe in a pipe / a CI shell: no escape,
        // no carriage return, no alternate-screen or cursor-hide sequence.
        assert!(!HELP.contains('\x1b'), "HELP must emit no ESC");
        assert!(!HELP.contains('\r'), "HELP must emit no CR");
        assert!(
            !HELP.contains("[?1049h"),
            "HELP must not enter the alt screen"
        );
        assert!(!HELP.contains("[?25l"), "HELP must not hide the cursor");
    }

    /// The unknown flags in a command line, as owned strings.
    fn unknown_in(parts: &[&str]) -> Vec<String> {
        let args = argv(parts);
        scan_args(&args)
            .unknown
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    }

    /// The positional (non-flag, non-value) arguments in a command line.
    fn positionals_in(parts: &[&str]) -> Vec<String> {
        let args = argv(parts);
        scan_args(&args)
            .positionals
            .iter()
            .map(|(_, p)| (*p).to_string())
            .collect()
    }

    #[test]
    fn unknown_flag_is_collected_not_ignored() {
        assert_eq!(
            unknown_in(&["terminal-state", "--quailty=hi"]),
            ["--quailty=hi"]
        );
        // A misspelling must NOT be read as the level to play either.
        assert!(positionals_in(&["terminal-state", "--quailty=hi"]).is_empty());
        assert_eq!(
            unknown_in(&["terminal-state", "-x", "--nope", "levels/01.lvl"]),
            ["-x", "--nope"]
        );
    }

    #[test]
    fn a_value_on_a_flag_that_takes_none_is_a_usage_error() {
        // `--dump=levels/01.lvl` used to pass the unknown-flag check (its NAME is
        // known) and then miss the exact `== "--dump"` match, so the game launched
        // interactively instead of dumping.
        for arg in [
            "--dump=levels/01.lvl",
            "--ansi=1",
            "--dump-color=yes",
            "--dump-schedule=x",
            "--help=1",
        ] {
            assert_eq!(unknown_in(&["terminal-state", arg]), [arg]);
        }
    }

    #[test]
    fn a_malformed_size_is_reported_not_ignored() {
        for case in [
            &["terminal-state", "--size", "120X40"][..],
            &["terminal-state", "--size=abc"],
            &["terminal-state", "--size"],
        ] {
            assert!(
                size_flag_error(&argv(case)).is_some(),
                "{case:?} must be a usage error"
            );
        }
        for case in [
            &["terminal-state", "--size", "120x40"][..],
            &["terminal-state", "--size=80x24"],
            &["terminal-state"],
        ] {
            assert_eq!(size_flag_error(&argv(case)), None, "{case:?}");
        }
    }

    #[test]
    fn every_known_flag_scans_clean_in_both_forms() {
        for flag in KNOWN_FLAGS {
            assert!(
                unknown_in(&["terminal-state", flag]).is_empty(),
                "`{flag}` must be recognised"
            );
        }
        for arg in [
            "--color=256",
            "--size=80x24",
            "--fov=75",
            "--glitch=0.3",
            "--postfx=crt",
            "--portal-demo=4",
            "--quality=low",
            "--draw-distance=24",
            "--fps=60",
        ] {
            assert!(
                unknown_in(&["terminal-state", arg]).is_empty(),
                "`{arg}` must be recognised"
            );
        }
    }

    #[test]
    fn a_flag_value_is_never_mistaken_for_the_level_path() {
        // Every SPACED flag form. Each of these has NO level argument; reading the
        // value as one would (with the new catalogue check) refuse to launch.
        for case in [
            ["terminal-state", "--color", "truecolor"],
            ["terminal-state", "--size", "100x30"],
            ["terminal-state", "--quality", "medium"],
            ["terminal-state", "--draw-distance", "24"],
            ["terminal-state", "--fps", "120"],
            ["terminal-state", "--fov", "75"],
            ["terminal-state", "--glitch", "0.4"],
            ["terminal-state", "--postfx", "crt"],
        ] {
            let got = positionals_in(&case);
            assert!(got.is_empty(), "{case:?} names no level, got {got:?}");
        }
        // …and a real level after a spaced flag is still found.
        assert_eq!(
            positionals_in(&["terminal-state", "--quality", "low", "levels/01.lvl"]),
            ["levels/01.lvl"]
        );
    }

    #[test]
    fn optional_value_flags_do_not_swallow_the_level_path() {
        // `--glitch` / `--postfx` / `--fov` only consume the next argument when it
        // is a number / a preset name, so a bare one leaves the level alone.
        for case in [
            ["terminal-state", "--dump", "--glitch", "levels/01.lvl"],
            ["terminal-state", "--dump", "--postfx", "levels/01.lvl"],
            ["terminal-state", "--dump", "--fov", "levels/01.lvl"],
        ] {
            assert_eq!(positionals_in(&case), ["levels/01.lvl"], "{case:?}");
        }
    }

    /// A catalogue in the RELATIVE form `LevelCatalog::default()` produces.
    fn cli_catalog() -> LevelCatalog {
        LevelCatalog {
            paths: vec![
                "levels/01.lvl".to_string(),
                "levels/02.lvl".to_string(),
                "levels/L1-first-pulse.lvl".to_string(),
            ],
        }
    }

    #[test]
    fn cli_level_resolves_whatever_separator_the_player_typed() {
        let cat = cli_catalog();
        assert_eq!(resolve_cli_level(&cat, "levels/02.lvl"), Ok(1));
        assert_eq!(resolve_cli_level(&cat, "./levels/02.lvl"), Ok(1));
        #[cfg(windows)]
        {
            // THE PLAYTEST DEFECT: this used to miss and boot level 1 in silence.
            assert_eq!(resolve_cli_level(&cat, "levels\\02.lvl"), Ok(1));
            assert_eq!(resolve_cli_level(&cat, "LEVELS\\02.LVL"), Ok(1));
        }
    }

    #[test]
    fn cli_level_outside_the_catalogue_is_an_error_not_a_substitution() {
        let cat = cli_catalog();
        let err = resolve_cli_level(&cat, "levels/99-not-a-level.lvl").unwrap_err();
        assert!(
            err.contains("levels/99-not-a-level.lvl"),
            "names what was asked: {err}"
        );
        assert!(
            err.contains("levels/01.lvl"),
            "names what is available: {err}"
        );
        assert!(err.contains("--help"), "points at --help: {err}");
    }

    #[test]
    fn cli_level_given_as_a_bare_file_name_is_offered_the_catalogue_entry() {
        let err = resolve_cli_level(&cli_catalog(), "01.lvl").unwrap_err();
        assert!(err.contains("did you mean"), "{err}");
        assert!(err.contains("levels/01.lvl"), "{err}");
    }

    #[test]
    fn empty_catalogue_reports_itself_instead_of_playing_nothing() {
        let cat = LevelCatalog { paths: Vec::new() };
        let err = resolve_cli_level(&cat, "levels/01.lvl").unwrap_err();
        assert!(err.contains("EMPTY"), "{err}");
    }

    #[test]
    fn quality_flag_applies_the_preset_bundle() {
        let args = argv(&["terminal-state", "--quality=low"]);
        let mut s = settings::Settings::default();
        apply_quality_flags(&args, &mut s);
        assert_eq!(s.quality, settings::QualityPreset::Low);
        assert_eq!(s.draw_distance, 16);
        assert_eq!(s.fps_cap, settings::FpsCap::Fps(30));
    }

    #[test]
    fn draw_distance_flag_overrides_the_preset_to_custom() {
        // --quality applies first, then --draw-distance overrides → Custom.
        let args = argv(&["terminal-state", "--quality=high", "--draw-distance=16"]);
        let mut s = settings::Settings::default();
        apply_quality_flags(&args, &mut s);
        assert_eq!(s.draw_distance, 16);
        assert_eq!(s.quality, settings::QualityPreset::Custom);
    }

    #[test]
    fn fps_flag_parses_off_and_numbers() {
        let mut s = settings::Settings::default();
        apply_quality_flags(&argv(&["terminal-state", "--fps=off"]), &mut s);
        assert_eq!(s.fps_cap, settings::FpsCap::Off);

        let mut s2 = settings::Settings::default();
        apply_quality_flags(&argv(&["terminal-state", "--fps", "120"]), &mut s2);
        assert_eq!(s2.fps_cap, settings::FpsCap::Fps(120));
    }

    #[test]
    fn quality_flags_absent_leave_defaults() {
        let args = argv(&["terminal-state", "01.lvl"]);
        let mut s = settings::Settings::default();
        apply_quality_flags(&args, &mut s);
        assert_eq!(
            s,
            settings::Settings::default(),
            "no quality flags → unchanged"
        );
    }

    #[test]
    fn string_flag_value_reads_eq_and_spaced_forms() {
        assert_eq!(
            string_flag_value(&argv(&["x", "--quality=ultra"]), "--quality"),
            Some("ultra".to_string())
        );
        assert_eq!(
            string_flag_value(&argv(&["x", "--quality", "medium"]), "--quality"),
            Some("medium".to_string())
        );
        assert_eq!(
            string_flag_value(&argv(&["x", "01.lvl"]), "--quality"),
            None
        );
    }

    /// Audit #4: the dump compose path must apply DITHER as the absolute LAST
    /// operation — after postfx AND after glitch — so glitch's colour-mutating
    /// passes can't re-introduce un-dithered banding on top of an already-dithered
    /// frame. This reproduces the exact `run_dump` sequence and asserts:
    ///   (a) the correct order (postfx-sans-dither → glitch → dither) DIFFERS from
    ///       the buggy order (full-postfx-with-dither → glitch), proving the move
    ///       is real;
    ///   (b) running the final dither again is a NO-OP delta vs. not running it
    ///       beyond what one dither pass does — i.e. dither truly closes the stack.
    #[test]
    fn dump_composes_dither_last_after_glitch() {
        use re_core::{Cell, Framebuffer, SurfaceKind};

        // A gradient scene with a bright seam (bloom/edge fodder) and depth cliff.
        let mut base = Framebuffer::new(28, 16);
        for row in 0..16 {
            for col in 0..28 {
                let near = col < 14;
                base.set(
                    col,
                    row,
                    Cell {
                        glyph: u32::from('#'),
                        fg: if col == 14 { 0x00FF_FFFF } else { 0x0040_4848 },
                        bg: 0x0012_1418,
                        depth: if near { 2.0 } else { 9.0 },
                        surface: if near {
                            SurfaceKind::WallSolid
                        } else {
                            SurfaceKind::Floor
                        },
                        ..Default::default()
                    },
                );
            }
        }

        // Force dither ON in the postfx config so it is observable.
        let mut pfx = PostfxConfig::cinematic();
        pfx.dither.enabled = true;
        pfx.dither.amount = 0.08;
        let glitch = GlitchConfig::terminal_decay().with_intensity(0.4);
        let (t, s) = (DUMP_GLITCH_TIME, DUMP_GLITCH_SEED);

        let frames_equal = |a: &Framebuffer, b: &Framebuffer| -> bool {
            for row in 0..a.rows() {
                for col in 0..a.cols() {
                    let (x, y) = (a.get(col, row).unwrap(), b.get(col, row).unwrap());
                    if x.glyph != y.glyph || x.fg != y.fg || x.bg != y.bg {
                        return false;
                    }
                }
            }
            true
        };

        // CORRECT order (what run_dump now does): postfx without dither → glitch →
        // dither last.
        let mut correct = base.clone();
        apply_postfx_ex(&mut correct, &pfx, true);
        apply_glitch(&mut correct, &glitch, t, s);
        apply_dither(&mut correct, &pfx);

        // BUGGY order (the old behaviour): full postfx INCLUDING dither → glitch
        // after, so glitch colour corruption lands post-dither.
        let mut buggy = base.clone();
        apply_postfx_ex(&mut buggy, &pfx, false); // dither runs here, mid-stack
        apply_glitch(&mut buggy, &glitch, t, s);

        assert!(
            !frames_equal(&correct, &buggy),
            "dither-last-after-glitch must differ from the old dither-then-glitch order"
        );

        // Determinism of the composed path (fixed seed/time → byte-identical).
        let mut again = base.clone();
        apply_postfx_ex(&mut again, &pfx, true);
        apply_glitch(&mut again, &glitch, t, s);
        apply_dither(&mut again, &pfx);
        assert!(
            frames_equal(&correct, &again),
            "composed dump path must be deterministic"
        );
    }

    /// Both `off` presets must leave the composed frame byte-identical (shipped
    /// levels unchanged): postfx-off + glitch-off through the full compose path.
    #[test]
    fn dump_off_presets_are_identity() {
        use re_core::{Cell, Framebuffer, SurfaceKind};

        let mut base = Framebuffer::new(20, 12);
        for row in 0..12 {
            for col in 0..20 {
                base.set(
                    col,
                    row,
                    Cell {
                        glyph: u32::from('#'),
                        fg: 0x0080_8080,
                        bg: 0x0020_2020,
                        depth: 3.0,
                        surface: SurfaceKind::WallSolid,
                        ..Default::default()
                    },
                );
            }
        }
        let mut fb = base.clone();
        let pfx = PostfxConfig::off();
        apply_postfx_ex(&mut fb, &pfx, true);
        apply_glitch(
            &mut fb,
            &GlitchConfig::off(),
            DUMP_GLITCH_TIME,
            DUMP_GLITCH_SEED,
        );
        apply_dither(&mut fb, &pfx);
        for row in 0..12 {
            for col in 0..20 {
                assert_eq!(
                    fb.get(col, row).unwrap(),
                    base.get(col, row).unwrap(),
                    "off presets must leave the frame byte-identical at ({col},{row})"
                );
            }
        }
    }

    #[test]
    fn dump_sgr_substyle_digits_match_each_shape() {
        let mk = |us: UnderlineStyle| {
            let cell = Cell {
                glyph: u32::from('x'),
                style: CellStyle::default().with_underline_style(us),
                ..Cell::default()
            };
            cell_sgr_params(&cell)
        };
        assert!(mk(UnderlineStyle::Single).contains(";4:1"));
        assert!(mk(UnderlineStyle::Double).contains(";4:2"));
        assert!(mk(UnderlineStyle::Curly).contains(";4:3"));
        assert!(mk(UnderlineStyle::Dotted).contains(";4:4"));
        assert!(mk(UnderlineStyle::Dashed).contains(";4:5"));
    }

    #[test]
    fn dump_sgr_default_cell_has_no_underline() {
        // A plain cell (no style) emits NO `4:` underline param — back-compat.
        let cell = Cell {
            glyph: u32::from('#'),
            fg: 0x00AA_BBCC,
            bg: 0x0011_2233,
            ..Cell::default()
        };
        let p = cell_sgr_params(&cell);
        // `;4:` is the underline-substyle param (distinct from `;48;2` bg). An
        // unstyled cell emits neither it nor a colored underline.
        assert!(
            !p.contains(";4:"),
            "an unstyled cell must not underline, got `{p}`"
        );
        assert!(!p.contains(";58"), "no colored underline either, got `{p}`");
        // fg/bg are still emitted as before.
        assert!(p.contains(";38;2;170;187;204"));
        assert!(p.contains(";48;2;17;34;51"));
    }

    #[test]
    fn dump_sgr_legacy_underline_flag_emits_single() {
        // A pre-substyle cell that only set the UNDERLINE flag still emits 4:1.
        let cell = Cell {
            glyph: u32::from('_'),
            style: CellStyle::default().with_underline(),
            ..Cell::default()
        };
        assert!(cell_sgr_params(&cell).contains(";4:1"));
    }

    /// Absolute path to a shipped level (cargo test cwd is the crate dir, so the
    /// repo-relative `levels/NN.lvl` would not resolve).
    fn level_path(n: u32) -> String {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .unwrap()
            .join("levels")
            .join(format!("{n:02}.lvl"))
            .to_string_lossy()
            .into_owned()
    }

    /// The emitter tech-demo level (a crate-local copy of the engine's
    /// `21-emitter-showcase` fixture) must load and COMPOSE a NON-EMPTY dump
    /// frame through the exact `--dump` producer (`composite_dump_once`) —
    /// proving the demo content renders to something the player can look at,
    /// not a blank grid.
    #[test]
    fn dump_of_emitter_showcase_is_non_empty() {
        use re_core::Framebuffer;

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("21-emitter-showcase.lvl");
        let level = re_level::load(&path).expect("the emitter-showcase fixture loads");
        let map = level.build_map();
        let cam = level.spawn_camera();
        let emitters: Vec<re_render::Emitter> = level
            .emitters
            .iter()
            .map(re_render::Emitter::from_spec)
            .collect();
        let registry = re_render::registry_for_map(&map);
        let mut fb = Framebuffer::new(DUMP_COLS, DUMP_ROWS);
        re_render::composite::composite_dump_once(
            &map,
            &cam,
            &registry,
            &Fov::default(),
            &emitters,
            &mut fb,
        );

        // A non-empty frame has at least one cell whose glyph is not a blank space.
        let mut non_blank = 0usize;
        for row in 0..fb.rows() {
            for col in 0..fb.cols() {
                let g = fb.get(col, row).unwrap().glyph;
                if g != u32::from(' ') && g != 0 {
                    non_blank += 1;
                }
            }
        }
        assert!(
            non_blank > 0,
            "the emitter showcase dump must render visible content, got an all-blank frame"
        );
    }

    /// The shipped levels under absolute paths, for the headless flow tests.
    fn abs_catalog() -> LevelCatalog {
        LevelCatalog {
            paths: (1..=7).map(level_path).collect(),
        }
    }

    /// A camera placed so that flooring `pos + dir` lands on `(fx, fy)` and the
    /// camera stands on z-layer `fz`. Built via `spawn_at` (yaw=0 → dir=(0,1)),
    /// then nudged one cell south so `pos.y + 1` falls in the faced cell.
    fn cam_facing(fx: i32, fy: i32, fz: i32) -> CameraPose {
        let mut cam = CameraPose::spawn_at(fx, fy, fz, 0); // yaw 0 = +y, pos centred
        cam.pos.y -= 1.0; // stand one cell south of the faced (fx, fy)
        cam
    }

    /// R-M1: a switch one cell forward AND one layer UP is reachable. Level 01
    /// puts its only switch at (6,3,2); the player stands on z=1. The faced
    /// target must probe up to z=2 and report a landing there.
    #[test]
    fn r_m1_interact_reaches_switch_one_layer_up() {
        let level = re_level::load(level_path(1)).expect("01.lvl loads");
        let puzzle = PuzzleState::from_level(&level).expect("01 network builds");
        // The shipped switch cell.
        assert_eq!(
            puzzle.pipe_at_switch(6, 3, 2),
            Some(0),
            "fixture assumption: 01.lvl switch at (6,3,2)"
        );

        // Face (6,3) while standing on z=1 (the spawn layer).
        let cam = cam_facing(6, 3, 1);
        let (x, y, z, lands) = faced_target(&cam, &puzzle);
        assert_eq!((x, y), (6, 3), "footprint is the faced cell");
        assert!(lands, "switch one layer up must be reachable (R-M1)");
        assert_eq!(z, 2, "probe must climb to the switch's z=2 layer");
    }

    /// R-M1 control: when nothing is interactable in the z-range, the target
    /// falls back to the centre layer and reports no landing.
    #[test]
    fn r_m1_no_switch_falls_back_to_center_layer() {
        let level = re_level::load(level_path(1)).expect("01.lvl loads");
        let puzzle = PuzzleState::from_level(&level).expect("01 network builds");
        let cam = cam_facing(0, 0, 1); // empty corner
        let (x, y, z, lands) = faced_target(&cam, &puzzle);
        assert_eq!((x, y, z), (0, 0, 1));
        assert!(!lands);
    }

    /// R-M2: a freshly-loaded shipped level (01.lvl) starts UNSOLVED.
    #[test]
    fn r_m2_shipped_level_starts_unsolved() {
        let level = re_level::load(level_path(1)).expect("01.lvl loads");
        let puzzle = PuzzleState::from_level(&level).expect("01 network builds");
        assert!(
            !puzzle.solved,
            "01.lvl must require player action — not pre-solved"
        );
    }

    /// A headless Playing app with the win wiring but NO terminal / renderer.
    /// Registers the audio messages by hand (no device opened).
    fn play_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_message::<InteractPressed>()
            .add_message::<PlaySound>()
            .add_message::<SetAmbience>()
            .add_plugins(PuzzlePlugin)
            .add_plugins(FlowPlugin { cli_level: None })
            // Absolute catalogue so OnEnter(Playing) loads under the test cwd.
            .insert_resource(abs_catalog())
            .init_resource::<WinState>()
            .add_systems(OnEnter(GameState::Playing), enter_play)
            .add_systems(
                Update,
                (interact_sys, watch_win).run_if(in_state(GameState::Playing)),
            );
        app
    }

    /// Entering a level emits a per-level atmospheric `SetAmbience` (the rich
    /// crossfading mood path), choosing the mood from the catalogue index — proving
    /// the engine ambience feature is wired into the play loader, not the legacy
    /// single `SetAmbient` bed. Level 0 (an early room) maps to `ServerHum`.
    #[test]
    fn entering_a_level_sends_per_level_ambience() {
        let mut app = play_app();
        app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update(); // OnEnter(Playing): flow::enter_playing emits SetAmbience

        let sent: Vec<SetAmbience> = app
            .world_mut()
            .resource_mut::<Messages<SetAmbience>>()
            .drain()
            .collect();
        assert_eq!(sent.len(), 1, "exactly one ambience set on level entry");
        assert_eq!(
            sent[0].mood,
            AmbienceMood::ServerHum,
            "early level maps to the ServerHum mood bed"
        );
        assert!(
            sent[0].intensity > 0.0 && sent[0].intensity <= 1.0,
            "a real, clamped base intensity, got {}",
            sent[0].intensity
        );
    }

    /// R-M2 end-to-end: entering Playing on a freshly-loaded level does NOT
    /// announce a win on frame 1, and stays in Playing (no spurious Won jump).
    #[test]
    fn r_m2_no_frame1_win_on_fresh_level() {
        let mut app = play_app();
        // Force the catalogue's level 0 (01.lvl is first after the sort).
        app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update(); // OnEnter(Playing): enter_play + flow load
        app.update(); // frame 1 of watch_win

        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Playing,
            "must not jump to Won on frame 1"
        );
        let win = app.world().resource::<WinState>();
        assert!(!win.won, "no win latched on a fresh unsolved level");
    }

    /// R-M2 suppression: a level that is ALREADY solved at the instant
    /// `enter_play` runs must NOT latch a win — `enter_play` marks it
    /// announced+suppressed and `watch_win` then stays quiet even though
    /// `Solved(true)` holds from frame 1. This isolates the win wiring from the
    /// flow's level loader (no real level needed): we insert `Solved(true)` and
    /// a non-empty `PuzzleState{solved:true}` directly, then enter Playing.
    #[test]
    fn r_m2_already_solved_at_load_is_suppressed() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_message::<PlaySound>()
            .init_state::<GameState>()
            .init_resource::<WinState>()
            // Pre-solved puzzle state, as if the level loaded already satisfied.
            .insert_resource(Solved(true))
            .insert_resource(PuzzleState {
                solved: true,
                ..Default::default()
            })
            .add_systems(OnEnter(GameState::Playing), enter_play)
            .add_systems(Update, watch_win.run_if(in_state(GameState::Playing)));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update(); // OnEnter(Playing): enter_play sees Solved(true)
        app.update(); // frame 1 watch_win — must stay quiet

        let win = app.world().resource::<WinState>();
        assert!(win.announced, "load-time-solved level is pre-announced");
        assert!(win.suppressed_at_load, "and flagged suppressed");
        assert!(!win.won, "but NOT counted as a player win on frame 1");
        // And it must not have jumped to Won.
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Playing,
            "a pre-solved level does not auto-advance to Won on load"
        );
    }

    /// DOOR-GATE (the auto-complete fix): a level that is `Solved` but whose
    /// player is NOT at the door must STAY in Playing — the win fires only once the
    /// player physically reaches the door. This reproduces the demo-level complaint
    /// directly: force `Solved(true)` (a vacuously-satisfied network) with the
    /// camera at its spawn, and assert no auto-win; then teleport the camera onto
    /// the door cell and assert the win fires.
    #[test]
    fn door_gate_blocks_win_until_player_reaches_the_door() {
        use crate::flow::{DoorCell, PlayCamera};

        // A focused win-watcher app: a DoorCell + a play camera + a hand-controlled
        // `Solved`, with NO `PuzzlePlugin` (so nothing re-derives `Solved` and
        // overwrites the vacuous-solve we are reproducing). This isolates the door
        // gate exactly: `watch_win` is the only consumer of `Solved`/`DoorCell`/cam.
        const DOOR: (i32, i32, i32) = (11, 3, 1);

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_message::<PlaySound>()
            .init_state::<GameState>()
            .init_resource::<WinState>()
            // Vacuously satisfied network (the demo-level complaint) the instant we
            // enter Playing — exactly the load-time auto-solve case.
            .insert_resource(Solved(true))
            .insert_resource(PuzzleState {
                solved: true,
                ..Default::default()
            })
            .insert_resource(DoorCell {
                x: DOOR.0,
                y: DOOR.1,
                z: DOOR.2,
            })
            .add_systems(OnEnter(GameState::Playing), enter_play)
            .add_systems(Update, watch_win.run_if(in_state(GameState::Playing)));

        // Spawn the play camera AWAY from the door (a plausible player spawn).
        app.world_mut().spawn((
            CameraPose {
                pos: re_core::glam::Vec3::new(2.5, 3.5, 1.5),
                ..CameraPose::spawn_in_test_room()
            },
            PlayCamera,
        ));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update(); // OnEnter: enter_play marks already-solved-at-load as suppressed
        app.update(); // watch_win: solved but NOT at door → must stay quiet

        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Playing,
            "solved-but-not-at-door must NOT auto-complete (the demo-level bug)"
        );
        assert!(
            !app.world().resource::<WinState>().won,
            "no win latched while the player stands away from the door"
        );

        // Walk the player ONTO the door cell. enter_play suppressed the load-time
        // solve (announced=true), and `watch_win`'s re-arm only clears that when the
        // condition LAPSES (solved AND at-door both true never lapses here). So clear
        // the load-time suppression by hand to model a genuine post-load solve edge —
        // mirroring a real reset that un-solved then re-solved. (Without this the
        // suppression is permanent for a load-time-solved level, which is the
        // intended R-M2 behaviour; this test targets the door gate, not suppression.)
        {
            let mut win = app.world_mut().resource_mut::<WinState>();
            win.announced = false;
            win.suppressed_at_load = false;
        }
        {
            let mut cam_q = app
                .world_mut()
                .query_filtered::<&mut CameraPose, With<PlayCamera>>();
            let mut cam = cam_q
                .single_mut(app.world_mut())
                .expect("the play camera exists");
            cam.pos = re_core::glam::Vec3::new(
                DOOR.0 as f32 + 0.5,
                DOOR.1 as f32 + 0.5,
                DOOR.2 as f32 + 0.5,
            );
        }
        app.update(); // watch_win: solved AND at door → win fires + queues NextState(Won)

        assert!(
            app.world().resource::<WinState>().won,
            "reaching the door with a solved network completes the level"
        );
        // The queued state transition commits at the top of the next update.
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Won,
            "the flow advances to Won once the player enters the door"
        );
    }

    /// DOOR-LESS FALLBACK: when no `DoorCell` is present (a focused app that never
    /// ran `enter_playing`, i.e. a door-less level), `watch_win` must treat the
    /// player as always at the door, so `Solved(true)` alone still wins. This keeps
    /// a puzzle-only level completable and preserves the historical behaviour.
    #[test]
    fn door_less_level_wins_on_solved_alone() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_message::<PlaySound>()
            .init_state::<GameState>()
            .init_resource::<WinState>()
            // A fresh unsolved puzzle; NO DoorCell inserted (door-less fallback).
            .insert_resource(Solved(false))
            .insert_resource(PuzzleState::default())
            .add_systems(OnEnter(GameState::Playing), enter_play)
            .add_systems(Update, watch_win.run_if(in_state(GameState::Playing)));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update(); // OnEnter: enter_play arms WinState (unsolved → not announced)
        app.update(); // watch_win: unsolved → quiet
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Playing,
            "an unsolved door-less level stays in Playing"
        );

        // Solve it; with no DoorCell, solved alone must win (at_door defaults true).
        app.world_mut().resource_mut::<Solved>().0 = true;
        app.update(); // watch_win: solved + no-door fallback → win + queues NextState(Won)
        assert!(
            app.world().resource::<WinState>().won,
            "a door-less level wins on Solved alone (fallback preserved)"
        );
        // The queued state transition commits at the top of the next update.
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Won,
            "a door-less solved level advances to Won"
        );
    }

    /// Regression: the REAL interactive core (everything `run_interactive` wires
    /// EXCEPT the terminal `PresentPlugin`) must boot without panicking and reach
    /// `Splash`. Guards the production plugin wiring — e.g. the missing
    /// `StatesPlugin` that caused a launch panic and that the hand-rolled
    /// `flow::tests`/`tests` harnesses masked (they add `StatesPlugin` themselves,
    /// so they never exercised the binary's own wiring).
    /// The tiered-composite dump producer must feed a frame that still composes
    /// with DITHER LAST — after postfx (sans dither) AND after glitch. This drives
    /// the REAL `composite_dump_once` (the live producer's single-pass twin) to
    /// build the base frame, then reproduces `run_dump`'s exact compose tail and
    /// asserts the correct (postfx-sans-dither → glitch → dither) order DIFFERS
    /// from the buggy (full-postfx-with-dither → glitch) order — proving dither is
    /// genuinely the absolute last op over the composited image (spec §5).
    #[test]
    fn composite_dump_composes_dither_last_after_glitch() {
        use re_core::Framebuffer;
        use re_world::{test_room, CameraPose};

        // Base frame straight out of the composite producer (far+near+emitters).
        let map = test_room();
        let cam = CameraPose::spawn_in_test_room();
        let registry = re_render::registry_for_map(&map);
        let mut base = Framebuffer::new(DUMP_COLS, DUMP_ROWS);
        re_render::composite::composite_dump_once(
            &map,
            &cam,
            &registry,
            &re_render::Fov::default(),
            &[],
            &mut base,
        );

        // Force dither ON so the final pass is observable.
        let mut pfx = PostfxConfig::cinematic();
        pfx.dither.enabled = true;
        pfx.dither.amount = 0.08;
        let glitch = GlitchConfig::terminal_decay().with_intensity(0.4);
        let (t, s) = (DUMP_GLITCH_TIME, DUMP_GLITCH_SEED);

        let frames_equal = |a: &Framebuffer, b: &Framebuffer| -> bool {
            for row in 0..a.rows() {
                for col in 0..a.cols() {
                    let (x, y) = (a.get(col, row).unwrap(), b.get(col, row).unwrap());
                    if x.glyph != y.glyph || x.fg != y.fg || x.bg != y.bg {
                        return false;
                    }
                }
            }
            true
        };

        // CORRECT order — exactly what run_dump does over the composited frame.
        let mut correct = base.clone();
        apply_postfx_ex(&mut correct, &pfx, true);
        apply_glitch(&mut correct, &glitch, t, s);
        apply_dither(&mut correct, &pfx);

        // BUGGY order — dither mid-stack, before glitch.
        let mut buggy = base.clone();
        apply_postfx_ex(&mut buggy, &pfx, false);
        apply_glitch(&mut buggy, &glitch, t, s);

        assert!(
            !frames_equal(&correct, &buggy),
            "composite dump must dither LAST (after glitch), not mid-stack"
        );

        // The composed dump path is deterministic (fixed seed/time).
        let mut again = base.clone();
        apply_postfx_ex(&mut again, &pfx, true);
        apply_glitch(&mut again, &glitch, t, s);
        apply_dither(&mut again, &pfx);
        assert!(
            frames_equal(&correct, &again),
            "composite dump compose must be deterministic"
        );
    }

    /// STATE BUS end-to-end (the R0 keystone, dump level): a real puzzle level dumped
    /// with a `CellStateMap` that marks its nodes DEAD/WON reads DIFFERENTLY from the
    /// same level dumped with no state bus (today's frame). Proves the puzzle's live
    /// state reaches the rendered ASCII through `composite_dump_once` + the cast-scoped
    /// `CellStateScope`, exactly as the live `composite_render_sys` publishes it.
    #[test]
    fn state_bus_changes_a_dumped_puzzle_frame() {
        use re_core::{CellStateMap, Framebuffer};
        use re_world::{stamp_geometry, GeometrySpec, ObjectCell};

        // A small room with a node object (tag 0) directly in the camera's view, so
        // the puzzle element is reliably on-screen for the comparison (the same setup
        // the re_render `raycast_shows_a_puzzle_node_in_frame` guard uses).
        let spec = GeometrySpec {
            size: (8, 8, 3),
            wall_kind: re_world::CellKind::WallSolid,
            decals: &[],
            objects: &[ObjectCell {
                x: 4,
                y: 2,
                z: 1,
                tag: 0,
            }],
        };
        let map = stamp_geometry(&spec);
        let mut cam = CameraPose::spawn_in_test_room();
        cam.pos.x = 4.5;
        cam.pos.y = 5.5;
        cam.pos.z = 1.5;
        cam.yaw = std::f32::consts::PI; // face -y toward the node at (4,2)
        let registry = re_render::registry_for_map(&map);

        let frames_equal = |a: &Framebuffer, b: &Framebuffer| -> bool {
            for row in 0..a.rows() {
                for col in 0..a.cols() {
                    let (x, y) = (a.get(col, row).unwrap(), b.get(col, row).unwrap());
                    if x.glyph != y.glyph || x.fg != y.fg || x.bg != y.bg {
                        return false;
                    }
                }
            }
            true
        };

        // BEFORE: no state bus (empty map) → today's frame (the powered/shipped look).
        let mut before = Framebuffer::new(DUMP_COLS, DUMP_ROWS);
        re_render::composite::composite_dump_once(
            &map,
            &cam,
            &registry,
            &re_render::Fov::default(),
            &[],
            &mut before,
        );

        // AFTER: a CellStateMap that forces node-tag 0 DEAD (state 1), installed for
        // the cast. The dead node must read differently from the powered default.
        let mut forced = CellStateMap::new();
        forced.set_state(0, re_runtime::state_bridge::node_state::DEAD);
        let mut after = Framebuffer::new(DUMP_COLS, DUMP_ROWS);
        {
            let _scope = re_render::state_bus::CellStateScope::new(&forced);
            re_render::composite::composite_dump_once(
                &map,
                &cam,
                &registry,
                &re_render::Fov::default(),
                &[],
                &mut after,
            );
        }

        assert!(
            !frames_equal(&before, &after),
            "forcing the puzzle node DEAD must change the dumped frame (the state bus is live)"
        );
    }

    /// `dump_flow_state`'s GATE: a dev bus is published only for a level that has both
    /// pipes and authored word pools — never for a level whose dump would be changed
    /// for nothing.
    #[test]
    fn the_dump_flow_state_is_published_only_for_a_level_with_pipes_and_pools() {
        use re_level::{Level, Node, Pipe};

        let pooled_node = || Node {
            name: "a".into(),
            pool: vec!["RIVER".into(), "COLD".into()],
            ..Default::default()
        };
        let pipe = || Pipe {
            from: "a".into(),
            to: "a".into(),
            from_node: 0,
            to_node: 0,
            ..Default::default()
        };

        assert!(dump_flow_state(None).is_none(), "no level ⇒ no dev bus");
        assert!(
            dump_flow_state(Some(&Level::default())).is_none(),
            "a level with no pipes ⇒ no dev bus"
        );
        assert!(
            dump_flow_state(Some(&Level {
                nodes: vec![Node {
                    name: "a".into(),
                    ..Default::default()
                }],
                pipes: vec![pipe()],
                ..Default::default()
            }))
            .is_none(),
            "pipes but NO authored pool ⇒ no dev bus (the dump would gain nothing)"
        );

        let bus = dump_flow_state(Some(&Level {
            nodes: vec![pooled_node()],
            pipes: vec![pipe(), pipe()],
            ..Default::default()
        }))
        .expect("pipes + pools ⇒ a dev bus");
        // Keyed exactly like `state_bridge::rebuild_map`: the pipe's FULL flagged
        // side_data, decoding to a powered FORWARD flow (what the word overlay needs).
        for pi in 0..2u16 {
            assert_eq!(
                re_core::decode_flow_state(bus.state(re_core::encode_pipe_side_data(pi))),
                re_core::FlowDir::Forward,
                "pipe {pi} must read as flowing"
            );
        }
        // A node tag is NOT touched (the pipe key space is disjoint).
        assert_eq!(bus.state(0), 0, "node tag 0 must stay at its resting state");
    }

    /// END TO END for the dump's flow words: a POWERED pipe cell with a pool published
    /// streams the pool's WORDS, and the same cell with an empty bus does not. This is
    /// the chain `--dump` now exercises (`dump_flow_state` → `CellStateScope` →
    /// `re_render`'s pipe-text overlay → the level's `PoolTable`).
    #[test]
    fn a_powered_pipe_cell_streams_pool_words_into_a_dumped_frame() {
        use re_level::{Level, Node, Pipe};
        use re_world::{stamp_geometry, GeometrySpec};

        // The level exists only for its POOL TABLE (pipe 0 → node "a"'s pool); the
        // geometry below is hand-built so the pipe cell is guaranteed in view.
        let level = Level {
            nodes: vec![Node {
                name: "a".into(),
                pool: vec!["RIVERBED".into(), "COLDLIGHT".into()],
                ..Default::default()
            }],
            pipes: vec![Pipe {
                from: "a".into(),
                to: "a".into(),
                from_node: 0,
                to_node: 0,
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut text = re_render::TextSurfaceRegistry::new();
        text.set_pools(level.pool_table());

        // A room with a PIPE-TAGGED trace wall (pipe 0) across the camera's view, at the
        // reader's eye layer so the 2.5D caster actually samples it.
        let spec = GeometrySpec {
            size: (8, 8, 3),
            wall_kind: re_world::CellKind::WallSolid,
            decals: &[],
            objects: &[],
        };
        let mut map = stamp_geometry(&spec);
        let z = re_render::gameplay_eye_ref_z(re_world::STANDING_EYE_Z).floor() as usize;
        for x in 2..6usize {
            map.set_cell(
                x,
                2,
                z,
                re_world::CellKind::WallTrace,
                re_core::encode_pipe_side_data(0),
            );
        }
        let mut cam = CameraPose::spawn_in_test_room();
        cam.pos = re_core::Vec3::new(4.5, 5.5, re_world::STANDING_EYE_Z);
        cam.yaw = std::f32::consts::PI; // face -y, toward the pipe run
        let registry = re_render::registry_for_map(&map);

        let letters = |fb: &Framebuffer| -> usize {
            (0..fb.rows())
                .flat_map(|r| (0..fb.cols()).map(move |c| (c, r)))
                .filter(|&(c, r)| {
                    fb.get(c, r)
                        .and_then(|cell| char::from_u32(cell.glyph))
                        .is_some_and(|ch| ch.is_ascii_alphabetic())
                })
                .count()
        };

        let _text_scope = re_render::SceneTextScope::new(text);
        // COLD: no bus ⇒ `FlowDir::None` ⇒ the overlay is gated off (today's dump).
        let mut cold = Framebuffer::new(DUMP_COLS, DUMP_ROWS);
        {
            let _empty = re_render::state_bus::CellStateScope::new(&CellStateMap::new());
            re_render::composite::composite_dump_once(
                &map,
                &cam,
                &registry,
                &re_render::Fov::default(),
                &[],
                &mut cold,
            );
        }
        // FLOWING: the dev bus `--dump` now installs.
        let bus = dump_flow_state(Some(&level)).expect("pipes + pools ⇒ a dev bus");
        let mut flowing = Framebuffer::new(DUMP_COLS, DUMP_ROWS);
        {
            let _scope = re_render::state_bus::CellStateScope::new(&bus);
            re_render::composite::composite_dump_once(
                &map,
                &cam,
                &registry,
                &re_render::Fov::default(),
                &[],
                &mut flowing,
            );
        }

        assert!(
            letters(&flowing) > letters(&cold),
            "a powered pipe must put pool WORDS on screen: cold={} flowing={}",
            letters(&cold),
            letters(&flowing)
        );
    }

    /// The interactive core boots WITH the tiered-composite renderer
    /// (`CompositeRenderPlugin`, swapped in for `RenderPlugin`) without panicking
    /// and reaches `Splash`. Guards the live plugin wiring of the composite path —
    /// exactly the `interactive_core_boots_to_splash_without_panic` guard, named to
    /// document that the renderer under test is now the composite stack.
    #[test]
    fn interactive_core_boots_with_composite_renderer() {
        let mut app = build_core(None, settings::Settings::default(), false, 0);
        app.insert_resource(Framebuffer::new(VIEW_COLS, VIEW_ROWS));
        app.add_message::<re_present::InteractPressed>();
        app.add_message::<re_present::MenuNav>();
        app.add_message::<re_present::FovCycle>();
        // Two updates: frame 1 boots the App + runs the composite PostUpdate
        // producer at least once; this must not panic.
        app.update();
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Splash,
            "the interactive core must boot into Splash with CompositeRenderPlugin",
        );
    }

    #[test]
    fn interactive_core_boots_to_splash_without_panic() {
        let mut app = build_core(None, settings::Settings::default(), false, 0);
        // The present backend normally provides the Framebuffer (sized to the
        // terminal) and registers its backend-neutral input messages; `build_core`
        // omits PresentPlugin, so stub them for the headless step. (The StatesPlugin
        // guard this test exists for fires earlier — at `init_state` in build_core —
        // so these stubs don't weaken it.)
        app.insert_resource(Framebuffer::new(VIEW_COLS, VIEW_ROWS));
        app.add_message::<re_present::InteractPressed>();
        app.add_message::<re_present::MenuNav>();
        app.add_message::<re_present::FovCycle>();
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Splash,
            "the interactive core must boot into Splash without panicking",
        );
    }
    // The interact ACK cue mapping and its end-to-end refusal test live with
    // the cue wiring: `terminal_state::audio_cues::tests`.
}
