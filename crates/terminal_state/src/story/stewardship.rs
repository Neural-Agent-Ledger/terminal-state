//! `story::stewardship` — the **held Stewardship corpus**: four documents
//! deliberately not placed on a level wall, reserved instead for the
//! intro/epilogue framing states (Hold: Incident Report (L8/L9), Form 44,
//! Sealed-Door notice, A-7 Addendum).
//!
//! Source of truth: `docs/lore/NORMATIVE_NOTES.md` (the NAL Corp · Legacy
//! Stewardship Division corpus) plus the `[boot.07]` handshake of
//! `docs/lore/STORY_BIBLE.md` §5 L1 / `docs/lore/PUZZLE_PROGRESSION_L1-L12.md`
//! L1(e). Every block below is quoted from those documents; where a line is
//! trimmed it is trimmed, never reworded.
//!
//! The frame changed 2026-08-06 — the corpus was re-authored from the retired
//! "Ministry of Computational Heritage" institutional frame to the NAL Corp ·
//! Legacy Stewardship Division / Apollyon Archive frame. This module was
//! renamed `story::ministry` → `story::stewardship` to match; the public API
//! (names, signatures) did not change.
//!
//! ## Token discipline
//!
//! These bodies flow through the shipped Form A-7 templating pass
//! ([`crate::story::interp`]) exactly like a `[note]` body does, so:
//!
//! * only the documented allowlist resolves (`{username}`, `{hostname}`,
//!   `{locale}`, `{local_time}`, `{sys_uptime}`, …) — and it honours the
//!   `lore_fourth_wall` opt-out by substituting the A-7 §8 fallbacks (`anon` /
//!   `unknown` / `C`);
//! * ANY other `{token}` run causes its whole line to be omitted (the structural
//!   containment rule).
//!
//! **Recorded placement decision:** NORMATIVE_NOTES #6's header reads
//! `> INCIDENT 0001-{prev}:`. `{prev}` is NOT on the allowlist, so under the
//! structural rule the whole headline — the most important line in the
//! document — would drop. It is therefore REWRITTEN TO A LITERAL here as
//! `> INCIDENT 0001:`, which the document itself authorises in its own
//! closing line ("this is incident 0001. there is no other number."). No
//! other token was changed.
//!
//! Pure data + pure composition functions: no Bevy, no I/O, no rendering.

use super::interp::{interpolate, OperatorProfile};

// ══════════════════════════ the held documents ══════════════════════════════

/// The Stewardship letterhead (NORMATIVE_NOTES §0 boilerplate), minus the
/// `Form {NN}` line — each document below carries its own form number.
///
/// Names the facility by TIER AND REGISTER, not by name: `Apollyon` is a
/// reveal the intro/epilogue framing states must not spend — the two AIs are
/// treated the same way, held for the ending, not handed out on the first
/// screen of every session. The `corpus_holds_the_canon_line`
/// test below (and `screens::tests::every_screen_holds_the_canon_line`)
/// enforce this; a bureaucratic form referring to a facility by tier/register
/// instead of its name is, if anything, MORE convincingly a form for it.
pub const LETTERHEAD: &str = "\
NAL CORP · LEGACY STEWARDSHIP DIVISION
Asset Custody Office
re: asset NCP-A-04 (tier 2 cold register, decommission deferred)";

/// **NORMATIVE_NOTES #14 — the Form A-7 addendum.** The "read it a second time"
/// bridge (`STORY_BIBLE` §8: "The form does not change. You do."). Shown TWICE by
/// design: once in the cold-boot [`Intro`](crate::flow::GameState::Intro), once in
/// the [`Epilogue`](crate::flow::GameState::Epilogue). The repetition IS the beat.
pub const A7_ADDENDUM: &str = "\
FORM A-7 — OPERATOR DIAGNOSTIC (addendum)
operators tend to read Form A-7 a second time after Level 7.
the form does not change. you do. on the second reading, note that every field it
lists it already knew about you before you arrived. it disclosed them as courtesy,
not as request.
                                          [ COMPLIANCE: PASSED — see §exceptions ]";

/// **NORMATIVE_NOTES #6 — the Incident Report.** The previous iteration's file.
/// See the module docs for the one recorded `{prev}` → literal rewrite.
pub const INCIDENT_REPORT: &str = "\
> INCIDENT 0001: operator reached final gate. selected: while(true).
> session looped. memory not retained. operator unaware of incident on next entry.
> recommended action: none. the operator will file this report again.
attached, in operator's own hand:
   \"I think I have been here before. the handwriting on the walls is mine.
    I am going to fix it this time.\" — A.   (this is incident 0001. there is no other number.)";

