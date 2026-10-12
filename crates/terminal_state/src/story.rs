//! `story` — the word→story mechanic (`docs/lore/WORD_STORY.md`).
//!
//! This is the thin game-side layer over the already-computed forward pass:
//! when a level is solved, the fired nodes' **anchor words** assemble — in
//! forward-pass topological order — into a one-line "memory readout" phrase.
//! The phrases accumulate across the campaign into one growing **memory log**
//! (the story so far), and the denial / `while(true)` path renders the
//! corrupted, multilingual `pool_alt` form instead of the clean `pool`.
//!
//! No new engine mechanic: it reads `re_level::Node` (the word pools + anchor
//! index) and `re_puzzle::PuzzleState` (the per-node `activation` + the topo
//! order via [`re_puzzle::PuzzleState::fired_in_topo_order`]). It is engine-agnostic and
//! fully headless-testable.
//!
//! ## What lives here
//!
//! * [`crate::story::word_for`] — one fired node → one word (`pool[anchor]` clean, or the
//!   joined `pool_alt` Babel for the corrupted denial form), with safe
//!   fallbacks (`pool[0]`, then the node glyph) so it never panics or yields
//!   an empty token.
//! * [`crate::story::assemble_phrase`] — the fired nodes' words joined `  →  ` in topo order.
//! * [`crate::story::MemoryLog`] — the persistent, append-only list of `(level_id, phrase)`
//!   the campaign accumulates; the story so far.
//! * [`crate::story::MemoryReadout`] — the resource the `Won` overlay reads to paint the
//!   just-solved level's phrase as a recovered-memory line.
//! * [`crate::story::NAME_SHARDS`] / [`crate::story::NameAssembly`] — the seven **name-shards** (one per
//!   campaign level L1–L7) that assemble into the true name
//!   `pid://aleks_v@R·learn·A·edge·seen·named·bloom`; the door reads
//!   [`crate::story::DOOR_NAME_UNKNOWN`] until they are all in hand.
//! * [`crate::story::interp`] — the Form A-7 Tier-1 templating pass over `[note]` bodies.
//! * [`crate::story::beats`] — the `TriggerFired` → stinger + narrative-beat cue table.
//! * [`crate::story::stewardship`] — the held Stewardship corpus (Incident Report, Form 44,
//!   Sealed-Door notice, A-7 addendum) + the composed Intro / Epilogue bodies
//!   the intro/epilogue framing states paint.

use bevy::prelude::*;
use re_level::Level;
use re_puzzle::PuzzleState;

pub mod beats;
pub mod interp;
pub mod stewardship;

/// How a phrase is rendered: the clean primary `pool` (acceptance / `exit 0`)
/// or the corrupted multilingual `pool_alt` Babel (denial / `while(true)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ReadoutForm {
    /// Clean primary-pool rendering — one readable word of memory per node.
    #[default]
    Clean,
    /// Corrupted `pool_alt` rendering — the word rassembled across every
    /// language at once (`огонь/火/fuego/feu…`): a name that lost the one
    /// language it was called right in. Used for the denial / cyclic ending.
    Corrupt,
}

/// The separator between words inside one level's phrase (matches WORD_STORY's
/// `join("  →  ", …)`, the visible signal path through the network).
pub const ARROW: &str = "  ->  ";

/// The word one fired node contributes to its level's phrase.
///
/// * [`ReadoutForm::Clean`] → `pool[anchor]` (the authored dominant word),
///   falling back to `pool[0]`, then the node glyph if the pool is empty.
/// * [`ReadoutForm::Corrupt`] → the node's `pool_alt` words joined with `/`
///   (the Babel collapse), falling back to the clean word when `pool_alt` is
///   empty.
///
/// Never returns an empty string and never panics on an out-of-range `anchor`.
pub fn word_for(node: &re_level::Node, form: ReadoutForm) -> String {
    let clean = node
        .pool
        .get(node.anchor)
        .or_else(|| node.pool.first())
        .cloned()
        .unwrap_or_else(|| node.glyph.to_string());
    match form {
        ReadoutForm::Clean => clean,
        ReadoutForm::Corrupt => {
            if node.pool_alt.is_empty() {
                clean
            } else {
                node.pool_alt.join("/")
            }
        }
    }
}

