//! `save` — the persistent progress store.
//!
//! Tracks which levels the player has solved so the menu can offer a
//! **Continue** entry and Level-Select can mark solved levels. The store is a
//! tiny TOML file; the on-disk shape is:
//!
//! ```toml
//! solved = ["01", "03", "07"]
//! ```
//!
//! ## Level identity
//!
//! Progress keys are **stable level ids**, NOT catalogue indices. A level's id
//! is its file stem (e.g. `levels/03.lvl` → `"03"`), via
//! [`crate::flow::LevelCatalog::label`]. Using the stem (not the volatile
//! catalogue position) means progress survives a catalogue reorder or a new
//! level being inserted in the middle.
//!
//! ## On-disk location
//!
//! Resolved by [`crate::save::progress_path`], in order:
//! 1. `$TS_SAVE_DIR/progress.toml` — explicit override (used by tests so
//!    they never touch the real save).
//! 2. `%APPDATA%\TerminalState\progress.toml` on Windows / `$XDG_CONFIG_HOME` (or
//!    `$HOME/.config`) `/TerminalState/progress.toml` elsewhere — the OS-appropriate
//!    config dir, resolved from environment variables (no extra crate needed).
//! 3. `save/progress.toml` next to the current working directory — the robust
//!    last-resort fallback when no config dir can be determined.
//!
//! ## Robustness
//!
//! Every failure path (missing file, unreadable file, corrupt TOML, a write
//! that cannot create its directory or is denied) is non-fatal: load returns an
//! empty [`crate::save::Progress`] and logs a warning, save logs a warning and returns. The
//! game never panics on save I/O.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

/// The player's persisted progress: the set of solved level ids (file stems).
///
/// A Bevy [`Resource`] loaded once at startup ([`Progress::load`]) and persisted
/// after each genuine solve ([`Progress::save`]). The `solved` set is ordered
/// (a [`BTreeSet`]) so the serialized file is deterministic.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    /// Stable ids (level file stems, e.g. `"01"`) the player has solved.
    #[serde(default)]
    pub solved: BTreeSet<String>,
    /// The assembled memory-readout phrase recorded for each solved level
    /// (`id → phrase`). This is the persistent **memory log** (the story so
    /// far, `docs/lore/WORD_STORY.md` §1.4): keyed by stable id so it survives
    /// a catalogue reorder, ordered (`BTreeMap`) so the file is deterministic.
    /// Optional + `#[serde(default)]` so an old save (solved-only) still loads.
    #[serde(default)]
    pub phrases: BTreeMap<String, String>,
    /// **How many times the campaign has been LOOPED** (W4 4.5).
    ///
    /// `0` on a save that has never taken the `while(true)` loop-back offer, so the
    /// diegetic session number is `session + 1` ([`session_label`](Self::session_label))
    /// and a fresh operator boots into `session 0001` — the canon handshake
    /// (`STORY_BIBLE` §5 L1: `session 0001 of N`). Accepting the loop offer
    /// increments it, and the next boot reads `session 0002`.
    ///
    /// Additive + `#[serde(default)]`: an old save (no `session` key) loads with `0`
    /// and is byte-identical in behaviour to before this field existed. There is no
    /// version field in this schema and none is added — per-field defaults ARE the
    /// schema-evolution mechanism here.
    #[serde(default)]
    pub session: u32,
    /// **The loop cursor** (W4 4.5): the stable id of the campaign level a
    /// session-loop is currently sitting on, or `None` when no loop is in progress.
    ///
    /// Set to the campaign's FIRST level by [`begin_next_session`](Self::begin_next_session)
    /// and walked forward by the flow as each level is re-solved. `Continue` honours
    /// it when present ([`crate::flow::LevelCatalog::first_unsolved`]), which is the
    /// whole point: after a loop-back every level is already in `solved`, so the
    /// ordinary "first unsolved" scan would send the player nowhere.
    ///
    /// **`solved` and `phrases` are NEVER cleared by the loop.** The journal survives
    /// — the system remembers even when "you" forget. That is the horror, and it is
    /// asserted by `loop_restart_keeps_solved_and_phrases`.
    #[serde(default)]
    pub campaign_cursor: Option<String>,
    /// **The FIRST STEPS onboarding checklist** (Swaggerfall study report item
    /// 2, `crate::first_steps`): the stable keys (`FirstStep::key`) of every
    /// item ticked by observation during play. Additive + `#[serde(default)]`
    /// so an old save (no `first_steps` key) loads with an empty set — nothing
    /// is retroactively marked done, and the HUD checklist simply starts fresh
    /// for that save. Per-SAVE, not per-level: ticking a step in one room
    /// keeps it ticked in every other.
    #[serde(default)]
    pub first_steps: BTreeSet<String>,
}

