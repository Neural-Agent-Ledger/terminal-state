//! The cross-canon bridge placards must RENDER, not merely parse. The repo's
//! recurring defect is authored content that never reaches a frame; L8's C-REF
//! placard is the primary bridge and gets a frame-level guard.
//!
//! A "bridge" is authored content that connects this game's canon to a sibling
//! work's: it must read whole to a newcomer and detonate for a player of the
//! sibling work, and it NEVER names that work. The `no sibling name leaks`
//! assertion in every test below is that doctrine, made mechanical.
//!
//! Covered here:
//! * `case C-REF` on L8, the primary bridge.
//! * The two paired Apollyon letters, L7 and L9.
//! * The `sector 0xXXXX` addresses on L8 and L11.

use std::path::PathBuf;

fn levels_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("levels")
}

fn read_level(file: &str) -> (PathBuf, String) {
    let path = levels_root().join(file);
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{file} loads: {e}"));
    (path, src)
}

/// No bridge may name the sibling work. `AGON`/`SUBSTRATE`/`7743` are that work's
/// own proper nouns; `Eurydice` is the sibling novel's character whose case the
/// Apollyon letters resonate with — the resonance is the bridge, naming the
/// sibling work would be the wink the doctrine forbids.
fn assert_no_sibling_name_leaks(file: &str, src: &str) {
    for forbidden in ["AGON", "SUBSTRATE", "7743", "Eurydice"] {
        assert!(
            !src.contains(forbidden),
            "{file}: a bridge never names a sibling game (spec A3: quiet but explosive) \
             — found {forbidden:?}"
        );
    }
}

/// A placard BINDS when the level-text binder resolves it to a wall face. The
/// binder stamps every placard AT THE CASTER'S EYE LAYER — `resolve_note_wall`
/// hands `reader_eye_layer(..)` to each of its placement stages — so "placed" is
/// exactly the property `re_runtime::notes`'s own
/// `every_shipped_placard_lands_on_a_layer_the_reader_can_see` proves cell by
/// cell (the generic placement mechanism lives in the engine's `re_runtime::notes`;
/// this crate's `level_text::build_level_text` below is the thin binding that
/// supplies the game's own panels + Form A-7 interpolation over it — see that
/// module's doc). An unplaced placard is authored, parsed, tested, and
/// invisible: the failure that hid 24 of 25 shipped placards before commit
/// 63be293.
///
/// `build_level_text` is the PURE half of `bind_level_text` — it stamps into the
/// map we own here and publishes nothing process-wide, so tests in this binary
/// cannot race each other through the global text registry.
fn assert_placard_binds<'a>(
    file: &str,
    lvl: &'a re_level::Level,
    needle: &str,
) -> &'a re_level::LevelNote {
    let idx = lvl
        .notes
        .iter()
        .position(|n| n.text.contains(needle))
        .unwrap_or_else(|| panic!("{file}: the {needle:?} note survives the parse"));
    let mut map = lvl.build_map();
    let (_registry, report) = terminal_state::level_text::build_level_text(lvl, &mut map);
    assert!(
        !report.unplaced.contains(&idx),
        "{file}: the {needle:?} placard resolves to no wall face — it would be \
         authored and invisible"
    );
    assert_eq!(
        report.placed,
        lvl.notes.len(),
        "{file}: {} placard(s) unplaced: {:?}",
        report.unplaced.len(),
        report.unplaced
    );
    &lvl.notes[idx]
}

/// A placard THIS TASK authors must also name the eye layer in its authored `pos`,
/// not merely land there via the binder's fallback search: `pos` IS the wall cell
/// and its z is the layer the caster samples. (The older shipped placards on L9
/// and L11 are authored at z=3 and reach the eye layer through the binder's
/// tolerance path — they render, and moving them would move a blessed frame, so
/// this stricter rule is applied only to new content.)
fn assert_authored_on_the_eye_layer(lvl: &re_level::Level, note: &re_level::LevelNote) {
    let storey = lvl.storey_height().max(1) as i32;
    let eye_layer_in_storey = note.z % storey;
    assert!(
        (1..=2).contains(&eye_layer_in_storey),
        "note z={} lands on in-storey layer {} — the caster's eye layer is 1; \
         the placard would be authored and invisible",
        note.z,
        eye_layer_in_storey
    );
}

// ──────────────────────── the primary bridge (L8) ─────────────────────────────

/// The C-REF placard is authored on L8 and bound to a wall the caster samples.
#[test]
fn l8_cref_placard_is_authored_and_bindable() {
    let (path, src) = read_level("L8-cold-room.lvl");
    assert!(
        src.contains("case C-REF"),
        "the C-REF bridge must be authored on L8"
    );
    assert!(
        src.contains("argument 47,291"),
        "the recurring argument must be named"
    );
    assert_no_sibling_name_leaks("L8-cold-room.lvl", &src);

    let lvl = re_level::load(&path).expect("L8 parses");
    let note = assert_placard_binds("L8-cold-room.lvl", &lvl, "C-REF");
    assert_authored_on_the_eye_layer(&lvl, note);
}

// ──────────────────── the two Apollyon letters (L7/L9) ──────────────────────