/// **NORMATIVE_NOTES #12 — Form 44, Disposal of Remains.** The forms refer to one
/// another and none of them refer to a way out; the way out is in the asset.
pub const FORM_44: &str = "\
FORM 44 — DISPOSAL OF REMAINS (digital)
the remains consist of: one (1) consciousness, snapshot, incomplete.
disposal method  not applicable. incomplete remains are not remains; they are a process.
                 see FORM 12 (Retention). see FORM 31 (Deferral). see FORM 44 (this form).
                 the forms refer to one another. none of them refer to a way out.
                 there is a way out. it is not in the forms. it is in the asset.";

/// **NORMATIVE_NOTES #13 — the Sealed-Door notice.** Its `[where]` is "splash /
/// финал": the splash already carries it (`crate::menu`), and this is the
/// финал half — the same posting, read from the other side, closing the frame.
pub const SEALED_DOOR: &str = "\
NOTICE — POSTED ON THE FACILITY DOOR (exterior, sealed 2007)
this building is decommissioned. one (1) rack remains powered by directive (FORM 19).
no personnel remain. one (1) steward remains. the steward is not personnel.
if you are reading this from the inside, you did not come in through this door.
you came in through the source.                          [ COPY 1 of 1 ]";

/// The single credits line. **Identical on both endings** — credits and content
/// are never path-exclusive by design, so this string has exactly one call
/// site shape: the shared epilogue.
pub const CREDITS_LINE: &str = "TERMINAL STATE — a first-person ASCII puzzle.  filed complete.";

/// The epilogue's own footer formula (NORMATIVE_NOTES §0). Left deliberately
/// unresolved-free: `{whoami}` is on the documented allowlist, so this line
/// survives the containment pass in BOTH opt states.
pub const RETENTION_FOOTER: &str = "\
This document is complete. If a field is blank, the value was withheld, not unknown.
Filed under operator session {whoami}. Retain until process terminates.";

// ═══════════════════════════ composed screens ═══════════════════════════════

/// The `[boot.07]` cold-start handshake, verbatim in shape from `STORY_BIBLE` §5
/// L1 (`> [boot.07] cold-start nominal. operator handle: … session 0001 of N.`)
/// and PROGRESSION L1(e), expanded across the Tier-1 fields (`{username}@{hostname}`).
///
/// `session_label` is substituted by the CALLER (it is game state, not a probe
/// field) before the A-7 pass runs, so it can never collide with a `{token}`.
fn boot_handshake(session_label: &str) -> String {
    format!(
        "\
> [boot.07] cold-start nominal.
> [boot.07] operator handle: {{username}}@{{hostname}}.
> [boot.07] session {session_label} of N.
> [boot.07] locale {{locale}}. machine time {{local_time}}.
> [boot.07] uptime {{sys_uptime}}. the rack has not been powered down (FORM 19).
> [boot.07] quarantine buffer: capacity 1. occupants reporting as occupant: 1.
> [boot.07] handshake accepted. a slot has been held for you."
    )
}

/// The full **Intro** body: letterhead + the `[boot.07]` handshake + the
/// Form A-7 addendum offer.
///
/// Pure: same `profile` + same `session_label` ⇒ same `Vec<String>`, always. Lines
/// whose tokens did not resolve are already gone (structural containment), so the
/// caller never has to think about the opt-out.
pub fn intro_lines(profile: &OperatorProfile, session_label: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    push_block(&mut out, LETTERHEAD, profile);
    out.push(String::new());
    push_block(&mut out, &boot_handshake(session_label), profile);
    out.push(String::new());
    push_block(&mut out, A7_ADDENDUM, profile);
    out
}

/// The full **Epilogue** body — the SHARED furniture, byte-identical on
/// both endings. Order: the re-read offer, the A-7 addendum it offers, Form 44
/// (the way out is in the asset), the Incident Report (the file the last
/// iteration left), the Sealed-Door notice read from the inside, the retention
/// footer, and the one credits line.
///
/// Nothing here knows which ending fired, by construction: it takes no ending
/// argument. That is the mechanical guarantee behind "both permanently
/// ambiguous / no path-exclusive credits".
pub fn epilogue_lines(profile: &OperatorProfile) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    out.push("the eye opens. there is no pupil in it. it is not looking for you any more.".into());
    out.push(String::new());
    out.push("re-read Form A-7?".into());
    push_block(&mut out, A7_ADDENDUM, profile);
    out.push(String::new());
    push_block(&mut out, FORM_44, profile);
    out.push(String::new());
    push_block(&mut out, INCIDENT_REPORT, profile);
    out.push(String::new());
    push_block(&mut out, SEALED_DOOR, profile);
    out.push(String::new());
    push_block(&mut out, RETENTION_FOOTER, profile);
    out.push(String::new());
    out.push(CREDITS_LINE.into());
    out
}