/// Assemble a level's phrase from the **fired** nodes' anchor words, in
/// forward-pass topological order, joined by [`ARROW`].
///
/// `level` provides the word pools + anchor indices; `puzzle` provides the
/// per-node `activation` and the topo order ([`PuzzleState::fired_in_topo_order`]).
/// Only nodes that fired (`activation == 1`) contribute a word, so a correct
/// solve yields exactly the signal path (inputs → hidden synth → output) and a
/// mis-wired solve that lights a decoy would inject that decoy's word — the
/// memory turns corrupt, by construction.
///
/// Returns an empty string only if no node fired (an unsolved / empty network).
pub fn assemble_phrase(level: &Level, puzzle: &PuzzleState, form: ReadoutForm) -> String {
    let words: Vec<String> = puzzle
        .fired_in_topo_order()
        .into_iter()
        .filter_map(|i| level.nodes.get(i))
        .map(|node| word_for(node, form))
        .collect();
    words.join(ARROW)
}

/// One recorded vignette: the level it came from and its assembled clean phrase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryEntry {
    /// The stable level id (file stem, e.g. `"03"`).
    pub level_id: String,
    /// The assembled clean-pool phrase for that level.
    pub phrase: String,
}

/// The persistent, append-only **memory log**: the story assembled so far.
///
/// Each solved level appends one [`MemoryEntry`]. Read top-to-bottom it is the
/// one growing story (WORD_STORY §1.4) — the seven vignettes of one life. A
/// Bevy [`Resource`] so the `Won` overlay and any "memory" view can read it; it
/// is derived deterministically from the solved levels, so it can also be
/// rebuilt from `Progress` on load.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryLog {
    /// The recorded vignettes, in the order their levels were solved.
    pub entries: Vec<MemoryEntry>,
}

impl MemoryLog {
    /// Append `entry` for a newly-solved level, replacing any prior entry with
    /// the same `level_id` (re-solving a level refreshes, never duplicates).
    /// Returns `true` if the log changed.
    pub fn record(&mut self, entry: MemoryEntry) -> bool {
        if let Some(slot) = self
            .entries
            .iter_mut()
            .find(|e| e.level_id == entry.level_id)
        {
            if slot.phrase == entry.phrase {
                return false;
            }
            slot.phrase = entry.phrase;
            return true;
        }
        self.entries.push(entry);
        true
    }

