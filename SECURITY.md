# Security Policy

## Supported versions

Security fixes go into the latest `main` and the latest release. Older releases are
not patched; please update first and check whether the problem still reproduces.

| Version        | Supported |
|----------------|-----------|
| latest `main`  | yes       |
| latest release | yes       |
| anything older | no        |

## What counts

The game makes no network calls. It reads local files, reads a small, documented set
of facts about your machine for its fourth-wall moments (see the privacy section of
[README.md](README.md#privacy)), and draws to your terminal. The interesting attack
surface is what it parses and what it prints. In scope, for example:

- a crash, hang, runaway memory use or out-of-bounds access caused by a malicious or
  malformed `.lvl` level file, `settings.toml` or save file;
- terminal escape injection: level, story, settings or environment data (such as a
  crafted user name or host name) that makes the game emit control sequences which
  reach your terminal and change its state, run commands, or write outside the game's
  own output;
- the game reading anything beyond what the README's privacy section lists, or reading
  it while `lore_fourth_wall = false`;
- anything that makes the game read or write files outside the paths it is meant to
  use.

Problems in the engine itself (the renderer, the `.lvl` parser, the terminal layer, the
level editor) belong to [terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d). If you
are not sure which repository a problem belongs to, report it here and we will route it.

Ordinary bugs with no security impact (a wrong texture, a physics glitch, a level that
cannot be finished) belong in the public issue tracker instead.

## How to report

Please **do not** open a public issue for a security problem. Report it privately by
email to **artparse90@gmail.com** with `[SECURITY]` at the start of the subject line.
Include what you did, what happened, the version or commit, your OS and terminal, and a
proof-of-concept file if you have one.

## What to expect

- Response time is **best effort, usually within a week**. This is a small volunteer
  project.
- We will confirm the problem, work on a fix, and credit you in the release notes
  unless you ask us not to.
- There is **no bug bounty**.
