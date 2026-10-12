//! `story::beats` — what a fired `[trigger]` field *means*: an audio stinger and
//! a one-line narrative beat.
//!
//! `re_level` parses `[trigger]` fields, this crate's `flow::trigger_sys` detects
//! the OUTSIDE→INSIDE entry edge and emits [`crate::flow::TriggerFired`]. That
//! message shipped with **zero subscribers** — it was built for exactly this. This
//! module is the data half of the subscriber: a table from the fired LABEL to
//!
//! * a [`re_audio::SoundId`] stinger (dispatched as a `PlaySound` message through
//!   the existing message-driven director), and optionally a bed change
//!   ([`re_audio::SetAmbience`]) for a beat that changes the room's mood;
//! * a short `>`-prefixed HUD line (the system speaking, `system_log` register),
//!   shown for [`BEAT_LINGER_SECS`] and then gone.
//!
//! ## What the label is
//!
//! [`crate::flow::TriggerFired::label`] is the authored `[trigger]` `action =
//! event <label>` string, or — for `action = activate <node>` — the NODE NAME.
//! The `T0-trigger` engine fixture uses `action = activate gate`, so its label is `gate`.
//! The table therefore keys on that label, case-insensitively, and any label with
//! no entry falls back to [`crate::story::beats::DEFAULT_ROW`] so a trigger is NEVER silent.
//!
//! ## Why a table of PLAIN TEXT and not a match
//!
//! The table names its sounds by **string** and resolves them through
//! [`re_audio::SoundId::from_name`] — the hook `re_audio` added for exactly this.
//! So a trigger label maps to a *sound name*, and `re_audio` can extend or rename
//! its stinger recipes (it grew this table to include `stinger_notice` /
//! `stinger_memory` / `stinger_corrupt`) with no enum reference to update here. An
//! unknown sound name degrades to "no stinger" instead of failing to compile —
//! and `every_cue_names_a_real_sound` is the test that keeps it honest.
//!
//! Labels match by **prefix**, per the register conventions `re_audio` published:
//! `gate`/`entry`/`checkpoint`/`door_`/`terminal_` → notice (Immediate — it never
//! lags the crossing); `memory_`/`mnemos`/`lethe`/`recall_`/`fragment_` → memory
//! (beat-quantized to a strong downbeat, so it can land up to ~a bar late BY
//! DESIGN — which is why its HUD line lingers ~4 s rather than flashing);
//! `corrupt_`/`glitch_`/`decay_`/`fail_`/`alarm_` → corrupt (accent track).
//!
//! No I/O, no terminal, no rendering: [`cue_for`] is a pure lookup and
//! [`NarrativeBeat`] is a plain resource with a `tick`.

use bevy::prelude::Resource;
use re_audio::{AmbienceMood, SetAmbience, SoundId};

/// How long a narrative beat stays on the HUD before it decays, in seconds.
/// ~4 s: long enough to read one line at a walk, short enough that it never
/// becomes furniture.
pub const BEAT_LINGER_SECS: f32 = 4.0;

/// The prefix every narrative beat carries — the `system_log` register's voice
/// (STORY_BIBLE §5: the system addresses you in `>`-prefixed lines).
pub const BEAT_PREFIX: &str = "> ";

/// The sound name every unmapped label falls back to: the cold "system notice"
/// ping. `re_audio` quantizes it `Immediate`, so it never lags the crossing — the
/// recommended default for an arbitrary trigger.
pub const DEFAULT_SOUND: &str = "stinger_notice";

/// One row of the cue table: a label PREFIX and what it means.
///
/// (`PartialEq` but not `Eq`: an ambience carries a float intensity.)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CueRow {
    /// Matched against the START of the fired label, case-insensitively.
    pub label_prefix: &'static str,
    /// The `re_audio` sound NAME (resolved via [`SoundId::from_name`]). Empty ⇒
    /// deliberately silent.
    pub sound: &'static str,
    /// The HUD line, WITHOUT the [`BEAT_PREFIX`]. Empty ⇒ audio only.
    pub beat: &'static str,
    /// A bed change for a beat that shifts the room's mood; `None` ⇒ leave the
    /// ambience alone (the common case — a stinger is punctuation, not weather).
    pub ambience: Option<SetAmbience>,
}