/// Run one document block through the A-7 pass and append its surviving lines.
/// A block whose every line dropped appends nothing (not even a blank), so an
/// opted-out screen has no holes in it.
fn push_block(out: &mut Vec<String>, block: &str, profile: &OperatorProfile) {
    let resolved = interpolate(block, profile);
    // `interpolate` already removed the omitted lines; an entirely-dropped block
    // collapses to the empty string, whose `split('\n')` would still yield one
    // spurious blank row. Emit nothing for it instead.
    if resolved.is_empty() {
        return;
    }
    out.extend(resolved.split('\n').map(str::to_string));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(lines: &[String]) -> String {
        lines.join("\n")
    }

    /// Tier-1 tokens resolve in the intro, and the session label is spliced in as
    /// plain text (never as a token).
    #[test]
    fn intro_resolves_tier1_and_session() {
        let profile = OperatorProfile::fallbacks();
        let text = joined(&intro_lines(&profile, "0001"));
        assert!(
            text.contains("session 0001 of N"),
            "canon handshake line: {text}"
        );
        assert!(text.contains("operator handle: anon@unknown."), "{text}");
        assert!(text.contains("locale C."), "{text}");
        assert!(text.contains("NAL CORP · LEGACY STEWARDSHIP DIVISION"));
        assert!(text.contains("the form does not change. you do."));
    }

    /// A looped save shows the incremented session in the same canon shape.
    #[test]
    fn intro_carries_the_loop_session_number() {
        let text = joined(&intro_lines(&OperatorProfile::fallbacks(), "0002"));
        assert!(text.contains("session 0002 of N"), "{text}");
    }

    /// STRUCTURAL CONTAINMENT: no composed screen may ever ship an unresolved
    /// `{token}` run to the framebuffer, in EITHER opt state.
    #[test]
    fn no_screen_leaks_an_unresolved_token() {
        for profile in [OperatorProfile::fallbacks(), OperatorProfile::probe()] {
            for (what, lines) in [
                ("intro", intro_lines(&profile, "0001")),
                ("epilogue", epilogue_lines(&profile)),
            ] {
                for line in &lines {
                    assert!(
                        !has_brace_run(line),
                        "{what} leaked an unresolved token: {line:?}"
                    );
                }
            }
        }
    }

    /// The `{prev}` rewrite: the Incident Report's headline SURVIVES the pass
    /// (it would have been dropped verbatim from NORMATIVE_NOTES #6).
    #[test]
    fn incident_report_headline_survives() {
        let text = joined(&epilogue_lines(&OperatorProfile::fallbacks()));
        assert!(
            text.contains("> INCIDENT 0001: operator reached final gate."),
            "{text}"
        );
        assert!(text.contains("there is no other number."), "{text}");
    }

    /// All four held documents land in the intro/epilogue states, and the
    /// epilogue carries exactly one credits line.
    #[test]
    fn all_four_held_documents_are_placed() {
        let profile = OperatorProfile::fallbacks();
        let intro = joined(&intro_lines(&profile, "0001"));
        let epi = joined(&epilogue_lines(&profile));
        assert!(intro.contains("FORM A-7 — OPERATOR DIAGNOSTIC (addendum)"));
        assert!(epi.contains("FORM A-7 — OPERATOR DIAGNOSTIC (addendum)"));
        assert!(epi.contains("FORM 44 — DISPOSAL OF REMAINS (digital)"));
        assert!(epi.contains("> INCIDENT 0001:"));
        assert!(epi.contains("NOTICE — POSTED ON THE FACILITY DOOR"));
        assert_eq!(epi.matches(CREDITS_LINE).count(), 1);
    }

    /// The epilogue is a pure function of the PROFILE only — it takes no ending
    /// argument, so it cannot be path-exclusive. (Compile-time in spirit; asserted
    /// here as "the same bytes twice".)
    #[test]
    fn epilogue_is_identical_across_calls() {
        let p = OperatorProfile::fallbacks();
        assert_eq!(epilogue_lines(&p), epilogue_lines(&p));
    }

    /// Canon guard: this corpus names neither warring AI and never resolves
    /// Mei (MNEMOS/LETHE named lines are capped, Mei stays open).
    #[test]
    fn corpus_holds_the_canon_line() {
        let text = format!(
            "{}\n{}",
            joined(&intro_lines(&OperatorProfile::fallbacks(), "0001")),
            joined(&epilogue_lines(&OperatorProfile::fallbacks()))
        );
        for forbidden in ["MNEMOS", "LETHE", "Apollyon", "Mei", "Emily", "— E."] {
            assert!(
                !text.contains(forbidden),
                "intro/epilogue corpus must not contain {forbidden:?}"
            );
        }
    }

    /// The hand-rolled `\{[^}]+\}` scan the containment rule specifies.
    fn has_brace_run(line: &str) -> bool {
        let chars: Vec<char> = line.chars().collect();
        chars.iter().enumerate().any(|(i, &c)| {
            c == '{'
                && chars[i + 1..]
                    .iter()
                    .position(|&d| d == '}')
                    .is_some_and(|rel| rel > 0)
        })
    }
}
