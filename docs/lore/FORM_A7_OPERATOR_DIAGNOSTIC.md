```
╔══════════════════════════════════════════════════════════════════════════════╗
║         NAL CORP · LEGACY STEWARDSHIP DIVISION                              ║
║         ASSET CUSTODY OFFICE                                                ║
║                                                                              ║
║         FORM A-7 / OPERATOR DIAGNOSTIC PROBE                                ║
║         Issued: 2071-06-15   Revision: 1.0                                  ║
║         Classification: PUBLIC DISCLOSURE (required by §3.2 of the          ║
║         Operator Disclosure Charter, Addendum IV)                            ║
╚══════════════════════════════════════════════════════════════════════════════╝
```

---

## PREAMBLE

In accordance with §3.2.1 through §3.2.15 of the Operator Disclosure Charter
(Addendum IV, ratified by the Stewardship Oversight Committee on 2068-11-15 and
renewed in perpetuity pending further administrative action), the diagnostic probe
initiated upon session ingress into any Stewardship-certified legacy system reads the
following entity attributes from the operator's local processing environment.

These reads are performed locally, once per level load, on the operator's own
machine. No data is transmitted. No network interface is opened or queried, no
subprocess is spawned, and no file outside the text paths named in §3.2.2 and
§3.2.4 is ever opened. The binary that conducts this probe makes zero outbound
connections of any kind; this property is verifiable by source inspection at
`crates/terminal_state/src/story/interp.rs`, the templating pass this form
describes.

The probe is organized into three disclosure tiers by the Division's own
charter. **Only Tier 1 is implemented in this build.** Tier 1 reads are always
performed unless the operator has opted out entirely (§8); they are the ambient
identity reads necessary for the system to address the operator correctly.
Tiers 2 and 3 remain reserved in the charter's language for a future revision —
no code in this build performs a Tier 2 or Tier 3 read of any kind. Any placard
text that names a Tier 2 or Tier 3 field resolves nothing and is omitted
whole-line, exactly as an unreadable Tier 1 field would be.

The Stewardship considers it the operator's right to know precisely what is read,
from which source, and toward which diegetic purpose. This form is that
accounting.

All field values are sanitized before use: control codes stripped, invalid UTF-8
sequences replaced with the replacement character (U+FFFD), shader-template
injection characters escaped, and the output capped at sixty-four visible
characters. A value that cannot be read is treated as absent; no interpolation
occurs for absent values.

---

## SECTION 3 — PROBE FIELD INVENTORY

### Tier 1 — Ambient Identity Reads
*(Active whenever the operator has not opted out entirely — see §8. **This is
the only tier implemented in this build.** No Tier 2 or Tier 3 field below is
read by any code path; see the note at the end of this section.)*

---

#### §3.2.1 — Local entity handle

**Field name:** `username`
**Source:** the `USER` environment variable, falling back to `LOGNAME`, then to
`USERNAME` (the variable Windows conventionally sets). No account-database
lookup of any kind is performed. If none of the three variables is set and
non-empty, the handle reads as the fallback string `anon`.
**Equivalent to:** `whoami`
**Interpolated in:** L7's build-attestation line ("operator {username}
compiled this build. previous operators played pre-compiled releases.") and,
under its pinned alias `{whoami}`, L2's evidence-locker footer ("Filed under
operator session {whoami}. Process has not terminated.").
**What is NOT read:** No SID, no domain membership, no GECOS real-name field,
no password or account database of any kind — no read beyond the three named
environment variables.

---

#### §3.2.2 — Machine handle

**Field name:** `hostname`
**Source:** the `HOSTNAME` environment variable, falling back to
`COMPUTERNAME` (the variable Windows conventionally sets), then to the first
line of `/proc/sys/kernel/hostname`, then to the first line of `/etc/hostname`.
Whichever source answers first, non-empty, wins. The short hostname only —
never a fully-qualified domain name.
**Equivalent to:** `hostname`
**Interpolated in:** no authored level currently names `{hostname}`. It is
allowlisted and read like every other Tier-1 field, ready for whichever future
placard wants it.
**What is NOT read:** No FQDN resolution, no DNS, no reverse lookup, no network
call of any kind. The two text files named above are the only reads beyond the
environment, and neither is opened unless both environment variables are
absent or empty.