impl Progress {
    /// The DIEGETIC session number, zero-padded to four digits: `0001` on a save
    /// that has never looped. (`session` counts loops taken; the operator is always
    /// *in* one session more than they have taken loops.)
    pub fn session_label(&self) -> String {
        format!("{:04}", self.session.saturating_add(1))
    }

    /// The label of the session the loop-back offer would OPEN — `0002` from a
    /// fresh campaign. This is the string the `while(true)` end-card and the menu
    /// offer show ("session 0002 — begin again?").
    pub fn next_session_label(&self) -> String {
        format!("{:04}", self.session.saturating_add(2))
    }

    /// **Loop-back.** Open the next session with the campaign cursor parked on
    /// `first_id` (the campaign's first level).
    ///
    /// Deliberately additive-only: it bumps [`session`](Self::session) and sets
    /// [`campaign_cursor`](Self::campaign_cursor) and touches NOTHING else —
    /// `solved` and `phrases` are carried through the loop untouched.
    pub fn begin_next_session(&mut self, first_id: &str) {
        self.session = self.session.saturating_add(1);
        self.campaign_cursor = Some(first_id.to_string());
    }

    /// Move (or clear) the loop cursor. Returns `true` when it actually changed, so
    /// the caller can skip a redundant disk write.
    pub fn set_campaign_cursor(&mut self, id: Option<&str>) -> bool {
        let next = id.map(str::to_string);
        if self.campaign_cursor == next {
            return false;
        }
        self.campaign_cursor = next;
        true
    }

    /// Is the level with this id marked solved?
    pub fn is_solved(&self, id: &str) -> bool {
        self.solved.contains(id)
    }

    /// Mark a level id solved. Returns `true` if this was newly inserted (so the
    /// caller can skip a redundant disk write when nothing changed).
    pub fn mark_solved(&mut self, id: &str) -> bool {
        self.solved.insert(id.to_string())
    }

    /// Record (or refresh) the assembled memory-readout phrase for a solved
    /// level id. Returns `true` if the stored phrase changed (so the caller can
    /// skip a redundant disk write). Marks the level solved as a side effect, so
    /// recording a phrase always keeps `solved` and `phrases` consistent.
    pub fn record_phrase(&mut self, id: &str, phrase: &str) -> bool {
        self.solved.insert(id.to_string());
        match self.phrases.get(id) {
            Some(existing) if existing == phrase => false,
            _ => {
                self.phrases.insert(id.to_string(), phrase.to_string());
                true
            }
        }
    }

    /// The recorded phrase for a level id, if any.
    pub fn phrase_for(&self, id: &str) -> Option<&str> {
        self.phrases.get(id).map(String::as_str)
    }

    /// Load progress from the resolved [`progress_path`]. Any error — the file
    /// is missing, unreadable, or corrupt — yields an empty `Progress` with a
    /// logged warning. Never panics.
    pub fn load() -> Self {
        match progress_path() {
            Some(path) => Self::load_from(&path),
            None => {
                eprintln!(
                    "terminal-state: no save location available — progress will not persist."
                );
                Self::default()
            }
        }
    }

    /// Load progress from an explicit path. Missing file → empty (silent, the
    /// normal first-run case). Unreadable or corrupt file → empty + a warning.
    pub fn load_from(path: &std::path::Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // First run: no file yet. Empty progress, no noise.
                return Self::default();
            }
            Err(e) => {
                eprintln!(
                    "terminal-state: could not read save '{}': {e} — starting with empty progress.",
                    path.display()
                );
                return Self::default();
            }
        };
        match toml::from_str::<Progress>(&text) {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "terminal-state: save '{}' is corrupt ({e}) — starting with empty progress.",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Persist progress to the resolved [`progress_path`]. All I/O failures are
    /// logged and swallowed — saving is best-effort and never panics.
    pub fn save(&self) {
        let Some(path) = progress_path() else {
            eprintln!("terminal-state: no save location available — progress not persisted.");
            return;
        };
        self.save_to(&path);
    }

    /// Persist progress to an explicit path, creating parent directories as
    /// needed. Failures are logged and swallowed.
    pub fn save_to(&self, path: &std::path::Path) {
        let text = match toml::to_string(self) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("terminal-state: could not serialize progress ({e}) — not saved.");
                return;
            }
        };
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!(
                        "terminal-state: could not create save dir '{}': {e} — progress not saved.",
                        parent.display()
                    );
                    return;
                }
            }
        }
        if let Err(e) = std::fs::write(path, text) {
            eprintln!(
                "terminal-state: could not write save '{}': {e} — progress not saved.",
                path.display()
            );
        }
    }
}

