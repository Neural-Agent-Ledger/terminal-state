// Test-module assert_eq! on f32 values are intentional exact-equal checks.
// Allow float_cmp only in test compilation units; library code remains denied.
#![cfg_attr(test, allow(clippy::float_cmp))]
#![warn(missing_docs)]
//! `terminal_state` — the TERMINAL STATE library: the story shell built on
//! top of the `terminal3d` engine. It owns the game-flow state machine, the
//! campaign/story data model, persisted settings/progress, and the menu/HUD
//! presentation glue; `terminal3d` supplies the raycaster, the puzzle
//! runtime, the terminal present layer and input, with no knowledge of this
//! game's states, settings or campaign data.
//!
//! This crate has BOTH a library target (this file) and a binary target
//! (`src/main.rs`, the `terminal-state` executable). The library owns the
//! module tree so it is reachable from `tests/*.rs` integration tests — a
//! `[[bin]]`-only crate cannot be `use`d by an integration test, since an
//! integration test is its own crate that can only depend on a package's
//! *library* target. `src/main.rs` stays a thin binary: it pulls its shared
//! modules in via `use terminal_state::{...}` and keeps only the CLI /
//! `App`-wiring code as its own private `mod` declarations.
//!
//! ## Module ownership
//!
//! Everything the game-flow state machine ([`flow`]) transitively needs lives
//! here: the campaign/story data ([`story`]), persisted progress/settings
//! ([`save`], [`settings`]), the per-level text-surface binder
//! ([`level_text`], [`code_panels`]), the fog/atmosphere curve ([`fog`]), the
//! campaign-progress → mirror-clarity ramp ([`mirror_ramp`]), and the
//! presentation glue driven purely by [`GameState`](flow::GameState) — the
//! menu/HUD overlays ([`menu`]). None of these touch the terminal directly
//! (only `re_present` does — see `rules/engine-api-contract.md`); they are
//! engine-agnostic Bevy systems and pure data, fully testable headless.
//!
//! ## The state machine and where each screen lives
//!
//! [`flow::GameState`] drives every screen transition; [`flow`] itself owns
//! only the states, their clocks/timers and the transition systems — never
//! the painting. Content and painting live one module per screen (or
//! cluster of related screens):
//!
//! | `GameState` variant(s) | Painted by |
//! |---|---|
//! | `BootConsole` | [`boot_console`] |
//! | `Splash`, `Menu`, `CategorySelect`, `LevelSelect`, `Settings`, `Pause`, `Won` | [`menu`] |
//! | `Diving`, `Playing` | [`flow`] (state/clock) + `re_runtime`'s play-session systems (painting) |
//! | `Guide` | [`guide`] |
//! | `Intro`, `MemoryView`, `Ending`, `Epilogue` | [`screens`] |
//!
//! ## The engine seam
//!
//! The generic `.lvl` play-session runtime (puzzle→render state bus, prop
//! power, emit parts, sound fields, FOV-zoom, colour drift, trigger fields,
//! the HUD probe and note placards) lives in `re_runtime`; the seeded 2D
//! ASCII effects + text-paint helpers live in `re_asciifx` — both engine
//! crates from `terminal3d`, generic over any `.lvl` play session and with no
//! knowledge of this game's states, settings or campaign data. This crate
//! calls them directly (`re_runtime::x`/`re_asciifx::x`) wherever a plain
//! re-export needs no concrete wrapper; the one place that supplies the
//! concrete value each generic plugin needs — every place a runtime plugin
//! needed a piece of `GameState`/`Settings`/campaign data the generic side
//! must never read — is [`runtime_glue`].
/// Game state → `re_audio` cue/room/listener/source wiring (the sound grammar's
/// game side). See [`audio_cues`].
pub mod audio_cues;
/// Phase 2 of the OPENING boot sequence: the pseudo boot console content +
/// painter + [`GameState::BootConsole`](flow::GameState::BootConsole)'s
/// render plugin. See [`boot_console`].
pub mod boot_console;
/// The in-game code wall: binds each level's designated wall panel to a live
/// source snapshot (including `main.rs`'s own source, on panel 3). See
/// [`code_panels`].
pub mod code_panels;
/// The level-**entry** "resolve" transition: a short, skippable hashed-reveal
/// sweep from black into the freshly loaded level. NOT yet wired into the
/// live binary — see the module doc.
pub mod entry_fx;
/// The self-ticking FIRST STEPS onboarding checklist: observation-driven,
/// persisted in [`save::Progress::first_steps`], shown compactly in the play
/// HUD. See [`first_steps`].
pub mod first_steps;
/// The game-flow state machine: [`flow::GameState`], its transitions, and the
/// per-state clocks/timers. See the crate doc's "state machine" section for
/// where each screen's content and painting live, and [`flow`] for the state
/// machine itself.
pub mod flow;
pub mod fog;
/// The in-game GUIDE: a chaptered rules reference reachable from both the
/// main menu and pause. See [`guide`].
pub mod guide;
/// Binds a level's authored `[note]`/placard text to the live per-level text
/// surfaces the renderer paints. See [`level_text`].
pub mod level_text;
/// Menus, the splash, and the play/pause/won HUD overlays — the presentation
/// glue for most of [`flow::GameState`]'s screens. See [`menu`].
pub mod menu;
/// Main-menu visual effects.
///
/// The deterministic overgrowth glyph-vine growth. See [`menu_fx`].
pub mod menu_fx;
/// Mouse-driven menu navigation (hover + click) over the SAME `MenuNav`/
/// `MenuCursor` pipeline the keyboard/gamepad path uses. See [`menu_mouse`].
pub mod menu_mouse;
/// Campaign progress → mirror-clarity ramp: how solved levels gradually
/// clear the reflective surfaces in later rooms. See [`mirror_ramp`].
pub mod mirror_ramp;
/// Campaign progress → the live [`re_render::palette_gate::PaletteGate`]. The
/// twin of [`mirror_ramp`] on the colour-unlock axis. See [`palette_ramp`].
pub mod palette_ramp;
/// Every place this game's own concrete types cross the `re_runtime` engine
/// seam. See the crate doc's "engine seam" section and [`runtime_glue`].
pub mod runtime_glue;
/// Persisted save data: campaign progress, first-steps completion, and the
/// story memory log. See [`save`].
pub mod save;
/// The framing states' content + painters (Intro, MemoryView, the two
/// end-cards, Epilogue). See [`screens`].
pub mod screens;
/// Persisted player settings (video/audio/gameplay options) and their
/// load/save round-trip. See [`settings`].
pub mod settings;
/// The campaign/story data model: manifest, movements, memory shards, and the
/// fourth-wall interpolation tokens. See [`story`].
pub mod story;
/// The ONE player-facing string table the guide + first-steps checklist read
/// from — English only for now (owner decision). See [`strings`].
pub mod strings;

/// Interactive framebuffer width (columns). Shared by [`flow`]'s
/// always-present `Framebuffer`/render-config wiring and the binary's
/// interactive/dump paths (`terminal-state`'s `main.rs`).
pub const VIEW_COLS: usize = 100;
/// Interactive framebuffer height (rows). See [`VIEW_COLS`] for the shared
/// wiring both dimensions feed.
pub const VIEW_ROWS: usize = 32;