    /// The story so far as one multi-line string: `id  PHRASE` per line, in
    /// recorded order. Empty when nothing has been solved yet.
    ///
    /// The Won overlay paints the entries individually (centered per line); THIS
    /// joined form is the single-pane journal's body — its consumer is
    /// [`crate::screens::memory_view_lines`], the `MemoryView` state's painter.
    /// It is also the `exit 0` end-card's readout body, so the same bytes the
    /// player has been accumulating are the bytes the finale prints.
    pub fn story(&self) -> String {
        self.entries
            .iter()
            .map(|e| format!("{}  {}", e.level_id, e.phrase))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Is the log empty (no level solved yet this session)?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The just-solved level's readout, surfaced for the `Won` overlay to paint.
///
/// Set on `OnEnter(Won)` from the live `Level` + `PuzzleState`; the menu
/// overlay reads `phrase` and renders it as a recovered-memory line. `form`
/// records whether this was the clean acceptance readout or the corrupted
/// denial collapse (the data path for the `while(true)` / `pool_alt` ending).
#[derive(Resource, Clone, Debug, Default)]
pub struct MemoryReadout {
    /// The assembled phrase to display (clean or corrupt per `form`).
    pub phrase: String,
    /// Which rendering produced `phrase`.
    pub form: ReadoutForm,
}

impl MemoryReadout {
    /// True once a phrase has been assembled (something to show).
    pub fn has_phrase(&self) -> bool {
        !self.phrase.is_empty()
    }
}

// ═══════════════════════════ the name-shard arc ═════════════════════════════
//
// WORD_STORY §5 item 5 / STORY_BIBLE §5 / PUZZLE_PROGRESSION header ("Shard
// map"): the true name is SEVEN shards, one per campaign level L1–L7, complete
// after L7. L8–L12 add NO new shards — they recontextualise, read back, and
// spend the name. Until it is whole the door reads `exit: name unknown`.
//
// Pure string work: every function below is a total function over a level id and
// the set of solved ids. No I/O, no Bevy, no rendering.

/// One of the seven shards of the true name.
///
/// Two strings, and the distinction is load-bearing:
///
/// * `label` is what the LEVEL reveals — the shard as the fiction names it
///   (STORY_BIBLE §5: shard 4 is the boundary `://`, shard 5 is the observer /
///   observed `merge`).
/// * `hash_part` is the byte that shard contributes to the assembled PID string,
///   which canon pins verbatim as `R·learn·A·edge·seen·named·bloom`
///   (`WORD_STORY.md` §4, `CONTENT_PACK_L1-L12_DRAFT.md` L12 readout).
///
/// They coincide for five of the seven; shards 4 and 5 differ, which is exactly
/// why both fields exist instead of one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NameShard {
    /// 1-based shard number (1..=7) — the level it comes from.
    pub index: usize,
    /// How the level names the shard when it is recovered.
    pub label: &'static str,
    /// The shard's byte inside the assembled `pid://…@<hash>` string.
    pub hash_part: &'static str,
}

/// The seven shards, in campaign order (L1 → L7). Fixed content — canon, not a
/// tuning knob.
pub const NAME_SHARDS: [NameShard; 7] = [
    NameShard {
        index: 1,
        label: "R",
        hash_part: "R",
    },
    NameShard {
        index: 2,
        label: "learn",
        hash_part: "learn",
    },
    NameShard {
        index: 3,
        label: "A",
        hash_part: "A",
    },
    NameShard {
        index: 4,
        label: "://",
        hash_part: "edge",
    },
    NameShard {
        index: 5,
        label: "merge",
        hash_part: "seen",
    },
    NameShard {
        index: 6,
        label: "named",
        hash_part: "named",
    },
    NameShard {
        index: 7,
        label: "final byte",
        hash_part: "bloom",
    },
];

/// The invariant prefix of the true name (`STORY_BIBLE` §5 / NAL `NCP-A-04`,
/// legacy handle `aleks_v`). The assembled hash follows the `@`.
pub const TRUE_NAME_PREFIX: &str = "pid://aleks_v@";

/// Separator between the hash's shard bytes (`R·learn·A·…`).
pub const SHARD_SEP: &str = "·";

/// Placeholder for a shard not yet recovered (`WORD_STORY` §4's
/// `░·░·░·░·░·░·░  [unresolved]`).
pub const SHARD_BLANK: &str = "░";

/// The suffix an incomplete name carries.
pub const UNRESOLVED_SUFFIX: &str = "  [unresolved]";

/// What the door/objective reads while the name is incomplete (STORY_BIBLE §5's
/// closing terminal block, verbatim).
pub const DOOR_NAME_UNKNOWN: &str = "exit: name unknown — assemble shards 1..7";

/// The shard a level carries, or `None` when it carries none.
///
/// Keyed by the level's stable id (its file stem) so it survives a catalogue
/// reorder, exactly like [`crate::save::Progress`]:
///
/// * a CAMPAIGN level (per [`crate::flow::categorize`]) whose ordinal is 1..=7 —
///   `01`..`07` legacy or `L1-…`..`L7-…` — carries shard *ordinal*;
/// * `08`, `L8`..`L12` are campaign levels that carry NO new shard (the name
///   completed at L7 — PROGRESSION header);
/// * a demo / showcase / feature level (`08-material-demo`, `T0-trigger`, `U1-…`)
///   is not on the name arc at all and carries none.
pub fn shard_for_level(level_id: &str) -> Option<NameShard> {
    if crate::flow::categorize(level_id) != crate::flow::Category::Campaign {
        return None;
    }
    let n = campaign_ordinal(level_id)?;
    NAME_SHARDS.iter().copied().find(|s| s.index == n)
}

/// The campaign ordinal encoded in a level id: the leading digits, after an
/// optional `L`/`l` prefix (`"01"` → 1, `"L7-hidden-depths"` → 7,
/// `"L12-closing"` → 12). `None` when the id starts with neither.
fn campaign_ordinal(level_id: &str) -> Option<usize> {
    let rest = level_id
        .strip_prefix('L')
        .or_else(|| level_id.strip_prefix('l'))
        .unwrap_or(level_id);
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Which of the seven shards the player holds — the running assembly of the true
/// name.
///
/// Derived, never stored: build it from the solved level ids
/// ([`crate::save::Progress::solved`]) so it needs no save-schema change and can
/// never drift from the progress it describes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NameAssembly {
    /// `held[i]` ⇒ shard `i + 1` is recovered.
    held: [bool; 7],
}

impl NameAssembly {
    /// Build the assembly from the ids of every solved level (order irrelevant;
    /// unknown / non-campaign / shard-less ids are ignored).
    pub fn from_solved_ids<'a>(ids: impl IntoIterator<Item = &'a str>) -> Self {
        let mut held = [false; 7];
        for id in ids {
            if let Some(s) = shard_for_level(id) {
                held[s.index - 1] = true;
            }
        }
        NameAssembly { held }
    }