/// The resolved cue for one fired label: exactly what the subscriber dispatches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriggerCue {
    /// The stinger to fire. `None` ⇒ silent (a deliberately silent row, or a
    /// sound name `re_audio` no longer knows).
    pub sound: Option<SoundId>,
    /// A bed change, or `None` to leave the ambience alone.
    pub ambience: Option<SetAmbience>,
    /// The HUD line, WITHOUT the [`BEAT_PREFIX`]. `None` ⇒ audio only.
    pub beat: Option<&'static str>,
}

/// The corruption bed: the one cue family that is weather, not punctuation.
/// (A struct literal rather than `SetAmbience::new` because this is a `const`
/// item; the intensity is already inside that constructor's `[0, 1]` clamp.)
const ALARM_BED: SetAmbience = SetAmbience {
    mood: AmbienceMood::AlarmTension,
    intensity: 0.75,
};

/// The label-prefix → cue table.
///
/// Rows are tried IN ORDER and the first prefix match wins, so a specific prefix
/// must precede a shorter one that would also match. This table grows as new
/// levels author `[trigger]` fields; the rows here cover the shipped demo path
/// (the `T0-trigger` engine fixture fires `gate`) plus the three register
/// conventions `re_audio` published for its stingers.
pub const TRIGGER_CUES: &[CueRow] = &[
    // ── notice register: the facility acknowledges you ──────────────────────
    // T0-trigger — `action = activate gate` ⇒ the label is the node.
    CueRow {
        label_prefix: "gate",
        sound: DEFAULT_SOUND,
        beat: "gate acknowledged. it was already open.",
        ambience: None,
    },
    CueRow {
        label_prefix: "entry",
        sound: DEFAULT_SOUND,
        beat: "entry logged. you are expected.",
        ambience: None,
    },
    CueRow {
        label_prefix: "checkpoint",
        sound: DEFAULT_SOUND,
        beat: "checkpoint. the record advances.",
        ambience: None,
    },
    CueRow {
        label_prefix: "door_",
        sound: DEFAULT_SOUND,
        beat: "the door reads you.",
        ambience: None,
    },
    CueRow {
        label_prefix: "terminal_",
        sound: DEFAULT_SOUND,
        beat: "terminal responds. it was already listening.",
        ambience: None,
    },
    // ── memory register: something is REMEMBERED (beat-quantized stinger) ────
    CueRow {
        label_prefix: "memory",
        sound: "stinger_memory",
        beat: "a word surfaces. it is not yours yet.",
        ambience: None,
    },
    CueRow {
        label_prefix: "mnemos",
        sound: "stinger_memory",
        beat: "something wants this kept.",
        ambience: None,
    },
    CueRow {
        label_prefix: "lethe",
        sound: "stinger_memory",
        beat: "something wants this let go.",
        ambience: None,
    },
    CueRow {
        label_prefix: "recall_",
        sound: "stinger_memory",
        beat: "recall path warm. it remembers being read.",
        ambience: None,
    },
    CueRow {
        label_prefix: "fragment_",
        sound: "stinger_memory",
        beat: "one fragment. not enough for a name.",
        ambience: None,
    },
    // ── corrupt register: the record DEGRADES (takes the bed with it) ────────
    CueRow {
        label_prefix: "corrupt_",
        sound: "stinger_corrupt",
        beat: "record damaged. re-reading from the copy.",
        ambience: Some(ALARM_BED),
    },
    CueRow {
        label_prefix: "glitch_",
        sound: "stinger_corrupt",
        beat: "signal disagrees with itself.",
        ambience: Some(ALARM_BED),
    },
    CueRow {
        label_prefix: "decay_",
        sound: "stinger_corrupt",
        beat: "this sector is not being maintained.",
        ambience: Some(ALARM_BED),
    },
    CueRow {
        label_prefix: "fail_",
        sound: "stinger_corrupt",
        beat: "attempt logged. attempts are cheap.",
        ambience: Some(ALARM_BED),
    },
    CueRow {
        label_prefix: "alarm_",
        sound: "stinger_corrupt",
        beat: "nothing is coming. that is the alarm.",
        ambience: Some(ALARM_BED),
    },
];

/// The cue every unmapped label gets: the notice ping and a register-neutral
/// acknowledgement. A trigger the author gave no cue is still *felt* — the
/// facility noticed you.
pub const DEFAULT_ROW: CueRow = CueRow {
    label_prefix: "",
    sound: DEFAULT_SOUND,
    beat: "something registered that.",
    ambience: None,
};