/// The on-disk progress file name.
const PROGRESS_FILE: &str = "progress.toml";

/// Resolve where the progress file lives. See the module docs for the search
/// order. Returns `None` only if no candidate directory can be determined
/// (extremely unlikely — the CWD fallback almost always applies).
pub fn progress_path() -> Option<PathBuf> {
    // 1. Explicit override (tests, or a user who wants a portable save).
    if let Some(dir) = std::env::var_os("TS_SAVE_DIR") {
        return Some(PathBuf::from(dir).join(PROGRESS_FILE));
    }
    // 2. OS-appropriate config dir, from env vars (no `dirs` crate dependency).
    if let Some(dir) = config_dir() {
        return Some(dir.join("TerminalState").join(PROGRESS_FILE));
    }
    // 3. Last-resort: a `save/` folder under the current working directory.
    Some(PathBuf::from("save").join(PROGRESS_FILE))
}

/// The OS config directory, resolved from environment variables only.
///
/// * Windows → `%APPDATA%` (roaming).
/// * Otherwise → `$XDG_CONFIG_HOME`, else `$HOME/.config`.
fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg));
            }
        }
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique temp path under the OS temp dir, namespaced per test so the
    /// suite can run concurrently without collisions. Cleaned by the OS.
    fn temp_path(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        p.push(format!("ts_save_{tag}_{nonce}"));
        p.push(PROGRESS_FILE);
        p
    }

    #[test]
    fn round_trips_through_save_and_load() {
        let path = temp_path("roundtrip");
        let mut prog = Progress::default();
        prog.mark_solved("01");
        prog.mark_solved("03");
        prog.save_to(&path);

        let loaded = Progress::load_from(&path);
        assert_eq!(loaded, prog);
        assert!(loaded.is_solved("01"));
        assert!(loaded.is_solved("03"));
        assert!(!loaded.is_solved("02"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_loads_empty_without_panic() {
        let path = temp_path("missing");
        // Ensure it does not exist.
        let _ = std::fs::remove_file(&path);
        let loaded = Progress::load_from(&path);
        assert_eq!(loaded, Progress::default());
        assert!(loaded.solved.is_empty());
    }

    #[test]
    fn corrupt_file_loads_empty_without_panic() {
        let path = temp_path("corrupt");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "this is not valid toml ][ = = =").unwrap();
        let loaded = Progress::load_from(&path);
        assert_eq!(loaded, Progress::default());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn mark_solved_reports_newness() {
        let mut prog = Progress::default();
        assert!(prog.mark_solved("05"), "first insert is new");
        assert!(!prog.mark_solved("05"), "second insert is not new");
    }

    #[test]
    fn phrases_round_trip_and_mark_solved() {
        let path = temp_path("phrases");
        let mut prog = Progress::default();
        assert!(
            prog.record_phrase("03", "FIRE  ->  SEA"),
            "first record is new"
        );
        assert!(
            !prog.record_phrase("03", "FIRE  ->  SEA"),
            "identical record is not new"
        );
        assert!(
            prog.record_phrase("03", "FIRE  ->  MAGENTA"),
            "changed phrase is new"
        );
        assert!(
            prog.is_solved("03"),
            "recording a phrase marks the level solved"
        );
        prog.save_to(&path);

        let loaded = Progress::load_from(&path);
        assert_eq!(loaded.phrase_for("03"), Some("FIRE  ->  MAGENTA"));
        assert!(loaded.is_solved("03"));
        let _ = std::fs::remove_file(&path);
    }

    /// Back-compat: an OLD save (solved-only, no `[phrases]`) still loads — the
    /// `#[serde(default)]` on `phrases` fills an empty map.
    #[test]
    fn old_solved_only_save_loads() {
        let path = temp_path("oldsave");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "solved = [\"01\", \"03\"]\n").unwrap();
        let loaded = Progress::load_from(&path);
        assert!(loaded.is_solved("01") && loaded.is_solved("03"));
        assert!(
            loaded.phrases.is_empty(),
            "missing phrases default to empty"
        );
        let _ = std::fs::remove_file(&path);
    }

    // ───────────────────── W4 4.5 — session + loop cursor ────────────────────

    /// An OLD save (no `session` / `campaign_cursor` keys at all) still loads, with
    /// both new fields defaulted — the additive-`#[serde(default)]` contract.
    #[test]
    fn old_save_defaults_session_and_cursor() {
        let path = temp_path("oldsession");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "solved = [\"L1-first-pulse\"]\n").unwrap();
        let loaded = Progress::load_from(&path);
        assert!(loaded.is_solved("L1-first-pulse"));
        assert_eq!(loaded.session, 0, "missing session defaults to 0");
        assert_eq!(
            loaded.campaign_cursor, None,
            "missing cursor defaults to None"
        );
        assert_eq!(
            loaded.session_label(),
            "0001",
            "a never-looped save reads session 0001"
        );
        assert_eq!(loaded.next_session_label(), "0002");
        let _ = std::fs::remove_file(&path);
    }

    /// The loop-back is ADDITIVE: it bumps the session and parks the cursor on the
    /// campaign's first level, and leaves `solved` / `phrases` byte-identical. This
    /// is the M2 save-semantics guarantee — the journal survives the loop.
    #[test]
    fn begin_next_session_keeps_solved_and_phrases() {
        let mut prog = Progress::default();
        prog.record_phrase("L1-first-pulse", "FIRE  ->  MODEL");
        prog.record_phrase("L12-closing", "KNOWN  ->  EXIT");
        let solved_before = prog.solved.clone();
        let phrases_before = prog.phrases.clone();

        prog.begin_next_session("L1-first-pulse");

        assert_eq!(prog.session, 1);
        assert_eq!(prog.session_label(), "0002");
        assert_eq!(prog.campaign_cursor.as_deref(), Some("L1-first-pulse"));
        assert_eq!(
            prog.solved, solved_before,
            "the loop must never clear `solved`"
        );
        assert_eq!(
            prog.phrases, phrases_before,
            "the loop must never clear `phrases`"
        );
    }

    /// Session + cursor survive a save/load round-trip, and the cursor setter
    /// reports change honestly (so callers can skip redundant disk writes).
    #[test]
    fn session_and_cursor_round_trip() {
        let path = temp_path("session_rt");
        let mut prog = Progress::default();
        prog.mark_solved("L12-closing");
        prog.begin_next_session("L1-first-pulse");
        assert!(
            !prog.set_campaign_cursor(Some("L1-first-pulse")),
            "no-op write reports unchanged"
        );
        assert!(prog.set_campaign_cursor(Some("L2-two-make-three")));
        prog.save_to(&path);

        let loaded = Progress::load_from(&path);
        assert_eq!(loaded.session, 1);
        assert_eq!(loaded.campaign_cursor.as_deref(), Some("L2-two-make-three"));
        assert_eq!(loaded, prog);

        // Clearing the cursor round-trips as absent, not as an empty string.
        let mut cleared = loaded;
        assert!(cleared.set_campaign_cursor(None));
        cleared.save_to(&path);
        assert_eq!(Progress::load_from(&path).campaign_cursor, None);
        let _ = std::fs::remove_file(&path);
    }

    /// `session` saturates instead of wrapping — a pathological save can never
    /// produce a negative-looking or reset session number.
    #[test]
    fn session_saturates_at_max() {
        let mut prog = Progress {
            session: u32::MAX,
            ..Default::default()
        };
        prog.begin_next_session("L1-first-pulse");
        assert_eq!(prog.session, u32::MAX);
        assert_eq!(prog.session_label(), format!("{:04}", u32::MAX));
    }

    // ───────────────────── FIRST STEPS (Swaggerfall study item 2) ────────────

    /// An OLD save (no `first_steps` key at all) still loads, with the new
    /// field defaulted to empty — the additive-`#[serde(default)]` contract.
    #[test]
    fn old_save_defaults_first_steps_to_empty() {
        let path = temp_path("old_first_steps");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "solved = [\"01\"]\n").unwrap();
        let loaded = Progress::load_from(&path);
        assert!(
            loaded.first_steps.is_empty(),
            "missing first_steps defaults to empty"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// The checklist round-trips through save/load like every other field.
    #[test]
    fn first_steps_round_trip() {
        let path = temp_path("first_steps_rt");
        let mut prog = Progress::default();
        prog.first_steps.insert("moved".to_string());
        prog.first_steps.insert("jumped".to_string());
        prog.save_to(&path);
        let loaded = Progress::load_from(&path);
        assert_eq!(loaded.first_steps, prog.first_steps);
        let _ = std::fs::remove_file(&path);
    }
}