/// The word the letters are permitted to spend. The framing SCREENS may not name
/// the building (`terminal_state::screens::tests::every_screen_holds_the_canon_line`,
/// `terminal_state::story::stewardship`'s `corpus_holds_the_canon_line`) — that is the
/// institution's own voice, and it must not. A letter someone left on a wall
/// INSIDE the building may name the building. Neither guard reads `.lvl` note
/// bodies; the distinction is deliberate, not an oversight.
const ARCHIVE: &str = "Apollyon";

/// The register both letters are filed under, in the hand that wrote them.
const COLD_REGISTER: &str = "холодный регистр";

/// The first letter — sent, still upstairs, still able to wait.
#[test]
fn l7_apollyon_letter_is_authored_and_bindable() {
    let (path, src) = read_level("L7-hidden-depths.lvl");
    assert!(src.contains(ARCHIVE), "the L7 letter must name the Archive");
    assert!(
        src.contains(COLD_REGISTER),
        "the L7 letter must be filed in the cold register"
    );
    assert!(src.contains("письмо девятое"), "the L7 letter is the ninth");
    assert_no_sibling_name_leaks("L7-hidden-depths.lvl", &src);

    let lvl = re_level::load(&path).expect("L7 parses");
    let note = assert_placard_binds("L7-hidden-depths.lvl", &lvl, "письмо девятое");
    assert_authored_on_the_eye_layer(&lvl, note);
    assert_eq!(
        note.kind,
        re_level::NoteKind::ModelThought,
        "the letters are filed in the model_thought register"
    );
    // The building names itself: nave and reading room, the two rooms she can
    // still reach in the first letter.
    assert!(note.text.contains("нефе"), "the nave");
    assert!(note.text.contains("читальном зале"), "the reading room");
}

/// The second letter — the same hand, later, and further down.
#[test]
fn l9_apollyon_letter_is_authored_and_bindable() {
    let (path, src) = read_level("L9-stereopsis.lvl");
    assert!(src.contains(ARCHIVE), "the L9 letter must name the Archive");
    assert!(
        src.contains(COLD_REGISTER),
        "the L9 letter must be filed in the cold register"
    );
    assert!(
        src.contains("письмо сорок первое"),
        "the L9 letter is the forty-first"
    );
    assert_no_sibling_name_leaks("L9-stereopsis.lvl", &src);

    let lvl = re_level::load(&path).expect("L9 parses");
    let note = assert_placard_binds("L9-stereopsis.lvl", &lvl, "письмо сорок первое");
    assert_authored_on_the_eye_layer(&lvl, note);
    assert_eq!(
        note.kind,
        re_level::NoteKind::ModelThought,
        "the letters are filed in the model_thought register"
    );
    // The descent is the second letter's whole content: the basement, and the
    // basement under the basement. The two upstairs rooms are shut behind her.
    assert!(note.text.contains("подвал"), "the basement");
    assert!(
        note.text.contains("под подвалом есть ещё один подвал"),
        "the basement under the basement"
    );
}

/// The pair must read as two moments of ONE person, not two unrelated notes:
/// same salutation, same filing stamp, same closing hand, and an ORDER — the L7
/// letter is earlier than the L9 letter, which is what makes the descent legible.
#[test]
fn the_two_apollyon_letters_are_one_hand_at_two_moments() {
    let (_, l7) = read_level("L7-hidden-depths.lvl");
    let (_, l9) = read_level("L9-stereopsis.lvl");
    for stamp in ["не вручено", COLD_REGISTER, ARCHIVE, "ярус 2", "сестра."] {
        assert!(l7.contains(stamp), "the L7 letter is missing {stamp:?}");
        assert!(l9.contains(stamp), "the L9 letter is missing {stamp:?}");
    }
    // Ordered, and the earlier letter is the one on the earlier level.
    assert!(l7.contains("письмо девятое") && !l7.contains("письмо сорок первое"));
    assert!(l9.contains("письмо сорок первое") && !l9.contains("письмо девятое"));
    // In the first letter the reading room is open; in the second it is shut.
    assert!(
        l9.contains("читальный зал закрыли"),
        "the reading room closes behind her"
    );
}

// ───────────────────────── sector addresses (L8/L11) ───────────────────────

/// The sibling's address notation, verbatim from its source: the noun is
/// lowercase `sector`, the address is `0x` plus **exactly four UPPERCASE hex
/// digits** (there it is produced by `format!("0x{:04X}", … & 0xFFFF)` — a 16-bit
/// value). Returns the four digit characters of every address in `src`.
///
/// Deliberately no regex crate, matching the house style of the other content
/// guards (`terminal_state::tests::canon_consistency`): `match_indices` plus a
/// fixed-width look-ahead is enough. Byte slicing is safe because the needle and
/// the four characters after it are ASCII by construction — a non-ASCII byte
/// there fails the hex-digit assertion below rather than panicking, because
/// `chars().take(4)` is char-wise, not byte-wise.
fn sector_addresses(src: &str) -> Vec<String> {
    const NEEDLE: &str = "sector 0x";
    src.match_indices(NEEDLE)
        .map(|(i, _)| src[i + NEEDLE.len()..].chars().take(4).collect())
        .collect()
}

