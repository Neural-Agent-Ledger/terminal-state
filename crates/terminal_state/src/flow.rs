//! `flow` — the game-flow state machine: splash → menu → level-select →
//! playing → won, built on Bevy STATES (`bevy_state`).
//!
//! ## Why the resources are always present
//!
//! The render layer (`re_render`) reads [`Map`](re_world::Map) (`Res<Map>`) every
//! `PostUpdate` and the present layer (`re_present`) blits the
//! [`Framebuffer`](re_core::Framebuffer) every `Last`. Neither can be state-gated
//! from here (we don't own those crates). So the flow keeps
//! [`Map`](re_world::Map), [`Framebuffer`](re_core::Framebuffer),
//! [`PuzzleState`](re_puzzle::PuzzleState), and [`Solved`](re_puzzle::Solved)
//! inserted at all times:
//!
//! * In [`GameState::Playing`](crate::flow::GameState::Playing) a real level is
//!   loaded into them on `OnEnter(Playing)` and a
//!   [`CameraPose`](re_world::CameraPose) entity is spawned — so the raycaster
//!   has a camera and draws the 3D scene.
//! * In every non-playing state NO [`CameraPose`](re_world::CameraPose) entity
//!   exists, so `re_render`'s `raycast_sys` early-returns (it needs a camera) and
//!   leaves the framebuffer untouched. The flow's own overlay systems (see
//!   [`crate::menu`]) then paint the splash / menu / select / won text directly
//!   into the framebuffer, which `re_present` presents. This is the one channel
//!   that reaches the real terminal without us touching the present crate.
//!
//! ## Input
//!
//! The present layer surfaces backend-neutral actions:
//! [`InteractPressed`](re_present::InteractPressed) (E / Enter / Space) for
//! gameplay, and — added for the menus — a [`MenuNav`](re_present::MenuNav)
//! message (`Up`/`Down`/`Left`/`Right`/`Confirm`/`Back`) mapped from the same
//! keys in BOTH backends. The flow drives every menu with `MenuNav`:
//! `Up`/`Down` move the highlight (wrapping), `Confirm` commits the highlighted
//! item, `Back` returns to the previous screen (or quits from the main menu).
//!
//! This replaces the old "press-and-pause" hack, where the only neutral signal
//! was [`InteractPressed`](re_present::InteractPressed), so each press stepped
//! the highlight forward and a short idle then committed it (you couldn't move
//! *up*, and picking required a deliberate pause). Now navigation is crisp and
//! immediate.
//!
//! `MenuNav` is emitted ALONGSIDE the camera-motion writes. That is harmless:
//! menu input is gated to the menu states (no camera there, so the motion is
//! inert), and gameplay input is gated to
//! [`GameState::Playing`](crate::flow::GameState::Playing) (where the flow
//! ignores `MenuNav`). The same keypress can drive both; each consumer reads
//! only the message its state cares about. Everything stays headlessly testable
//! by writing [`MenuNav`](re_present::MenuNav) messages directly — no terminal
//! needed.

use bevy::prelude::*;
use re_core::{Framebuffer, InputGate};
use re_glitch::GlitchConfig;
use re_level::Level;
use re_present::{
    ColorMode, FovCycle, InteractPressed, MenuMouseClick, MenuMouseMove, MenuNav, MouseSettings,
};
use re_puzzle::{network_forward, ForwardError, PuzzleState, Solved};
use re_world::{CameraPose, Map};

// `PlayCamera` and `CurrentLevelData` live in `re_runtime::session` — re-exported
// here so every existing `crate::flow::PlayCamera` / `crate::flow::CurrentLevelData`
// path in this crate keeps compiling unchanged.
pub use re_runtime::session::{CurrentLevelData, PlayCamera};

use crate::story::beats::{cue_for, NarrativeBeat};
use crate::story::{assemble_phrase, MemoryEntry, MemoryLog, MemoryReadout, ReadoutForm};
use crate::{VIEW_COLS, VIEW_ROWS};

/// The top-level game-flow state.
#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GameState {
    /// **Optional pre-splash pseudo boot console** (the OPENING two-phase
    /// boot sequence, 2026-09-24): a kernel/BIOS-style log that types real
    /// diagnostics (terminal size, colour mode, level count, build version)
    /// line by line, then breaks down — bit-rot, line shear, colour bleed,
    /// row collapse, spreading noise — dissolving into the
    /// [`Splash`](Self::Splash) eye. Content + painting live in
    /// [`crate::boot_console`]; this state only owns the clock
    /// ([`BootConsoleTimer`]) and the nav-driven skip, mirroring
    /// [`Diving`](Self::Diving)'s split between `flow` (state/clock) and its
    /// content module (composition/painting).
    ///
    /// Reached ONLY when the live binary's TTY/VT-capability probe and
    /// `Settings::boot_intro` gate allow it (see `main.rs`'s
    /// `maybe_enter_boot_console`, which mirrors the SAME gate Phase 1's
    /// `re_present::intro_tear::should_run_tear` uses). `Splash` stays
    /// `#[default]`, so every existing test and every non-interactive entry
    /// path (`--dump`, headless, `cargo test`'s `FlowPlugin`-only harness) is
    /// completely unaffected — nothing transitions here unless something
    /// explicitly sets `NextState<GameState>` to it.
    BootConsole,
    /// Title card. Any interact press or a short timeout → [`Menu`](Self::Menu).
    #[default]
    Splash,
    /// Main menu: Start / Select / Quit.
    Menu,
    /// Pick a category "folder" (Campaign, Demos, …). The first level of the
    /// two-level browse: confirming a category enters [`LevelSelect`](Self::LevelSelect)
    /// scoped to that category.
    CategorySelect,
    /// Pick one of the levels WITHIN the chosen category (see [`BrowseCategory`]).
    /// `Back` returns to [`CategorySelect`](Self::CategorySelect).
    LevelSelect,
    /// The settings screen: color mode, mouse-look, audio, font size.
    Settings,
    /// Skippable full-screen "Diving" tunnel transition, played on the path INTO
    /// a level (a player-initiated level-entry commit routes here, then this state
    /// auto-advances to [`Playing`](Self::Playing)). The camera gate is closed
    /// while diving and the tunnel set-piece fills the framebuffer each frame;
    /// any interact / nav key skips it. See `enter_diving` / `tick_diving`.
    Diving,
    /// The live first-person puzzle loop.
    Playing,
    /// In-play pause overlay (Esc during play): Resume / Memory / Settings / Quit.
    /// Drawn over the frozen scene like [`Won`](Self::Won); gameplay systems are
    /// gated to [`Playing`](Self::Playing), so they halt automatically here.
    Pause,
    /// Solved overlay: Next level / Menu.
    Won,
    // ──────────────────────── the framing states ─────────────────────────────
    //
    // All four are the `Diving` template (a state clock + a nav-driven exit + one
    // painter). Their content composition and painting live in `crate::screens`.
    /// **The doorway in**: the Stewardship cold-boot handshake shown before
    /// the campaign's first level — on a NEW campaign only, never on `Continue`
    /// (see [`is_fresh_campaign`]). Any key → [`Diving`](Self::Diving) → Playing.
    Intro,
    /// **The journal**: a scrollable single pane of everything recovered
    /// so far — the accumulated memory-log phrases, the running `pid://…`
    /// assembly, the shard count, the session counter. Reachable from the main
    /// [`Menu`](Self::Menu) and from [`Pause`](Self::Pause); `Esc` returns to
    /// whichever opened it ([`MemoryViewReturn`]).
    MemoryView,
    /// **The guide**: a scrollable, chaptered rules reference (Goal / Controls
    /// / Nodes & switches / The forward pass / Materials). Reachable from the
    /// main [`Menu`](Self::Menu) AND from [`Pause`](Self::Pause), so a player
    /// can look the rules up mid-run without losing progress. `Esc` returns
    /// to whichever opened it ([`GuideReturn`]). Content + painting live in
    /// [`crate::guide`].
    Guide,
    /// **The end-card**: entered INSTEAD of the generic
    /// [`Won`](Self::Won) presentation when the campaign's finale is solved. Which
    /// of the two cards is shown comes from [`EndingOutcome`], captured from the
    /// live puzzle before the level is torn down.
    Ending,
    /// **The closing furniture**: the SHARED epilogue both endings reach —
    /// the Form A-7 re-read offer, the held Stewardship documents, and one credits
    /// line. Identical on both paths, by construction. `Enter`/`Esc` → Menu.
    Epilogue,
}

/// **Which of the two finales the player's last wiring act produced.**
///
/// The distinction is read straight off the shipped puzzle runtime — no
/// `re_puzzle` change was needed:
///
/// * [`WhileTrue`](Self::WhileTrue) ⇔ [`PuzzleState::cycle_held`] — the level's
///   `[room] cycle_keep` accepted a closed loop as a terminal state, so the ring
///   the player deliberately closed STANDS.
/// * [`Exit0`](Self::Exit0) ⇔ the win came through the ordinary (inverted) target
///   check instead: `[door] target_parts` fired while every `[door] target_off`
///   node read 0.
///
/// The two are mutually exclusive by construction: `PuzzleState::refresh_solved`
/// short-circuits to the kept cycle first, so a held cycle is never also an
/// inverted-target win.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ending {
    /// LETHE releases — the graph resolves and the process terminates cleanly.
    Exit0,
    /// MNEMOS keeps — the graph closes into a loop that is allowed to stand.
    WhileTrue,
}

/// The captured finale outcome, or `None` when the level that just ended was not
/// the campaign's finale (or was not solved at all).
///
/// Written on `OnExit(Playing)` by `capture_ending` — BEFORE the teardown drops
/// the live [`PuzzleState`] — and read by `OnEnter(Won)`'s
/// `route_finale`, by the end-card painter, and by the `Won` overlay (which
/// stands down when a finale is pending, so the generic banner never flashes
/// over the ending).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EndingOutcome(pub Option<Ending>);

/// The scroll offset of whichever framing pane is on screen
/// ([`Intro`](GameState::Intro), [`MemoryView`](GameState::MemoryView),
/// [`Ending`](GameState::Ending), [`Epilogue`](GameState::Epilogue)).
///
/// ONE resource for all four because the states are mutually exclusive and every
/// entry resets it; a scroll offset per state would be four resources that can
/// only ever be non-zero one at a time.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct PaneScroll(pub usize);

impl PaneScroll {
    /// Scroll up one row (saturating at the top).
    fn up(&mut self) {
        self.0 = self.0.saturating_sub(1);
    }
    /// Scroll down one row. The painter clamps against the real body length
    /// ([`crate::screens::clamp_scroll`]), so an over-scroll here is inert.
    fn down(&mut self) {
        self.0 = self.0.saturating_add(1);
    }
}

/// Where the [`GameState::MemoryView`] journal was opened from, so `Esc` returns
/// to the right place — the main [`GameState::Menu`] or the in-play
/// [`GameState::Pause`] overlay (whose live level is deliberately kept alive
/// behind it). Same shape, and the same reason, as [`SettingsReturn`].
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MemoryViewReturn {
    /// Opened from the main menu.
    #[default]
    Menu,
    /// Opened from the in-play pause overlay.
    Pause,
}

/// Where the [`GameState::Guide`] was opened from, so `Esc` returns to the
/// right place — same shape and reason as [`MemoryViewReturn`].
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuideReturn {
    /// Opened from the main menu.
    #[default]
    Menu,
    /// Opened from the in-play pause overlay.
    Pause,
}

/// Where the [`GameState::Settings`] screen was entered from, so `Back` returns
/// to the right place: the main [`GameState::Menu`] (the default) or the in-play
/// [`GameState::Pause`] overlay. Set by whoever routes into Settings.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsReturn {
    /// Settings was opened from the main menu — `Back` returns to the menu.
    #[default]
    Menu,
    /// Settings was opened from the in-play pause overlay — `Back` returns to Pause.
    Pause,
}

/// Auto-advance from the splash to the menu after this long, if the player does
/// not press a key first.
///
/// PUBLIC because the splash PAINTER needs it: `menu::draw_splash` divides this
/// window into its phases (eye animation → title card) so the two always add up to
/// exactly the state's lifetime — see [`crate::menu::splash_phase`].
pub const SPLASH_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(2200);

// ════════════════════════ the campaign manifest ═════════════════════════════
//
// The authored order of the twelve story levels lives in `levels/campaign.toml`,
// EMBEDDED here with `include_str!`. Two reasons it is embedded rather than read:
//
//   1. The shipped build is a single static executable (`crt-static`) with no asset
//      directory, so a runtime path would make the campaign order depend on the cwd
//      — the exact fragility `LevelCatalog`'s directory scan already has to defend
//      against with a hardcoded fallback list.
//   2. It makes the manifest PURE DATA available to plain functions
//      ([`browse_category`], [`catalog_sort_key`]) and to unit tests with no `App`,
//      no I/O, and no ordering hazard. The parse happens once, in a `OnceLock`.
//
// Editing the campaign order therefore requires a rebuild. That is deliberate: the
// order is content, and content that changes the shape of the game belongs under the
// same review as code.

/// The embedded `levels/campaign.toml` source — the campaign's single source of
/// truth for order, display names, movement tags, and the fog escalation curve.
const CAMPAIGN_MANIFEST_TOML: &str = include_str!("../../../levels/campaign.toml");

/// One of PROGRESSION §1's four movements. These map to the audio
/// director's movement themes; nothing else consumes them yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Movement {
    /// L1–L3 "First Sight" — sort true memory from script-projection.
    P1,
    /// L4–L6 "Splitting / the angle of the gaze" — we ARE the gaze.
    P2,
    /// L7–L9 "The Phantom Sister" — a node that never answers.
    P3,
    /// L10–L12 "The Closing / Exit-Exit" — the choice.
    P4,
}

impl Movement {
    /// Parse a manifest `movement = "Pn"` tag. Anything unrecognised reads as
    /// [`Movement::P1`] — a manifest typo must not drop a level from the campaign,
    /// and the movement tag drives only flavour (theme selection), never progress.
    fn from_tag(tag: &str) -> Self {
        match tag.trim() {
            "P2" => Self::P2,
            "P3" => Self::P3,
            "P4" => Self::P4,
            _ => Self::P1,
        }
    }

    /// The audio director's THEME for this movement.
    ///
    /// One-to-one with `re_audio`'s four movement themes, which were added
    /// alongside — never replacing — the shipped `Default`/`Cold`/`Garden` palette.
    /// Each is a mood, not a narrator: MNEMOS and LETHE stay ambient forces (v6.1),
    /// so no movement introduces a voice-like lead.
    #[must_use]
    pub const fn theme(self) -> re_audio::Theme {
        match self {
            Self::P1 => re_audio::Theme::P1FirstLight,
            Self::P2 => re_audio::Theme::P2TheGaze,
            Self::P3 => re_audio::Theme::P3Grief,
            Self::P4 => re_audio::Theme::P4Closing,
        }
    }

    /// The movement's title, for the level-select header — PROGRESSION §2's own
    /// names for the four blocks.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::P1 => "I · FIRST SIGHT",
            Self::P2 => "II · THE ANGLE OF THE GAZE",
            Self::P3 => "III · THE PHANTOM SISTER",
            Self::P4 => "IV · THE CLOSING",
        }
    }
}

/// One campaign level, in authored order.
#[derive(Clone, Debug, PartialEq)]
pub struct CampaignLevel {
    /// The STABLE progress key: the level's file stem, the same id
    /// [`crate::save::Progress`] stores. Reordering the campaign never orphans a
    /// save because nothing is keyed by index.
    pub id: String,
    /// Workspace-relative path, as it appears in [`LevelCatalog::paths`].
    pub path: String,
    /// The authored display name for level-select (not derived from the filename).
    pub name: String,
    /// Which of the four movements this level belongs to.
    pub movement: Movement,
    /// The escalation contract's `fog_far` for this level (`STORY_BIBLE` §7). Recorded
    /// here so the curve is one table instead of twelve scattered numbers; asserted
    /// monotone non-increasing by `tests::campaign_fog_far_is_monotonic`.
    pub fog_far: f32,
}

/// The parsed campaign manifest: the twelve story levels in authored order.
#[derive(Clone, Debug, Default)]
pub struct CampaignManifest {
    levels: Vec<CampaignLevel>,
}

/// The serde shape of `campaign.toml`. Kept private — callers see
/// [`CampaignManifest`], not the file format.
#[derive(serde::Deserialize)]
struct ManifestFile {
    #[serde(default)]
    #[allow(dead_code)] // reserved: a future schema bump reads this.
    version: u32,
    #[serde(default, rename = "level")]
    levels: Vec<ManifestEntry>,
}

#[derive(serde::Deserialize)]
struct ManifestEntry {
    id: String,
    path: String,
    name: String,
    #[serde(default)]
    movement: String,
    #[serde(default)]
    fog_far: f32,
}

/// The manifest POSITIONS (0-based) of the campaign's two PEAK levels — the 7th
/// and the 11th. `PUZZLE_PROGRESSION` §3 gives their solves a held breath: the
/// generative score mutes for a few seconds (see [`peak_solve_silence`]).
///
/// Positions, not ids, because "the seventh and eleventh level of the campaign" is
/// the DESIGN statement — renaming a level must not silently drop the beat.
/// [`CampaignManifest::is_peak_solve`] bounds-checks against the live manifest, so
/// a shorter (or empty) manifest simply has fewer peaks and never mis-fires.
const PEAK_SOLVE_POSITIONS: [usize; 2] = [6, 10];

impl CampaignManifest {
    /// Parse a manifest from TOML source. A malformed file yields an EMPTY manifest
    /// plus a warning — never a panic. An empty manifest degrades cleanly: every
    /// campaign-class level falls back to the pre-manifest lexical browse (see
    /// [`browse_category`]), so a content typo cannot make levels unreachable.
    #[must_use]
    pub fn parse(src: &str) -> Self {
        match toml::from_str::<ManifestFile>(src) {
            Ok(f) => Self {
                levels: f
                    .levels
                    .into_iter()
                    .map(|e| CampaignLevel {
                        id: e.id,
                        path: e.path,
                        name: e.name,
                        movement: Movement::from_tag(&e.movement),
                        fog_far: e.fog_far,
                    })
                    .collect(),
            },
            Err(e) => {
                eprintln!(
                    "terminal-state: levels/campaign.toml is malformed ({e}) — \
                     falling back to the lexical level browse."
                );
                Self::default()
            }
        }
    }

    /// The campaign levels, in authored order.
    #[must_use]
    pub fn levels(&self) -> &[CampaignLevel] {
        &self.levels
    }

    /// How many levels the campaign holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.levels.len()
    }

    /// Is the manifest empty (unparsed / absent)? When true the browse degrades to
    /// the pre-manifest lexical categories.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    /// The manifest position of a level id, or `None` when the id is not a campaign
    /// level.
    #[must_use]
    pub fn position(&self, id: &str) -> Option<usize> {
        self.levels.iter().position(|l| l.id == id)
    }

    /// The manifest entry for a level id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&CampaignLevel> {
        self.levels.iter().find(|l| l.id == id)
    }

    /// Is this level id part of the authored campaign?
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.position(id).is_some()
    }

    /// Is this level one of the two PEAK levels (manifest positions
    /// `PEAK_SOLVE_POSITIONS`) whose solve the score answers with a brief
    /// silence? A non-campaign level is never a peak.
    #[must_use]
    pub fn is_peak_solve(&self, id: &str) -> bool {
        self.position(id)
            .is_some_and(|pos| PEAK_SOLVE_POSITIONS.contains(&pos))
    }

    /// How many campaign levels the player has solved — the campaign's own progress
    /// counter, independent of how many `.lvl` files happen to be on disk.
    #[must_use]
    pub fn solved_count(&self, progress: &crate::save::Progress) -> usize {
        self.levels
            .iter()
            .filter(|l| progress.is_solved(&l.id))
            .count()
    }

    /// **The unlock gate.** A campaign level is playable when the level BEFORE it in
    /// the manifest is solved. The first level is always unlocked, and any level that
    /// is not in the manifest at all is unlocked (demos, archive, test fixtures — the
    /// gate is a campaign rule, not a global one).
    ///
    /// Keyed by level ID, never by catalogue index, so a catalogue reorder or a
    /// manifest edit can never lock a player out of a level they already reached
    /// (plan watch-item: "campaign manifest reorder can orphan old progress.toml
    /// entries — key progress by level id not index").
    ///
    /// An already-solved level stays unlocked even if the level before it somehow is
    /// not: replaying is never taken away.
    #[must_use]
    pub fn is_unlocked(&self, id: &str, progress: &crate::save::Progress) -> bool {
        let Some(pos) = self.position(id) else {
            return true; // not a campaign level ⇒ no gate
        };
        if pos == 0 || progress.is_solved(id) {
            return true;
        }
        progress.is_solved(&self.levels[pos - 1].id)
    }

    /// The campaign's FIRST level id — where a loop-back parks its cursor.
    #[must_use]
    pub fn first_id(&self) -> Option<&str> {
        self.levels.first().map(|l| l.id.as_str())
    }

    /// Is this the campaign's LAST level — the finale that ends in one of the two
    /// end-cards? Positional ("the last authored level"), not a hardcoded `L12`, so
    /// re-ordering or extending the manifest moves the finale with it. An empty
    /// manifest has no finale.
    #[must_use]
    pub fn is_finale(&self, id: &str) -> bool {
        self.levels.last().is_some_and(|l| l.id == id)
    }

    /// The manifest entry AFTER `id`, or `None` at the end of the campaign (or for
    /// a non-campaign id). Drives the loop cursor's walk.
    #[must_use]
    pub fn next_after(&self, id: &str) -> Option<&CampaignLevel> {
        let pos = self.position(id)?;
        self.levels.get(pos + 1)
    }

    /// **Is the campaign complete?** Every authored level solved. Both endings
    /// reach this state — the finale's `OnEnter(Won)` progress write runs on either
    /// path before the end-card is routed to — which is the mechanical half of
    /// "both endings mark the campaign COMPLETE".
    ///
    /// An EMPTY manifest is never complete (there is no campaign to finish), so a
    /// content typo cannot spuriously unlock the loop offer.
    #[must_use]
    pub fn is_complete(&self, progress: &crate::save::Progress) -> bool {
        !self.levels.is_empty() && self.levels.iter().all(|l| progress.is_solved(&l.id))
    }
}

