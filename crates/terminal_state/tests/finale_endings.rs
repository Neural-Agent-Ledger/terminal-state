//! **Both-endings scripted walk**: a scripted walkthrough of `L1`→`L12` for
//! BOTH endings, asserting state transitions, `MemoryLog` final content, and
//! save mutations.
//!
//! It drives the REAL production state machine —
//! `terminal_state::flow::FlowPlugin` + `terminal_state::menu::MenuRenderPlugin` on a headless
//! Bevy app with no terminal and no renderer — through the campaign's finale
//! twice, once per ending, and records:
//!
//! * the **state-transition log** (printed, and asserted as an exact sequence);
//! * the **painted frames** (the framebuffer read back as text, printed as the
//!   `--dump`-equivalent evidence and asserted on).
//!
//! Nothing is faked: the finale's `.lvl` is loaded by `re_level`, the pipes are
//! toggled through `re_puzzle::puzzle_interact` at their authored switch cells,
//! and the ending that fires is whatever `re_puzzle` actually settled on
//! (`PuzzleState::cycle_held`).
//!
//! # How to run
//! ```sh
//! cargo test -p terminal_state --test finale_endings -- --nocapture
//! ```

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::MinimalPlugins;

use re_core::Framebuffer;
use re_present::{InteractPressed, MenuNav};
use re_puzzle::{puzzle_interact, PuzzlePlugin, PuzzleState, Solved};
use terminal_state::flow::{
    campaign_manifest, CurrentLevel, CurrentLevelData, Ending, EndingOutcome, EndingTimer,
    FlowPlugin, GameState, LevelCatalog, MenuCursor, MenuItem, ENDING_EYE_WINDOW,
};
use terminal_state::menu::MenuRenderPlugin;
use terminal_state::save::Progress;
use terminal_state::story::{MemoryLog, ReadoutForm};
use terminal_state::{VIEW_COLS, VIEW_ROWS};

// ───────────────────────────── harness ──────────────────────────────────────

/// The workspace `levels/` directory, absolute (`cargo test` runs with the crate
/// dir as cwd, so the shipped relative catalogue would not resolve).
fn levels_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .join("levels")
}

/// The twelve campaign levels, absolute, in manifest order.
fn campaign_catalog() -> LevelCatalog {
    let root = levels_root();
    let paths = campaign_manifest()
        .levels()
        .iter()
        .map(|l| {
            let stem = std::path::Path::new(&l.path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            root.join(stem).to_string_lossy().replace('\\', "/")
        })
        .collect();
    LevelCatalog { paths }
}

/// Point `Progress::save` at a unique temp dir so a walk never touches the real
/// save file. Returns the directory so the test can read the file back.
fn isolate_save_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ts_w4walk_{tag}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::env::set_var("TS_SAVE_DIR", &dir);
    dir
}

/// A headless app with the real flow, the real puzzle plugin, and the real
/// overlay painters. No `re_present` backend ⇒ no terminal.
fn walk_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .add_message::<InteractPressed>()
        .add_plugins(PuzzlePlugin)
        .add_plugins(FlowPlugin { cli_level: None })
        .add_plugins(MenuRenderPlugin)
        .insert_resource(campaign_catalog())
        .insert_resource(Framebuffer::new(VIEW_COLS, VIEW_ROWS));
    app
}

fn state(app: &App) -> GameState {
    *app.world().resource::<State<GameState>>().get()
}

/// Set a state and apply the transition (running OnExit/OnEnter), recording it in
/// the walk log.
fn go(app: &mut App, log: &mut Vec<String>, s: GameState) {
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(s);
    app.update();
    log.push(format!("{:?}", state(app)));
}

/// Push a nav key, advance a frame, and record the resulting state.
fn key(app: &mut App, log: &mut Vec<String>, n: MenuNav) {
    app.world_mut().write_message(n);
    app.update();
    app.update(); // apply any queued transition
    log.push(format!("{:?}", state(app)));
}

