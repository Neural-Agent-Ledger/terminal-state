//! Canon consistency — the shipped corpus must sit in the AGON era and carry the
//! NAL frame (the setting's canonical custodial institution).
//!
//! This is a GREP guard, deliberately. The defect it prevents is not a logic error —
//! it is a stale string surviving in a file nobody re-read.
//!
//! # Two generations of this guard
//! The first cut checked four literal needle strings (`"2007."`, `"2026.06.15"`, …).
//! That version went BLIND on three real bugs found in the level corpus: a
//! hyphen-separator variant of a converted date (`2007-03-14` vs `2007.03.14`), a
//! copy-pasted researcher-note date that used a different day than the literal
//! string it was cloned from, and — worst — a line that had ONE of its two dates
//! converted and not the other, which a presence-only check cannot see because
//! both the correct and the inverted line satisfy "both dates appear somewhere in
//! the file." This version checks the date's SHAPE and RANGE instead of a fixed
//! string list, plus a dedicated test for the 19-year relationship between the two
//! canon anchor dates (`a_line_stating_both_slot_clocks_keeps_them_nineteen_years_apart`),
//! not just their independent presence.
//!
//! # How to run
//! ```sh
//! cargo test -p terminal_state --test canon_consistency
//! ```

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Is `path`'s file name a shipped CAMPAIGN (`L1-first-pulse.lvl` … `L12-closing.lvl`)
/// or ARCHIVE (`01.lvl` … `08.lvl`) level — the two sets that ship with the game repo?
/// Every other `.lvl` under `levels/` is an engine example/showcase/fixture level that
/// lives in the engine repo and is out of scope for a canon-consistency check about the
/// game's OWN fiction and lore.
fn is_campaign_or_archive_level(path: &std::path::Path) -> bool {
    const ARCHIVE: [&str; 8] = [
        "01.lvl", "02.lvl", "03.lvl", "04.lvl", "05.lvl", "06.lvl", "07.lvl", "08.lvl",
    ];
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let is_campaign =
        name.starts_with('L') && name.as_bytes().get(1).is_some_and(u8::is_ascii_digit);
    is_campaign || ARCHIVE.contains(&name)
}

/// Every file whose text the PLAYER can reach: the lore corpus and the shipped
/// campaign + archive level data (`is_campaign_or_archive_level`). Used by the
/// Ministry-frame needle scan and the interval-presence check, which are string
/// checks, not date-shape checks, and remain safe to run over the full shipped set.
///
/// Engine example/showcase/fixture levels (`08-material-demo`, `10-calibration-
/// vault`, `A1-zone-materials`, …) are DROPPED here: they carry no game fiction,
/// they live in the engine repo where this test does not run, and scanning them
/// was never this check's job — it is a canon check for the SHIPPED STORY, not
/// the engine's demo corpus.
fn shipped_text_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut out = Vec::new();
    let lore = root.join("docs/lore");
    for e in std::fs::read_dir(&lore).unwrap_or_else(|e| panic!("{}: {e}", lore.display())) {
        let p = e.expect("dir entry").path();
        if p.extension().and_then(|s| s.to_str()) == Some("md") {
            out.push(p);
        }
    }
    out.extend(level_files());
    assert!(
        out.len() > 25,
        "expected the full shipped corpus (lore .md + campaign/archive .lvl), found {}",
        out.len()
    );
    out
}

/// Only the shipped campaign + archive level corpus (`levels/*.lvl`, filtered by
/// `is_campaign_or_archive_level`) — the file set the date-SHAPE/RANGE checks below
/// apply to.
///
/// `docs/lore/*.md` legitimately carries real-world revision-date prose in many
/// different natural-language shapes ("Дата: 2026-06-17", "AMENDED 2026-06-18",
/// "## ⟦Revision 2026-07-30 …⟧", "Resolved per the user's 2026-07-30 …") that
/// cannot be safely enumerated as a metadata allowlist without either false
/// positives or edits to `docs/` — and this task round is explicitly not
/// authorized to touch `docs/`. The level corpus is the file set this round owns,
/// is far smaller, and every real-world date already in it funnels through the
/// small, closed set of shapes `line_is_authoring_metadata` recognizes (verified
/// against the actual corpus, not assumed — see that function's doc comment).
/// The needle-based Ministry-frame and interval-presence checks still cover the
/// full `shipped_text_files()` set; only the stricter shape/range/ordering checks
/// are narrowed to `levels/`.
fn level_files() -> Vec<PathBuf> {
    let root = repo_root();
    let d = root.join("levels");
    let mut out: Vec<PathBuf> = std::fs::read_dir(&d)
        .unwrap_or_else(|e| panic!("{}: {e}", d.display()))
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("lvl"))
        .filter(|p| is_campaign_or_archive_level(p))
        .collect();
    assert!(
        out.len() == 20,
        "expected exactly the shipped campaign (12) + archive (8) level corpus, found {}",
        out.len()
    );
    out.sort();
    out
}