/// **Is this a brand-new campaign?** No authored level solved AND no loop taken.
///
/// The gate on the cold-boot [`GameState::Intro`]: shown on a NEW campaign
/// only, not on Continue. Deliberately conservative: a player who has solved
/// even one campaign level, or who has ever accepted the loop offer, is past the
/// boot handshake and does not see it again.
#[must_use]
pub fn is_fresh_campaign(progress: &crate::save::Progress) -> bool {
    progress.session == 0 && campaign_manifest().solved_count(progress) == 0
}

/// The process-wide parsed campaign manifest. Parsed ONCE from
/// `CAMPAIGN_MANIFEST_TOML` — a compile-time constant, so this is pure and
/// deterministic (no I/O, no cwd dependence, no ordering hazard between systems).
pub fn campaign_manifest() -> &'static CampaignManifest {
    static MANIFEST: std::sync::OnceLock<CampaignManifest> = std::sync::OnceLock::new();
    MANIFEST.get_or_init(|| CampaignManifest::parse(CAMPAIGN_MANIFEST_TOML))
}

/// A browse "folder" — the category a level belongs to, derived from its file
/// name by [`categorize`]. The level-select UI groups every `.lvl` under one of
/// these so the flat ~47-file directory becomes a two-level browse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    /// The story/puzzle campaign: `01`..`08`, `L5`/`L9`..`L12`.
    Campaign,
    /// The PRE-MANIFEST numbered story rooms (`01`..`08`) — the L-files' ancestors.
    ///
    /// Only [`browse_category`] ever returns this, and only for a level that
    /// [`categorize`] calls `Campaign` but the campaign manifest does not list. They
    /// remain playable, remain shard-bearing on the name arc, and keep their golden
    /// frames; they are simply not the twelve authored levels any more.
    Archive,
    /// Material / render technique demos (holo, mirror, code-wall, catalog, …).
    Demos,
    /// Grand showcases / emitter / semantic / reactive set-pieces.
    Showcases,
    /// Non-euclidean portal demos (fractal, warp).
    Portals,
    /// Scalable-material overlay demos (`A*`).
    Overlay,
    /// Texture / sprite-sheet demos (`TX*`).
    Texture,
    /// Single-feature demos (`U*` underline, `S*` ramp, `T*` trigger).
    Feature,
    /// Loader / smoke-test fixtures (`00-test-load`).
    Test,
}

impl Category {
    /// Every category, in the fixed display order the browse menu uses. ARCHIVE sits
    /// directly after CAMPAIGN: the pre-manifest rooms are the campaign's own history,
    /// not a demo, and `Continue`/`Start` (which walk catalogue order) should reach
    /// the twelve authored levels before anything else.
    pub const ALL: [Self; 9] = [
        Self::Campaign,
        Self::Archive,
        Self::Demos,
        Self::Showcases,
        Self::Portals,
        Self::Overlay,
        Self::Texture,
        Self::Feature,
        Self::Test,
    ];

    /// The screen label for the category list.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Campaign => "CAMPAIGN",
            Self::Archive => "ARCHIVE",
            Self::Demos => "DEMOS",
            Self::Showcases => "SHOWCASES",
            Self::Portals => "PORTALS",
            Self::Overlay => "OVERLAY",
            Self::Texture => "TEXTURE",
            Self::Feature => "FEATURE",
            Self::Test => "TEST",
        }
    }
}

/// The level's **STORY class** — is it on the campaign arc at all?
///
/// Purely lexical, so a NEW `.lvl` is auto-classified by its prefix with no
/// per-level table to maintain. Every stem resolves to exactly one [`Category`]; the
/// final fallback is [`Category::Demos`] so nothing is ever dropped from the browse.
/// This function NEVER returns [`Category::Archive`].
///
/// # Two classifiers, on purpose
///
/// This is the SEMANTIC question, and [`crate::story::shard_for_level`] gates the
/// seven-shard name arc on it: `01`..`07` and `L1`..`L7` carry shards, everything else
/// does not. The manifest must not change that — the numbered rooms are still story
/// levels and a player who solved `03` still holds shard 3.
///
/// [`browse_category`] answers the different question "which FOLDER does this level
/// live in", and that one does move `01`..`08` out of CAMPAIGN and into
/// [`Category::Archive`]. Collapsing the two would either retire seven shards or
/// leave the archive inside the campaign folder; keeping them apart costs one
/// function and says what it means.
#[must_use]
pub fn categorize(stem: &str) -> Category {
    let lower = stem.to_ascii_lowercase();

    // Loader / smoke-test fixtures.
    if lower.starts_with("00") || lower.contains("test") {
        return Category::Test;
    }
    // Overlay (A*), Texture (TX*), and the single-feature demos (U*/S*/T*) are
    // letter-prefixed; check those before the L-campaign letter rule.
    if lower.starts_with("tx") {
        return Category::Texture;
    }
    if lower.starts_with('a') && lower[1..].starts_with(|c: char| c.is_ascii_digit()) {
        return Category::Overlay;
    }
    if (lower.starts_with('u') || lower.starts_with('s') || lower.starts_with('t'))
        && lower[1..].starts_with(|c: char| c.is_ascii_digit())
    {
        return Category::Feature;
    }
    // Campaign: the numbered story rooms (01..08) and the L-series (L5/L9..L12).
    if lower.starts_with('l') && lower[1..].starts_with(|c: char| c.is_ascii_digit()) {
        return Category::Campaign;
    }
    // Leading-digit levels split by their numeric prefix.
    if let Some(num) = leading_number(&lower) {
        // Portals: 92/93 are the fractal / warp non-euclidean demos.
        if lower.contains("portal") {
            return Category::Portals;
        }
        return match num {
            // 01..08 are the campaign rooms (08-material-demo is a demo, handled
            // below by the name check before this point would mis-bucket it).
            1..=8 if !lower.contains('-') => Category::Campaign,
            // 9x grand/semantic/reactive set-pieces and the 2x showcases read as
            // showcases; the remaining named technique demos are Demos.
            20 | 21 | 91 | 96 | 97 | 98 | 99 => Category::Showcases,
            _ => Category::Demos,
        };
    }
    Category::Demos
}

/// The **BROWSE FOLDER** a level is listed under.
///
/// Identical to [`categorize`] except for one rule: a level that [`categorize`] calls
/// [`Category::Campaign`] but the campaign manifest does NOT list is filed under
/// [`Category::Archive`]. That moves the pre-manifest numbered rooms `01`..`08` out of
/// the CAMPAIGN folder without deleting them, without touching their goldens, and
/// without taking their name shards away (see [`categorize`]'s note).
///
/// DEGRADES CLEANLY: if the manifest failed to parse it is empty, and this function is
/// then exactly [`categorize`] — the pre-manifest browse. A content typo can never
/// make a level unreachable.
#[must_use]
pub fn browse_category(stem: &str) -> Category {
    let cat = categorize(stem);
    let manifest = campaign_manifest();
    if cat == Category::Campaign && !manifest.is_empty() && !manifest.contains(stem) {
        Category::Archive
    } else {
        cat
    }
}

/// The leading integer of a stem (e.g. `"13b-holomesh"` → `13`, `"08"` → `8`),
/// or `None` when the stem does not start with a digit.
fn leading_number(stem: &str) -> Option<u32> {
    let digits: String = stem.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// A friendly display title for a level path: the file stem with any leading
/// `NN-`/`NNx-`/`Ln-` ordinal stripped and the remaining `kebab-case` words
/// title-cased (e.g. `"10-calibration-vault"` → `"Calibration Vault"`,
/// `"01"` → `"01"`). Pure string work — no I/O.
#[must_use]
pub fn display_title(path: &str) -> String {
    let stem = LevelCatalog::label(path);
    // Split off a leading ordinal token (everything up to the first '-'), but only
    // when there IS descriptive text after it; a bare "01" keeps its name.
    let body = match stem.split_once('-') {
        Some((_, rest)) if !rest.is_empty() => rest,
        _ => stem,
    };
    if body == stem && !body.contains('-') {
        // No descriptive remainder (e.g. "01", "L5-soft-footstep" → handled below).
        // A bare numeric/short id is shown as-is.
        if body.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return body.to_string();
        }
    }
    let mut out = String::new();
    for (i, word) in body.split('-').filter(|w| !w.is_empty()).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(&chars.as_str().to_ascii_lowercase());
        }
    }
    if out.is_empty() {
        stem.to_string()
    } else {
        out
    }
}

/// Locate the shipped `levels/` directory.
///
/// **Anchored on the executable, not the working directory.** The CWD is set by
/// whoever launched us — Explorer uses the exe's folder (which is why
/// double-clicking happened to work), a Start-menu shortcut uses `%HOMEPATH%`, a
/// PATH install uses wherever the player was standing, and `cargo` uses the crate
/// directory. A `terminal-state.exe` copied anywhere on its own therefore found no
/// levels at all — breaking the promise of a single static `.exe` a player can
/// drop anywhere with nothing else to install.
///
/// [`re_level::assets::find_content_dir`] probes, in order: the exe's own directory
/// (the shipped layout — exe beside `levels/`), its bounded ancestors (so a
/// `target/debug/` build finds the repo tree from ANY cwd), and finally the CWD
/// itself (so `cargo run` from the repo root is unchanged).
///
/// On failure the error carries every directory that was probed, so the caller can
/// tell the player exactly where it looked. There is deliberately **no fallback to
/// a fabricated list of level names** — the old `01..=07` fallback produced paths
/// that could not be loaded and turned a missing content tree into a menu that
/// silently refused to start a level.
///
/// # Errors
///
/// Returns [`re_level::assets::NotFound`] when no `levels/` directory can be
/// located from the executable's own directory, its bounded ancestors, or the
/// CWD.
pub fn locate_levels_dir() -> Result<std::path::PathBuf, re_level::assets::NotFound> {
    re_level::assets::find_content_dir("levels")
}

/// How the catalogue SPELLS a level under `dir`.
///
/// When `dir` is the CWD's own `levels/`, the historical relative spelling
/// (`levels/01.lvl`) is kept, so running from the repo root produces byte-identical
/// catalogue strings to before this anchor change — including everything that
/// compares or displays them. Any other location is spelled absolutely, because a
/// relative string would not name that file from this process's CWD.
fn catalog_prefix(dir: &std::path::Path) -> std::path::PathBuf {
    let cwd_levels = std::env::current_dir().ok().map(|c| c.join("levels"));
    let same = match (&cwd_levels, dir.canonicalize().ok()) {
        (Some(c), Some(d)) => c.canonicalize().ok() == Some(d),
        _ => false,
    };
    if same {
        std::path::PathBuf::from("levels")
    } else {
        dir.to_path_buf()
    }
}

/// The level catalogue. Scanned from the `levels/` directory located by
/// [`locate_levels_dir`] at startup.
///
/// Paths are spelled `levels/NAME.lvl` when that directory is the CWD's (the
/// historical form), and absolutely otherwise. An unlocatable `levels/` yields an
/// EMPTY catalogue — never a fabricated one; `main` reports the failure with the
/// full search before the terminal is touched.
#[derive(Resource, Clone, Debug)]
pub struct LevelCatalog {
    /// Every scanned level path, in catalogue order.
    pub paths: Vec<String>,
}

impl LevelCatalog {
    /// A short display label for a level path (the file stem).
    #[must_use]
    pub fn label(path: &str) -> &str {
        std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(path)
    }

    /// Resolve a level path (typically the one given on the command line) to its
    /// catalogue slot, or `None` when the catalogue does not contain it.
    ///
    /// **Path-aware, not string-equal.** The catalogue stores workspace-relative,
    /// forward-slash paths (`levels/01.lvl`), but the natural thing to type on
    /// Windows is `levels\01.lvl`, and a launcher or shell completion may hand us
    /// `./levels/01.lvl` or an absolute path. A plain `==` matched none of those
    /// and the caller silently fell back to level 0 — a player then played a level
    /// they did not ask for (the defect this method exists to kill).
    ///
    /// Three tiers, cheapest first:
    /// 1. exact string equality — the historical fast path;
    /// 2. FILESYSTEM IDENTITY via [`std::fs::canonicalize`]. That is the std
    ///    primitive for this job: it resolves separators, `.`/`..`, relative vs
    ///    absolute, and (on Windows) the on-disk letter case. Only applies to
    ///    paths that actually exist;
    /// 3. a LEXICAL comparison over [`std::path::Path::components`] for paths that
    ///    do not exist on disk (the built-in fallback catalogue, unit-test
    ///    fixtures, a run from another cwd). `Path` treats `\` as a separator on
    ///    Windows, so this is std's normalisation, not a hand-rolled one.
    ///
    /// Never guesses: a name the catalogue does not hold returns `None` so the
    /// caller can report it instead of substituting a different level.
    #[must_use]
    pub fn index_of_path(&self, path: &str) -> Option<usize> {
        if let Some(i) = self.paths.iter().position(|c| c == path) {
            return Some(i);
        }
        if let Ok(want) = std::fs::canonicalize(path) {
            if let Some(i) = self
                .paths
                .iter()
                .position(|c| std::fs::canonicalize(c).is_ok_and(|got| got == want))
            {
                return Some(i);
            }
        }
        let want = lexical_path_key(path);
        self.paths.iter().position(|c| lexical_path_key(c) == want)
    }

    /// The catalogue entries whose FILE NAME matches `path`'s — the "did you
    /// mean" set for a level the catalogue does not contain (e.g. a bare
    /// `01.lvl`, or a copy of the file kept in another directory). Purely
    /// advisory: [`index_of_path`](Self::index_of_path) deliberately does NOT
    /// match on file name alone, because `elsewhere/01.lvl` is a different file
    /// from `levels/01.lvl` and resolving it would be another silent substitution.
    pub fn same_name_as(&self, path: &str) -> Vec<&str> {
        let Some(name) = lexical_path_key(path).last().cloned() else {
            return Vec::new();
        };
        self.paths
            .iter()
            .filter(|c| lexical_path_key(c).last() == Some(&name))
            .map(String::as_str)
            .collect()
    }

    /// The catalogue slots (as global indices) that belong to `cat`, in
    /// catalogue order. Used by the two-level browse to list a category's
    /// levels. Empty when no level falls in the category.
    #[must_use]
    pub fn indices_in(&self, cat: Category) -> Vec<usize> {
        self.paths
            .iter()
            .enumerate()
            .filter(|(_, p)| browse_category(Self::label(p)) == cat)
            .map(|(i, _)| i)
            .collect()
    }

    /// How many levels fall under `cat`.
    #[must_use]
    pub fn count_in(&self, cat: Category) -> usize {
        self.paths
            .iter()
            .filter(|p| browse_category(Self::label(p)) == cat)
            .count()
    }

    /// The stable progress id for the catalogue slot at `index` (the level's
    /// file stem). `None` when the index is out of range. This — not the
    /// catalogue index — is what [`crate::save::Progress`] keys on, so progress
    /// survives a catalogue reorder.
    #[must_use]
    pub fn id_at(&self, index: usize) -> Option<&str> {
        self.paths.get(index).map(|p| Self::label(p))
    }

    /// The index of the first level NOT marked solved in `progress`. Drives the
    /// **Continue** entry. Returns `None` only when the catalogue is empty; when
    /// *every* level is solved it returns `Some(0)` (Continue replays from the top —
    /// see [`MenuItem::Continue`]).
    ///
    /// **Continue follows the CAMPAIGN.** The first unsolved level in campaign-manifest
    /// order is preferred, because that is what a player means by "continue" — resume
    /// the story where I left it, not "the first `.lvl` I have not opened". By
    /// construction that level is also the deepest one the unlock gate allows, so
    /// Continue can never land on a locked level. Only when the campaign is complete
    /// (or the manifest is empty, or the catalogue does not contain the next campaign
    /// level — e.g. a focused test fixture) does it fall back to the historical
    /// whole-catalogue scan.
    #[must_use]
    pub fn first_unsolved(&self, progress: &crate::save::Progress) -> Option<usize> {
        if self.paths.is_empty() {
            return None;
        }
        // A SESSION LOOP in progress pins Continue to its cursor. After a
        // loop-back every campaign level is still marked solved — `solved` is never
        // cleared — so the "first unsolved" scan below would skip the whole replay
        // and land on the catalogue's first unsolved demo. The cursor is the only
        // thing that knows where the loop actually is.
        if let Some(cursor) = progress.campaign_cursor.as_deref() {
            if let Some(i) = self.paths.iter().position(|p| Self::label(p) == cursor) {
                return Some(i);
            }
        }
        if let Some(next) = campaign_manifest()
            .levels()
            .iter()
            .find(|l| !progress.is_solved(&l.id))
        {
            if let Some(i) = self.paths.iter().position(|p| Self::label(p) == next.id) {
                return Some(i);
            }
        }
        let first = self
            .paths
            .iter()
            .position(|p| !progress.is_solved(Self::label(p)));
        Some(first.unwrap_or(0))
    }
}

/// The LEXICAL comparison key for a path: its [`std::path::Path::components`]
/// with `.` segments dropped, case-folded on Windows (whose filesystem is
/// case-insensitive) and left verbatim elsewhere (where it is not).
///
/// `Path::components` IS the normalisation — on Windows it splits on both `\` and
/// `/` and collapses repeated separators, so nothing is hand-rolled here. The one
/// thing it deliberately keeps is a LEADING `.` (`Components` only normalises away
/// interior ones), which is why `CurDir` is filtered out: `./levels/01.lvl` and
/// `levels/01.lvl` name the same level.
///
/// Used by [`LevelCatalog::index_of_path`] as the fallback for paths that do not
/// exist on disk, so it must stay purely lexical — no filesystem access.
fn lexical_path_key(path: &str) -> Vec<String> {
    std::path::Path::new(path)
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .map(|c| {
            let s = c.as_os_str().to_string_lossy();
            if cfg!(windows) {
                s.to_lowercase()
            } else {
                s.into_owned()
            }
        })
        .collect()
}

/// Order key for a level path so the catalogue groups by [`Category`] (in
/// [`Category::ALL`] order) and then, WITHIN a category, by the campaign manifest
/// order for CAMPAIGN levels and by stem for everything else. This makes the flat
/// directory a deterministic, category-grouped list: `Start` (slot 0) and `Continue`
/// (first unsolved) walk the twelve authored levels in AUTHORED order — not
/// `L1, L10, L11, L12, L2, …`, which is what a lexical sort of the L-files gives —
/// and the two-level browse reads contiguous runs per category.
///
/// The secondary key is a string in both cases so the tuple type stays uniform; a
/// campaign level's key is its zero-padded manifest index, which sorts numerically
/// as text up to 999 levels.
fn catalog_sort_key(path: &str) -> (usize, String) {
    let stem = LevelCatalog::label(path);
    let cat = browse_category(stem);
    let order = Category::ALL.iter().position(|c| *c == cat).unwrap_or(0);
    let within = match campaign_manifest().position(stem) {
        Some(i) if cat == Category::Campaign => format!("{i:03}"),
        _ => stem.to_ascii_lowercase(),
    };
    (order, within)
}