/// Read the framebuffer back as text — the `--dump`-equivalent evidence.
fn frame(app: &App) -> String {
    let fb = app.world().resource::<Framebuffer>();
    (0..fb.rows())
        .map(|r| {
            let s: String = (0..fb.cols())
                .map(|c| {
                    fb.get(c, r)
                        .and_then(|cell| char::from_u32(cell.glyph))
                        .unwrap_or(' ')
                })
                .collect();
            s.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Toggle the pipe `from → to` (by node name) through the real interact seam.
fn toggle(app: &mut App, from: &str, to: &str) {
    let coord = {
        let data = app.world().resource::<CurrentLevelData>();
        let level = data.0.as_ref().expect("the finale is loaded");
        let idx = |n: &str| {
            level
                .nodes
                .iter()
                .position(|x| x.name == n)
                .unwrap_or_else(|| panic!("no node '{n}'"))
        };
        let (f, t) = (idx(from), idx(to));
        let puzzle = app.world().resource::<PuzzleState>();
        puzzle
            .pipes
            .iter()
            .find(|p| p.from == f && p.to == t)
            .and_then(|p| p.switch_coord())
            .unwrap_or_else(|| panic!("no switchable pipe {from}->{to}"))
    };
    let (x, y, z) = coord;
    let mut puzzle = app.world_mut().resource_mut::<PuzzleState>();
    assert!(
        puzzle_interact(&mut puzzle, x, y, z),
        "toggling {from}->{to} at its authored switch cell"
    );
}

/// Seed the save as if the player had solved L1..L11 with real phrases recorded —
/// so the journal the finale prints is a real accumulated story, and solving the
/// finale completes the campaign.
fn seed_campaign_up_to_the_finale(app: &mut App) {
    let mut p = app.world_mut().resource_mut::<Progress>();
    for l in campaign_manifest().levels() {
        if campaign_manifest().is_finale(&l.id) {
            continue;
        }
        p.record_phrase(&l.id, &format!("PHRASE-{}", l.id));
    }
}

/// Rebuild the in-memory `MemoryLog` from the seeded progress (Startup's
/// `hydrate_memory_log` already ran with an empty save).
fn hydrate(app: &mut App) {
    let entries: Vec<(String, String)> = {
        let p = app.world().resource::<Progress>();
        campaign_manifest()
            .levels()
            .iter()
            .filter_map(|l| p.phrase_for(&l.id).map(|ph| (l.id.clone(), ph.to_string())))
            .collect()
    };
    let mut log = app.world_mut().resource_mut::<MemoryLog>();
    log.entries.clear();
    for (level_id, phrase) in entries {
        log.record(terminal_state::story::MemoryEntry { level_id, phrase });
    }
}

/// Walk into the finale and commit the win. `wire` performs the branch's toggles.
fn play_the_finale(app: &mut App, log: &mut Vec<String>, wire: impl Fn(&mut App)) {
    let slot = campaign_manifest().len() - 1;
    app.world_mut().resource_mut::<CurrentLevel>().0 = slot;
    go(app, log, GameState::Playing);
    wire(app);
    app.update();
    assert!(
        app.world().resource::<PuzzleState>().solved,
        "the wiring must actually satisfy the finale's win condition"
    );
    // Mirror the win watcher (which additionally requires the player to be at the
    // door — a camera-position gate this headless walk does not simulate).
    app.world_mut().resource_mut::<Solved>().0 = true;
    go(app, log, GameState::Won);
    app.update(); // route_finale's queued transition lands
    log.push(format!("{:?}", state(app)));
}

// ═══════════════════════════ ENDING 1 — exit 0 ══════════════════════════════

/// Wire the outward door with recognition dark (`[door] target_off`), win, and
/// walk the release path all the way to the Menu.
#[test]
fn walk_the_exit0_ending_end_to_end() {
    let dir = isolate_save_dir("exit0");
    let mut app = walk_app();
    let mut log: Vec<String> = vec![format!("{:?}", state(&app))];
    seed_campaign_up_to_the_finale(&mut app);
    hydrate(&mut app);

    // L12: the level boots with
    // `gather->face->REC` already ON (REC lit). `exit 0` prunes the self-feed
    // (`face->REC`), then builds `unname` (T2 = gather + face) and routes it.
    play_the_finale(&mut app, &mut log, |a| {
        toggle(a, "face", "REC");
        toggle(a, "gather", "unname");
        toggle(a, "face", "unname");
        toggle(a, "unname", "EXIT");
    });

    // ── the ending that fired ────────────────────────────────────────────────
    assert_eq!(
        app.world().resource::<EndingOutcome>().0,
        Some(Ending::Exit0),
        "no kept cycle + inverted target satisfied == the release ending"
    );
    assert_eq!(state(&app), GameState::Ending);
    assert_eq!(
        app.world()
            .resource::<terminal_state::story::MemoryReadout>()
            .form,
        ReadoutForm::Clean,
        "the release readout is the clean primary pool"
    );

    // ── the end-card, painted ────────────────────────────────────────────────
    app.world_mut().resource_mut::<EndingTimer>().0 = ENDING_EYE_WINDOW;
    app.update();
    let card = frame(&app);
    println!("──── exit 0 end-card ────\n{card}\n─────────────────────────");
    assert!(card.contains("$ exit 0"), "{card}");
    assert!(
        card.contains("Process finished with exit code 0."),
        "{card}"
    );
    assert!(
        card.contains("pid://aleks_v@R·learn·A·edge·seen·named·bloom"),
        "the whole name is spent at the door: {card}"
    );
    assert!(
        card.contains("PHRASE-L1-first-pulse"),
        "the journal is printed: {card}"
    );
    assert!(
        !card.contains("BEGIN AGAIN"),
        "the release card offers no loop: {card}"
    );

    // ── the campaign is COMPLETE ─────────────────────────────────────────────
    assert!(
        campaign_manifest().is_complete(app.world().resource::<Progress>()),
        "the release ending completes the campaign"
    );

    // ── on to the SHARED epilogue, then the Menu ─────────────────────────────
    key(&mut app, &mut log, MenuNav::Confirm);
    assert_eq!(state(&app), GameState::Epilogue);
    let epi = frame(&app);
    println!("──── epilogue (release path) ────\n{epi}\n─────────────────────────");
    assert!(epi.contains("re-read Form A-7?"), "{epi}");

    key(&mut app, &mut log, MenuNav::Confirm);
    assert_eq!(state(&app), GameState::Menu);

    // ── the state-transition log ─────────────────────────────────────────────
    println!("exit 0 transition log: {log:?}");
    assert_eq!(
        log,
        vec!["Splash", "Playing", "Won", "Ending", "Epilogue", "Menu"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
    );

    // ── and the save state, through the on-disk format ───────────────────────
    // (explicit path — see the note in `the_loop_restart_keeps_every_phrase`).
    let path = dir.join("progress.toml");
    app.world().resource::<Progress>().save_to(&path);
    let saved = Progress::load_from(&path);
    assert!(
        saved.is_solved("L12-closing"),
        "the finale is recorded solved"
    );
    assert_eq!(
        saved.session, 0,
        "the release path never opens a new session"
    );
    assert_eq!(saved.campaign_cursor, None);
}

// ════════════════════════ ENDING 2 — while(true) ════════════════════════════

/// Close the ring so `cycle_keep` holds it, win, and walk the cage path — first
/// to the shared epilogue, then (in the second half) accepting the loop offer.
#[test]
fn walk_the_while_true_ending_end_to_end() {
    isolate_save_dir("loop");
    let mut app = walk_app();
    let mut log: Vec<String> = vec![format!("{:?}", state(&app))];
    seed_campaign_up_to_the_finale(&mut app);
    hydrate(&mut app);

    // L12: REC is already lit from the load
    // draw (`gather->face->REC`), so `while(true)` just closes the ring.
    play_the_finale(&mut app, &mut log, |a| {
        toggle(a, "REC", "MEM");
        toggle(a, "MEM", "REC");
    });

    assert_eq!(
        app.world().resource::<EndingOutcome>().0,
        Some(Ending::WhileTrue),
        "a kept cycle == the cage ending"
    );
    assert_eq!(state(&app), GameState::Ending);
    assert_eq!(
        app.world()
            .resource::<terminal_state::story::MemoryReadout>()
            .form,
        ReadoutForm::Corrupt,
        "the cage readout collapses into the pool_alt Babel"
    );

    app.world_mut().resource_mut::<EndingTimer>().0 = ENDING_EYE_WINDOW;
    app.update();
    let card = frame(&app);
    println!("──── while(true) end-card ────\n{card}\n─────────────────────────");
    assert!(
        card.contains("while(true) { remember(); forget(); }"),
        "{card}"
    );
    assert!(
        card.contains("pid://aleks_v@░·░·░·░·░·░·░"),
        "the name loses the language it was called right in: {card}"
    );
    assert!(card.contains("> attempts: 0002"), "{card}");
    assert!(card.contains("> resolving session 0001 ..."), "{card}");
    assert!(
        card.contains("SESSION 0002 — BEGIN AGAIN?"),
        "the loop is OFFERED: {card}"
    );
    assert!(
        !card.contains("exit code 0"),
        "no clean receipt on the cage card: {card}"
    );

    assert!(
        campaign_manifest().is_complete(app.world().resource::<Progress>()),
        "the cage ending completes the campaign too"
    );

    // The SAME epilogue furniture the release path saw.
    key(&mut app, &mut log, MenuNav::Confirm);
    assert_eq!(state(&app), GameState::Epilogue);
    let epi = frame(&app);
    println!("──── epilogue (cage path) ────\n{epi}\n─────────────────────────");
    assert!(epi.contains("re-read Form A-7?"), "{epi}");

    key(&mut app, &mut log, MenuNav::Confirm);
    assert_eq!(state(&app), GameState::Menu);

    println!("while(true) transition log: {log:?}");
    assert_eq!(
        log,
        vec!["Splash", "Playing", "Won", "Ending", "Epilogue", "Menu"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
    );

    // The MENU now carries the loop offer (campaign complete), and the Menu is
    // where the loop stays reachable without being imposed.
    app.update();
    let menu = frame(&app);
    println!("──── menu after the cage ending ────\n{menu}\n─────────────────────────");
    assert!(menu.contains("campaign complete"), "{menu}");
    assert!(menu.contains("session 0002 — begin again?"), "{menu}");
    assert!(
        terminal_state::flow::menu_items(app.world().resource::<Progress>())
            .contains(&MenuItem::BeginAgain)
    );
}

// ═════════════════════ the loop carries the memory ══════════════════════════

/// **The horror, asserted.** Accepting the loop offer opens session 0002 with the
/// cursor back on level 1 — and `solved` / `phrases` are byte-identical. The
/// system remembers even when "you" forget.
#[test]
fn the_loop_restart_keeps_every_phrase() {
    let dir = isolate_save_dir("loopkeep");
    let mut app = walk_app();
    let mut log: Vec<String> = vec![format!("{:?}", state(&app))];
    seed_campaign_up_to_the_finale(&mut app);
    hydrate(&mut app);

    // L12: REC is already lit from the load
    // draw (`gather->face->REC`), so `while(true)` just closes the ring.
    play_the_finale(&mut app, &mut log, |a| {
        toggle(a, "REC", "MEM");
        toggle(a, "MEM", "REC");
    });

    let (solved_before, phrases_before) = {
        let p = app.world().resource::<Progress>();
        (p.solved.clone(), p.phrases.clone())
    };
    assert_eq!(phrases_before.len(), 11, "eleven vignettes were recorded");

    // Reveal the readout, step onto the loop offer, accept it.
    app.world_mut().resource_mut::<EndingTimer>().0 = ENDING_EYE_WINDOW;
    key(&mut app, &mut log, MenuNav::Right);
    key(&mut app, &mut log, MenuNav::Confirm);
    assert_eq!(
        state(&app),
        GameState::Diving,
        "accepting the loop begins again"
    );

    let p = app.world().resource::<Progress>().clone();
    assert_eq!(p.session, 1, "the session increments");
    assert_eq!(p.session_label(), "0002");
    assert_eq!(p.campaign_cursor.as_deref(), campaign_manifest().first_id());
    assert_eq!(
        p.solved, solved_before,
        "`solved` is NEVER cleared by the loop"
    );
    assert_eq!(
        p.phrases, phrases_before,
        "`phrases` is NEVER cleared by the loop"
    );
    assert_eq!(
        app.world().resource::<CurrentLevel>().0,
        0,
        "the loop opens on the campaign's first level"
    );

    // And the same is true through the on-disk format. The round-trip is driven
    // through an EXPLICIT path rather than the env-resolved one: `TS_SAVE_DIR` is
    // process-global and the tests in this file run concurrently, so reading back
    // "wherever the game saved" would race. `save::tests` covers path resolution;
    // what matters here is that the mutated fields survive serialization.
    let path = dir.join("progress.toml");
    p.save_to(&path);
    let saved = Progress::load_from(&path);
    assert_eq!(saved.session, 1);
    assert_eq!(
        saved.campaign_cursor.as_deref(),
        campaign_manifest().first_id()
    );
    assert_eq!(saved.solved, solved_before);
    assert_eq!(saved.phrases, phrases_before);

    // Continue now resumes the LOOP, not "the first unsolved" (there is none).
    let idx = app
        .world()
        .resource::<LevelCatalog>()
        .first_unsolved(&saved)
        .unwrap();
    assert_eq!(
        app.world().resource::<LevelCatalog>().id_at(idx),
        campaign_manifest().first_id()
    );
}

// ═════════════ the framing states, through the real painters ════════════════

/// The Intro shows on a FRESH save only, and paints the Stewardship cold boot.
#[test]
fn the_intro_frames_a_new_campaign_only() {
    isolate_save_dir("intro");
    let mut app = walk_app();
    let mut log: Vec<String> = vec![format!("{:?}", state(&app))];

    // Fresh save → START opens the intro.
    go(&mut app, &mut log, GameState::Menu);
    let start = terminal_state::flow::menu_items(app.world().resource::<Progress>())
        .iter()
        .position(|i| *i == MenuItem::Start)
        .unwrap();
    app.world_mut().resource_mut::<MenuCursor>().index = start;
    app.world_mut().resource_mut::<MenuCursor>().pending_commit = true;
    app.update();
    app.update();
    assert_eq!(state(&app), GameState::Intro);
    app.update();
    let intro = frame(&app);
    println!("──── intro ────\n{intro}\n─────────────────────────");
    assert!(intro.contains("[boot.07] cold-start nominal."), "{intro}");
    assert!(intro.contains("session 0001 of N"), "{intro}");
    assert!(
        intro.contains("NAL CORP · LEGACY STEWARDSHIP DIVISION"),
        "{intro}"
    );
    assert!(
        !intro.contains('{'),
        "no unresolved token on screen: {intro}"
    );

    // One solved campaign level later, START goes straight to the dive.
    let mut app2 = walk_app();
    app2.world_mut()
        .resource_mut::<Progress>()
        .mark_solved(campaign_manifest().first_id().unwrap());
    let mut log2: Vec<String> = Vec::new();
    go(&mut app2, &mut log2, GameState::Menu);
    app2.world_mut().resource_mut::<MenuCursor>().index = start;
    app2.world_mut().resource_mut::<MenuCursor>().pending_commit = true;
    app2.update();
    app2.update();
    assert_eq!(
        state(&app2),
        GameState::Diving,
        "the intro is a NEW-campaign frame only"
    );
}

/// The journal renders the accumulated content — every vignette, the assembled
/// name, the shard count, the session — and returns where it came from.
#[test]
fn the_memory_view_renders_the_accumulated_journal() {
    isolate_save_dir("memory");
    let mut app = walk_app();
    let mut log: Vec<String> = Vec::new();
    seed_campaign_up_to_the_finale(&mut app);
    hydrate(&mut app);

    go(&mut app, &mut log, GameState::Menu);
    let idx = terminal_state::flow::menu_items(app.world().resource::<Progress>())
        .iter()
        .position(|i| *i == MenuItem::Memory)
        .unwrap();
    app.world_mut().resource_mut::<MenuCursor>().index = idx;
    app.world_mut().resource_mut::<MenuCursor>().pending_commit = true;
    app.update();
    app.update();
    assert_eq!(state(&app), GameState::MemoryView);
    app.update();

    let journal = frame(&app);
    println!("──── memory view ────\n{journal}\n─────────────────────────");
    assert!(journal.contains("M E M O R Y"), "{journal}");
    assert!(journal.contains("PHRASE-L1-first-pulse"), "{journal}");
    assert!(
        journal.contains("pid://aleks_v@R·learn·A·edge·seen·named·bloom"),
        "the name is whole after L7: {journal}"
    );
    assert!(
        journal.contains("name-shards recovered: 7 of 7"),
        "{journal}"
    );
    assert!(journal.contains("session 0001"), "{journal}");

    key(&mut app, &mut log, MenuNav::Back);
    assert_eq!(
        state(&app),
        GameState::Menu,
        "Esc returns to where it opened from"
    );
}