/// The repo's supersession marker vocabulary: append-only lore documents never
/// erase history, they mark it superseded in place — see the real banners in
/// `docs/lore/EMILY.md` ("⟦**SUPERSEDED BY v6.1 …**⟧") and
/// `docs/lore/STORY_BIBLE.md` ("`OBSOLETE`/`RESOLVED`", "banner-marked
/// SUPERSEDED;").
const SUPERSESSION_MARKERS: [&str; 3] = ["OBSOLETE", "SUPERSEDED", "RESOLVED"];

/// Is `line` marked as superseded by the repo's own convention?
///
/// This is a TOKEN match, not a substring match: a marker only counts if the
/// characters immediately before and after it (if any) are not ASCII
/// alphanumeric — i.e. the marker stands alone, bounded by start/end of line,
/// whitespace, or punctuation (`` ` ``, `*`, `_`, `[`, `]`, `(`, `)`, `:`,
/// `;`, `—`, `.`, `,`, quotes, …— anything that is not a letter or digit).
/// A bare `.contains("RESOLVED")` would also fire on "the incident remains
/// unRESOLVED" or "never RESOLVEDLY" — an accidental substring, not the
/// repo's all-caps standalone marker — and would silently exempt every
/// needle sharing that line, including a live pre-AGON date or Ministry-frame
/// hit. Deliberately no regex crate: `match_indices` + a boundary check on
/// the adjacent `char`s is enough and keeps this test dependency-free.
fn line_is_superseded(line: &str) -> bool {
    fn is_word_char(c: char) -> bool {
        c.is_ascii_alphanumeric()
    }
    SUPERSESSION_MARKERS.iter().any(|marker| {
        line.match_indices(marker).any(|(idx, matched)| {
            let before_ok = line[..idx]
                .chars()
                .next_back()
                .is_none_or(|c| !is_word_char(c));
            let after_ok = line[idx + matched.len()..]
                .chars()
                .next()
                .is_none_or(|c| !is_word_char(c));
            before_ok && after_ok
        })
    })
}

/// Real-world authoring metadata: a date on one of these lines describes OUR
/// world (when a spec was revised, when a file was last touched by a tool, when
/// a design decision was approved) — never the fiction's AGON-era clock. The
/// date-range and slot-interval checks must not "fix" these; they are correct
/// as real calendar dates and are explicitly out of this task's substitution
/// mandate ("real-world dates stay, in-world fiction moves").
///
/// The distinction can't be made from the digits alone — `2026-07-30` is a
/// correct real revision date on a `spec_rev` line and a leftover fictional
/// date on an un-converted placard line, same shape, same era. It has to be
/// made from what KIND of line carries the date. Every marker below is backed
/// by a real, currently-shipping line in `levels/` (verified by grep, not
/// assumed) — this is not a speculative allowlist. Examples are drawn from the
/// shipped CAMPAIGN corpus, since `level_files()` is narrowed to campaign +
/// archive and the engine-only levels these examples used to cite
/// (`A1-zone-materials`, `10-calibration-vault`, `08-material-demo`) are no
/// longer scanned by this test:
///
/// * `spec_rev` — the `[meta]` field every level carries, e.g.
///   `levels/L2-two-make-three.lvl`: `spec_rev    = 2026-07-30`.
/// * `docs/` — an inline reference to a design-note file path in a level's
///   header comment, recording which internal working note a pass followed.
/// * `.md` — a markdown doc filename. Needed IN ADDITION to `docs/` because a
///   long design-note-path comment can wrap, landing `docs/` on one line and
///   the dated filename (still ending `.md`) on the next — several shipped
///   level headers split exactly this way.
/// * `(real-world)` — an explicit marker authors add by hand next to a date
///   that must not be read as fiction, for a one-off case that fits none of
///   the other shapes, e.g. `levels/L2-two-make-three.lvl:246`:
///   `# WHY THE DETOUR (real-world) 2026-08-07: …`.
/// * `user-approved` — the `POWER BUDGET (v0.2 re-scope, user-approved …)`
///   boilerplate comment repeated near-verbatim across every campaign level.
/// * `ts_editor` — the level-editor round-trip dev note, e.g. (historically)
///   `# NOTE 2026-07-30: this file was round-tripped through ts_editor …`.
///   No shipped campaign/archive level currently carries this marker (it only
///   ever appeared on the now out-of-scope `08-material-demo` level); it is kept
///   in the vocabulary rather than removed because it is a real, unambiguous
///   authoring-metadata shape that a future campaign level's editor round-trip
///   note could use, and a marker with no current match is harmless — it never
///   fires, so it cannot mask a real fictional date.
/// * `tail.log` — the one RENDERED exception. `levels/L3-full-spectrum.lvl`'s
///   `[note system_log]` body line `> tail.log  modified 2026-06-18 03:14` is
///   the canonical "Future mtime" real-machine-time device
///   (`docs/lore/STORY_BIBLE.md:332`, §7) — it is *meant* to look like our
///   calendar, not the fiction's. Because it is a live note body the player
///   reads, no visible `(real-world)` tag can be stitched into it without
///   breaking the in-game read (and a leading `#` there would make the parser
///   treat the line as a comment) — the filename token is the marker instead.
fn line_is_authoring_metadata(line: &str) -> bool {
    const MARKERS: [&str; 7] = [
        "spec_rev",
        "docs/",
        ".md",
        "(real-world)",
        "user-approved",
        "ts_editor",
        "tail.log",
    ];
    MARKERS.iter().any(|m| line.contains(m))
}