impl Default for LevelCatalog {
    fn default() -> Self {
        // Scan EVERY *.lvl in the located `levels/` directory so all shipped levels
        // are reachable through the two-level browse. Each level is categorised by
        // `categorize`; the sort groups them so the campaign leads.
        //
        // NO FABRICATED FALLBACK. This used to invent `levels/01.lvl … 07.lvl` when
        // the scan failed — names that then failed to load one by one, so a
        // content-less install showed a full menu and did nothing when you pressed
        // Start. An unlocatable `levels/` is now an EMPTY catalogue, and `main`
        // turns that into a message naming every place it looked.
        let Ok(dir) = locate_levels_dir() else {
            return Self { paths: Vec::new() };
        };
        let prefix = catalog_prefix(&dir);
        let mut paths: Vec<String> = std::fs::read_dir(&dir)
            .ok()
            .map(|rd| {
                rd.filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "lvl"))
                    .filter_map(|p| p.file_name().map(|n| prefix.join(n)))
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        paths.sort_by_key(|p| catalog_sort_key(p));
        Self { paths }
    }
}

/// Which catalogue index is currently being played (or about to be played).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct CurrentLevel(pub usize);

/// The category the player drilled into from [`GameState::CategorySelect`]. The
/// [`GameState::LevelSelect`] screen lists only this category's levels and maps
/// its cursor back to the GLOBAL catalogue index when a level is confirmed.
/// Defaults to [`Category::Campaign`] so a direct jump into `LevelSelect`
/// (e.g. a focused test) shows the campaign.
#[derive(Resource, Clone, Copy, Debug)]
pub struct BrowseCategory(pub Category);

impl Default for BrowseCategory {
    fn default() -> Self {
        Self(Category::Campaign)
    }
}

/// The active level's DOOR cell, stashed at level load so the win watcher can
/// require the player to physically ENTER the door (not merely satisfy the
/// network) before the level completes. Inserted by `enter_playing` from
/// `level.door`, removed by `teardown_level`.
///
/// PRESENCE IS THE DOOR-LESS FALLBACK: when this resource is ABSENT (a level with
/// no door, or a focused test app that bypasses `enter_playing`), the win watcher
/// treats the player as always "at the door", preserving the historical
/// solved-alone-wins behaviour so a puzzle-only level is never made
/// uncompletable. When PRESENT, the win requires both `Solved` and
/// [`re_world::at_door`].
#[derive(Resource, Clone, Copy, Debug)]
pub struct DoorCell {
    /// The door cell's X coordinate.
    pub x: i32,
    /// The door cell's Y coordinate.
    pub y: i32,
    /// The door cell's Z (storey) coordinate.
    pub z: i32,
}

/// The optional explicit level path passed on the CLI. When present, "Start"
/// and the initial play target use it instead of the catalogue's first entry,
/// preserving the `terminal-state <path.lvl>` behaviour.
#[derive(Resource, Clone, Debug, Default)]
pub struct CliLevel(pub Option<String>);

/// One menu screen's cursor.
///
/// `index` is the highlighted item and `len` the item count. Navigation is now
/// direct: [`MenuNav::Up`]/[`MenuNav::Down`] move the highlight (wrapping) and
/// [`MenuNav::Confirm`] commits it — there is no idle timer any more.
///
/// `pending_commit` is a test/escape hatch: setting it true makes the per-state
/// `commit_*` system fire for the highlighted item on its next run, exactly as a
/// `Confirm` would (so existing headless tests can commit without synthesizing a
/// key message).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct MenuCursor {
    /// The highlighted item's index.
    pub index: usize,
    /// The current screen's item count, for wrapping navigation.
    pub len: usize,
    /// Test/escape hatch: when true the highlighted item commits on the next
    /// `commit_*` run, just like a [`MenuNav::Confirm`].
    pub pending_commit: bool,
}

impl MenuCursor {
    fn reset(&mut self, len: usize) {
        self.index = 0;
        self.len = len.max(1);
        self.pending_commit = false;
    }

    /// Move the highlight one step (wrapping). `delta` is +1 (down/next) or -1
    /// (up/prev).
    fn step(&mut self, delta: isize) {
        if self.len == 0 {
            return;
        }
        let len = self.len as isize;
        let i = self.index as isize + delta;
        self.index = i.rem_euclid(len) as usize;
    }
}

/// Splash auto-advance timer.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct SplashTimer(pub std::time::Duration);

/// How long the [`GameState::Diving`] tunnel transition plays before it
/// auto-advances to [`GameState::Playing`] (the player can skip it sooner with
/// any interact / nav key). ~2.5 s — long enough to read as a dive, short enough
/// not to nag.
pub const DIVE_DURATION: std::time::Duration = std::time::Duration::from_millis(2500);

/// The elapsed-time clock for the active [`GameState::Diving`] transition. Reset
/// to zero on `OnEnter(Diving)`; advanced by `time.delta` each `Update` frame.
/// `elapsed_secs()` feeds `re_render::render_tunnel` as the dive time `t`.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct DiveTimer(pub std::time::Duration);

impl DiveTimer {
    /// Elapsed dive time in seconds — the `t` argument to `render_tunnel`.
    #[must_use]
    pub fn elapsed_secs(&self) -> f32 {
        self.0.as_secs_f32()
    }
}

/// How long the [`GameState::Intro`] cold-boot pane holds before it auto-advances
/// (any key skips it sooner). Long enough to read the handshake and the Form A-7
/// addendum without pressure; short enough that a player who taps through is not
/// waiting on the game.
pub const INTRO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(14);

/// The elapsed-time clock for the [`GameState::Intro`] pane. Zeroed on entry,
/// advanced each `Update`. Exactly [`DiveTimer`]'s shape and role.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct IntroTimer(pub std::time::Duration);

/// Total on-screen time for the [`GameState::BootConsole`] pseudo-boot
/// sequence (the OPENING design, 2026-09-24): a kernel/BIOS-style log types
/// out, then breaks down into the splash eye. Within the approved "~4-6 s"
/// window. [`crate::boot_console`] splits this into a log-typewriter phase
/// and a breakdown phase (see its `boot_phase`/`log_phase_duration`).
pub const BOOT_CONSOLE_DURATION: std::time::Duration = std::time::Duration::from_millis(5000);

/// The elapsed-time clock for the active [`GameState::BootConsole`] state.
/// Zeroed on entry (`enter_boot_console`), advanced each `Update`
/// (`tick_boot_console`). Exactly [`IntroTimer`]'s shape and role;
/// [`crate::boot_console`]'s pure timeline/paint functions take
/// `elapsed_secs()` as their clock input.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct BootConsoleTimer(pub std::time::Duration);

impl BootConsoleTimer {
    /// Elapsed time in seconds.
    #[must_use]
    pub fn elapsed_secs(&self) -> f32 {
        self.0.as_secs_f32()
    }
}

/// How long the [`GameState::Ending`] card holds on its EYE frame before the
/// readout replaces it. Matches [`SPLASH_TIMEOUT`] deliberately: it is the same
/// beat, at the other end of the campaign.
pub const ENDING_EYE_WINDOW: std::time::Duration = SPLASH_TIMEOUT;

/// Which half of the end-card is on screen.
///
/// Two phases rather than one long scroll because the eye art is 22 rows tall on a
/// 32-row grid: sharing the pane would push `Process finished with exit code 0.`
/// below the fold. The splash solves the identical problem the identical way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndingPhase {
    /// The chosen eye frame (open-without-pupil, or never-opening).
    Eye,
    /// The scrollable readout + the offer row.
    Readout,
}

/// The elapsed-time clock for the [`GameState::Ending`] card.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct EndingTimer(pub std::time::Duration);

impl EndingTimer {
    /// Which phase the card is in.
    #[must_use]
    pub fn phase(&self) -> EndingPhase {
        if self.0 < ENDING_EYE_WINDOW {
            EndingPhase::Eye
        } else {
            EndingPhase::Readout
        }
    }

    /// Fast-forward past the eye (a keypress during the eye phase).
    fn skip_eye(&mut self) {
        self.0 = self.0.max(ENDING_EYE_WINDOW);
    }
}

/// The flow plugin: registers the state, the flow resources, and the per-state
/// enter/exit/update systems. Gameplay glue (interact, win watch) is gated to
/// [`GameState::Playing`] by the caller in `main`.
pub struct FlowPlugin {
    /// The CLI-supplied level path, if any (see `main`'s `--dump`/positional
    /// argument handling). `None` plays the catalogue's first entry.
    pub cli_level: Option<String>,
}

