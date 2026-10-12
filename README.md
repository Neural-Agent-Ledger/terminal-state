# TERMINAL STATE

A first-person 3D puzzle game that runs entirely in your terminal. Every frame is
ray-cast into text glyphs and 24-bit colour, and every sound is synthesized while you
play. There is no GPU, no window and no audio file.

TERMINAL STATE is written in Rust on **Bevy 0.19** (headless ECS) and is built on
**[terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d)**, an open-source terminal
3D ASCII engine and level editor that lives in its own repository. This repository
holds the game: the story shell, the twelve-level campaign, the archive rooms, the
lore and the splash art.

## Play it in five minutes

You need Rust from [rustup](https://rustup.rs) (on Windows also the MSVC build
tools; on Linux `pkg-config libasound2-dev libudev-dev`). Then, in Windows Terminal
or any modern Linux terminal of at least 120×40:

```sh
git clone https://github.com/Neural-Agent-Ledger/terminal-state
cd terminal-state
cargo run --release
```

The first build downloads the engine and compiles Bevy, so it takes a few minutes.
Move with `WASD`, look with the mouse or the arrow keys, interact with `E`, and press
`Esc` for the menu, settings and the in-game guide. [How to play](#how-to-play) lists
every control.

To check that the game works on your machine without opening it:

```sh
cargo run --release -- --dump levels/L1-first-pulse.lvl --ansi   # print one frame
cargo test --workspace                                          # all tests
```

The level editor is part of the engine repository,
[terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d).

---

## What it is

You are **Aleks Volkov**, a junior interpretability technician sent by the Ministry
of Computational Heritage to run a final diagnostic on **Model 07**. Model 07 is a
small vision classifier built at the Kharkiv-Sapporo Joint Lab in the early 2020s. It
was abandoned nineteen years ago and is still running on a single dust-caked rack.

You walk through its latent space. You read the word streams flowing through its
neural pipes. You flip switches at the junctions to set binary neurons, and you open
each door by driving the network's output to the target.

The deeper you go, the less clear it is who is debugging whom.

```
████████                     ···                    ···       ····
┓████████┃┃ ┃╱              ··········          ···                ····
+━◣▖█   █┃┃ ┃╱┃           ···                ····                     ······
▌▌▌▌◣▖██ ┃┃ ┃┃╱           ··             ··           ···   ···
━━▚▄▚━▄◣█┃┃█┃┃┃████·██████████████████████████████████████████████████████████
  ─││││~▄┃┃█┃┃┃███████████████████████████████████████████████████████████████
○○○    │─┃┃◣┃┃┃██████┃██████████████████████████████████████████████████*██··▗
  ────│~─┃┃┃┃┃┃━━◣▄▄▄┃·▄▄▄▄▄▄▄▖██████████┃┃█████████████┃██▗▄▄▄▄▄━━━━━━━━━━━━▐
 ○○○○○○~~┃┃.┃┃┃+@+@··┃+@+@++@@+━━━━━━━━━━┃┃━━━━━━━━━━━━━┃━━++++@+..++.++...+..
         ┃┃.┃┃+++*+·:┃+++++++++++*━━━━+━━┃━━━┃@@@@@*@:.@┃@**..@@.++..·+.+.+*+.
       ┼─┃┃.┃┃┃+@**+@┃┃┃┃┃━┃━━━━┃━┃━@@@··┃┃@@┃@@@@@@··@+┃·.++++++·+*++·*·++++.
───    ─ ┃┃~┃┃┃━━━━━━━┃┃┃┃+┃@+.*┃+┃@━━━━━┃┃━━┃━━━━━━┃━@+┃*@@+··@@+@+++..*++..+
││││   ─~┃┃┃┃┃┃██·█···█┃━━━┃+@++·━━│━━━━━┃@.·┃*  .  ┃@━━━━━━━━━━━━━━━━━+*·.+++
       ─ ┃┃┛█████·██·.██·┃┃┃━++·┃┃┃━━━│━━┃━━━┃┃     ┃*@━━━+@++.+.@+·+++━━━━━━.
   ││││─━┛█████·████·█·████┃┃━++┃·─·│──── ┃*:┏━━━━━━┃━━━━·━━━  ·@·.+··+...+..+
━━━━━━━┛█████ █████··*███·████━┫┃━━━▖━━━━━━━━ · ·  ·┃M  ▐━━━━━━━━▄━━━━    +...
████████████  ███·█··*█████████·┃┃▌▌▌▌▌▌▌▌┃▌·IMIMSS┏┃━━━━━━━━▄▄▄ ▄·      .··.·
```

*One frame of campaign level 3, "Full Spectrum", produced by
`cargo run --release -- --dump levels/L3-full-spectrum.lvl --size 78x20`. The dump
prints glyphs only. In a real terminal every cell also carries a truecolor foreground
and background.*

**What is in the box**

- A twelve-level campaign, `L1` to `L12`, in four movements. Each level unlocks the
  next. The order lives in [`levels/campaign.toml`](levels/campaign.toml).
- An **archive** of eight earlier rooms, `01` to `08`, playable from the level
  browser.
- An in-game **guide** reachable from the main menu and from the pause menu, and a
  **first-steps checklist** that ticks itself off as you play.
- A proximity **interaction prompt** that tells you what you are facing and which key
  acts on it.
- An optional boot-console intro, a splash with an eye that follows your mouse,
  animated menu backdrops, and mouse support in every menu.
- Saved progress and settings: quality presets, FOV, draw distance, FPS cap,
  post-FX, colour mode, brightness, colour steps and audio.

---

## How to play

Each room is a small binary neural network built into the architecture.

- **Nodes** are the neurons. A node fires once enough of its wired inputs are on.
- **Pipes** carry the signal from one node to the next. Every pipe has a **switch**
  somewhere along its run.
- Face a switch (the crosshair brightens from a faint `+` to a bright diamond) and
  press `E` to turn its pipe on or off.
- The room is solved once every required node has fired. Watch the `[lit/total]`
  readout on the objective line. Some rooms cap how many pipes may be on at once, and
  the HUD then shows `power N/M`.
- Once the room is solved, walk to the door. It opens.

The in-game **GUIDE** (main menu, or `Esc` → GUIDE while playing) explains the goal,
the controls, nodes and switches, the forward pass and how to read the world.

### Controls

These are the bindings of the terminal3d input layer, which the game uses unchanged.

| Action | Keys |
|---|---|
| Move forward / back | `W` / `S` |
| Strafe left / right | `A` / `D` |
| Turn left / right | `←` / `→` (or the mouse) |
| Look up / down | `↑` / `↓` (or the mouse) |
| Jump | `Space` |
| Sprint | hold `Shift` |
| Interact (flip the switch you face) | `E` or `Enter` |
| Pause / back | `Esc` or `Q` |
| Toggle mouse-look | `M` |
| Narrow / widen the field of view | `[` / `]` |
| Zoom ("binoculars") | `Z` |
| Cycle the post-FX preset | `G` |
| Performance overlay | `F3` |

Menus use the arrow keys, `Enter` and `Esc`, and they also accept the mouse. A gamepad
works in the menus and in play.

---

## Getting the game

**The source is free and it stays free.** Everything you need to build and play the
full game, every level and every line of story, is in this repository and in the
terminal3d repository, under the licences below.

If you would rather not build it yourself, a **prebuilt, packaged build** for Windows
and Linux will be sold on itch.io, with a **free demo** of the first levels (the page
is not live yet; it will be linked here). Buying it is a way to support the work. It does not unlock anything that the
source does not already give you.

---

## Building from source

You need a **stable Rust toolchain** from [rustup](https://rustup.rs).
`rust-toolchain.toml` selects the channel and components for you. The full setup,
per platform, is in [INSTALL.md](INSTALL.md).

- **Windows 10+:** install the MSVC build tools (the "Desktop development with C++"
  workload). Nothing else is needed. The binary links the C runtime statically, so
  players need no redistributable.
- **Linux:** install the audio and gamepad headers, for example on Debian or Ubuntu:

  ```sh
  sudo apt install pkg-config libasound2-dev libudev-dev
  ```

Then clone and run:

```sh
git clone https://github.com/Neural-Agent-Ledger/terminal-state
cd terminal-state
cargo run --release                                  # play
cargo run --release -- levels/L1-first-pulse.lvl     # start on a given level
cargo run --release -- --help                        # every option
```

You do **not** need to clone the engine for this. `Cargo.toml` declares the terminal3d
crates as git dependencies on a tagged engine release (the block between
`# BEGIN engine-deps` and `# END engine-deps`), and cargo fetches them on the first
build. The first build compiles Bevy and takes a while.

If you want to change the engine and the game together, clone
[terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d) next to this repository and
point cargo at your local copy. [docs/DEVELOPING_WITH_LOCAL_ENGINE.md](docs/DEVELOPING_WITH_LOCAL_ENGINE.md)
shows how, without editing any tracked file.

Render one frame without taking over the terminal. This is handy for bug reports:

```sh
cargo run --release -- --dump levels/L1-first-pulse.lvl          # glyphs only
cargo run --release -- --dump levels/L1-first-pulse.lvl --ansi   # with truecolor
```

Build the lean shipping binary (stripped, fat LTO):

```sh
cargo build --profile dist -p terminal_state
```

### Terminal requirements

- **Windows Terminal** on Windows 10+, or any modern Linux terminal (xterm, kitty,
  WezTerm, GNOME Terminal and so on). The legacy Windows console host is not a target.
- **Truecolor** is preferred. On Linux set `COLORTERM=truecolor` if your terminal does
  not set it already. Without truecolor the game falls back to 256 colours, and
  `--color=256` forces that mode.
- **UTF-8** and a monospace font with good Unicode coverage.
- A grid of at least 80×24. 120×40 or larger looks best.

---

## Privacy

The game makes **no network calls**. A test (`no_net_in_game_build`) fails the test suite
if any networking crate becomes reachable from the game.

For its fourth-wall moments, the story can address you by your real handle: a few
in-level notes, the boot console and some story screens read a small, fixed set of
facts from your own machine. This is exactly what is read, and from where:

| Shown as | Read from |
|---|---|
| user name | the environment variables `USER`, then `LOGNAME`, then `USERNAME` |
| host name | `HOSTNAME`, then `COMPUTERNAME`; on Linux also `/proc/sys/kernel/hostname`, then `/etc/hostname`. No DNS lookup |
| local time | the system clock, with the offset from `TZ` if set |
| locale | `LC_ALL`, then `LANG`, then `LANGUAGE` |
| uptime | `/proc/uptime` on Linux. Not read on Windows |

Nothing else is read. The terminal emulator's name is not read. Each value is cleaned
before use (control characters removed, cut to 64 characters) and is only drawn as
text on your screen. It is **not** written to your save or settings files and it
**never leaves your machine**.

Separately from the list above, the boot console shows your terminal size, colour mode
and your CPU's logical core count. It shows these whatever the setting below says.

**Turning it off.** Set `lore_fourth_wall = false` in `settings.toml`. With it off, the
game performs **none** of the reads in the table and shows `anon`, `unknown` and `C`
in their place. There is no menu row and no environment variable for this. The file
is the one switch. `settings.toml` sits next to your save:

- Windows: `%APPDATA%\TerminalState\settings.toml`
- Linux: `$XDG_CONFIG_HOME/TerminalState/settings.toml`, or
  `~/.config/TerminalState/settings.toml`
- If `TS_SAVE_DIR` is set: `$TS_SAVE_DIR/settings.toml`

The game creates the file the first time you change a setting. To opt out before your
first launch, create the file yourself. The single line `lore_fourth_wall = false` is
a valid settings file, because every other key falls back to its default.

---

## Contributing

**Contributions are welcome, and we keep them upstream.** Pull requests are reviewed,
merged here and maintained here, so you never have to carry a fork. Bug fixes, levels,
story polish, terminal-compatibility reports, docs and tests are all in scope.
Engine, renderer and editor changes belong in
[terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d).

Start with [CONTRIBUTING.md](CONTRIBUTING.md). Small, self-contained tasks are
listed under **Good first issues** in [ROADMAP.md](ROADMAP.md). We use the
[Developer Certificate of Origin](DCO), not a CLA: sign off your commits with
`git commit -s`. Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).
Security problems go by email, as described in [SECURITY.md](SECURITY.md).

Unless you explicitly state otherwise, any contribution you intentionally submit is
licensed as described below (code under `GPL-3.0-or-later`, content under
CC BY-SA 4.0), without any additional terms or conditions.

---

## Licence

- **Code** is licensed under the **GNU General Public License, version 3 or later**
  (`GPL-3.0-or-later`). See [LICENSE](LICENSE). This covers everything under
  `crates/`, except the copies of engine files listed in `PROVENANCE.toml`, which keep
  the engine's licence.

  > TERMINAL STATE is free software: you can redistribute it and/or modify it under
  > the terms of the GNU General Public License as published by the Free Software
  > Foundation, either version 3 of the License, or (at your option) any later
  > version. This program is distributed in the hope that it will be useful, but
  > WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
  > FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
  > details. You should have received a copy of the GNU General Public License along
  > with this program. If not, see <https://www.gnu.org/licenses/>.
- **Content** is licensed under **CC BY-SA 4.0**. See
  [LICENSE-CONTENT](LICENSE-CONTENT). This covers the campaign and archive levels, the
  campaign manifest, the story text, `docs/lore/` and the splash art. Attribute it as
  "TERMINAL STATE contributors".
- [PROVENANCE.toml](PROVENANCE.toml) records the origin and licence of every file
  under `levels/`, `assets/` and `docs/lore/`.
- **The engine** this game builds on, terminal3d, is licensed MIT OR Apache-2.0, at your
  option. Those permissive terms are compatible with the GPL, so the combined game
  binary is distributed under GPL-3.0-or-later. **Third-party crates** keep their own
  licences, as declared in their own manifests (`cargo metadata` lists them).
- **Binaries.** Anyone distributing a build of the game, including the packaged builds
  on itch.io, ships `LICENSE` and `LICENSE-CONTENT` with it and offers the
  corresponding source: this repository and terminal3d, at the tags the build was
  made from.

Contact: artparse90@gmail.com

## Credits

The game is built on [terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d), which in
turn builds on [Bevy](https://bevyengine.org) (headless ECS),
[ratatui](https://ratatui.rs) and [crossterm](https://github.com/crossterm-rs/crossterm),
[kira](https://github.com/tesselode/kira), `glam`, `noise`, `dot_vox` and `gilrs`.

`docs/lore/` contains the story bible and the puzzle solutions. It is full of
spoilers, so play first.