impl CueRow {
    /// Resolve this row's plain-text sound name into a live [`SoundId`]. An empty
    /// name (deliberate silence) or a name `re_audio` no longer knows yields
    /// `None` — a missing recipe costs a stinger, never a crash.
    fn resolve(self) -> TriggerCue {
        TriggerCue {
            sound: if self.sound.is_empty() {
                None
            } else {
                SoundId::from_name(self.sound)
            },
            ambience: self.ambience,
            beat: if self.beat.is_empty() {
                None
            } else {
                Some(self.beat)
            },
        }
    }
}

/// The cue for a fired trigger label: the first table row whose `label_prefix`
/// the label starts with (case-insensitively, surrounding whitespace ignored),
/// else [`DEFAULT_ROW`]. Never returns "nothing to do".
pub fn cue_for(label: &str) -> TriggerCue {
    let key = label.trim().to_ascii_lowercase();
    TRIGGER_CUES
        .iter()
        .copied()
        .find(|row| key.starts_with(&row.label_prefix.to_ascii_lowercase()))
        .unwrap_or(DEFAULT_ROW)
        .resolve()
}

/// The live narrative beat: at most ONE line, decaying.
///
/// A resource rather than a message queue because the HUD shows one line at a
/// time: a second trigger firing REPLACES the first (the newest beat is the
/// interesting one) and restarts the timer. Ticked only while playing, so a beat
/// does not bleed away behind the pause overlay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct NarrativeBeat {
    /// The line to show, already `>`-prefixed. Empty ⇒ nothing to show.
    text: String,
    /// Seconds of life left. `<= 0` ⇒ expired.
    remaining: f32,
}

impl NarrativeBeat {
    /// Show `line` (a cue's raw text) for [`BEAT_LINGER_SECS`], replacing any
    /// beat already on screen. An empty / whitespace-only line clears instead of
    /// showing a bare `>`.
    pub fn show(&mut self, line: &str) {
        let body = line.trim();
        if body.is_empty() {
            self.clear();
            return;
        }
        self.text = format!("{BEAT_PREFIX}{body}");
        self.remaining = BEAT_LINGER_SECS;
    }

    /// Age the beat by `dt` seconds, clearing it when it expires. Cheap no-op
    /// once nothing is showing.
    pub fn tick(&mut self, dt: f32) {
        if self.remaining <= 0.0 {
            return;
        }
        self.remaining -= dt.max(0.0);
        if self.remaining <= 0.0 {
            self.clear();
        }
    }

    /// Drop any active beat (level load / teardown).
    pub fn clear(&mut self) {
        self.text.clear();
        self.remaining = 0.0;
    }

    /// The line to paint, or `None` when nothing is active.
    pub fn line(&self) -> Option<&str> {
        if self.remaining > 0.0 && !self.text.is_empty() {
            Some(&self.text)
        } else {
            None
        }
    }