impl Plugin for FlowPlugin {
    fn build(&self, app: &mut App) {
        let catalog = LevelCatalog::default();
        app.init_state::<GameState>()
            // The present backend also registers `MenuNav`; `add_message` is
            // idempotent (it just inits the message resource), so doing it here
            // too guarantees the flow's readers never face an unregistered
            // message — including in headless tests that add no present plugin.
            .add_message::<MenuNav>()
            // Trigger-fields: the observable "arbitrary event" message + the live
            // per-level runtime state. Registered here so headless flow tests
            // (which add only `FlowPlugin`) can read both.
            .add_message::<TriggerFired>()
            .init_resource::<TriggerStates>()
            // The live one-line narrative beat a fired trigger paints on the HUD.
            // Registered here beside the trigger systems so the headless
            // flow tests — which add only `FlowPlugin` — can observe it.
            .init_resource::<NarrativeBeat>()
            // The trigger stinger channel. `AudioPlugin` registers
            // `PlaySound` too; `add_message` is idempotent, so registering here
            // keeps the subscriber writable in a headless test with no audio.
            .add_message::<re_audio::PlaySound>()
            .insert_resource(CliLevel(self.cli_level.clone()))
            .insert_resource(catalog)
            .init_resource::<CurrentLevel>()
            .init_resource::<CurrentLevelData>()
            // The category the level-select browse is scoped to.
            .init_resource::<BrowseCategory>()
            .init_resource::<MenuCursor>()
            .init_resource::<SplashTimer>()
            // The splash eye's per-launch seed + live mouse-gaze target.
            // Registered here beside `SplashTimer` for the same reason:
            // headless flow tests that add only `FlowPlugin` still exercise
            // `enter_splash`/
            // `track_splash_mouse`.
            .init_resource::<SplashEyeSeed>()
            .init_resource::<SplashMouse>()
            // The Diving-transition clock. Registered HERE (where its enter/tick
            // systems live) so the headless flow tests — which add only
            // `FlowPlugin` — have the resource the dive systems require.
            .init_resource::<DiveTimer>()
            // The framing states' own resources. Registered HERE (beside their
            // systems) for the same reason as `DiveTimer`: the headless flow tests
            // add only `FlowPlugin` and must still be able to drive Intro /
            // MemoryView / Ending / Epilogue end to end.
            .init_resource::<IntroTimer>()
            // The pseudo-boot-console clock. Registered HERE for the same
            // reason as `IntroTimer`/`DiveTimer`: the headless flow tests add
            // only `FlowPlugin` and must still be able to drive BootConsole
            // end to end (entering it directly via `NextState`, bypassing the
            // live TTY gate that decides whether production ever does).
            .init_resource::<BootConsoleTimer>()
            .init_resource::<EndingTimer>()
            .init_resource::<EndingOutcome>()
            .init_resource::<PaneScroll>()
            .init_resource::<MemoryViewReturn>()
            // Where the GUIDE was opened from (Menu vs the in-play Pause
            // overlay), so its `Back` returns to the right screen — same
            // reason as `MemoryViewReturn` right above.
            .init_resource::<GuideReturn>()
            // Where Settings was entered from (Menu vs the in-play Pause overlay),
            // so its `Back` returns to the right screen.
            .init_resource::<SettingsReturn>()
            // Word→story state (docs/lore/WORD_STORY.md): the just-solved
            // readout the Won overlay paints, and the accumulating memory log
            // (the story so far). Both engine-agnostic, headless-testable.
            .init_resource::<MemoryReadout>()
            .init_resource::<MemoryLog>()
            // Always-present render/puzzle resources (see module docs). The play
            // loop overwrites these on OnEnter(Playing); a 1x1 empty map keeps
            // `Res<Map>` valid for the raycaster in the menus.
            .insert_resource(empty_map())
            .insert_resource(Framebuffer::new(VIEW_COLS, VIEW_ROWS))
            .init_resource::<PuzzleState>()
            .init_resource::<Solved>()
            // The camera-motion gate the OnEnter/OnExit(Playing) systems below
            // drive. Registered HERE (where those systems live) so the headless
            // flow tests — which add only `FlowPlugin` — have the resource the
            // gate systems require. `init_resource` is insert-if-absent, so the
            // real app's build_core copy (also init'd) is unaffected.
            .init_resource::<InputGate>()
            // Persistent progress. `main` inserts the disk-loaded copy BEFORE
            // adding this plugin, so `init_resource` (insert-if-absent) keeps
            // it; headless tests that add only `FlowPlugin` get a fresh empty
            // one. Either way `OnEnter(Won)` can read+mutate it.
            .init_resource::<crate::save::Progress>()
            // Player settings. `main` inserts the disk-loaded copy BEFORE adding
            // this plugin (so `init_resource` keeps it); headless tests that add
            // only `FlowPlugin` get a fresh default. The settings screen mutates
            // it and re-applies/persists on Back.
            .init_resource::<crate::settings::Settings>()
            // P5 death-timer fog curve (reactive-materials §6). Registered here so
            // `enter_playing`'s `Res<FogCurve>` always resolves — including in the
            // headless flow tests that add only `FlowPlugin`. `init_resource` is
            // insert-if-absent, so a campaign that wants a different curve can
            // insert its own copy before adding this plugin.
            .init_resource::<crate::fog::FogCurve>()
            // The present-layer resources the settings screen drives, registered
            // here so the headless flow tests (which add no present plugin) can
            // observe the menu mutating them. `main`/the present backend insert
            // their own copies; `init_resource` is insert-if-absent, so the real
            // app keeps the backend's resource and tests get a default.
            .init_resource::<re_present::ColorMode>()
            .init_resource::<re_present::MouseSettings>()
            // The live present-cache colour deadband the COLOUR STEPS settings
            // row drives. `main` inserts the disk-loaded
            // value at `Startup` (see `apply_color_steps_on_startup` — this
            // resource is a special case among the present-layer ones: it is
            // inserted with a HARD `insert_resource` inside `PresentPlugin`'s
            // build, not `init_resource`, so the ordinary "pre-insert before
            // the plugin" trick cannot apply here); headless flow tests that
            // add no present plugin get the engine default (6) so
            // `settings_nav` can observe the menu mutating it.
            .init_resource::<re_present::PresentConfig>()
            // The live raycaster FOV the settings FOV/stretch rows and the
            // `[`/`]` hotkey drive. `main` inserts the disk-loaded copy BEFORE
            // adding CompositeRenderPlugin (whose `init_resource` is
            // insert-if-absent), so the saved FOV wins in the real app; headless
            // flow tests that add no renderer get a default here so they can
            // observe the menu/hotkey mutating it.
            .init_resource::<re_render::Fov>()
            // The live cast-shadow quality the SHADOWS settings row drives. `main`
            // inserts the disk-loaded copy BEFORE adding CompositeRenderPlugin
            // (whose `init_resource` is insert-if-absent), so the saved value wins
            // in the real app; headless flow tests that add no renderer get the
            // engine default (Soft) here so they can observe the menu mutating it.
            .init_resource::<re_render::ShadowQuality>()
            // The live GRAPHICS-QUALITY render levers the quality settings rows drive
            // (draw distance → FarFreeze, LOD aggressiveness → LodConfig, sub-cell →
            // SubCellConfig, post-fx → PostfxConfig). `main` inserts the disk-loaded
            // copies BEFORE the renderer/postfx plugins (whose inits are
            // insert-if-absent), so the saved values win in the real app; headless flow
            // tests that add no renderer get the engine defaults here so `settings_nav`
            // can observe the menu mutating them without a missing-resource panic.
            .init_resource::<re_render::FarFreeze>()
            .init_resource::<re_render::LodConfig>()
            .init_resource::<re_render::composite::SubCellConfig>()
            .init_resource::<re_postfx::PostfxConfig>()
            // The live particle EMITTER SET the composite renderer ticks each frame.
            // `enter_playing` OVERWRITES it from the loaded level's `[emitter]`
            // specs and teardown restores the default. `main`'s CompositeRenderPlugin
            // (via EmitterPlugin) also `init_resource`s it — insert-if-absent, same
            // default — so there is no double-init conflict; headless flow tests that
            // add no renderer get the default here so teardown's `ResMut` resolves.
            .init_resource::<re_render::EmitterSet>()
            // The FOV-cycle message the present backends emit (`[`/`]`, gamepad
            // Y). Registered here (idempotent) so the flow's consumer never faces
            // an unregistered message — including in headless tests with no
            // present plugin.
            .add_message::<FovCycle>()
            // Backend-neutral mouse hover/click, registered here (idempotent —
            // `re_present::PresentPlugin` also registers both) so
            // `clear_stale_nav` — which drains them alongside `MenuNav` on
            // every state entry — never faces an unregistered message,
            // including in headless flow tests that add no present plugin.
            .add_message::<MenuMouseMove>()
            .add_message::<MenuMouseClick>()
            // The glitch post-process config the flow drives per-level. `main`'s
            // `GlitchPlugin` inserts its own copy; `init_resource` is
            // insert-if-absent, so the real app keeps the plugin's resource while
            // headless flow tests (no GlitchPlugin) get a default (= off).
            .init_resource::<GlitchConfig>()
            // Audio volume changes are surfaced as a message; register it so the
            // settings nav can write it even in a headless test with no audio
            // plugin. The real app's `AudioPlugin` also registers it (idempotent).
            .add_message::<re_audio::SetMasterVolume>()
            // The per-level ambience mood the play loader emits on OnEnter(Playing).
            // Registered here (idempotent — `AudioPlugin` registers it too) so the
            // headless flow tests, which add no audio plugin, can observe the bed
            // being set per level.
            .add_message::<re_audio::SetAmbience>()
            // The two campaign-story audio channels the flow drives: the
            // MOVEMENT theme (sent on OnEnter(Playing) for a campaign level) and
            // the peak-solve SILENCE (sent on OnEnter(Won) for L7 / L11). Registered
            // here too (idempotent) so the headless flow tests, which add no audio
            // plugin, can observe both.
            .add_message::<re_audio::SetTheme>()
            .add_message::<re_audio::SolveSilence>()
            // The campaign-progress → MIRROR-CLARITY ramp. Registered as
            // part of the play flow (a nested plugin, so the module owns its own
            // resource init + system) rather than from the binary, so every consumer
            // of `FlowPlugin` — including the headless flow tests — gets the ramp.
            .add_plugins(crate::mirror_ramp::MirrorRampPlugin)
            // The campaign-progress → PALETTE-GATE ramp: the twin of the
            // mirror-clarity ramp above, on the colour-unlock axis. Only
            // computes the `PaletteGateState` resource here (`Update`,
            // order-independent); the per-frame `apply_gate` call is wired
            // directly by the binary (`PaletteGateFramePlugin`) so it can sit
            // precisely between `PostfxPlugin` and `GlitchPlugin`.
            .add_plugins(crate::palette_ramp::PaletteRampPlugin)
            // The colour-drift field's source computation: powered-pipe hue
            // wave + player-gaze desaturation. Only computes the
            // `DriftSources` resource here (`Update`); the
            // per-frame `apply_drift` call is wired by the binary between
            // `CompositeRenderPlugin` and `PostfxPlugin`.
            .add_plugins(re_runtime::color_drift_sources::ColorDriftPlugin)
            // Hydrate the in-memory story from persisted progress at startup so
            // the accumulated memory log survives a restart (the "story so far"
            // the Won overlay shows includes prior sessions' phrases).
            .add_systems(Startup, hydrate_memory_log)
            // COLOUR STEPS: apply the saved deadband once at boot — see
            // `apply_color_steps_on_startup`'s doc for why this MUST be
            // `Startup` rather than a build_core pre-insert.
            .add_systems(Startup, apply_color_steps_on_startup)
            // The live FOV-cycle hotkey: drains `FovCycle` (`[`/`]`, gamepad Y)
            // in EVERY state — playing AND the menus — so the player can see the
            // 3D view change immediately. Cheap when no message is queued.
            .add_systems(Update, cycle_fov_live)
            // BootConsole (the OPENING pseudo-boot console, before Splash).
            .add_systems(OnEnter(GameState::BootConsole), enter_boot_console)
            .add_systems(
                Update,
                tick_boot_console.run_if(in_state(GameState::BootConsole)),
            )
            // Splash.
            .add_systems(OnEnter(GameState::Splash), enter_splash)
            .add_systems(Update, tick_splash.run_if(in_state(GameState::Splash)))
            .add_systems(
                Update,
                track_splash_mouse.run_if(in_state(GameState::Splash)),
            )
            // Menu. `clear_stale_nav` runs FIRST on entry (OnEnter order is
            // add-order) so the `MenuNav::Confirm` that committed the PREVIOUS
            // screen — still alive in the double-buffered `Messages<MenuNav>` for
            // one more frame — cannot leak into THIS screen's nav system and
            // auto-commit at index 0. See [`clear_stale_nav`].
            .add_systems(OnEnter(GameState::Menu), (clear_stale_nav, enter_menu))
            .add_systems(Update, menu_nav.run_if(in_state(GameState::Menu)))
            // Category select (the first level of the two-level browse). The
            // stale-nav clear prevents the menu's `Confirm` (still double-buffered)
            // from auto-entering a category.
            .add_systems(
                OnEnter(GameState::CategorySelect),
                (clear_stale_nav, enter_category_select),
            )
            .add_systems(
                Update,
                category_select_nav.run_if(in_state(GameState::CategorySelect)),
            )
            // Level select. The stale-nav clear is the fix for the "Select always
            // starts level 0" bug: without it the menu's commit `Confirm` carried
            // over and `level_select_nav` committed the highlighted (index-0) level
            // on the very first frame, before the player could move the cursor.
            .add_systems(
                OnEnter(GameState::LevelSelect),
                (clear_stale_nav, enter_level_select),
            )
            .add_systems(
                Update,
                level_select_nav.run_if(in_state(GameState::LevelSelect)),
            )
            // Settings screen.
            .add_systems(
                OnEnter(GameState::Settings),
                (clear_stale_nav, enter_settings),
            )
            .add_systems(Update, settings_nav.run_if(in_state(GameState::Settings)))
            // Playing. `assemble_readout` runs on OnExit(Playing) BEFORE
            // `exit_playing` tears the level data down (OnExit order is
            // add-order), so the live Level + PuzzleState are still intact when
            // the readout is built. It only emits a phrase when the level was
            // genuinely solved (it reads `Solved`), so leaving Playing for any
            // other reason records nothing.
            // Diving transition (spec §4.2). A player-initiated level-entry
            // commit (menu Start/Continue, level-select, won Next-level) routes
            // here instead of straight to Playing; this short, skippable tunnel
            // set-piece plays, then auto-advances to Playing (which loads the
            // level exactly as before). `OnEnter` closes the camera gate + resets
            // the clock; `OnExit` re-opens the gate the same way OnEnter(Playing)
            // does so motion resumes. The per-frame `paint_dive` fills the
            // framebuffer with the tunnel and runs `.before(PresentSet)` — the
            // established over-frame ordering fix (same as the Pause/HUD painters)
            // so the present blit never races the paint. `tick_diving` advances
            // the clock and drives the auto-exit / skip transition.
            .add_systems(OnEnter(GameState::Diving), (clear_stale_nav, enter_diving))
            .add_systems(Update, tick_diving.run_if(in_state(GameState::Diving)))
            .add_systems(
                Last,
                paint_dive
                    .run_if(in_state(GameState::Diving))
                    .before(re_present::PresentSet),
            )
            .add_systems(OnExit(GameState::Diving), exit_diving)
            // `clear_stale_nav` FIRST, for the same reason every other state entry
            // carries it — and Playing needs it most. `Messages<MenuNav>` is
            // double-buffered (~2 frames), so the `Back` that `pause_nav` consumed to
            // RESUME is still alive when the transition lands. `watch_pause` runs in
            // `Update` under `in_state(Playing)` and drains the whole queue, so it
            // finds that same `Back` and re-opens Pause on the very next frame: Esc
            // could never resume, and Esc-to-skip the dive dropped you into Pause
            // instead of the level. Playing was the one entry point missing the
            // established defence.
            .add_systems(
                OnEnter(GameState::Playing),
                (clear_stale_nav, enter_playing),
            )
            // Camera-motion gate (the authoritative half of the pause fix).
            // `InputGate.camera_motion` is enabled ONLY while in Playing: set
            // true on entry here, cleared on EVERY exit (see the OnExit
            // registration below). Kept as small dedicated systems, separate
            // from the enter_playing/exit_playing teardown-skip logic, so the
            // gate is unconditional and trivially testable. Resume (Pause →
            // Playing) re-fires this OnEnter, re-enabling motion.
            .add_systems(OnEnter(GameState::Playing), open_camera_gate)
            .add_systems(OnExit(GameState::Playing), close_camera_gate)
            // While playing, re-derive the glitch config if the player toggles the
            // CORRUPTION accessibility setting at runtime (e.g. via the pause/menu
            // path) so the change takes effect immediately.
            .add_systems(
                Update,
                reapply_glitch_on_settings_change.run_if(in_state(GameState::Playing)),
            )
            // Floor-zone inputs (L5 "Soft-Footstep"): each frame, hold every
            // `[zone_input]` node at 1 while the player stands inside its box.
            // Runs in Playing; `re_puzzle::apply_interactions` (PuzzlePlugin,
            // also Update) mirrors the refreshed `solved` into `Solved` for the
            // win watcher. Cheap when a level authored no zones.
            .add_systems(Update, zone_input_sys.run_if(in_state(GameState::Playing)))
            // Trigger-fields: each frame, fire any trigger the player just walked
            // INTO (OUTSIDE→INSIDE edge), respecting `once`. Runs in Playing;
            // re-arming happens on level (re-)load in `enter_playing`. Cheap when
            // a level authored no `[trigger]` section.
            //
            // `trigger_subscribers` is CHAINED after the firing system so a walk-in
            // stings and paints its narrative beat on the SAME frame it fires
            // (messages live two frames, so an unordered pair would lag by one).
            .add_systems(
                Update,
                (trigger_sys, trigger_subscribers)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            // While playing, Back/Esc opens the Pause overlay (it does NOT quit).
            .add_systems(Update, watch_pause.run_if(in_state(GameState::Playing)))
            // Pause overlay: Resume / Settings / Quit-to-menu. `watch_pause`
            // already drained `MenuNav` when it opened the overlay, but clear again
            // on entry for symmetry with the other menu screens (and so a Settings
            // → Pause return starts clean).
            .add_systems(OnEnter(GameState::Pause), (clear_stale_nav, enter_pause))
            .add_systems(Update, pause_nav.run_if(in_state(GameState::Pause)))
            // Quit-to-menu (Pause → Menu) tears the kept-alive level down here,
            // since this edge never fires OnExit(Playing).
            .add_systems(OnExit(GameState::Pause), teardown_level_on_quit)
            // `capture_ending` sits BETWEEN them: it must see the live
            // `PuzzleState` (so `cycle_held` is still meaningful) and must run
            // after `assemble_readout` so the two agree on which form the readout
            // took. `exit_playing` then tears the level down.
            .add_systems(
                OnExit(GameState::Playing),
                (assemble_readout, capture_ending, exit_playing).chain(),
            )
            // Won. `record_progress` runs alongside `enter_won` (OnEnter order
            // is add-order; both are independent), marking the just-solved
            // level and persisting it to disk. `assemble_readout` then builds
            // this level's memory-readout phrase from the live Level +
            // PuzzleState and appends it to the growing memory log.
            // Won. `record_progress` runs BEFORE `route_finale` (OnEnter order is
            // add-order), so the finale is already marked solved — the campaign is
            // COMPLETE on both endings — by the time the end-card is routed to.
            .add_systems(
                OnEnter(GameState::Won),
                (
                    clear_stale_nav,
                    enter_won,
                    record_progress,
                    peak_solve_silence,
                    route_finale,
                )
                    .chain(),
            )
            .add_systems(Update, won_nav.run_if(in_state(GameState::Won)))
            // ──────────────────── the framing states ─────────────────────────
            // Every one is the Diving triple: OnEnter resets the pane clock +
            // scroll, an Update system reads `MenuNav`, and `crate::screens` owns
            // the painter (registered by `MenuRenderPlugin`, since painting is
            // presentation).
            .add_systems(OnEnter(GameState::Intro), (clear_stale_nav, enter_intro))
            .add_systems(Update, tick_intro.run_if(in_state(GameState::Intro)))
            .add_systems(
                OnEnter(GameState::MemoryView),
                (clear_stale_nav, enter_pane),
            )
            .add_systems(
                Update,
                memory_view_nav.run_if(in_state(GameState::MemoryView)),
            )
            // GUIDE. `enter_guide` sizes the chapter cursor AND ticks the
            // `opened_guide` first-step — the one first-step observed at
            // STATE ENTRY rather than during ordinary play.
            .add_systems(OnEnter(GameState::Guide), (clear_stale_nav, enter_guide))
            .add_systems(Update, guide_nav.run_if(in_state(GameState::Guide)))
            .add_systems(OnEnter(GameState::Ending), (clear_stale_nav, enter_ending))
            .add_systems(Update, ending_nav.run_if(in_state(GameState::Ending)))
            .add_systems(OnEnter(GameState::Epilogue), (clear_stale_nav, enter_pane))
            .add_systems(Update, epilogue_nav.run_if(in_state(GameState::Epilogue)));
    }
}

/// A tiny always-valid map so `re_render`'s `Res<Map>` never panics while no
/// real level is loaded (menus). It has no camera spawned, so the raycaster
/// early-returns and never actually reads it.
fn empty_map() -> Map {
    Map::new(1, 1, 1)
}

/// `OnEnter(Playing)`: open the camera-motion gate. `re_present` only applies
/// move/turn/look while [`InputGate::camera_motion`] is set, so this is the
/// state-driven authoritative enable. Resume from Pause re-fires this entry.
fn open_camera_gate(mut gate: ResMut<InputGate>) {
    gate.camera_motion = true;
}

/// `OnExit(Playing)`: close the camera-motion gate so the camera freezes in
/// every non-Playing state — Pause, Won, and Quit-to-menu all leave Playing and
/// hit this. Unconditional (no teardown-skip coupling); the matching
/// [`open_camera_gate`] re-enables on the next OnEnter(Playing).
fn close_camera_gate(mut gate: ResMut<InputGate>) {
    gate.camera_motion = false;
}

// ─────────────────────────────── diving ─────────────────────────────────────

/// `OnEnter(Diving)`: close the camera-motion gate (freeze the camera exactly as
/// the pause path does) and reset the [`DiveTimer`] so the tunnel plays from
/// `t = 0`. The matching [`exit_diving`] re-opens the gate when the dive ends.
fn enter_diving(mut gate: ResMut<InputGate>, mut timer: ResMut<DiveTimer>) {
    gate.camera_motion = false;
    timer.0 = std::time::Duration::ZERO;
}

/// `OnExit(Diving)`: re-open the camera-motion gate the same way
/// [`open_camera_gate`] (`OnEnter(Playing)`) does, so camera motion resumes the
/// instant the dive hands off to play. (The dive only ever exits INTO Playing, so
/// this is symmetric with that entry; keeping it here as a dedicated unconditional
/// system mirrors the `open_camera_gate`/`close_camera_gate` split.)
fn exit_diving(mut gate: ResMut<InputGate>) {
    gate.camera_motion = true;
}

/// `Update` (Diving only): advance the [`DiveTimer`] and transition to
/// [`GameState::Playing`] when the dive duration elapses OR the player skips with
/// any interact / nav key. The tunnel itself is painted by [`paint_dive`] in
/// `Last`; this system only owns the clock + the exit decision.
fn tick_diving(
    time: Res<Time>,
    mut timer: ResMut<DiveTimer>,
    mut nav: MessageReader<MenuNav>,
    mut presses: MessageReader<InteractPressed>,
    mut next: ResMut<NextState<GameState>>,
) {
    timer.0 += time.delta();
    // Skip on any neutral input (the same events the menus / interact use).
    let skipped = nav.read().count() > 0 || presses.read().count() > 0;
    if skipped || timer.0 >= DIVE_DURATION {
        next.set(GameState::Playing);
    }
}

/// `Last` (Diving only): fill the whole framebuffer with the tunnel set-piece for
/// the current dive time. Reuses `re_render::render_tunnel` with the default
/// curling-Droste params. Runs `.before(PresentSet)` so the present blit shows
/// this frame, not a stale one. No camera exists during the dive (the level is
/// not loaded until OnEnter(Playing)), so the raycaster never overwrites this.
fn paint_dive(timer: Res<DiveTimer>, mut fb: ResMut<Framebuffer>) {
    re_render::render_tunnel(
        &mut fb,
        timer.elapsed_secs(),
        &re_render::TunnelParams::default(),
    );
}

/// `Startup`: rebuild the in-memory [`MemoryLog`] from the persisted phrases in
/// [`crate::save::Progress`], in catalogue (level) order, so the accumulated
/// story spans sessions. Levels solved before phrases were persisted (an old
/// save) simply contribute no line until re-solved — additive + back-compatible.
fn hydrate_memory_log(
    catalog: Res<LevelCatalog>,
    progress: Res<crate::save::Progress>,
    mut log: ResMut<MemoryLog>,
) {
    log.entries.clear();
    for path in &catalog.paths {
        let id = LevelCatalog::label(path);
        if let Some(phrase) = progress.phrase_for(id) {
            log.record(MemoryEntry {
                level_id: id.to_string(),
                phrase: phrase.to_string(),
            });
        }
    }
}

// ─────────────────────────────── splash ─────────────────────────────────────

/// The procedural splash eye's per-LAUNCH seed (`re_asciifx::eye::Eye::random`'s
/// input) — resolved ONCE from the wall clock the first time this resource is
/// initialized, so the eye's variant/size/hue/frequencies differ launch to
/// launch while staying perfectly stable — same eye, same frame for the same
/// `(elapsed, gaze)` — for the rest of that run:
/// [`crate::menu::paint_splash`] reads this resource's VALUE, never the clock
/// directly, so it stays a pure function of its arguments — exactly what
/// `boot_console::breakdown_reaches_the_real_splash_frame` depends on (it
/// calls `paint_splash` twice with a literal seed and asserts byte-identical
/// output).
#[derive(Resource, Clone, Copy, Debug)]
pub struct SplashEyeSeed(pub u64);

impl Default for SplashEyeSeed {
    fn default() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        Self(re_asciifx::hash::hash64(nanos, 0x5E1D_5EED, 0))
    }
}
/// Last mouse position observed while on the splash screen, in terminal-cell
/// coordinates. `None` until the player's first mouse move — until then the
/// procedural eye has no target and falls back to its own autonomous
/// saccade pattern (see `re_asciifx::eye::Eye::with_target`).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct SplashMouse(pub Option<(u16, u16)>);

fn enter_splash(mut timer: ResMut<SplashTimer>, mut mouse: ResMut<SplashMouse>) {
    timer.0 = std::time::Duration::ZERO;
    *mouse = SplashMouse::default();
}

/// `Update` (Splash only): fold this frame's `MenuMouseMove` into
/// [`SplashMouse`] (last-wins) — the procedural splash eye's live gaze
/// target. Runs whether or not the mouse has moved yet; a level with no
/// mouse motion at all leaves `SplashMouse` at `None`, so the eye falls back
/// to its own autonomous saccade pattern instead of tracking a stale
/// position.
fn track_splash_mouse(
    mut mouse_events: MessageReader<MenuMouseMove>,
    mut splash_mouse: ResMut<SplashMouse>,
) {
    for m in mouse_events.read() {
        splash_mouse.0 = Some((m.col, m.row));
    }
}

/// Any navigation key (`Confirm`/`Up`/`Down`/…), an interact press, or a
/// timeout leaves the splash for the menu. `Back` (Esc/q) on the splash is
/// treated as "skip to menu" too — there is nothing behind the splash.
fn tick_splash(
    time: Res<Time>,
    mut timer: ResMut<SplashTimer>,
    mut nav: MessageReader<MenuNav>,
    mut presses: MessageReader<InteractPressed>,
    mut next: ResMut<NextState<GameState>>,
) {
    timer.0 += time.delta();
    let keyed = nav.read().count() > 0 || presses.read().count() > 0;
    if keyed || timer.0 >= SPLASH_TIMEOUT {
        next.set(GameState::Menu);
    }
}

// ──────────────────────────── shared cursor ─────────────────────────────────

/// `OnEnter` (every menu screen): drop any [`MenuNav`] already sitting in the
/// double-buffered message queue BEFORE this screen's nav system runs.
///
/// THE BUG THIS FIXES (#5 "Select always starts the first level"): Bevy's
/// `Messages<MenuNav>` are double-buffered — a message stays readable for the
/// frame it was written AND the next frame. The keypress that COMMITTED the
/// previous screen (e.g. `MenuNav::Confirm` on "SELECT LEVEL" in `menu_nav`) is
/// therefore still in the queue when the freshly-entered screen's nav system
/// (`level_select_nav`) runs on the next frame. That system uses a *fresh*
/// `MessageReader`, whose cursor starts at the front of the live buffer, so it
/// re-reads the stale `Confirm` and immediately commits the highlighted item —
/// which is index 0, because the player has not had a frame to move the cursor.
/// Result: Level Select (and any menu reached by a Confirm) auto-picked entry 0.
///
/// Draining the queue on entry makes every screen start from a clean slate:
/// only navigation the player performs WHILE on the screen is honoured. This is
/// the same defense [`watch_pause`] already uses (`nav.drain()`), generalised to
/// every menu `OnEnter`. Headless tests that drive transitions with
/// `NextState` directly never wrote the stale `Confirm`, which is exactly why
/// they passed while the live keyboard flow broke.
fn clear_stale_nav(
    mut nav: ResMut<Messages<MenuNav>>,
    mut mouse_move: ResMut<Messages<MenuMouseMove>>,
    mut mouse_click: ResMut<Messages<MenuMouseClick>>,
) {
    nav.clear();
    // The mouse-hover/click bridge (`crate::menu_mouse`) has the EXACT same
    // stale-double-buffer exposure as `MenuNav`: each state's own mouse-nav
    // system keeps its own `MessageReader` cursor, so a click emitted just
    // before a state transition (e.g. clicking "SELECT LEVEL" on the main
    // menu) is still live in the double-buffered queue when the freshly
    // entered screen's mouse-nav system runs on the next frame — and that
    // system would hit-test the SAME (col, row) against ITS OWN, different,
    // item rects and could spuriously commit whatever happens to sit there.
    mouse_move.clear();
    mouse_click.clear();
}

/// The outcome of reading [`MenuNav`] for a menu screen: how the highlight
/// moved and whether the player asked to commit or go back. Shared by every
/// menu-like state; the per-state system maps `commit`/`back` to transitions.
#[derive(Clone, Copy, Debug, Default)]
struct NavOutcome {
    /// `Confirm` was pressed (or [`MenuCursor::pending_commit`] was set).
    commit: bool,
    /// `Back` (Esc/q) was pressed.
    back: bool,
}

/// Drain [`MenuNav`] for a menu screen: move the highlight on `Up`/`Down`
/// (wrapping) and report `Confirm`/`Back`. `Left`/`Right` are accepted as
/// `Up`/`Down` aliases so the strafe keys also step the list. The
/// [`MenuCursor::pending_commit`] escape hatch is folded into `commit` and
/// cleared, so tests that set it behave exactly like a real `Confirm`.
fn read_nav(cursor: &mut MenuCursor, nav: &mut MessageReader<MenuNav>) -> NavOutcome {
    let mut out = NavOutcome::default();
    for msg in nav.read() {
        match msg {
            MenuNav::Up | MenuNav::Left => cursor.step(-1),
            MenuNav::Down | MenuNav::Right => cursor.step(1),
            MenuNav::Confirm => out.commit = true,
            MenuNav::Back => out.back = true,
        }
    }
    if cursor.pending_commit {
        cursor.pending_commit = false;
        out.commit = true;
    }
    out
}

// ──────────────────────────────── menu ──────────────────────────────────────

/// The main-menu items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    /// Play the first level (or the CLI level, if one was given).
    Start,
    /// Jump straight to the first UNSOLVED level (the player's progress
    /// frontier). When every level is solved, Continue replays from level 0.
    /// While a session LOOP is in progress it follows the loop cursor instead
    /// ([`crate::save::Progress::campaign_cursor`]).
    Continue,
    /// Open the [`GameState::MemoryView`] journal.
    Memory,
    /// Open the level picker.
    Select,
    /// Open the settings screen.
    Settings,
    /// Open the [`GameState::Guide`].
    Guide,
    /// **The loop offer.** Shown ONLY when the campaign is complete —
    /// on either ending, since both mark it complete. Accepting opens the next
    /// session with the cursor back on level 1, carrying the journal.
    BeginAgain,
    /// Quit the game.
    Quit,
}