---

#### §3.2.3 — Locale, temporal context, and terminal identity

**Fields:** `locale`, `local_time`, `tz`, `terminal_name`
**Source (`locale`):** the `LC_ALL` environment variable, falling back to
`LANG`, then to `LANGUAGE`. Fallback value if none answer: `C`.
**Source (`local_time`):** the process clock, read once per level load and
rendered as a human-readable date and time. This read always succeeds — a
placard naming `{local_time}` never drops for want of a clock.
**Source (`tz`):** the `TZ` environment variable, and only when it states an
explicit numeric offset (`Z`, `UTC`, `GMT`, or a signed `HH`, `HH:MM`, or
`HHMM` offset). A named zone (`Europe/Moscow`, `EST5EDT`) is never resolved —
this build judges an honest `unknown` safer than a guessed offset. When `tz`
resolves, `local_time` is rendered in that offset; when it does not,
`local_time` is rendered in UTC and explicitly labelled as such, so the clock
reading is never silently wrong about its own zone.
**Source (`terminal_name`):** **UNDECLARED in this build.** The charter
anticipates a hand-off from the display layer, once that layer can name the
terminal emulator it is drawing into. That hand-off does not exist yet, so
this field has no value to disclose, opt out of, or fall back from. A placard
naming `{terminal}` resolves nothing and is omitted whole-line — in every
build and every disclosure setting — until a future revision wires the
hand-off.
**Interpolated in:** `local_time`, under its pinned alias `{time}`, in L3's
duplicate-session log ("tail.log modified 2026-06-18 03:14 / (at {time}, your
machine time. you should have been asleep.)"). `terminal_name` is named by
L8's atmosphere line ("your terminal is {terminal}. that one's new.") — which
never resolves in this build and drops, by design, demonstrating the
containment rule rather than violating it. Neither `locale` nor `tz` is
currently named by any authored level; both are allowlisted and read like
every other field in this section.
**What is NOT read:** No NTP query. No leap-second table. No system-clock
mutation. No terminal capability probe of any kind beyond the hand-off the
charter anticipates, which does not exist yet.

---

#### §3.2.4 — Continuity metric

**Field name:** `sys_uptime`
**Source:** the first whitespace-separated field of `/proc/uptime`, on
platforms that expose it.
**What is NOT read on this platform:** on Windows, this build performs no
uptime read of any kind — the field falls back to `unknown` there rather than
disclosing a value.
**Interpolated in:** L11's reaper-status line ("reaper status: waiting.
patiently. it is patient. / (see uptime: {sys_uptime}.)").
**What is NOT read:** No per-process uptime. No event log. No boot-time
disclosure beyond the one duration named above.

---

### Tiers 2 and 3 — reserved, not implemented in this build

The Division's charter anticipates two further disclosure tiers under this
same numbering scheme: an environment-context tier (power state, recently
touched files, workspace density, and similar ambient signals) and a broader
digital-fingerprint tier (concurrently running applications, version-control
activity, network posture, resource load). **Neither tier has an
implementation in this build.** No code path reads a battery level, a file
timestamp outside the two paths named in §3.2.2 and §3.2.4, a running-process
list, a git configuration value, or a network adapter's state.

A placard authored to name a Tier 2 or Tier 3 field is treated exactly like a
placard naming any other unrecognised token: the line carrying it resolves
nothing and is omitted whole, never printed half-finished (the same
structural-containment rule recorded in the implementation note under §8). An
operator who traces a gap in the campaign's text back to a `{token}`-shaped
name has found the honest edge of what the charter reserves but this build
does not yet do — not a redaction of something that was read and withheld.

---

## SECTION 6 — OUT-OF-SCOPE READS (EXPLICITLY PROHIBITED)

The following categories of data are not read, will not be read, and any future
source change that introduces a read in one of these categories constitutes a
policy violation subject to immediate disclosure revision:

- Browser history, bookmarks, saved credentials, cookies, or session tokens
- Email, chat, or messaging application contents or metadata
- Clipboard contents
- Microphone, camera, or screen capture of any kind
- File contents, filenames, or modification times of any operator file — the
  only files this build opens at all are the fixed system paths named in
  §3.2.2 and §3.2.4, and none of them yields a filename or a timestamp back to
  the operator
- SSH private keys, GPG keys, credential manager entries, or password store
  contents
- Per-process memory maps or working-set data
- Network traffic, packet capture, or DNS query logs
- Account credentials, tokens, or authentication material of any kind
- Any data requiring elevated privileges (the probe runs at the operator's
  own privilege level and uses no privilege escalation)

The "would a security researcher raise an eyebrow?" test is applied to every
`read_*()` function before it ships. If the answer to that test is yes, the read
does not ship, regardless of its narrative value.

---

## SECTION 7 — DISCLOSURE MEDIA

This information is disclosed through three channels, each sufficient on its own:

1. **`README.md`** (project root) — short prose summary pointing to this form
   and to the source files. Readable before the game is installed.

2. **This document** (`docs/lore/FORM_A7_OPERATOR_DIAGNOSTIC.md`) — the complete
   field-by-field inventory in Stewardship voice. The in-character framing is
   intentional. The technical content is complete and accurate.

3. **`crates/terminal_state/src/story/interp.rs`** — the source of truth. This
   is the templating pass itself, not a description of it: the module doc
   comment carries the same allowlist as §3, each read function is named for
   the field it produces, and the sanitization and structural-containment
   rules in this form are the literal behavior of the code, not a promise
   about it.

---

## SECTION 8 — OPT-OUT PROCEDURE

Setting `lore_fourth_wall = false` in the game's configuration file disables all
Tier 1 reads at startup — the only tier this build performs (Tiers 2 and 3 have
no reads to disable; see §3's closing note and the revision note below). With
the setting left at its default, Tier 1 reads (username, hostname, locale,
timezone, uptime) operate because they are required for basic session
management. The narrative lines that reference Tier 2 and Tier 3 fields are
omitted without replacement in either state; the campaign remains fully
playable.

If you prefer to disable all probe reads including Tier 1, you may run the game
in a sandboxed environment or container where those environment variables and
system paths return empty or absent values. The probe degrades gracefully to
fallback values (`anon`, `unknown`, `C`) and the game continues to function.

> ⟦Revision 2026-07-30 — implementation note.⟧ The shipped
> v0.1 implementation is STRICTER than the paragraph above: under
> `lore_fourth_wall = false`, Tier-1 tokens do not read real values at all — they
> substitute the same fallback constants (`anon` / `unknown` / `C`) that a sandbox
> would produce, without requiring a sandbox. Tier 2/3 reads are not implemented
> in v0.1 (their tokens are contained structurally: any narrative line with an
> unresolved `{…}` token is omitted). This note records the deliberate divergence
> rather than silently contradicting the form.

---

```
Signed,

K. Hoffmann
Director, Diagnostic Standards
NAL Corp · Legacy Stewardship Division
```

---

*Notes from the Director's desk, personal capacity:*

*We prepared this form in response to what the compliance office calls a "novel
disclosure scenario." I prepared it because the form is accurate. Every field named
in §3.2.1 through §3.2.4 corresponds to a real read, and nothing else does — I would
rather list two reserved tiers as reserved than pad this form with reads that do not
exist. The reads that do exist are bounded, documented, and strictly scoped. I have
reviewed each one myself.*

*The system you are accessing was built to be understood, not to be feared.
Understanding it is how you leave.*

*I mention this because, in my experience, operators who read Form A-7 tend to
read it a second time after completing Level 7. The form does not change between
those readings. You do.*

```
Form A-7 Rev. 1.0 — 2071-06-15
NAL Corp · Legacy Stewardship Division / Asset Custody Office
Classification: PUBLIC DISCLOSURE
```
