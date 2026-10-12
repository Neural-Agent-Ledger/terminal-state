# tests/golden — Campaign Golden Framebuffer Corpus

A regression suite that pins what seven of the campaign levels look like. For each
curated level it renders one frame and compares two SHA-256 digests of that frame
against the values stored in `campaign_fb_hashes.txt`. Any change to glyph choice,
colour, shading, geometry or camera placement in a curated level fails the suite
until someone looks at it and decides.

The render and hash harness comes from the terminal3d engine's `re_golden` crate
(a dev-dependency of the game crate). The engine pins its own example and archive
levels the same way in its own repository.

## Running it

```sh
cargo test -p terminal_state --test golden_campaign
```

The curated list is in `crates/terminal_state/tests/golden_campaign.rs`.

## What is hashed

Each curated level is loaded with `re_level::load`, the camera is placed at the
level's spawn point, and one frame is rendered at time 0 with the default FOV
through the engine's `composite_dump_once` — the same entry point
`terminal-state --dump` uses — into a fixed 100 × 32 grid (3200 cells). Two
digests are taken over that grid:

| digest | computed over | answers |
|--------|---------------|---------|
| `shade` | row-major `{glyph u32, fg u32, bg u32}` — 12 bytes/cell, little-endian | did the **look** change? |
| `geom` | row-major `{depth, side u8, surface u32, material u16}` — 11 bytes/cell, little-endian; `depth` via `f32::to_bits` | did the **cast** change? |

Not hashed: ANSI escape bytes and the `style` channel (attributes and coloured
underline). Game-side overlays drawn at play time (HUD, notes, menus) are not
part of this frame either.

Because the digests are taken before any terminal output is produced, they are
the same on Windows and Linux, in any terminal emulator and at any real terminal
size, and the same whether the engine is a local path dependency or a released
version.

## Store format

`campaign_fb_hashes.txt` is plain text. Lines starting with `#` are comments.
Every other line is

```
<key> = <shade-sha256> <geom-sha256>
```

where `<key>` is the level file name without `.lvl`. Do not edit the store by
hand; it is written by the bless run described below.

## Reading a failure

The suite fails closed: every curated level must be positively matched. A failing
run lists each problem level under one of these categories.

| category | meaning | what to do |
|----------|---------|------------|
| `MISMATCH` | the level rendered differently | see the table below |
| `UNREADABLE` | the level file is missing or no longer parses | fix or restore the level; blessing cannot clear it |
| `NO-GOLDEN` | curated, but no hash is stored | bless only if this is a deliberate new anchor |
| `ORPHAN-GOLDEN` | a hash is stored for a level that is no longer curated | restore the entry, or retire the hash deliberately by blessing |

For a `MISMATCH`, which digest moved tells you what kind of change it was:

| moved | means |
|-------|-------|
| `shade` only | The cast is unchanged; a shader, palette or glyph change did this — expected after an engine render change or a colour/material-only level edit. |
| both | The geometry moved — the engine's raycaster, the map or the camera. Treat it as a regression until proven otherwise (an engine change, or a `.lvl` edit that altered the layout). |
| `geom` only | Geometry changed with no visible consequence (for example a material swap behind an identical glyph and colour). Unusual; read it before blessing. |

## Curated levels

| Key | File | Why pinned |
|-----|------|-----------|
| `L1-first-pulse` | `levels/L1-first-pulse.lvl` | Two-storey furnished opening level |
| `L5-soft-footstep` | `levels/L5-soft-footstep.lvl` | Story-arc level |
| `L8-cold-room` | `levels/L8-cold-room.lvl` | Snow-particle anchor |
| `L9-stereopsis` | `levels/L9-stereopsis.lvl` | Story-arc level |
| `L10-lying-door` | `levels/L10-lying-door.lvl` | Story-arc level |
| `L11-inner-circuit` | `levels/L11-inner-circuit.lvl` | Story-arc level |
| `L12-closing` | `levels/L12-closing.lvl` | Final campaign level |

`L2`–`L4`, `L6`, `L7` and the archive rooms `01`–`08` are not in this store. They
can be added as new anchors.

## Re-blessing after an intentional render change

Bless only when a change to the rendered output is intended and has been
reviewed: moving to an engine release that changes the look, or a lighting,
material or layout edit to a campaign level.

1. **Run the suite unchanged first** and confirm which levels moved and which
   digest moved for each. If you are on a new toolchain or a new engine version,
   confirm the suite is green before your own change; a bless on a red baseline
   silently absorbs whatever else changed.
2. Capture before/after frames for every moved level
   (`cargo run --release -- --dump levels/<level>.lvl --size 100x32`, add `--ansi`
   when colour matters) and include them, or a description of the difference, in
   the pull request.
3. Re-bless:
   ```sh
   TS_BLESS_GOLDEN=1 cargo test -p terminal_state --test golden_campaign
   ```
   This re-renders every curated level and rewrites the store. It still fails on
   `UNREADABLE` levels, so it cannot hide a missing file.
4. Commit the updated `campaign_fb_hashes.txt` in the same signed-off commit as
   the change, and add an entry to *History* below:

   ```
   ### YYYY-MM-DD — <levels that moved>, <shade | geom | both>
   Author: <name>. Change: <what changed, and why>.
   Reviewed: <PR link or reviewer>.
   ```

**Never bless to silence an unexpected failure.** If a level you did not intend to
change now hashes differently, that is a regression to investigate, not a hash to
update.

## History

- **2026-09-28** — This store was created when the game moved into its own
  repository. It holds the seven campaign levels that had been pinned alongside
  the engine's levels; every hash kept its previous value (a split, not a
  re-render).