/// The main-menu items that are ALWAYS present, in display order. The loop offer
/// ([`MenuItem::BeginAgain`]) is conditional and lives in [`menu_items`]; this
/// array stays the base list so nothing that only needs "where is SETTINGS" has to
/// care about campaign state.
pub const MENU_ITEMS: [MenuItem; 7] = [
    MenuItem::Start,
    MenuItem::Continue,
    MenuItem::Memory,
    MenuItem::Select,
    MenuItem::Settings,
    MenuItem::Guide,
    MenuItem::Quit,
];

/// The main-menu items for THIS save: [`MENU_ITEMS`], plus the diegetic loop offer
/// directly above QUIT once the campaign is complete.
///
/// The offer is an ENTRY, not a forced transition — the plan's "the loop is
/// OFFERED, not imposed". A completed campaign otherwise behaves exactly as
/// before.
#[must_use]
pub fn menu_items(progress: &crate::save::Progress) -> Vec<MenuItem> {
    let mut items = MENU_ITEMS.to_vec();
    if campaign_manifest().is_complete(progress) {
        items.insert(items.len() - 1, MenuItem::BeginAgain);
    }
    items
}

fn enter_menu(mut cursor: ResMut<MenuCursor>, progress: Res<crate::save::Progress>) {
    cursor.reset(menu_items(&progress).len());
}

/// The read-only level context the main menu consults to resolve a play target:
/// the CLI override, the catalogue, and the player's progress. Bundled as one
/// [`SystemParam`] so `menu_nav` stays within the argument budget.
#[derive(bevy::ecs::system::SystemParam)]
struct MenuLevelCtx<'w> {
    cli: Res<'w, CliLevel>,
    catalog: Res<'w, LevelCatalog>,
    /// Mutable because the loop offer ([`MenuItem::BeginAgain`]) writes the
    /// session + cursor here. Every other item only reads it.
    progress: ResMut<'w, crate::save::Progress>,
}

/// The state-transition sinks `menu_nav` may write, bundled to stay inside Bevy's
/// system-argument budget now that the menu routes to four more screens.
#[derive(bevy::ecs::system::SystemParam)]
struct MenuSinks<'w> {
    current: ResMut<'w, CurrentLevel>,
    settings_return: ResMut<'w, SettingsReturn>,
    memory_return: ResMut<'w, MemoryViewReturn>,
    guide_return: ResMut<'w, GuideReturn>,
    next: ResMut<'w, NextState<GameState>>,
    exit: MessageWriter<'w, AppExit>,
}

/// Main-menu navigation: `Up`/`Down` move the highlight, `Confirm` commits the
/// highlighted item, `Back` (Esc/q) quits the game.
fn menu_nav(
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageReader<MenuNav>,
    mut level: MenuLevelCtx,
    mut sinks: MenuSinks,
) {
    let out = read_nav(&mut cursor, &mut nav);
    if out.back {
        // From the top menu, Back exits the game (Esc/q is the universal quit).
        sinks.exit.write(AppExit::Success);
        return;
    }
    if !out.commit {
        return;
    }
    let items = menu_items(&level.progress);
    match items[cursor.index.min(items.len() - 1)] {
        MenuItem::Start => {
            // CLI level (if any) maps to its catalogue slot; else level 0.
            //
            // The match is PATH-AWARE ([`LevelCatalog::index_of_path`]), so
            // `levels\01.lvl` — the natural form on Windows — resolves to the same
            // slot as `levels/01.lvl` instead of missing and quietly playing level
            // 0. The `unwrap_or(0)` below is NOT a silent fallback in the shipped
            // binary: `main.rs` resolves the CLI level against this same catalogue
            // at startup and exits non-zero with a clear message when it is not
            // there, so an unresolvable path can never reach here from the CLI. It
            // remains only as the panic-free default for a headless test that
            // inserts a `CliLevel` its fixture catalogue does not contain.
            sinks.current.0 = level
                .cli
                .0
                .as_deref()
                .and_then(|p| level.catalog.index_of_path(p))
                .unwrap_or(0);
            // A BRAND-NEW campaign boots through the Stewardship cold-start
            // first, on campaign start (first manifest level); every other
            // Start goes straight to the dive, exactly as before. Three conditions,
            // all necessary: no CLI level (that is a direct request for a specific
            // room, never the campaign opening), the target IS the campaign's first
            // authored level (a catalogue whose slot 0 is an archive room or a demo
            // is not a campaign start), and the save is untouched.
            let opens_campaign = level
                .catalog
                .id_at(sinks.current.0)
                .is_some_and(|id| campaign_manifest().first_id() == Some(id));
            if level.cli.0.is_none() && opens_campaign && is_fresh_campaign(&level.progress) {
                sinks.next.set(GameState::Intro);
            } else {
                // Play the Diving tunnel transition on the way INTO the level; it
                // auto-advances to Playing (which does the actual level load).
                sinks.next.set(GameState::Diving);
            }
        }
        MenuItem::Continue => {
            // Jump to the player's frontier: the loop cursor if a session loop is
            // running, else the first level not yet solved. When all are solved and
            // no loop is open, `first_unsolved` returns 0 (replay from the top); an
            // empty catalogue stays on the menu rather than crashing. Continue NEVER
            // shows the intro — it is by definition not a new campaign.
            if let Some(idx) = level.catalog.first_unsolved(&level.progress) {
                sinks.current.0 = idx;
                sinks.next.set(GameState::Diving);
            }
        }
        MenuItem::Memory => {
            *sinks.memory_return = MemoryViewReturn::Menu;
            sinks.next.set(GameState::MemoryView);
        }
        MenuItem::Select => sinks.next.set(GameState::CategorySelect),
        MenuItem::Settings => {
            *sinks.settings_return = SettingsReturn::Menu;
            sinks.next.set(GameState::Settings);
        }
        MenuItem::Guide => {
            *sinks.guide_return = GuideReturn::Menu;
            sinks.next.set(GameState::Guide);
        }
        MenuItem::BeginAgain => {
            if accept_loop_offer(&mut level.progress, &level.catalog, &mut sinks.current) {
                sinks.next.set(GameState::Diving);
            }
        }
        MenuItem::Quit => {
            sinks.exit.write(AppExit::Success);
        }
    }
}

// ──────────────────────────── category select ───────────────────────────────

/// The categories `catalog` actually holds at least one level in, in
/// [`Category::ALL`] order.
///
/// Hides empty categories by design: a folder that always dead-ends on
/// `Back` is a UI trap, not neutral information, and the game repo's own
/// shipped `levels/` genuinely holds nothing in six of the nine categories
/// (Demos, Showcases, Portals, Texture, Feature, Test all live in the engine
/// repo's example levels instead). The browse screen offers only what
/// `catalog` can actually play.
#[must_use]
pub fn visible_categories(catalog: &LevelCatalog) -> Vec<Category> {
    Category::ALL
        .into_iter()
        .filter(|cat| catalog.count_in(*cat) > 0)
        .collect()
}

fn enter_category_select(mut cursor: ResMut<MenuCursor>, catalog: Res<LevelCatalog>) {
    cursor.reset(visible_categories(&catalog).len());
}

/// Category-select navigation (the first level of the two-level browse):
/// `Up`/`Down` move between the folders [`visible_categories`] offers,
/// `Confirm` enters the highlighted category's level list, `Back` (Esc/q)
/// returns to the main menu.
fn category_select_nav(
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageReader<MenuNav>,
    mut browse: ResMut<BrowseCategory>,
    catalog: Res<LevelCatalog>,
    mut next: ResMut<NextState<GameState>>,
) {
    let out = read_nav(&mut cursor, &mut nav);
    if out.back {
        next.set(GameState::Menu);
        return;
    }
    if out.commit {
        let visible = visible_categories(&catalog);
        // `visible` can only be empty when the catalogue itself is empty (no
        // levels found at all) — nothing to drill into, so Confirm is inert,
        // same as it always was for a truly empty catalogue.
        if let Some(cat) = visible.get(cursor.index.min(visible.len().saturating_sub(1))) {
            browse.0 = *cat;
            next.set(GameState::LevelSelect);
        }
    }
}

// ──────────────────────────── level select ──────────────────────────────────

fn enter_level_select(
    mut cursor: ResMut<MenuCursor>,
    catalog: Res<LevelCatalog>,
    browse: Res<BrowseCategory>,
) {
    // The cursor sizes to the CHOSEN category's level count (min 1 so an empty
    // category still has a valid, inert cursor that only `Back` acts on).
    cursor.reset(catalog.count_in(browse.0));
}

/// Level-select navigation: `Up`/`Down` scroll the chosen category's level list,
/// `Confirm` plays the highlighted level (mapping the in-category cursor back to
/// the GLOBAL catalogue index), `Back` (Esc/q) returns to the category list.
///
/// **The unlock gate.** A campaign level whose predecessor is unsolved is INERT:
/// `Confirm` on it does nothing (the cursor stays, the state stays), so the player
/// can browse the whole campaign and read the arc's shape while only being able to
/// enter as far as they have earned. Non-campaign levels are never gated —
/// [`CampaignManifest::is_unlocked`] returns `true` for anything the manifest does
/// not list, so demos, the archive, and the test fixtures behave exactly as before.
fn level_select_nav(
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageReader<MenuNav>,
    catalog: Res<LevelCatalog>,
    browse: Res<BrowseCategory>,
    progress: Res<crate::save::Progress>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<GameState>>,
) {
    let out = read_nav(&mut cursor, &mut nav);
    if out.back {
        // Back goes UP one browse level (to the category list), not to the menu.
        next.set(GameState::CategorySelect);
        return;
    }
    let indices = catalog.indices_in(browse.0);
    if out.commit && !indices.is_empty() {
        let local = cursor.index.min(indices.len() - 1);
        // Map the in-category cursor back to the GLOBAL catalogue slot to play.
        let gidx = indices[local];
        let Some(id) = catalog.id_at(gidx) else {
            return;
        };
        if !campaign_manifest().is_unlocked(id, &progress) {
            // Locked: swallow the confirm. No state change, no level load.
            return;
        }
        current.0 = gidx;
        // Dive into the chosen level (the transition then enters Playing).
        next.set(GameState::Diving);
    }
}

// ────────────────────────────── settings ────────────────────────────────────

/// The rows of the settings screen, in display order. Each row maps to one real
/// system the screen controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsRow {
    /// Truecolor ⇄ Ansi256 — drives the [`ColorMode`] resource.
    Color,
    /// Mouse-look on/off — drives [`MouseSettings::enabled`].
    MouseEnabled,
    /// Mouse-look sensitivity preset — drives [`MouseSettings::sensitivity`].
    MouseSens,
    /// Audio mute on/off — drives the master volume message.
    Mute,
    /// Master volume 0..1 — drives the master volume message.
    Volume,
    /// Inert informational note: the terminal emulator owns the cell size, so
    /// this row points the player at their terminal's own zoom (Ctrl +/-).
    FontSize,
    /// Visual-corruption layer on/off — drives the `re_glitch` [`GlitchConfig`]
    /// resource (accessibility / motion-sensitivity toggle).
    Glitch,
    /// Horizontal field of view preset (60/65/70/75°) — drives the live
    /// [`re_render::Fov::h_preset_deg`] the raycaster reads.
    Fov,
    /// Cell-aspect "stretch" override — drives [`re_render::Fov::stretch`].
    Stretch,
    /// Cast-shadow quality (Off/Hard/Soft) — drives the live
    /// [`re_render::ShadowQuality`] resource the raycaster's shadow pass reads.
    Shadows,
    /// GRAPHICS QUALITY preset (Low/Medium/High/Ultra/Custom) — applies the whole
    /// quality bundle (draw distance, shadows, LOD, sub-cell, post-fx, FPS).
    Quality,
    /// DRAW DISTANCE (world cells) — drives the live [`re_render::FarFreeze`] +
    /// [`re_render::LodConfig`] far cap.
    DrawDistance,
    /// FPS CAP (30/60/120/Uncapped) — startup run-loop interval + the live throttle.
    Fps,
    /// LOD aggressiveness (Aggressive/Balanced/Near) — drives [`re_render::LodConfig`].
    Lod,
    /// 2×2 sub-cell silhouette refinement on/off — drives
    /// [`re_render::composite::SubCellConfig`].
    SubCell,
    /// POST-FX preset — drives [`re_postfx::PostfxConfig`].
    Postfx,
    /// Campaign colour-unlock gate on/off — drives [`crate::palette_ramp`]'s
    /// live `PaletteGate` (accessibility toggle: off = full colour always).
    ColorUnlock,
    /// `SOUND FIELDS` intensity (Off/Low/Medium/Full) — drives
    /// `re_runtime::sound_fields`'s `re_audio::field::FieldSettings::intensity`.
    SoundFields,
    /// `FIELD EFFECTS` (Full/Reduced) — drives
    /// `re_audio::field::FieldSettings::reduced`, an accessibility mode
    /// INDEPENDENT of [`SettingsRow::Glitch`] (owner decision 2026-09-27).
    SoundFieldsReduced,
    /// `BRIGHTNESS` — drives [`crate::settings::Settings::postfx_config`]'s
    /// lift/gain override onto the existing `re_postfx` grade knobs.
    Brightness,
    /// `COLOUR STEPS` — drives the live
    /// [`re_present::PresentConfig::color_deadband`] the present layer's
    /// temporal colour-deadband reads.
    ColorSteps,
    /// `FIRST STEPS` — shows/hides the self-ticking onboarding checklist in
    /// the play HUD.
    FirstSteps,
}

/// The settings rows in display / cursor order. The font-size row is an inert
/// informational note (the terminal emulator owns the cell size); changing it
/// is a no-op.
pub const SETTINGS_ROWS: [SettingsRow; 22] = [
    // Quality preset leads the screen (selecting it bundles the knobs below).
    SettingsRow::Quality,
    SettingsRow::DrawDistance,
    SettingsRow::Fps,
    SettingsRow::Lod,
    SettingsRow::SubCell,
    SettingsRow::Postfx,
    SettingsRow::Shadows,
    SettingsRow::Brightness,
    SettingsRow::ColorSteps,
    SettingsRow::Color,
    SettingsRow::MouseEnabled,
    SettingsRow::MouseSens,
    SettingsRow::Mute,
    SettingsRow::Volume,
    SettingsRow::FontSize,
    SettingsRow::Fov,
    SettingsRow::Stretch,
    SettingsRow::SoundFields,
    // The COMFORT / PHOTOSENSITIVITY cluster: every accessibility toggle
    // grouped adjacently, headed by ONE section marker
    // (`settings_section_before`) painted directly above `Glitch` — see that
    // function's doc for why this is a display-only grouping, not a second
    // list.
    SettingsRow::Glitch,
    SettingsRow::ColorUnlock,
    SettingsRow::SoundFieldsReduced,
    SettingsRow::FirstSteps,
];

/// A one-line section header the settings screen paints directly ABOVE the
/// given row — display-only: [`SETTINGS_ROWS`] is still the single list the
/// cursor, `adjust_row`, and the click rects all walk 1:1, so a header can
/// never desync the cursor from the row it names.
///
/// Groups CORRUPTION under a comfort / photosensitivity header.
/// [`SettingsRow::Glitch`] is where that cluster starts in [`SETTINGS_ROWS`]
/// above.
#[must_use]
pub fn settings_section_before(row: SettingsRow) -> Option<&'static str> {
    match row {
        SettingsRow::Glitch => Some("COMFORT / PHOTOSENSITIVITY"),
        _ => None,
    }
}

fn enter_settings(mut cursor: ResMut<MenuCursor>) {
    cursor.reset(SETTINGS_ROWS.len());
}

/// Apply the live [`crate::settings::Settings`] onto the runtime resources it
/// controls: the [`ColorMode`] the terminal blits with, the [`MouseSettings`]
/// the mouse-look systems read, and (via a message) the audio master volume.
fn apply_settings(settings: &crate::settings::Settings, sinks: &mut SettingsSinks) {
    *sinks.color = settings.color;
    *sinks.mouse = settings.mouse_settings();
    sinks
        .volume
        .write(re_audio::SetMasterVolume(settings.effective_volume()));
    apply_fov(settings, &mut sinks.fov);
    // SHADOWS is now a quality knob; push it from the unified path. Only writes on a
    // real difference so the cast dirty-flag doesn't fire needlessly.
    let next_shadows = settings.shadow_quality();
    if *sinks.shadows != next_shadows {
        *sinks.shadows = next_shadows;
    }
    // GRAPHICS QUALITY: push each live render lever, only on a real change so the
    // composite dirty-flag / `Changed<>` consumers don't fire needlessly.
    let next_ff = settings.far_freeze();
    if *sinks.far_freeze != next_ff {
        *sinks.far_freeze = next_ff;
    }
    let next_lod = settings.lod_config();
    if *sinks.lod != next_lod {
        *sinks.lod = next_lod;
    }
    let next_sub = settings.subcell_config();
    if *sinks.subcell != next_sub {
        *sinks.subcell = next_sub;
    }
    let next_pfx = settings.postfx_config();
    if *sinks.postfx != next_pfx {
        *sinks.postfx = next_pfx;
    }
    // COLOUR STEPS: live-drive the present layer's colour-deadband.
    let next_deadband = settings.color_steps_deadband();
    if sinks.present.color_deadband != next_deadband {
        sinks.present.color_deadband = next_deadband;
    }
}

/// `Startup`: apply the saved `COLOUR STEPS` choice to the live present-cache
/// deadband. Every other live-settings resource (FOV, `ShadowQuality`, …) gets
/// its saved value at boot via a pre-insert in `main.rs::build_core`, which
/// works because those resources are inserted by their owning plugin with
/// `init_resource` (insert-if-absent). [`re_present::PresentConfig`] is instead
/// inserted with a HARD `insert_resource(PresentConfig::from_env())` inside
/// `PresentPlugin`'s build, which would silently overwrite a pre-insert made
/// before `PresentPlugin` is added. `Startup` systems run once `App::run()`
/// starts — strictly after every plugin (including `PresentPlugin`, added
/// LAST in `run_interactive`) has finished building — so this always runs
/// last and the saved choice always wins, with no dependency on plugin-add
/// order.
fn apply_color_steps_on_startup(
    settings: Res<crate::settings::Settings>,
    mut present_cfg: ResMut<re_present::PresentConfig>,
) {
    present_cfg.color_deadband = settings.color_steps_deadband();
}

/// Push the live [`re_render::Fov`] from the settings (preset + stretch), so a
/// change to the FOV / ASPECT rows updates the raycaster immediately. The
/// raycaster re-casts when `Fov` changes, so this is all the wiring the live
/// FOV needs. Only writes on a real difference so `Changed<Fov>` consumers (and
/// the dirty-flag) don't fire needlessly.
fn apply_fov(settings: &crate::settings::Settings, fov: &mut re_render::Fov) {
    let next = settings.fov();
    if *fov != next {
        *fov = next;
    }
}

/// `Update` (every state): drain [`FovCycle`] (emitted by the present backends
/// for `[`/`]` and gamepad Y) and apply each step LIVE — cycle the saved FOV
/// preset, push it onto the raycaster's [`re_render::Fov`] resource (so the next
/// cast re-renders at the new FOV), and persist so the change survives a
/// restart. Runs in playing AND the menus so the player always sees the view
/// react to the key. Net steps are summed first so a burst of presets in one
/// frame is a single save.
fn cycle_fov_live(
    mut fov_msgs: MessageReader<FovCycle>,
    mut settings: ResMut<crate::settings::Settings>,
    mut fov: ResMut<re_render::Fov>,
) {
    let mut steps = 0i32;
    for msg in fov_msgs.read() {
        steps += i32::from(msg.0);
    }
    if steps == 0 {
        return;
    }
    let forward = steps > 0;
    for _ in 0..steps.unsigned_abs() {
        settings.cycle_fov(forward);
    }
    apply_fov(&settings, &mut fov);
    settings.save();
}

/// All the things `settings_nav` may touch, bundled to stay within Bevy's
/// system-argument budget.
#[derive(bevy::ecs::system::SystemParam)]
struct SettingsSinks<'w> {
    color: ResMut<'w, ColorMode>,
    mouse: ResMut<'w, MouseSettings>,
    volume: MessageWriter<'w, re_audio::SetMasterVolume>,
    fov: ResMut<'w, re_render::Fov>,
    shadows: ResMut<'w, re_render::ShadowQuality>,
    // GRAPHICS QUALITY live sinks: the render resources the quality rows drive.
    far_freeze: ResMut<'w, re_render::FarFreeze>,
    lod: ResMut<'w, re_render::LodConfig>,
    subcell: ResMut<'w, re_render::composite::SubCellConfig>,
    postfx: ResMut<'w, re_postfx::PostfxConfig>,
    // COLOUR STEPS — the present layer's colour-deadband knob.
    // `PresentConfig` is inserted with a HARD `insert_resource` inside
    // `PresentPlugin`'s build (not insert-if-absent), so unlike FOV/Shadows/…
    // a pre-insert in `main.rs::build_core` cannot make the saved value win at
    // boot — see `apply_color_steps_on_startup` for the `Startup`-scheduled fix
    // that runs after every plugin (including `PresentPlugin`) has built.
    present: ResMut<'w, re_present::PresentConfig>,
}

