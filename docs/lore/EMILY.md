# EMILY — the protagonist, and the mirror

> ⟦**SUPERSEDED — 2026-07-30.**⟧ This document conflicted with the later
> NAL-integration canon, in which the assembled name is HERS — APHRODITE, `NCP-A-04` /
> legacy `aleks_v` (see the 2026-07-30 revision at the top of `STORY_BIBLE.md`).
> **The Emily identity is retired.** What survives, re-aimed: the
> mirror-as-manifestation mechanic (`MirrorClarity` ramps with memory progress) — the
> reflection now resolves toward the *contested* self, and the L9 read-back is a NAME,
> not a face (`PUZZLE_PROGRESSION_L1-L12.md` L9(d)). The earlier text is kept below
> unchanged, as a record of that direction.

> Decision (2026-06-18): the protagonist is a girl, **EMILY**. First-person —
> you ARE Emily. She is the strong recurring *concept* of the game; the **mirror** is
> her manifestation. This supersedes the placeholder "aleks_v" operator handle.

## Who Emily is

Emily is the digitized / dying consciousness inside TERMINAL STATE — the system is
reconstructing **her** life and death. Everything the puzzles recover (the
word→story: the colours, the garden, the senses in `WORD_STORY.md`) is **Emily's**
life remembered sense-by-sense. The F1 purgatory spine is Emily reassembling who she
was. The true-name spine (`STORY_BIBLE.md`) is Emily recovering **her own name**.

The name **EMILY** was chosen for the shape of the story it carries: an anthology
of one life, told in fragments, ending in recognition.

## The mirror = the manifestation

In first person you never see Emily — except in a **mirror**. That makes the mirror
the emotional core: the single moment of **self-recognition**. It's a recurring
motif (a mirror appears across levels), and it's mechanically bound to memory:

- The engine's mirror material has a **`clarity` 0..1** (driven live via the
  `MirrorClarity` resource — see the mirror render feature). Early, with little
  memory recovered, the reflection is **glitched / empty / a stranger** (clarity→0).
  As Emily recovers memory + name-shards, the reflection **resolves** toward her own
  face / silhouette (clarity→1). The game ramps `MirrorClarity` with progress and the
  renderer re-casts so the reflection visibly *resolves*.
- The **final mirror** is the choice: `exit 0` — Emily accepts who she was and passes
  on (the reflection holds, clear) — vs `while(true)` — denial — the reflection
  **shatters / glitches**, and the loop restarts at session 0001.

## The loop, reconciled

The prior-iteration "operator" who left the notes (Form A-7, the unsigned placards)
is a **prior Emily** — the iteration that chose `while(true)` and is leaving warnings
for the next her. The unsigned note now signs **"— E."** (retcon from "— A."). The
duplicate-operator/loop horror (`tail -f` logs someone else writes, a process from
your own address) is Emily haunted by her own previous attempts.

## How it threads the existing systems (no new mechanics required)

- Word→story = Emily's life; the assembled true name = **Emily's** name → unlocks the
  final mirror.
- Mirror `clarity` = the visible dial of memory recovery (engine feature already in).
- Two endings already exist (`exit 0` / `while(true)`) — the mirror is their staging.

See `STORY_BIBLE.md` and `WORD_STORY.md` in this directory.