#[test]
fn authoring_metadata_rule_recognizes_real_and_rejects_fictional() {
    // Positive: a real `spec_rev` line states OUR world's revision date.
    assert!(line_is_authoring_metadata("spec_rev    = 2026-07-30"));
    // Positive: the round-trip dev note, verbatim shape from the shipped corpus.
    assert!(line_is_authoring_metadata(
        "# NOTE 2026-07-30: this file was round-tripped through ts_editor (which drops `#`"
    ));
    // Negative: a fictional placard date — the corpus's own clock, carrying
    // none of the metadata shapes — must NOT be waved through.
    assert!(!line_is_authoring_metadata(
        "    >   aleks_v currently logged session 0001 at timestamp 2026.06.15"
    ));
}

/// Scan every shipped text file for any of `needles`, reporting `path:line — text`
/// hits.
///
/// EXEMPTION: a line for which `line_is_superseded` is true is skipped
/// entirely. `docs/lore/STORY_BIBLE.md` is append-only and its revision
/// history is expected to CONTAIN the retired Ministry frame / pre-AGON dates
/// on a line that also carries a genuine marker — that is the record of what
/// was replaced, not a survival of the non-canon frame. An unmarked line
/// still fails: this is a narrow, explicit carve-out for the historical
/// record, not a general weakening of the guard.
fn scan(needles: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for path in shipped_text_files() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (i, line) in src.lines().enumerate() {
            if line_is_superseded(line) {
                continue;
            }
            for n in needles {
                if line.contains(n) {
                    hits.push(format!("{}:{} — {}", path.display(), i + 1, line.trim()));
                }
            }
        }
    }
    hits
}

/// A `YYYY-MM-DD` or `YYYY.MM.DD` date-shaped token found in a line: the byte
/// offset of its first digit and its parsed year.
#[derive(Clone, Copy)]
struct DateToken {
    pos: usize,
    year: u32,
}