    /// Seconds of life left (0 when nothing is showing) — the decay observable
    /// for tests.
    pub fn remaining_secs(&self) -> f32 {
        self.remaining.max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_demo_label_has_its_own_cue() {
        // The T0-trigger engine fixture fires `gate` (its ActivateNode node name).
        let c = cue_for("gate");
        assert_eq!(c.sound, Some(SoundId::StingerNotice));
        assert_eq!(
            c.beat,
            Some("gate acknowledged. it was already open."),
            "the demo path paints a HUD line"
        );
        // Case + whitespace insensitive.
        assert_eq!(cue_for("  GATE ").beat, c.beat);
    }

    #[test]
    fn labels_match_by_prefix_so_authored_suffixes_still_resolve() {
        // The convention is a PREFIX: `corrupt_l7` picks the corrupt register.
        assert_eq!(cue_for("corrupt_l7").sound, Some(SoundId::StingerCorrupt));
        assert_eq!(
            cue_for("memory_shard_3").sound,
            Some(SoundId::StingerMemory)
        );
        assert_eq!(cue_for("Gate_North").sound, Some(SoundId::StingerNotice));
    }

    #[test]
    fn an_unknown_label_still_stings() {
        let c = cue_for("no-such-trigger");
        assert_eq!(c, DEFAULT_ROW.resolve());
        assert_eq!(
            c.sound,
            Some(SoundId::StingerNotice),
            "the default is the Immediate notice ping — a trigger is never silent"
        );
        assert_eq!(cue_for("").sound, c.sound, "an empty label still stings");
    }

    /// The table is plain text; this is the test that keeps it honest against a
    /// `re_audio` rename. (Its counterpart — a missing name degrading to silence
    /// rather than panicking — is `an_unknown_sound_name_degrades_to_silence`.)
    #[test]
    fn every_cue_names_a_real_sound() {
        for row in TRIGGER_CUES.iter().chain(std::iter::once(&DEFAULT_ROW)) {
            assert!(
                SoundId::from_name(row.sound).is_some(),
                "cue '{}' names sound '{}', which re_audio does not know",
                row.label_prefix,
                row.sound
            );
        }
    }

    #[test]
    fn an_unknown_sound_name_degrades_to_silence_not_a_panic() {
        let row = CueRow {
            label_prefix: "x",
            sound: "stinger_that_never_shipped",
            beat: "still speaks",
            ambience: None,
        };
        let c = row.resolve();
        assert_eq!(
            c.sound, None,
            "a missing recipe costs the stinger, not the beat"
        );
        assert_eq!(c.beat, Some("still speaks"));
    }

    #[test]
    fn a_mood_cue_carries_a_bed_change() {
        let c = cue_for("corrupt_record");
        assert_eq!(c.sound, Some(SoundId::StingerCorrupt));
        let amb = c.ambience.expect("corruption takes the bed with it");
        assert_eq!(amb.mood, AmbienceMood::AlarmTension);
        // …while punctuation cues leave the bed alone.
        assert!(cue_for("gate").ambience.is_none());
    }

    /// Rows are first-match, so no row may be SHADOWED by an earlier row whose
    /// prefix is a prefix of its own (that later row could never fire).
    #[test]
    fn no_cue_row_is_shadowed_by_an_earlier_one() {
        for (i, row) in TRIGGER_CUES.iter().enumerate() {
            for earlier in &TRIGGER_CUES[..i] {
                assert!(
                    !row.label_prefix
                        .to_ascii_lowercase()
                        .starts_with(&earlier.label_prefix.to_ascii_lowercase()),
                    "'{}' is shadowed by the earlier '{}'",
                    row.label_prefix,
                    earlier.label_prefix
                );
            }
            assert!(
                !row.label_prefix.is_empty(),
                "an empty prefix would swallow every label; that is DEFAULT_ROW's job"
            );
        }
    }

    #[test]
    fn a_beat_is_prefixed_and_decays_to_nothing() {
        let mut b = NarrativeBeat::default();
        assert_eq!(b.line(), None, "nothing shows by default");
        b.show("gate acknowledged.");
        assert_eq!(b.line(), Some("> gate acknowledged."));
        assert!((b.remaining_secs() - BEAT_LINGER_SECS).abs() < 1e-6);

        // Half the window: still visible.
        b.tick(BEAT_LINGER_SECS / 2.0);
        assert!(b.line().is_some(), "still visible at half the window");
        // Past the window: gone, and it stays gone.
        b.tick(BEAT_LINGER_SECS);
        assert_eq!(b.line(), None, "expired after ~{BEAT_LINGER_SECS}s");
        assert_eq!(b.remaining_secs(), 0.0);
        b.tick(1.0);
        assert_eq!(b.line(), None);
    }

    #[test]
    fn a_second_beat_replaces_the_first_and_restarts_the_timer() {
        let mut b = NarrativeBeat::default();
        b.show("first");
        b.tick(BEAT_LINGER_SECS - 0.5);
        b.show("second");
        assert_eq!(b.line(), Some("> second"), "the newest beat wins");
        assert!(
            (b.remaining_secs() - BEAT_LINGER_SECS).abs() < 1e-6,
            "the timer restarts"
        );
    }

    #[test]
    fn an_empty_line_clears_instead_of_showing_a_bare_prefix() {
        let mut b = NarrativeBeat::default();
        b.show("something");
        b.show("   ");
        assert_eq!(b.line(), None, "never paint a lone '>'");
    }

    #[test]
    fn clear_drops_an_active_beat() {
        let mut b = NarrativeBeat::default();
        b.show("x");
        b.clear();
        assert_eq!(b.line(), None);
    }

    #[test]
    fn a_negative_dt_never_extends_a_beat() {
        let mut b = NarrativeBeat::default();
        b.show("x");
        b.tick(-5.0);
        assert!(b.remaining_secs() <= BEAT_LINGER_SECS);
    }
}