    /// Is shard `index` (1-based) recovered? Out-of-range ⇒ `false`.
    pub fn holds(&self, index: usize) -> bool {
        (1..=7).contains(&index) && self.held[index - 1]
    }

    /// How many of the seven are in hand.
    pub fn count(&self) -> usize {
        self.held.iter().filter(|h| **h).count()
    }

    /// All seven recovered — the name is whole (canonically: after L7).
    pub fn is_complete(&self) -> bool {
        self.count() == 7
    }

    /// The assembled hash: each shard's byte, or [`SHARD_BLANK`] for one not yet
    /// recovered, joined by [`SHARD_SEP`].
    pub fn hash(&self) -> String {
        NAME_SHARDS
            .iter()
            .map(|s| {
                if self.held[s.index - 1] {
                    s.hash_part
                } else {
                    SHARD_BLANK
                }
            })
            .collect::<Vec<_>>()
            .join(SHARD_SEP)
    }

    /// The full PID line: `pid://aleks_v@R·learn·░·…`, with
    /// [`UNRESOLVED_SUFFIX`] appended while the name is incomplete
    /// (`WORD_STORY` §4's two readouts).
    pub fn pid(&self) -> String {
        let mut s = format!("{TRUE_NAME_PREFIX}{}", self.hash());
        if !self.is_complete() {
            s.push_str(UNRESOLVED_SUFFIX);
        }
        s
    }

    /// The journal / readout line: `true_name: <pid>` (WORD_STORY §1.4 layout).
    pub fn true_name_line(&self) -> String {
        format!("true_name: {}", self.pid())
    }

    /// What the DOOR reads: [`DOOR_NAME_UNKNOWN`] until the name is whole, then
    /// the name itself. This is the "name-shard on the door" beat (WORD_STORY §5
    /// item 5) — pre-completion the exit does not know who is leaving.
    pub fn door_line(&self) -> String {
        if self.is_complete() {
            format!("exit: {TRUE_NAME_PREFIX}{}", self.hash())
        } else {
            DOOR_NAME_UNKNOWN.to_string()
        }
    }
}

/// The one-line announcement for a shard just recovered by solving `level_id`,
/// or `None` when that level carries no shard (a demo, or L8+).
///
/// e.g. `[ name-shard 3 of 7 recovered ]  A`
pub fn shard_recovered_line(level_id: &str) -> Option<String> {
    let s = shard_for_level(level_id)?;
    Some(format!(
        "[ name-shard {} of {} recovered ]  {}",
        s.index,
        NAME_SHARDS.len(),
        s.label
    ))
}

#[cfg(test)]
mod tests;