/// Settings navigation: `Up`/`Down` move between rows; `Left`/`Right` or
/// `Confirm` change the focused row's value (and apply it LIVE so the change is
/// visible immediately); `Back` (Esc/q) saves to disk and returns to the menu.
///
/// Every value change is applied to the real runtime resource the same frame, so
/// e.g. toggling color mode re-blits in the new mode on the very next present.
fn settings_nav(
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageReader<MenuNav>,
    mut settings: ResMut<crate::settings::Settings>,
    mut sinks: SettingsSinks,
    settings_return: Res<SettingsReturn>,
    mut next: ResMut<NextState<GameState>>,
) {
    // Read nav by hand: Up/Down move rows, Left/Right adjust the focused value,
    // Confirm cycles it forward, Back saves + exits. (We do NOT use `read_nav`
    // here because Left/Right must adjust values, not move the cursor.)
    let mut back = false;
    let mut changed = false;
    let mut do_apply = false;
    for msg in nav.read() {
        match msg {
            MenuNav::Up => cursor.step(-1),
            MenuNav::Down => cursor.step(1),
            MenuNav::Left => {
                adjust_row(&mut settings, current_row(&cursor), false);
                changed = true;
                do_apply = true;
            }
            MenuNav::Right | MenuNav::Confirm => {
                adjust_row(&mut settings, current_row(&cursor), true);
                changed = true;
                do_apply = true;
            }
            MenuNav::Back => back = true,
        }
    }
    // The test/escape hatch commits (cycles) the focused row forward, like Confirm.
    if cursor.pending_commit {
        cursor.pending_commit = false;
        adjust_row(&mut settings, current_row(&cursor), true);
        changed = true;
        do_apply = true;
    }

    if do_apply {
        apply_settings(&settings, &mut sinks);
    }

    if back {
        // Persist on the way out (best-effort), and make sure the live resources
        // reflect the final values even if nothing changed this exact frame.
        apply_settings(&settings, &mut sinks);
        settings.save();
        // Return to wherever Settings was opened from: the main menu, or the
        // in-play Pause overlay (so pausing → Settings → Back lands back on Pause,
        // with the level still frozen behind it).
        next.set(match *settings_return {
            SettingsReturn::Menu => GameState::Menu,
            SettingsReturn::Pause => GameState::Pause,
        });
        return;
    }

    // `changed` is intentionally unused beyond gating the apply; left explicit
    // for readability of which arms mutate state.
    let _ = changed;
}

/// The settings row the cursor is on (clamped into range).
fn current_row(cursor: &MenuCursor) -> SettingsRow {
    SETTINGS_ROWS[cursor.index.min(SETTINGS_ROWS.len() - 1)]
}

/// Change one settings row's value. `forward` is the Right/Confirm direction;
/// `false` is Left. Toggles flip; presets cycle; the volume steps ±0.1.
fn adjust_row(settings: &mut crate::settings::Settings, row: SettingsRow, forward: bool) {
    match row {
        SettingsRow::Color => settings.cycle_color(),
        SettingsRow::MouseEnabled => settings.mouse_enabled = !settings.mouse_enabled,
        SettingsRow::MouseSens => {
            settings.mouse_sens = if forward {
                settings.mouse_sens.next()
            } else {
                settings.mouse_sens.prev()
            };
        }
        SettingsRow::Mute => settings.muted = !settings.muted,
        SettingsRow::Volume => settings.step_volume(if forward { 0.1 } else { -0.1 }),
        // Inert informational note — the terminal emulator owns the cell size.
        SettingsRow::FontSize => {}
        SettingsRow::Glitch => settings.glitch_enabled = !settings.glitch_enabled,
        SettingsRow::ColorUnlock => {
            settings.colour_unlock_enabled = !settings.colour_unlock_enabled;
        }
        SettingsRow::Fov => settings.cycle_fov(forward),
        SettingsRow::Stretch => settings.step_stretch(if forward { 0.05 } else { -0.05 }),
        // SHADOWS as a quality knob: cycle AND resync the preset label (Custom unless
        // the new bundle matches a preset). The non-quality `cycle_shadows` is unused
        // from here so the preset stays honest.
        SettingsRow::Shadows => settings.cycle_shadows_quality(forward),
        SettingsRow::Quality => settings.cycle_quality(forward),
        SettingsRow::DrawDistance => settings.cycle_draw_distance(forward),
        SettingsRow::Fps => settings.cycle_fps(forward),
        SettingsRow::Lod => settings.cycle_lod_quality(forward),
        SettingsRow::SubCell => settings.toggle_subcell(),
        SettingsRow::Postfx => settings.cycle_postfx(forward),
        SettingsRow::SoundFields => settings.cycle_sound_fields(forward),
        SettingsRow::SoundFieldsReduced => {
            settings.sound_fields_reduced = !settings.sound_fields_reduced;
        }
        SettingsRow::Brightness => settings.cycle_brightness(forward),
        SettingsRow::ColorSteps => settings.cycle_color_steps(forward),
        SettingsRow::FirstSteps => {
            settings.first_steps_hud = !settings.first_steps_hud;
        }
    }
}

// ────────────────────────────── glitch ──────────────────────────────────────

/// The [`GlitchConfig`] for a level given its authored corruption density and the
/// player's accessibility toggle. THE RULE:
///
/// * `glitch_enabled == false` (the "no corruption" toggle) OR `level_glitch <= 0`
///   (the level authored no corruption) → [`GlitchConfig::off`] (frame untouched,
///   byte-identical to the no-glitch path).
/// * otherwise → the thematic [`GlitchConfig::terminal_decay`] preset with its
///   master `intensity` OVERRIDDEN to the level's authored density. `terminal_decay`
///   hard-codes `intensity = 0.5`; the authored `[effects] glitch` is the narrative
///   decay timer (`STORY_BIBLE` §7), so it must drive the master directly.
#[must_use]
pub fn glitch_config_for(level_glitch: f32, glitch_enabled: bool) -> GlitchConfig {
    if !glitch_enabled || level_glitch <= 0.0 {
        GlitchConfig::off()
    } else {
        let mut cfg = GlitchConfig::terminal_decay();
        cfg.intensity = level_glitch.clamp(0.0, 1.0);
        cfg
    }
}

/// `Update` (Playing only): re-apply the glitch config whenever the player toggles
/// the CORRUPTION setting at runtime, so flipping it OFF in the settings menu takes
/// effect immediately on returning to play. Reads the held [`Level`]'s authored
/// density from [`CurrentLevelData`] and the live [`crate::settings::Settings`].
///
/// Runs only on a `Settings` change (and once on enter, since `Changed` fires for a
/// freshly-inserted/edited resource), so the steady-state cost is nil.
fn reapply_glitch_on_settings_change(
    settings: Res<crate::settings::Settings>,
    level_data: Res<CurrentLevelData>,
    mut glitch: ResMut<GlitchConfig>,
) {
    if !settings.is_changed() {
        return;
    }
    let level_glitch = level_data.0.as_ref().map_or(0.0, |l| l.effects.glitch);
    let next = glitch_config_for(level_glitch, settings.glitch_enabled);
    if *glitch != next {
        *glitch = next;
    }
}

// ────────────────────────────── ambience ────────────────────────────────────

/// The atmospheric AMBIENCE bed for a level, chosen by catalogue index and a
/// tension flag. The mapping gives each level a distinct, evolving drone bed and
/// lets dread ramp with the narrative:
///
/// * Mood by index — early levels run the indifferent machine
///   [`ServerHum`](re_audio::AmbienceMood::ServerHum); the middle turns to the
///   cold abandoned [`ColdVoid`](re_audio::AmbienceMood::ColdVoid); the late,
///   themed levels rot into
///   [`OrganicDecay`](re_audio::AmbienceMood::OrganicDecay); the final stretch
///   (the closing `while(true)`) is the imminent-threat
///   [`AlarmTension`](re_audio::AmbienceMood::AlarmTension). An out-of-range /
///   huge catalogue falls back to `ServerHum` (the indifferent default).
/// * Intensity — a modest base (~0.4–0.5) that climbs slightly with the level
///   index (later rooms feel heavier), and is RAISED toward the top of the dread
///   curve when `tension` is set (the puzzle entered the denial / corrupted /
///   `while(true)` path, or the room has been solved). Always clamped by
///   [`re_audio::SetAmbience::new`].
///
/// Engine-pure: returns a plain [`re_audio::SetAmbience`] message; the caller
/// writes it. No notcurses, no render — just the mood→bed decision.
#[must_use]
pub fn ambience_for(level_index: usize, tension: bool) -> re_audio::SetAmbience {
    use re_audio::AmbienceMood;
    // Per-level mood: a small, deliberate mapping over catalogue order.
    let mood = match level_index {
        0 | 1 => AmbienceMood::ServerHum, // early: the machine still running
        2 | 3 => AmbienceMood::ColdVoid,  // mid: empty, frozen, abandoned
        4 | 5 => AmbienceMood::OrganicDecay, // late/themed: wet biological failure
        _ => AmbienceMood::AlarmTension,  // final stretch: the while(true) closes in
    };
    // Modest base dread that climbs slightly with depth, then a tension bump.
    let base = 0.4 + (level_index.min(5) as f32) * 0.02; // 0.40 ..= 0.50
    let intensity = if tension {
        (base + 0.35).min(1.0)
    } else {
        base
    };
    re_audio::SetAmbience::new(mood, intensity)
}

// ────────────────────────────── playing ─────────────────────────────────────

// `objective_progress`, `sequence_progress`, `power_readout` and
// `faced_switch` live in `re_runtime::hud_probe` — pure reads of a live
// `PuzzleState`/`CameraPose`, no seams. Re-exported here so every existing
// `crate::flow::X` / `terminal_state::flow::X` path (and `main::faced_target`'s /
// `menu::draw_play_hud`'s own imports) keeps compiling unchanged.
pub use re_runtime::hud_probe::{
    faced_switch, objective_progress, power_readout, sequence_progress,
};

// ─────────────────────────── floor-zone inputs ──────────────────────────────

// `apply_zone_inputs` lives in `re_runtime::triggers` — pure over `Level`,
// `PuzzleState`, `CameraPose`, no seams. Re-exported here so every existing
// `crate::flow::apply_zone_inputs` path (including this module's own
// `zone_input_sys`, below) keeps compiling unchanged.
pub use re_runtime::triggers::apply_zone_inputs;

/// `Update` (Playing only): push the player's position into every floor-zone
/// input. Reads the live [`CameraPose`] + the loaded level's zones (from
/// [`CurrentLevelData`]) and mutates the [`PuzzleState`]; the [`Solved`] mirror
/// is refreshed by `re_puzzle::apply_interactions` next frame (it copies
/// `state.solved`), and the binary's `watch_win` then sees the win. Cheap when
/// the level has no zones (early return) or the player has not crossed a zone
/// boundary (`set_input_value` no-ops on an unchanged value).
fn zone_input_sys(
    cam_q: Query<&CameraPose, With<PlayCamera>>,
    level_data: Res<CurrentLevelData>,
    mut puzzle: ResMut<PuzzleState>,
) {
    let Some(level) = level_data.0.as_ref() else {
        return;
    };
    if level.zone_inputs.is_empty() {
        return;
    }
    let Ok(cam) = cam_q.single() else {
        return;
    };
    apply_zone_inputs(&level.zone_inputs, cam, &mut puzzle);
}

// `TriggerFired`, `TriggerRuntime`, `TriggerStates`, `FiredTrigger` and
// `apply_triggers` live in `re_runtime::triggers` — pure over `Trigger`,
// `PuzzleState`, `CameraPose`, no seams. Re-exported here so every existing
// `crate::flow::X` / `terminal_state::flow::X` path (this module's own `trigger_sys`
// below, plus `main.rs`) keeps compiling unchanged.
pub use re_runtime::triggers::{
    apply_triggers, FiredTrigger, TriggerFired, TriggerRuntime, TriggerStates,
};

/// `Update` (Playing only): drive the trigger-fields. Reads the live map's
/// resolved triggers + the camera, fires any freshly-entered trigger via
/// [`apply_triggers`] (mutating the [`PuzzleState`] for an `ActivateNode`), and
/// emits one [`TriggerFired`] message + a log line per fire. Cheap when the level
/// has no triggers (early return). The [`Solved`] mirror is refreshed by
/// `re_puzzle::apply_interactions` next frame (it copies `state.solved`), and the
/// binary's `watch_win` then sees any new win.
fn trigger_sys(
    cam_q: Query<&CameraPose, With<PlayCamera>>,
    map: Res<Map>,
    mut states: ResMut<TriggerStates>,
    mut puzzle: ResMut<PuzzleState>,
    mut fired_msgs: MessageWriter<TriggerFired>,
) {
    let triggers = map.resolved_triggers();
    if triggers.is_empty() {
        return;
    }
    let Ok(cam) = cam_q.single() else {
        return;
    };
    // Keep the runtime vec sized to the live trigger list (defensive against a
    // mid-session map swap; `enter_playing` already sizes it on load).
    if states.0.len() != triggers.len() {
        states.0 = vec![TriggerRuntime::default(); triggers.len()];
    }
    let fired = apply_triggers(triggers, cam, &mut states.0, &mut puzzle);
    for f in fired {
        // Observable now (`println!`, not `info!`, so it survives the minimal
        // headless build's log feature set) AND wired via the message (see
        // [`trigger_subscribers`]).
        println!("TRIGGER FIRED — {}", f.label);
        fired_msgs.write(TriggerFired { label: f.label });
    }
}

/// The message sinks [`trigger_subscribers`] writes into, bundled to stay inside
/// Bevy's system-argument budget.
#[derive(bevy::ecs::system::SystemParam)]
struct TriggerSinks<'w> {
    /// The stinger channel — the audio director consumes these.
    sound: MessageWriter<'w, re_audio::PlaySound>,
    /// The ambient-bed channel, for a cue that changes the room's mood.
    ambience: MessageWriter<'w, re_audio::SetAmbience>,
    /// The live one-line HUD beat (`menu::draw_play_hud` paints it).
    beat: ResMut<'w, NarrativeBeat>,
}

/// `Update` (Playing, chained after [`trigger_sys`]): **the [`TriggerFired`]
/// subscribers**.
///
/// Two effects per fired label, both looked up in the data-driven
/// [`crate::story::beats`] table:
///
/// * (a) **audio stinger** — a [`re_audio::PlaySound`] (and, for a mood cue, a
///   [`re_audio::SetAmbience`]) through the existing message-driven director. The
///   ids live in the table, so `re_audio` can extend its stingers without a change
///   here.
/// * (b) **narrative beat** — one `>`-prefixed line into [`NarrativeBeat`], which
///   the in-play HUD paints for ~4 s and then drops.
///
/// Also AGES the beat each frame (in this system rather than a second one, so the
/// tick and the replace happen in a fixed order: a beat that fires this frame is
/// never aged by this frame's `dt`). Ticking only in `Playing` means a beat freezes
/// behind the pause overlay instead of expiring unseen.
fn trigger_subscribers(
    time: Res<Time>,
    mut fired: MessageReader<TriggerFired>,
    mut sinks: TriggerSinks,
) {
    sinks.beat.tick(time.delta_secs());
    for msg in fired.read() {
        let cue = cue_for(&msg.label);
        if let Some(id) = cue.sound {
            sinks.sound.write(re_audio::PlaySound(id));
        }
        if let Some(amb) = cue.ambience {
            sinks.ambience.write(amb);
        }
        if let Some(line) = cue.beat {
            sinks.beat.show(line);
        }
    }
}

/// The most recent `Playing`↔`Pause` transition direction, derived from the
/// latest [`StateTransitionEvent`]. Returns `(entering_pause, leaving_pause)`:
///
/// * `entering_pause` — the transition was `Playing → Pause` (the player paused).
///   The OnExit(Playing) systems use this to SKIP teardown so the live level
///   (camera + puzzle) survives behind the pause overlay.
/// * `leaving_pause` — the transition was `Pause → Playing` (Resume). The
///   OnEnter(Playing) loader uses this to SKIP the reload so play resumes exactly
///   where it left off instead of restarting the level.
fn pause_edge(events: &mut MessageReader<StateTransitionEvent<GameState>>) -> (bool, bool) {
    // The last event in the queue is this frame's transition.
    let Some(ev) = events.read().last() else {
        return (false, false);
    };
    let entering_pause =
        ev.exited == Some(GameState::Playing) && ev.entered == Some(GameState::Pause);
    let leaving_pause =
        ev.exited == Some(GameState::Pause) && ev.entered == Some(GameState::Playing);
    (entering_pause, leaving_pause)
}

/// Resolve a loaded level's authored `[emitter]` specs into a live
/// [`re_render::EmitterSet`]. Each [`re_world::EmitterSpec`] becomes one
/// [`re_render::Emitter`] via `Emitter::from_spec_lit`, baking in the level's
/// own authored `[light]`s — the light-less `Emitter::from_spec` would leave
/// `visibility = lit` dust and `Flock.seek_light` inert for every level with
/// lights but no explicit light-carrying emitter call. A level
/// with no `[emitter]` sections yields an EMPTY set (no demo fallback in real
/// levels — see the call site in [`enter_playing`]).
fn emitter_set_for(level: &Level) -> re_render::EmitterSet {
    re_render::EmitterSet {
        emitters: level
            .emitters
            .iter()
            .map(|spec| re_render::Emitter::from_spec_lit(spec, &level.lights))
            .collect(),
    }
}

/// The read-only resources [`enter_playing`] consults to decide WHICH level to
/// load and how to modulate it (catalogue path, current index, accessibility
/// settings, fog curve). Bundled into one [`SystemParam`] so the system stays
/// under clippy's argument threshold; the access pattern is identical to passing
/// the four `Res<…>` individually.
#[derive(bevy::ecs::system::SystemParam)]
struct LevelSource<'w> {
    catalog: Res<'w, LevelCatalog>,
    current: Res<'w, CurrentLevel>,
    settings: Res<'w, crate::settings::Settings>,
    fog_curve: Res<'w, crate::fog::FogCurve>,
}

/// The audio channels [`enter_playing`] writes on a level load: the per-level
/// ambience BED (every level) and the campaign MOVEMENT theme (campaign levels
/// only). Bundled as one [`SystemParam`] so the loader stays inside Bevy's
/// system-argument budget.
#[derive(bevy::ecs::system::SystemParam)]
struct LevelAudio<'w> {
    /// The atmospheric drone bed — sent for EVERY level (see [`ambience_for`]).
    ambience: MessageWriter<'w, re_audio::SetAmbience>,
    /// The generative director's movement theme — sent ONLY for a level listed in
    /// the campaign manifest.
    theme: MessageWriter<'w, re_audio::SetTheme>,
}