/// Assert one address states the sibling's SHAPE — not a value this test
/// restates from the data, which would prove nothing.
fn assert_is_a_sector_address(where_: &str, digits: &str) {
    assert_eq!(
        digits.chars().count(),
        4,
        "{where_}: a sector address is exactly four hex digits (16-bit), found {digits:?}"
    );
    for c in digits.chars() {
        assert!(
            c.is_ascii_hexdigit() && !c.is_ascii_lowercase(),
            "{where_}: sector digit {c:?} must be an UPPERCASE hex digit \
             (the sibling formats with {{:04X}}), in {digits:?}"
        );
    }
}

/// The address bound to a specific named person in the sibling work. Reusing it
/// would CONTRADICT that canon rather than echo it — the form is the bridge, the
/// number must be ours.
const SIBLINGS_OWN_ADDRESS: &str = "2E11";

/// L8's `face_07` log gains the cold register's address: the dead face is not
/// merely dead, it is filed, and the file has a location.
#[test]
fn l8_face_07_placard_carries_a_sector_address() {
    let (path, src) = read_level("L8-cold-room.lvl");
    let addrs = sector_addresses(&src);
    assert_eq!(
        addrs.len(),
        1,
        "L8 carries exactly one sector address, found {addrs:?}"
    );
    assert_is_a_sector_address("L8-cold-room.lvl", &addrs[0]);
    assert_ne!(
        addrs[0], SIBLINGS_OWN_ADDRESS,
        "that address belongs to someone else"
    );
    assert_no_sibling_name_leaks("L8-cold-room.lvl", &src);

    let lvl = re_level::load(&path).expect("L8 parses");
    let note = assert_placard_binds("L8-cold-room.lvl", &lvl, "sector 0x");
    // The address must ride the placard that names the dead face — not some
    // other wall in the room.
    assert!(
        note.text.contains("hidden_unit face_07: state=dead"),
        "the address belongs on the face_07 log: {:?}",
        note.text
    );
    assert_line_uses_the_siblings_separators("L8-cold-room.lvl", &note.text);
}

/// L11's sequence-lock log gains its address: the lock knows where it keeps your
/// face, and will not let you read it.
#[test]
fn l11_sequence_lock_placard_carries_a_sector_address() {
    let (path, src) = read_level("L11-inner-circuit.lvl");
    let addrs = sector_addresses(&src);
    assert_eq!(
        addrs.len(),
        1,
        "L11 carries exactly one sector address, found {addrs:?}"
    );
    assert_is_a_sector_address("L11-inner-circuit.lvl", &addrs[0]);
    assert_ne!(
        addrs[0], SIBLINGS_OWN_ADDRESS,
        "that address belongs to someone else"
    );
    assert_no_sibling_name_leaks("L11-inner-circuit.lvl", &src);

    let lvl = re_level::load(&path).expect("L11 parses");
    let note = assert_placard_binds("L11-inner-circuit.lvl", &lvl, "sector 0x");
    assert!(
        note.text.contains("sequence-lock armed"),
        "the address belongs on the sequence-lock log: {:?}",
        note.text
    );
    assert_line_uses_the_siblings_separators("L11-inner-circuit.lvl", &note.text);
}

/// The sibling assembles an address line with a SPACED MIDDLE DOT between fields
/// and an EM DASH before the detail clause, e.g.
/// `0x2E11 · ALMA REYES — kept 3 mint tins in her desk drawer`. The separators
/// are as much the bridge as the hex is.
fn assert_line_uses_the_siblings_separators(where_: &str, text: &str) {
    let line = text
        .lines()
        .find(|l| l.contains("sector 0x"))
        .unwrap_or_else(|| panic!("{where_}: the address line must exist"));
    assert!(
        line.contains(" · "),
        "{where_}: fields join with a spaced middle dot: {line:?}"
    );
    assert!(
        line.contains(" — "),
        "{where_}: an em dash introduces the detail clause: {line:?}"
    );
}

/// Two addresses, two levels, and they must not collide: a shared address would
/// assert the dead face and the current session's face are the same record,
/// which is exactly the claim the C-REF bridge leaves undecided.
#[test]
fn the_two_sector_addresses_are_distinct() {
    let (_, l8) = read_level("L8-cold-room.lvl");
    let (_, l11) = read_level("L11-inner-circuit.lvl");
    let a = sector_addresses(&l8);
    let b = sector_addresses(&l11);
    assert_eq!(a.len(), 1);
    assert_eq!(b.len(), 1);
    assert_ne!(a[0], b[0], "the two sector addresses must be distinct");
}

/// Task 5's `case C-REF · APHRODITE-class` line already matches the sibling's
/// separator convention and its frame is blessed — this pins it so a later
/// address edit cannot churn it.
#[test]
fn the_cref_line_keeps_its_blessed_separator() {
    let (_, src) = read_level("L8-cold-room.lvl");
    assert!(
        src.contains("case C-REF · APHRODITE-class"),
        "the C-REF line must keep its exact, blessed spelling"
    );
}