/// Scan `line` for every date-shaped token `YYYY-MM-DD` / `YYYY.MM.DD` (both
/// separators must be the SAME character within one token — `2026-06.15` does
/// not count), returning one `DateToken` per match.
///
/// Deliberately no regex crate: fixed-width byte matching is enough for this
/// corpus's date shapes and keeps this test dependency-free. Operates on raw
/// bytes (never slices the `&str`) so it cannot panic on a UTF-8 char boundary
/// even though several shipped files mix in Cyrillic/accented text on the same
/// line as a date.
fn find_date_tokens(line: &str) -> Vec<DateToken> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 10 <= bytes.len() {
        let d = |b: u8| b.is_ascii_digit();
        let matches = d(bytes[i])
            && d(bytes[i + 1])
            && d(bytes[i + 2])
            && d(bytes[i + 3])
            && (bytes[i + 4] == b'-' || bytes[i + 4] == b'.')
            && d(bytes[i + 5])
            && d(bytes[i + 6])
            && bytes[i + 7] == bytes[i + 4]
            && d(bytes[i + 8])
            && d(bytes[i + 9]);
        if matches {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_digit();
            let after_ok = i + 10 == bytes.len() || !bytes[i + 10].is_ascii_digit();
            if before_ok && after_ok {
                let year = u32::from(bytes[i] - b'0') * 1000
                    + u32::from(bytes[i + 1] - b'0') * 100
                    + u32::from(bytes[i + 2] - b'0') * 10
                    + u32::from(bytes[i + 3] - b'0');
                out.push(DateToken { pos: i, year });
                i += 10;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// The AGON era's first year — TS's "now" is set within 2069-75.
const AGON_ERA_START: u32 = 2069;

/// The corpus's own established founding/decommission year — the lab was
/// sealed ~2052, nineteen years before the "current" 2071 reading
/// (`docs/lore/NORMATIVE_NOTES.md:20`). This is legitimate in-fiction content,
/// not a leftover: the date-range check below must not flag it just because
/// it is numerically < 2069. Kept as its own named constant (not folded into
/// `AGON_ERA_START`) because the two years mean different things: one is
/// "the fiction's present", the other is "the fiction's own past".
const FOUNDING_ERA_YEAR: u32 = 2052;

/// No shipped level date may predate the AGON era. TS is set in 2069-75; the
/// corpus's own founding-era anchor (2052) and authoring metadata are exempt
/// (see `FOUNDING_ERA_YEAR` and `line_is_authoring_metadata`).
///
/// This replaces the original four-literal-needle version of this test, which
/// could only catch an EXACT string and went blind on: a hyphen-separator
/// variant of an already-converted date (`2007-03-14` next to the converted
/// `2007.03.14`), and five researcher-note dates (`2026.MM.DD`, `01/02/03/05.lvl`)
/// that were never in the needle list at all because they were never the
/// literal `2026.06.15` string being searched for. A SHAPE + RANGE check finds
/// any of them regardless of separator, day, or which specific pre-AGON year
/// they use.
#[test]
fn no_shipped_text_carries_a_pre_agon_date() {
    let mut hits = Vec::new();
    for path in level_files() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (i, line) in src.lines().enumerate() {
            if line_is_superseded(line) || line_is_authoring_metadata(line) {
                continue;
            }
            for tok in find_date_tokens(line) {
                if tok.year < AGON_ERA_START && tok.year != FOUNDING_ERA_YEAR {
                    hits.push(format!(
                        "{}:{} — year {}: {}",
                        path.display(),
                        i + 1,
                        tok.year,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "pre-AGON-shaped dates survive in the level corpus:\n{}",
        hits.join("\n")
    );
}

/// `spec_rev` points at a DESIGN DOCUMENT in our world, not at a date in the
/// fiction. A level that "conforms to spec revision 2071-06-15" points at a
/// design-note revision that will never exist and silently breaks the
/// correspondence between a level and the design note it implements. The
/// authoring-metadata exemption in `line_is_authoring_metadata` exists for
/// exactly this case — this test proves the exemption actually fires AND that
/// every shipped `spec_rev` value is still a real, pre-AGON calendar date, not
/// just assumed to be.
///
/// Nine `spec_rev` fields were once mistakenly converted to the fiction's own
/// `2071-06-15` clock instead of staying a real calendar date — this test is
/// what would have caught that.
#[test]
fn spec_rev_stays_in_the_real_world() {
    let mut hits = Vec::new();
    for path in level_files() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (i, line) in src.lines().enumerate() {
            if !line.trim_start().starts_with("spec_rev") {
                continue;
            }
            // The exemption must actually fire for this exact line — proves
            // the range check above will not trip on it, rather than assuming.
            assert!(
                line_is_authoring_metadata(line),
                "{}:{} — a spec_rev line must be recognized as authoring metadata: {}",
                path.display(),
                i + 1,
                line.trim()
            );
            let tokens = find_date_tokens(line);
            if tokens.is_empty() {
                hits.push(format!(
                    "{}:{} — spec_rev has no parseable date: {}",
                    path.display(),
                    i + 1,
                    line.trim()
                ));
                continue;
            }
            for tok in &tokens {
                if tok.year >= AGON_ERA_START {
                    hits.push(format!(
                        "{}:{} — spec_rev value (year {}) is not a real-world, pre-AGON date: {}",
                        path.display(),
                        i + 1,
                        tok.year,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "spec_rev must point at a real design-document revision date, not the fiction's \
         clock:\n{}",
        hits.join("\n")
    );
}

/// The Ministry of Computational Heritage has ZERO occurrences anywhere in the
/// wider fiction this game crosses over with. It is replaced by the Apollyon
/// Archive (place) + NAL Legacy Stewardship (the institution filing).
#[test]
fn no_shipped_text_carries_the_ministry_frame() {
    let hits = scan(&["Ministry", "Kharkiv", "MODEL-07"]);
    assert!(
        hits.is_empty(),
        "the non-canon Ministry frame survives in shipped content:\n{}",
        hits.join("\n")
    );
}

/// Proves the exemption is a TOKEN match, not a substring match. This test
/// FAILS against a bare `line.contains("OBSOLETE") || …` implementation (the
/// first two asserts below all pass for a substring check too, but the
/// substring check also wrongly exempts an unrelated word merely containing
/// the letters — confirmed by temporarily reverting `line_is_superseded` to
/// `.contains(..)` while developing this fix: this test then fails on the
/// first assert).
#[test]
fn the_supersession_exemption_needs_a_real_marker_not_a_substring() {
    // A line that merely CONTAINS the letters must not be exempted.
    assert!(!line_is_superseded(
        "the incident remains unRESOLVED, per §4"
    ));
    assert!(!line_is_superseded("this was never RESOLVEDLY settled"));
    // ...while a genuine marker line is.
    assert!(line_is_superseded(
        "> **SUPERSEDED** — retired 2026-07-30 in favour of v6.1"
    ));
    assert!(line_is_superseded("OBSOLETE: the earlier framing"));
    // The repo's real banners, verbatim.
    assert!(line_is_superseded(
        "> ⟦**SUPERSEDED BY v6.1 — 2026-07-30.**⟧ This document conflicted with the same-day"
    ));
    assert!(line_is_superseded(
        "> не стираются, только помечаются `OBSOLETE`/`RESOLVED`."
    ));
    assert!(line_is_superseded(
        "universe basis. EMILY.md is banner-marked SUPERSEDED; its mirror-clarity-as-memory-dial"
    ));
}

/// The interval is the horror, not the years. Slot 0001 is occupied twice,
/// NINETEEN years apart, and the previous occupant is not relocated.
#[test]
fn the_slot_interval_is_exactly_nineteen_years() {
    let first = 2052;
    let second = 2071;
    assert_eq!(second - first, 19, "the 19-year slot gap is load-bearing");

    let hits = scan(&["2052.03.14"]);
    assert!(
        !hits.is_empty(),
        "the first slot load must appear in the corpus"
    );
    let hits = scan(&["2071.06.15"]);
    assert!(
        !hits.is_empty(),
        "the second slot load must appear in the corpus"
    );
}

/// The 19-year interval is a RELATIONSHIP, not two strings. A guard that only
/// asserts both dates appear somewhere (as `the_slot_interval_is_exactly_
/// nineteen_years` above does) cannot see a line that states them in the WRONG
/// ORDER — which is exactly how `L11-inner-circuit.lvl`'s rack/session clocks
/// got inverted: the rack clock was converted to `2052-03-14` and the session
/// clock was left at a pre-AGON `2026-06-17`, so the "founding" date read as
/// numerically NEWER than the "current" date by 26 years instead of older by
/// 19. Both the founding (2052.03.14) and current (2071.06.15) anchor strings
/// were present in the corpus throughout — the presence-only test never had a
/// chance to catch it.
///
/// Rule: for every line that states a `FOUNDING_ERA_YEAR` (2052) date, any
/// OTHER date-shaped token on that same line must be exactly the current-era
/// year (2071) AND must appear AFTER the 2052 date in reading order — founding
/// first, current second, in that order, every time the two are stated
/// together. A line with 2052 next to any other year (an un-converted 2026, a
/// typo, a future 2052-and-2052 duplicate) fails, as does 2071 appearing
/// before 2052.
#[test]
fn a_line_stating_both_slot_clocks_keeps_them_nineteen_years_apart() {
    const CURRENT_ERA_YEAR: u32 = 2071;
    let mut hits = Vec::new();
    for path in level_files() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (i, line) in src.lines().enumerate() {
            if line_is_superseded(line) || line_is_authoring_metadata(line) {
                continue;
            }
            let tokens = find_date_tokens(line);
            for (fi, f) in tokens.iter().enumerate() {
                if f.year != FOUNDING_ERA_YEAR {
                    continue;
                }
                for (oi, other) in tokens.iter().enumerate() {
                    if oi == fi {
                        continue;
                    }
                    if other.year != CURRENT_ERA_YEAR || other.pos < f.pos {
                        hits.push(format!(
                            "{}:{} — founding date (year {}) at byte {} is not followed, in \
                             order, by the current-era date (year {}); found year {} at byte \
                             {} instead: {}",
                            path.display(),
                            i + 1,
                            f.year,
                            f.pos,
                            CURRENT_ERA_YEAR,
                            other.year,
                            other.pos,
                            line.trim()
                        ));
                    }
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "a shipped line states the founding-era (2052) slot clock beside a wrong-year or \
         wrong-order companion date — the 19-year gap must always read earlier-then-later, \
         2052 then 2071:\n{}",
        hits.join("\n")
    );
}