/// Load the current level into the render/puzzle resources and spawn the camera.
///
/// Overwrites [`Map`], [`PuzzleState`], and [`Solved`] in place and spawns a
/// [`CameraPose`] tagged [`PlayCamera`]. A failed load (bad path / malformed
/// level) logs and bounces back to the menu rather than killing the process —
/// the flow must survive a bad level.
fn enter_playing(
    mut commands: Commands,
    src: LevelSource,
    mut audio: LevelAudio,
    mut next: ResMut<NextState<GameState>>,
    mut transitions: MessageReader<StateTransitionEvent<GameState>>,
    mut postfx: ResMut<re_postfx::PostfxConfig>,
) {
    let LevelSource {
        catalog,
        current,
        settings,
        fog_curve,
    } = src;
    // Resuming from Pause: the live level is still intact (OnExit(Playing) skipped
    // teardown), so do NOT reload — that would restart the puzzle and the camera.
    let (_, leaving_pause) = pause_edge(&mut transitions);
    if leaving_pause {
        return;
    }
    let Some(path) = catalog.paths.get(current.0) else {
        println!("flow: no level at index {} — returning to menu.", current.0);
        next.set(GameState::Menu);
        return;
    };

    let mut level: Level = match re_level::load(path) {
        Ok(l) => l,
        Err(e) => {
            println!("flow: failed to load '{path}': {e} — returning to menu.");
            next.set(GameState::Menu);
            return;
        }
    };
    // ASSET NOTICE: name every file this level references that is NOT on disk,
    // once, here at load. The game keeps its lenient fallback — a missing wall
    // texture renders untextured rather than refusing to launch — but it no
    // longer keeps it silent (`asset_notice` documents the split, and
    // `re_lvlcheck` is the hard gate). Cheap: path resolution only, no decode.
    re_runtime::asset_notice::warn_missing_assets(&level, path);
    // FORM A-7 INTERPOLATION: the level-load templating pass over the
    // `[note]` bodies, BEFORE anything reads them. Runs here — not in the parser —
    // because it depends on the player's `lore_fourth_wall` opt-out, and because the
    // interpolated bodies must be what `CurrentLevelData` holds too (so every later
    // consumer sees the same text the wall shows). Once per load, never per frame.
    let profile = crate::story::interp::OperatorProfile::for_fourth_wall(settings.lore_fourth_wall);
    crate::story::interp::interpolate_level_notes(&mut level, &profile);
    let puzzle = match PuzzleState::from_level(&level) {
        Ok(p) => p,
        Err(e) => {
            println!("flow: level '{path}' has a malformed network: {e:?} — returning to menu.");
            next.set(GameState::Menu);
            return;
        }
    };

    let solved = Solved(puzzle.solved);
    // Drive the glitch layer from THIS level's authored corruption density × the
    // player's accessibility toggle (the exact rule lives in `glitch_config_for`).
    commands.insert_resource(glitch_config_for(
        level.effects.glitch,
        settings.glitch_enabled,
    ));
    // Fog as the death-timer (`STORY_BIBLE` §1/§7): tighten + blacken the
    // level's AUTHORED fog by how deep into the campaign we are.
    // `build_map` already stamped the authored `[palette]` fog as the map's
    // atmosphere (the baseline mood); read it back, contract it by the
    // campaign-progress scalar, and write it back. progress 0 (level 1 / a
    // single-level catalogue / a standalone `--dump`) leaves it byte-identical.
    let mut map = level.build_map();
    // TEXT SURFACES: publish THIS level's text — the embedded code panels, one
    // panel per `[note]` placard, and the pipe-tag → node word-pool table — and stamp
    // the placards onto the wall faces their `pos`/`facing` resolve to. Runs here, at
    // load, so it is static world data: no per-frame cost, the render dirty-flag is
    // untouched. Installing REPLACES the previous level's panels (they are owned, not
    // leaked), and `teardown_level` clears them.
    //
    // Order matters: this appends the note materials to the map, and the render
    // `MaterialRegistry` is derived from the map afterwards.
    let notes = crate::level_text::bind_level_text(&level, &mut map);
    if !notes.unplaced.is_empty() {
        println!(
            "flow: level '{path}' has {} [note] placard(s) whose pos/facing resolve to no              wall face (indices {:?}) — they are not rendered.",
            notes.unplaced.len(),
            notes.unplaced
        );
    }
    // AUTHORED `[emit]` PARTS: bind each `[emit SOURCE=1] parts = …` clause to the
    // world cells its `[part]` mask resolves to (anchored on the `[pedestal]`) and
    // append one colour material per clause. Must run AFTER `bind_level_text` (both
    // append to the SAME colour palette, and the note ids must stay stable) and
    // BEFORE the map is inserted, so `registry_for_map` sees the emit materials.
    // Nothing is stamped here — `emit_parts_sys` lights a clause on the first frame
    // its source node reads 1, which is also what makes a reload start unlit.
    let emit_parts = re_runtime::emit_parts::bind_emit_parts(&level, &mut map);
    commands.insert_resource(emit_parts);
    // POWER-REACTIVE PROPS (set-dressing §3.6): bind every `[prop] power =` gate to
    // the cells / lines / lamps / emitter slots it flips. The built map already shows
    // each gate's static initial state; `prop_power_sys` reconciles it with the live
    // puzzle on the first frame and then follows it edge by edge. Bound after the
    // palette appenders above (prop ids precede theirs and stay stable).
    commands.insert_resource(re_runtime::prop_power::bind_prop_power(&level, &map));
    let progress = crate::fog::progress_for(current.0, catalog.paths.len());
    let tightened = crate::fog::modulate_atmosphere(map.atmosphere(), progress, &fog_curve);
    map.set_atmosphere(tightened);
    // LEVEL-REQUESTED POST-FX (lighting spec §1.3): `[atmosphere] postfx = NAME`
    // is the level's own look, applied here at load — but only while the player
    // has never touched the POST-FX settings row. An explicit player choice
    // outranks the level, and `apply_settings` keeps pushing that choice the
    // moment the row is adjusted. Resolved on EVERY load (not only when the level
    // names a preset) so a preset can never leak from the previous room into a
    // level that authored none. Written only on a real difference so the post-fx
    // pass's `Changed<>` consumers don't fire for nothing.
    let next_pfx = postfx_for_level(&settings, map.atmosphere().postfx.as_deref());
    if *postfx != next_pfx {
        *postfx = next_pfx;
    }
    // (Re-)ARM the trigger-fields: one fresh outside/disarmed `TriggerRuntime`
    // per resolved trigger. Rebuilding here on every load means a level RESET
    // re-arms every trigger (a `once` trigger can fire again after a restart),
    // exactly as `watch_win` re-arms the win.
    commands.insert_resource(TriggerStates(vec![
        TriggerRuntime::default();
        map.resolved_triggers().len()
    ]));
    // Drop any narrative beat left over from the previous level / a re-load, so a
    // fresh room never inherits the last room's line — EXCEPT on the campaign's
    // finale, which opens with its framing beat. One line, on the same
    // timed HUD channel a `[trigger]` uses, stating only the rule the finale runs
    // on. The model's plea is NOT repeated here: it is already an L12 `[note]`.
    let mut beat = NarrativeBeat::default();
    if campaign_manifest().is_finale(LevelCatalog::label(path)) {
        beat.show(FINALE_FRAMING_BEAT);
    }
    commands.insert_resource(beat);
    // RUNTIME SAFETY (DO item 3, set-dressing follow-up): if the authored spawn
    // ends up embedded in a collider, push it to the nearest free standable
    // cell BEFORE the camera is ever spawned, rather than trusting the level
    // data unconditionally. `re_lvlcheck::no_stuck` already gates every
    // shipped level's spawn at save time; this is the runtime backstop for a
    // level that reached the game some other way (a WIP export, a corrupt
    // `.lvl`, a future authoring tool).
    let mut cam = level.spawn_camera();
    re_runtime::unstuck::ensure_camera_safe(&map, &mut cam);
    commands.insert_resource(map);
    commands.insert_resource(puzzle);
    commands.insert_resource(solved);
    // Stash the door cell so the win watcher (`main::watch_win`) gates the Won
    // transition on the player physically ENTERING the door, not on the network
    // being (possibly vacuously) satisfied at load. Every parsed level has a
    // `[door] pos` (the loader requires it), so this is always meaningful for a
    // shipped level; teardown removes it so the menus carry no stale door.
    commands.insert_resource(DoorCell {
        x: level.door.x,
        y: level.door.y,
        z: level.door.z,
    });
    // PARTICLE EMITTERS: resolve THIS level's authored `[emitter]` specs into live
    // `re_render::Emitter`s and OVERWRITE the `EmitterSet` resource. The composite
    // renderer's `EmitterPlugin::tick_emitters_sys` reads `Res<EmitterSet>` every
    // frame, so this insert is the live lever — particles appear/disappear exactly
    // with the loaded level. A real level with no `[emitter]` section yields an
    // EMPTY set (NOT the demo fallback): `EmitterSet::default()` = the two demo
    // sources is for the empty/menu map only (teardown re-installs it); a shipped
    // level must show only what it authored, nothing implicit.
    commands.insert_resource(emitter_set_for(&level));
    commands.spawn((cam, PlayCamera));

    // Per-level ATMOSPHERIC AMBIENCE: choose a drone mood by catalogue index and
    // start it at a modest base intensity, RAISED when this level carries active
    // corruption (the authored `[effects] glitch` density × the player's
    // CORRUPTION toggle) — the denial / `while(true)` dread bleeds into the bed.
    // This supersedes the legacy single `SetAmbient` bed (sent by `main::enter_play`
    // before; now removed) with the rich crossfading mood path.
    let corruption_active = settings.glitch_enabled && level.effects.glitch > 0.0;
    audio
        .ambience
        .write(ambience_for(current.0, corruption_active));

    // CAMPAIGN MOVEMENT THEME: a level listed in `levels/campaign.toml`
    // switches the generative director to its movement's theme (P1 first light →
    // P4 closing). A level that is NOT in the manifest — a demo, an archive room, a
    // test fixture — sends NO `SetTheme` at all, so it keeps whatever theme is live
    // (today's behaviour: the director's default palette plus the ambience bed
    // above). That is the "non-campaign levels keep today's logic" rule.
    if let Some(entry) = campaign_manifest().get(LevelCatalog::label(path)) {
        audio
            .theme
            .write(re_audio::SetTheme(entry.movement.theme()));
    }

    // Keep the parsed Level around (its node word-pools) so the word→story
    // layer can assemble the readout on solve. Map/PuzzleState drop the pools.
    commands.insert_resource(CurrentLevelData(Some(level)));
}

/// Resolve the post-fx chain a level should load with (lighting spec §1.3).
///
/// `level_pfx` is the level's `[atmosphere] postfx = NAME` request, already
/// validated by the parser. It is honoured ONLY while the player's POST-FX row
/// still holds the shipped default — an explicit player choice outranks the
/// level. An unknown name (a hand-edited `.lvl` that skipped the parser) falls
/// back to the player's setting rather than to a surprise look.
///
/// Total, not conditional: a level that requests NOTHING resolves to the
/// player's setting, which is what stops a previous room's preset from leaking
/// into the next one.
fn postfx_for_level(
    settings: &crate::settings::Settings,
    level_pfx: Option<&str>,
) -> re_postfx::PostfxConfig {
    match (settings.postfx_is_default(), level_pfx) {
        (true, Some(name)) => re_postfx::PostfxConfig::from_preset_name(name)
            .unwrap_or_else(|| settings.postfx_config()),
        _ => settings.postfx_config(),
    }
}

/// The resources + the camera query a level teardown touches, bundled so both
/// [`exit_playing`] and [`teardown_level_on_quit`] share one implementation.
#[derive(bevy::ecs::system::SystemParam)]
struct LevelTeardown<'w, 's> {
    commands: Commands<'w, 's>,
    cams: Query<'w, 's, Entity, With<PlayCamera>>,
    map: ResMut<'w, Map>,
    puzzle: ResMut<'w, PuzzleState>,
    solved: ResMut<'w, Solved>,
    glitch: ResMut<'w, GlitchConfig>,
    level_data: ResMut<'w, CurrentLevelData>,
    /// The live particle sources. Reset to the default (menu/empty) set on
    /// teardown so the menus do not inherit the last level's authored emitters.
    emitters: ResMut<'w, re_render::EmitterSet>,
    /// The reactive-materials state bus + its bridge bookkeeping. Reset on teardown
    /// so the next level rebuilds its reactive state (flow/pulses) from scratch and
    /// the menus shade with an empty bus (no stale powered pipes / pulses). `Option`
    /// so a test app that omits `StateBridgePlugin` (these resources absent) tears
    /// down cleanly — the reset is simply skipped.
    cell_state: Option<ResMut<'w, re_core::CellStateMap>>,
    state_bridge: Option<ResMut<'w, re_runtime::state_bridge::StateBridge>>,
    /// The level's bound `[emit]`-part clauses. Cleared on teardown so no stale
    /// clause can address the NEXT level's map (the bindings hold absolute cell
    /// coordinates). `Option` so a focused test app that never inserted the
    /// resource tears down cleanly.
    emit_parts: Option<ResMut<'w, re_runtime::emit_parts::EmitParts>>,
    /// The level's bound prop power gates — cleared for the same reason as
    /// `emit_parts` (absolute cells / line / light / emitter indices).
    prop_power: Option<ResMut<'w, re_runtime::prop_power::PropPower>>,
    /// The live post-fx chain + the player's own preference. A level may request
    /// its own preset (`[atmosphere] postfx =`, applied in [`enter_playing`]); on
    /// teardown the chain is restored to the PLAYER's setting so a level's look
    /// never follows them back into the menus. `Option` so a focused test app that
    /// never inserted either resource tears down cleanly.
    postfx: Option<ResMut<'w, re_postfx::PostfxConfig>>,
    postfx_pref: Option<Res<'w, crate::settings::Settings>>,
}

/// Despawn the play camera and reset the live render/puzzle resources to their
/// empty (menu) state. Idempotent — safe to call when no level is loaded.
fn teardown_level(t: &mut LevelTeardown) {
    for e in &t.cams {
        t.commands.entity(e).despawn();
    }
    *t.map = empty_map();
    *t.puzzle = PuzzleState::default();
    *t.solved = Solved(false);
    // Drop any corruption so the menus/won overlays render clean.
    *t.glitch = GlitchConfig::off();
    // Drop the level's TEXT SURFACES (note placards, code panels, word pools) so the
    // menus carry no stale level text and the panels are actually freed — the whole
    // point of the per-level registry over a leaked `'static` panel set.
    crate::level_text::clear_level_text();
    // Restore the default (menu/empty) emitter set so a torn-down level's authored
    // particles do not linger behind the menus. `EmitterSet::default()` is the two
    // demo sources — the deliberate "non-level" ambience for the empty map.
    *t.emitters = re_render::EmitterSet::default();
    // Reset the reactive state bus + bridge so the next level (even one with the
    // same node/pipe counts) rebuilds its flow/pulses from a clean baseline, and the
    // menus shade with an empty bus → today's look. Skipped when the bridge plugin
    // is absent (a focused test app).
    if let Some(cs) = t.cell_state.as_mut() {
        cs.clear();
    }
    if let Some(sb) = t.state_bridge.as_mut() {
        sb.reset();
    }
    // Drop the `[emit]`-part bindings. `*t.map` above is already a fresh empty map, so
    // the stamped cells are gone with it; clearing the bindings is what makes the
    // RESET contract hold — a reload re-binds from scratch, unlit, and no clause
    // survives to write into a map it was never bound against.
    if let Some(ep) = t.emit_parts.as_mut() {
        **ep = re_runtime::emit_parts::EmitParts::default();
    }
    if let Some(pp) = t.prop_power.as_mut() {
        **pp = re_runtime::prop_power::PropPower::default();
    }
    // Restore the PLAYER's post-fx preset: a level that requested its own look
    // (`[atmosphere] postfx =`) must not leave that look on the menus behind it.
    if let (Some(pfx), Some(settings)) = (t.postfx.as_mut(), t.postfx_pref.as_ref()) {
        let want = settings.postfx_config();
        if **pfx != want {
            **pfx = want;
        }
    }
    // Clear the held Level so no stale word-pool leaks into the next state. The
    // readout assembled on solve is already captured in `MemoryReadout` + log.
    t.level_data.0 = None;
    // Drop the stashed door cell so the menus / next level carry no stale door.
    // Its ABSENCE is the door-less fallback the win watcher reads (solved-alone
    // wins), which is exactly what we want outside an actively-played level.
    t.commands.remove_resource::<DoorCell>();
    // Drop any live narrative beat — the Won/menu overlays must not carry a
    // half-decayed trigger line from the level we just left.
    t.commands.insert_resource(NarrativeBeat::default());
}

/// `OnExit(Playing)`: tear the level down — UNLESS this is a `Playing → Pause`
/// transition, in which case the live scene must survive behind the pause overlay
/// (Resume returns to it). The Pause→Menu quit path tears down separately in
/// [`teardown_level_on_quit`] (there is no OnExit(Playing) on that edge).
fn exit_playing(
    mut teardown: LevelTeardown,
    mut transitions: MessageReader<StateTransitionEvent<GameState>>,
) {
    let (entering_pause, _) = pause_edge(&mut transitions);
    if entering_pause {
        return;
    }
    teardown_level(&mut teardown);
}

/// `OnExit(Pause)`: when the player chose Quit-to-menu (Pause → Menu) the level
/// was kept alive by [`exit_playing`]'s pause skip, so it must be torn down HERE.
/// Resume (Pause → Playing) and opening Settings (Pause → Settings) keep the
/// scene, so the teardown only fires for the Pause → Menu edge.
fn teardown_level_on_quit(
    mut teardown: LevelTeardown,
    mut transitions: MessageReader<StateTransitionEvent<GameState>>,
) {
    let quitting = transitions.read().last().is_some_and(|ev| {
        ev.exited == Some(GameState::Pause) && ev.entered == Some(GameState::Menu)
    });
    if quitting {
        teardown_level(&mut teardown);
    }
}

// ──────────────────────────────── pause ─────────────────────────────────────

/// The pause-overlay items (Esc during play). `Resume` returns to
/// [`Playing`](GameState::Playing);
/// `Settings` opens the settings screen (which returns HERE, not to the menu —
/// see [`SettingsReturn`]); `QuitToMenu` abandons the level for the main menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseItem {
    /// Return to [`Playing`](GameState::Playing) with the level untouched.
    Resume,
    /// Open the [`GameState::MemoryView`] journal without leaving the level — the
    /// live scene is kept alive behind it exactly as it is behind Pause itself
    /// (`teardown_level_on_quit` only fires on the Pause → Menu edge).
    Memory,
    /// Open the [`GameState::Guide`] without leaving the level — same "keep the
    /// live scene frozen behind it" contract as [`Memory`](Self::Memory).
    Guide,
    /// Open the settings screen; it returns to Pause, not to the menu.
    Settings,
    /// Abandon the level and return to the main menu.
    QuitToMenu,
}

/// The fixed pause-overlay item list (drives the cursor length + the labels in
/// [`crate::menu`]).
pub const PAUSE_ITEMS: [PauseItem; 5] = [
    PauseItem::Resume,
    PauseItem::Memory,
    PauseItem::Guide,
    PauseItem::Settings,
    PauseItem::QuitToMenu,
];

/// `Update` (Playing only): Back/Esc opens the [`GameState::Pause`] overlay
/// instead of quitting. Reads the same neutral [`MenuNav`] both backends emit;
/// only `Back` is consumed here (movement/interact stay with the gameplay
/// systems). The OnExit(Playing) teardown is skipped for this transition (see
/// [`exit_playing`]), so the live scene freezes behind the overlay.
fn watch_pause(mut nav: ResMut<Messages<MenuNav>>, mut next: ResMut<NextState<GameState>>) {
    let pause = nav.drain().any(|msg| matches!(msg, MenuNav::Back));
    if pause {
        // We drained the whole queue, so the SAME Esc press cannot leak into
        // `pause_nav` next frame (which would instantly Resume). Draining also
        // discards any movement nav from this frame — harmless, the camera is
        // about to freeze anyway.
        next.set(GameState::Pause);
    }
}

fn enter_pause(mut cursor: ResMut<MenuCursor>) {
    cursor.reset(PAUSE_ITEMS.len());
}

/// Pause-overlay navigation: `Up`/`Down` move the highlight, `Confirm` commits,
/// `Back` (Esc/q) resumes (same as Resume). `Settings` records that it was
/// entered from Pause so the settings `Back` returns here, not to the menu.
fn pause_nav(
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageReader<MenuNav>,
    mut settings_return: ResMut<SettingsReturn>,
    mut memory_return: ResMut<MemoryViewReturn>,
    mut guide_return: ResMut<GuideReturn>,
    mut next: ResMut<NextState<GameState>>,
) {
    let out = read_nav(&mut cursor, &mut nav);
    if out.back {
        // Back/Esc resumes — never quits from the pause overlay.
        next.set(GameState::Playing);
        return;
    }
    if !out.commit {
        return;
    }
    match PAUSE_ITEMS[cursor.index.min(PAUSE_ITEMS.len() - 1)] {
        PauseItem::Resume => next.set(GameState::Playing),
        PauseItem::Memory => {
            *memory_return = MemoryViewReturn::Pause;
            next.set(GameState::MemoryView);
        }
        PauseItem::Guide => {
            *guide_return = GuideReturn::Pause;
            next.set(GameState::Guide);
        }
        PauseItem::Settings => {
            *settings_return = SettingsReturn::Pause;
            next.set(GameState::Settings);
        }
        PauseItem::QuitToMenu => next.set(GameState::Menu),
    }
}

// ──────────────────────────────── won ───────────────────────────────────────

/// The won-overlay items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WonItem {
    /// Advance straight into the next campaign level.
    NextLevel,
    /// Return to the main menu.
    Menu,
}

/// Won-overlay item list (Next level / Menu).
pub const WON_ITEMS: [WonItem; 2] = [WonItem::NextLevel, WonItem::Menu];

fn enter_won(mut cursor: ResMut<MenuCursor>) {
    cursor.reset(WON_ITEMS.len());
}

