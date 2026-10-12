# Level design

How to author and test a level for TERMINAL STATE. Levels are plain-text `.lvl`
files. The format, the level editor and the linter all belong to the engine,
[terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d), so you work with both
repositories side by side:

```
ts-dev/
├── terminal3d/        the editor (terminal-state-editor) and the linter (lvlcheck)
└── terminal-state/    this repository: levels/, the game
```

The full `.lvl` reference, the material, lighting, sky, emitter and sound-field docs,
and the editor key map are in the terminal3d repository. This page is the short loop.

> **Spoilers.** The campaign's puzzle designs and solutions are in `docs/lore/`. You do
> not need them to build a new room.

## The puzzle in one paragraph

A room is a small binary neural network. **Nodes** fire once the sum of their active
incoming pipes reaches their threshold. **Pipes** run from node to node, and each one
carries a **switch** the player flips with `E`. Some nodes are inputs, fixed by the
room. The room is solved when every required node fires, and then the door opens. A
room may cap how many pipes can be on at once (a power budget). Good rooms reveal
something when they are solved: the solution is the story beat.

## 1. Open or create the level in the editor

From the engine clone:

```sh
cd ts-dev/terminal3d
cargo run --release -p ts_editor -- ../terminal-state/levels/my-room.lvl
```

- `W` opens the new-level wizard.
- `?` shows the full key map, and `:` opens the command palette.
- The right-hand pane is the game's own renderer, so what you see is what plays.
- `s` saves. Every save runs `lvlcheck`, and the status line shows its verdict. `V`
  opens the full report from the last save.
- `F5` play-tests the level. The editor looks for a `terminal-state` binary next to
  itself. If it does not find one, set `TS_PLAY_CMD` to the path of your game binary,
  for example `ts-dev/terminal-state/target/release/terminal-state`.

## 2. Lint it

```sh
cd ts-dev/terminal3d
cargo run --release -p re_lvlcheck -- ../terminal-state/levels/my-room.lvl
```

`lvlcheck` checks the elements, solvability, reachability, whether the player can get
stuck, and sound fields. It exits non-zero on any FAIL. **A level must pass before it is
submitted.** Warnings are allowed, but say in the PR why each one is fine.

## 3. Look at it

From this repository:

```sh
cd ts-dev/terminal-state
cargo run --release -- --dump levels/my-room.lvl --size 100x32           # glyphs
cargo run --release -- --dump levels/my-room.lvl --size 100x32 --ansi    # with colour
```

`--dump` renders the spawn view and exits. It also reports notes it could not place on
a wall.

## 4. Play it

```sh
cargo run --release -- levels/my-room.lvl
```

Solve it at least once in a real terminal. Check that the switches are reachable,
that the interaction prompt appears where you expect, that the door opens, and that
the room reads at 80×24 as well as at a large size.

## 5. Campaign levels

The twelve campaign levels are listed, in order, in
[levels/campaign.toml](../levels/campaign.toml), with their movement (`P1` to `P4`)
and fog load. A campaign level also:

- sets `[meta] prop_style` to match its movement (a test checks this);
- declares a power budget (a test checks this);
- must stay solvable and walkable (`campaign_solve_through`, `campaign_walkable`);
- may be pinned by the golden test. If you change one of the seven pinned levels, the
  golden test fails until the change is reviewed and re-blessed, see
  [CONTRIBUTING.md](../CONTRIBUTING.md#the-campaign-golden-test).

Run the campaign tests after any change to a campaign level:

```sh
export TS_AUDIO_SILENT=1 TS_SAVE_DIR="$(mktemp -d)"
cargo test -p terminal_state --no-fail-fast
```

Changing the campaign order is a design decision. Open an issue first.

## 6. Submitting

Follow [CONTRIBUTING.md](../CONTRIBUTING.md#submitting-a-level): your own work, CC BY-SA
4.0, a passing `lvlcheck` summary, a `--dump` frame, and an entry in
[PROVENANCE.toml](../PROVENANCE.toml).