/// `OnEnter(Won)`: the current level was genuinely solved (the binary's
/// `watch_win` only transitions to [`GameState::Won`] on a real player win, not
/// the load-time-already-solved case). Mark its stable id solved in
/// [`crate::save::Progress`] and persist — but only write to disk if this is a
/// NEW solve, so re-winning an already-recorded level costs no I/O.
fn record_progress(
    catalog: Res<LevelCatalog>,
    current: Res<CurrentLevel>,
    readout: Res<MemoryReadout>,
    mut progress: ResMut<crate::save::Progress>,
) {
    let Some(id) = catalog.id_at(current.0) else {
        return;
    };
    // `assemble_readout` ran first (OnExit(Playing) precedes OnEnter(Won)), so
    // the readout phrase for this level is already published. Persist BOTH the
    // solved flag and the assembled phrase (the memory log on disk), writing
    // only when something actually changed.
    let solved_new = progress.mark_solved(id);
    // Only the CLEAN acceptance readout is written to the persistent journal — the
    // same rule `assemble_readout` applies to the in-memory `MemoryLog`. A corrupt
    // `pool_alt` collapse is the `while(true)` ending, not a recovered vignette, so
    // it must not overwrite the level's remembered phrase. The level is still marked
    // SOLVED on that path, which is what makes both endings complete the campaign.
    let phrase_new = if readout.has_phrase() && readout.form == ReadoutForm::Clean {
        progress.record_phrase(id, &readout.phrase)
    } else {
        false
    };
    // LOOP CURSOR: while a session loop is running, walk the cursor
    // forward with the player so `Continue` resumes the replay instead of sticking
    // on level 1 forever. Cleared at the end of the campaign.
    let cursor_new = if progress.campaign_cursor.as_deref() == Some(id) {
        let next_id = campaign_manifest().next_after(id).map(|l| l.id.clone());
        progress.set_campaign_cursor(next_id.as_deref())
    } else {
        false
    };
    if solved_new || phrase_new || cursor_new {
        progress.save();
    }
}

/// `OnEnter(Won)`: **the peak-solve silence** (`PUZZLE_PROGRESSION` §3).
///
/// The campaign's two hardest solves — the 7th and 11th levels
/// ([`PEAK_SOLVE_POSITIONS`]) — are answered by the score holding its breath for a
/// few seconds. This sends [`re_audio::SolveSilence::solve()`], which mutes only
/// the generative director's arrangement: the ambience bed keeps running and the
/// game's own one-shots (UI, story stingers) still sound, so the world does not
/// die — the music just stops breathing.
///
/// Fires on the genuine-win edge only: `OnEnter(Won)` is reached from the win
/// watcher, never from a load-time already-solved level. Every other level (and
/// every non-campaign level) sends nothing. Re-winning a peak level re-sends it,
/// which is correct — [`re_audio::SolveSilence`] extends rather than stacks.
fn peak_solve_silence(
    catalog: Res<LevelCatalog>,
    current: Res<CurrentLevel>,
    mut silence: MessageWriter<re_audio::SolveSilence>,
) {
    let Some(id) = catalog.id_at(current.0) else {
        return;
    };
    if campaign_manifest().is_peak_solve(id) {
        silence.write(re_audio::SolveSilence::solve());
    }
}

/// `OnExit(Playing)` (runs before [`exit_playing`]): if the level was genuinely
/// solved, assemble its memory-readout phrase from the live [`Level`] word-pools
/// plus the [`PuzzleState`] activations, publish it as [`MemoryReadout`] for the
/// Won overlay to paint, and append it to the persistent [`MemoryLog`] (the
/// story so far). Leaving Playing for any other reason (Back to menu) emits
/// nothing.
///
/// Clean vs. corrupt: the readout form is chosen from the live network. An
/// ordinary win is a valid DAG → the clean primary-pool readout. If the network
/// is CYCLIC ([`ForwardError::Cycle`] — the `while(true)` denial path, `WORD_STORY`
/// §4), the SAME [`assemble_phrase`] renders the corrupted `pool_alt` Babel
/// instead. Ordinary play rolls cycles back (`puzzle_interact`), so the clean
/// form is what the shipped levels produce; the corrupt branch is the real data
/// path for the denial ending (only the clean phrase is persisted to the memory
/// log — a corrupt collapse is the ending, not a recovered vignette).
fn assemble_readout(
    solved: Res<Solved>,
    level_data: Res<CurrentLevelData>,
    puzzle: Res<PuzzleState>,
    catalog: Res<LevelCatalog>,
    current: Res<CurrentLevel>,
    mut readout: ResMut<MemoryReadout>,
    mut log: ResMut<MemoryLog>,
) {
    // Pausing never assembles a readout: a solve transitions straight to Won (via
    // `watch_win`), never to Pause, so `solved.0` is always false on the pause
    // edge and the guard below already early-returns. No transition check needed.
    if !solved.0 {
        return;
    }
    let Some(level) = level_data.0.as_ref() else {
        return;
    };
    // Denial detection, two shapes of the same thing:
    //
    // * an ORDINARY level rolls a cycle back, so a cyclic graph here can only be a
    //   `ForwardError::Cycle`;
    // * the FINALE opts into `[room] cycle_keep`, where the ring is accepted as a
    //   terminal state instead of erroring — the pass returns `Ok` and raises
    //   `cycle_held`. That is the `while(true)` ending, and it must read as the
    //   same corrupted `pool_alt` Babel (WORD_STORY §4), not as a clean memory.
    let mut probe = puzzle.clone();
    let form = if puzzle.cycle_held {
        ReadoutForm::Corrupt
    } else {
        match network_forward(&mut probe) {
            Err(ForwardError::Cycle) => ReadoutForm::Corrupt,
            _ => ReadoutForm::Clean,
        }
    };
    let phrase = assemble_phrase(level, &puzzle, form);
    if phrase.is_empty() {
        return;
    }
    readout.phrase.clone_from(&phrase);
    readout.form = form;
    // Only the clean acceptance readout accumulates into the story; the corrupt
    // collapse is the ending, not a vignette to remember.
    if form == ReadoutForm::Clean {
        if let Some(id) = catalog.id_at(current.0) {
            log.record(MemoryEntry {
                level_id: id.to_string(),
                phrase,
            });
        }
    }
}

/// Won-overlay navigation: `Up`/`Down` toggle Next-level / Menu, `Confirm`
/// commits, `Back` (Esc/q) returns to the main menu.
fn won_nav(
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageReader<MenuNav>,
    catalog: Res<LevelCatalog>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<GameState>>,
) {
    let out = read_nav(&mut cursor, &mut nav);
    if out.back {
        next.set(GameState::Menu);
        return;
    }
    if !out.commit {
        return;
    }
    match WON_ITEMS[cursor.index.min(WON_ITEMS.len() - 1)] {
        WonItem::NextLevel => {
            let n = catalog.paths.len();
            if n > 0 && current.0 + 1 < n {
                current.0 += 1;
                // Dive into the next level (the transition then enters Playing).
                next.set(GameState::Diving);
            } else {
                // No next level — wrap to the menu.
                next.set(GameState::Menu);
            }
        }
        WonItem::Menu => next.set(GameState::Menu),
    }
}

// ──────────────────────── the framing states ─────────────────────────────────

/// The `[final gate]` framing beat, painted on the HUD the moment the campaign's
/// finale loads.
///
/// It states the ONE rule the finale runs on — the choice is reversible right up
/// until you walk into the door — and nothing else. It deliberately does NOT
/// re-state the model's plea: that is already authored as an L12 `[note]`
/// (`model_thought`, "please match…") and rendered on the wall. Saying
/// it twice would flatten the level's best line into UI chrome.
pub const FINALE_FRAMING_BEAT: &str =
    "[final gate] the wiring is reversible. the door is not. choose with your hands.";

/// `OnEnter(Intro)`: zero the pane clock + scroll.
fn enter_intro(mut timer: ResMut<IntroTimer>, mut scroll: ResMut<PaneScroll>) {
    timer.0 = std::time::Duration::ZERO;
    scroll.0 = 0;
}

/// `Update` (Intro only): advance the clock, let `↑`/`↓` scroll the boot text, and
/// leave for the dive on any other key or on the timeout.
///
/// The exit is [`GameState::Diving`], not [`GameState::Playing`]: the intro frames
/// the campaign, the dive is still the transition INTO the room, and the level
/// load stays exactly where it has always been (`OnEnter(Playing)`).
fn tick_intro(
    time: Res<Time>,
    mut timer: ResMut<IntroTimer>,
    mut scroll: ResMut<PaneScroll>,
    mut nav: MessageReader<MenuNav>,
    mut presses: MessageReader<InteractPressed>,
    mut next: ResMut<NextState<GameState>>,
) {
    timer.0 += time.delta();
    let mut leave = presses.read().count() > 0;
    for msg in nav.read() {
        match msg {
            MenuNav::Up | MenuNav::Left => scroll.up(),
            MenuNav::Down | MenuNav::Right => scroll.down(),
            MenuNav::Confirm | MenuNav::Back => leave = true,
        }
    }
    if leave || timer.0 >= INTRO_TIMEOUT {
        next.set(GameState::Diving);
    }
}

/// `OnEnter(BootConsole)`: zero the pseudo-boot clock.
fn enter_boot_console(mut timer: ResMut<BootConsoleTimer>) {
    timer.0 = std::time::Duration::ZERO;
}

/// `Update` (`BootConsole` only): advance the clock; any interact/nav key OR the
/// timeout hands off to [`GameState::Splash`] — mirrors [`tick_intro`]'s
/// "never trap the player" contract. Unlike Intro there is no scroll to
/// intercept first: `BootConsole` has nothing to read, only to watch break, so
/// EVERY key (not just some) is a skip.
fn tick_boot_console(
    time: Res<Time>,
    mut timer: ResMut<BootConsoleTimer>,
    mut nav: MessageReader<MenuNav>,
    mut presses: MessageReader<InteractPressed>,
    mut next: ResMut<NextState<GameState>>,
) {
    timer.0 += time.delta();
    let skipped = nav.read().count() > 0 || presses.read().count() > 0;
    if skipped || timer.0 >= BOOT_CONSOLE_DURATION {
        next.set(GameState::Splash);
    }
}

/// `OnEnter(MemoryView | Epilogue)`: reset the shared pane scroll so a screen
/// always opens at its top.
fn enter_pane(mut scroll: ResMut<PaneScroll>) {
    scroll.0 = 0;
}

/// `Update` (`MemoryView` only): `↑`/`↓` scroll the journal, `Confirm`/`Back` return
/// to whichever screen opened it.
fn memory_view_nav(
    mut scroll: ResMut<PaneScroll>,
    mut nav: MessageReader<MenuNav>,
    ret: Res<MemoryViewReturn>,
    mut cursor: ResMut<MenuCursor>,
    mut next: ResMut<NextState<GameState>>,
) {
    let mut leave = false;
    for msg in nav.read() {
        match msg {
            MenuNav::Up | MenuNav::Left => scroll.up(),
            MenuNav::Down | MenuNav::Right => scroll.down(),
            MenuNav::Confirm | MenuNav::Back => leave = true,
        }
    }
    if cursor.pending_commit {
        cursor.pending_commit = false;
        leave = true;
    }
    if leave {
        next.set(match *ret {
            MemoryViewReturn::Menu => GameState::Menu,
            MemoryViewReturn::Pause => GameState::Pause,
        });
    }
}

/// `OnEnter(Guide)`: size the chapter cursor to
/// [`crate::guide::GUIDE_CHAPTERS`], reset the body scroll, and tick the
/// `opened_guide` first-step — the one checklist item observed at state entry
/// rather than during ordinary play, since "open the guide" IS a state
/// transition, not a camera/puzzle predicate.
fn enter_guide(
    mut cursor: ResMut<MenuCursor>,
    mut scroll: ResMut<PaneScroll>,
    mut progress: ResMut<crate::save::Progress>,
) {
    cursor.reset(crate::guide::GUIDE_CHAPTERS.len());
    scroll.0 = 0;
    if crate::first_steps::mark_done(&mut progress, crate::first_steps::FirstStep::OpenedGuide) {
        progress.save();
    }
}

/// `Update` (Guide only): `↑`/`↓` change the highlighted chapter (resetting the
/// body scroll to its top, so a new chapter always opens at its own top);
/// `←`/`→` scroll the current chapter's body; `Back` (Esc/q) returns to
/// whichever screen opened it ([`GuideReturn`]). `Confirm` is inert — a rules
/// reference has nothing to commit.
fn guide_nav(
    mut cursor: ResMut<MenuCursor>,
    mut scroll: ResMut<PaneScroll>,
    mut nav: MessageReader<MenuNav>,
    ret: Res<GuideReturn>,
    mut next: ResMut<NextState<GameState>>,
) {
    let mut leave = false;
    for msg in nav.read() {
        match msg {
            MenuNav::Up => {
                cursor.step(-1);
                scroll.0 = 0;
            }
            MenuNav::Down => {
                cursor.step(1);
                scroll.0 = 0;
            }
            MenuNav::Left => scroll.up(),
            MenuNav::Right => scroll.down(),
            MenuNav::Confirm => {}
            MenuNav::Back => leave = true,
        }
    }
    if cursor.pending_commit {
        cursor.pending_commit = false;
    }
    if leave {
        next.set(match *ret {
            GuideReturn::Menu => GameState::Menu,
            GuideReturn::Pause => GameState::Pause,
        });
    }
}

/// `OnExit(Playing)` (chained AFTER [`assemble_readout`], BEFORE [`exit_playing`]):
/// **capture which finale fired**, while the live [`PuzzleState`] still exists.
///
/// This is the whole of the "distinguish the two endings" wiring, and it needed no
/// `re_puzzle` change: [`PuzzleState::cycle_held`] is already public, already
/// re-derived from scratch by every forward pass, and already the exact flag
/// `refresh_solved` short-circuits the `while(true)` win on.
///
/// Clears the outcome on every other exit — pausing, quitting to menu, finishing a
/// non-finale level — so a stale ending can never re-route a later win.
fn capture_ending(
    solved: Res<Solved>,
    puzzle: Res<PuzzleState>,
    catalog: Res<LevelCatalog>,
    current: Res<CurrentLevel>,
    mut outcome: ResMut<EndingOutcome>,
) {
    outcome.0 = None;
    if !solved.0 {
        return;
    }
    let Some(id) = catalog.id_at(current.0) else {
        return;
    };
    if !campaign_manifest().is_finale(id) {
        return;
    }
    outcome.0 = Some(if puzzle.cycle_held {
        Ending::WhileTrue
    } else {
        Ending::Exit0
    });
}

/// `OnEnter(Won)` (chained after [`record_progress`]): route the campaign's finale
/// to its own end-card instead of the generic solved banner.
///
/// Won is still ENTERED for the one frame this takes, and that is deliberate: it
/// is where `record_progress` marks the finale solved (so both endings complete the
/// campaign) and where the win audio has already fired. The `Won` overlay itself
/// stands down while an ending is pending (`menu::draw_won` early-returns), so the
/// player never sees the generic banner flash.
fn route_finale(outcome: Res<EndingOutcome>, mut next: ResMut<NextState<GameState>>) {
    if outcome.0.is_some() {
        next.set(GameState::Ending);
    }
}

/// `OnEnter(Ending)`: zero the card's clock + scroll and size the offer row.
fn enter_ending(
    mut timer: ResMut<EndingTimer>,
    mut scroll: ResMut<PaneScroll>,
    mut cursor: ResMut<MenuCursor>,
    outcome: Res<EndingOutcome>,
) {
    timer.0 = std::time::Duration::ZERO;
    scroll.0 = 0;
    // `exit 0` offers one item (onward); `while(true)` offers two (onward, or the
    // loop). The cursor starts on the FIRST — the loop is never pre-selected.
    let items = match outcome.0 {
        Some(Ending::WhileTrue) => 2,
        _ => 1,
    };
    cursor.reset(items);
}

/// The end-card's offer row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndingItem {
    /// Go on to the shared epilogue.
    Continue,
    /// Accept the loop: open the next session with the cursor back on level 1.
    /// Offered on the `while(true)` card ONLY, and never pre-selected.
    BeginAgain,
}

/// The offer row for an ending, in display order.
#[must_use]
pub fn ending_items(ending: Ending) -> Vec<EndingItem> {
    match ending {
        Ending::Exit0 => vec![EndingItem::Continue],
        Ending::WhileTrue => vec![EndingItem::Continue, EndingItem::BeginAgain],
    }
}

/// The state the end-card's offer row may write, bundled to stay inside Bevy's
/// system-argument budget.
#[derive(bevy::ecs::system::SystemParam)]
struct EndingSinks<'w> {
    progress: ResMut<'w, crate::save::Progress>,
    catalog: Res<'w, LevelCatalog>,
    current: ResMut<'w, CurrentLevel>,
    next: ResMut<'w, NextState<GameState>>,
}

/// **Accept the loop offer.** Opens the next session (`session += 1`), parks the
/// campaign cursor on level 1, persists, and points the play target at that level.
///
/// `solved` and `phrases` are NOT touched — the journal survives the loop. That is
/// the horror of this ending, and it is asserted by the scripted walk.
/// Returns `false` (and changes nothing) when the manifest holds no levels.
///
/// A plain function over borrowed state (not a system), so both entry points — the
/// `while(true)` end-card and the main-menu offer — run the exact same mutation.
fn accept_loop_offer(
    progress: &mut crate::save::Progress,
    catalog: &LevelCatalog,
    current: &mut CurrentLevel,
) -> bool {
    let Some(first) = campaign_manifest().first_id().map(str::to_string) else {
        return false;
    };
    progress.begin_next_session(&first);
    progress.save();
    if let Some(i) = catalog
        .paths
        .iter()
        .position(|p| LevelCatalog::label(p) == first)
    {
        current.0 = i;
    }
    true
}

/// `Update` (Ending only): drive the end-card.
///
/// Hand-rolled `MenuNav` reading (the precedent is `settings_nav`): `↑`/`↓` scroll
/// the readout, `←`/`→` move along the offer row, `Confirm` commits, `Back` goes
/// to the Menu. During the EYE phase every key simply fast-forwards to the
/// readout, so the card can never be committed by a keypress the player meant as
/// "skip".
fn ending_nav(
    time: Res<Time>,
    mut timer: ResMut<EndingTimer>,
    mut scroll: ResMut<PaneScroll>,
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageReader<MenuNav>,
    outcome: Res<EndingOutcome>,
    mut sinks: EndingSinks,
) {
    timer.0 += time.delta();
    let items = ending_items(outcome.0.unwrap_or(Ending::Exit0));
    let eye = timer.phase() == EndingPhase::Eye;

    let mut commit = cursor.pending_commit;
    cursor.pending_commit = false;
    let mut back = false;
    let mut any_key = false;
    for msg in nav.read() {
        any_key = true;
        match msg {
            MenuNav::Up => scroll.up(),
            MenuNav::Down => scroll.down(),
            MenuNav::Left => cursor.index = cursor.index.saturating_sub(1),
            MenuNav::Right => cursor.index = (cursor.index + 1).min(items.len() - 1),
            MenuNav::Confirm => commit = true,
            MenuNav::Back => back = true,
        }
    }

    if eye {
        // Any input during the eye phase only reveals the readout.
        if any_key || commit {
            timer.skip_eye();
            scroll.0 = 0;
            cursor.index = 0;
        }
        return;
    }

    if back {
        // The Menu always stays reachable from an end-card — the loop is an offer.
        sinks.next.set(GameState::Menu);
        return;
    }
    if !commit {
        return;
    }
    match items[cursor.index.min(items.len() - 1)] {
        // Both endings hand off to the SAME epilogue.
        EndingItem::Continue => sinks.next.set(GameState::Epilogue),
        EndingItem::BeginAgain => {
            let EndingSinks {
                progress,
                catalog,
                current,
                next,
            } = &mut sinks;
            if accept_loop_offer(progress, catalog, current) {
                next.set(GameState::Diving);
            }
        }
    }
}

/// `Update` (Epilogue only): `↑`/`↓` scroll, `Confirm`/`Back` return to the Menu.
fn epilogue_nav(
    mut scroll: ResMut<PaneScroll>,
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageReader<MenuNav>,
    mut next: ResMut<NextState<GameState>>,
) {
    let mut leave = cursor.pending_commit;
    cursor.pending_commit = false;
    for msg in nav.read() {
        match msg {
            MenuNav::Up | MenuNav::Left => scroll.up(),
            MenuNav::Down | MenuNav::Right => scroll.down(),
            MenuNav::Confirm | MenuNav::Back => leave = true,
        }
    }
    if leave {
        next.set(GameState::Menu);
    }
}

#[cfg(test)]
mod tests;
