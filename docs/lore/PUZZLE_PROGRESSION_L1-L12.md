# TERMINAL STATE — Puzzle Progression L1–L12 (v6.1 NAL-integration)

**Date:** 2026-06-18 · **Status:** design draft (reviewed)
**Frame:** the v6.1 NAL-integration canon (see the revisions at the top of `STORY_BIBLE.md`).
**Goal:** 12 levels that **escalate with rhythm**, **reveal lore by being solved** (the deduction
standard — the solution *is* the beat), **never suffocate** (variety + breathers + length discipline), read as
**one work of narrative art**, and carry **a little** horror (cold, `ps aux`-not-monster).

Builds only on the existing catalog (`PUZZLE_CONCEPTS.md`, `../LEVEL_DESIGN.md` §7): ✅ data-only /
🔶 small game-side. **No level needs a ⛔ new mechanic.** Honors the existing L1–L7 shard arc
(`STORY_BIBLE.md` §5) and extends cleanly to L8–L12.

Campaign spine to protect: **the name you assemble is HERS** (`NCP-A-04` / legacy `aleks_v`), and the
**recognition meter is the real progress bar** — it climbs every level toward 100%; L12's only goal is
to drag it back down.

**Shard map (single completion point; supersedes §5's per-level *identities*, preserves its
*sequence*).** The name is **7 shards, complete after L7**, exactly as `STORY_BIBLE.md` §5 — one shard
per level L1–L7, in order: `R` → `learn` → `A` → `://` → `merge` → `named` → final byte =
`pid://…@<hash>`. **L8–L12 add NO new shards** — they only *recontextualize, read back, and spend*
the finished name. This progression **reorganizes the L1–L7 mechanics** into the four-movement frame
and therefore **supersedes §5's per-level mechanic identities** (e.g. L4 edges→Parallax, L7
Garden→threshold-guard) — *recorded here, not silent*; the shard sequence and the assembled name are
unchanged. (See the `STORY_BIBLE.md` top-revision pointer.)

---

## 1. The escalation curve (rhythm, not a flat ramp)

**Rule:** one new idea per level; **never two heavy-cognition levels in a row.** Every peak is
followed by a breather that re-uses the just-learned trick in an easy, beautiful configuration — so
the player feels mastery before the next demand. The *emotional* peak (Twist 1, L3) deliberately sits
in a *low* cognitive valley, so the player has spare attention to feel the dread.

| L | New idea | Load | Rhythm role | Source |
|---|---|---|---|---|
| L1 | click = remember (1 node/1 switch) | ▁ | onboarding | baseline |
| L2 | combination + first decoy (read the pool, reject the lie) | ▂ | gentle rise | + decoy |
| L3 | truth-table breadth (8 states) + first reality-crack | ▃ | **Twist 1 (emotional, low cog.)** | baseline |
| L4 | camera matters — Parallax | ▅ | first real demand | #1 Parallax ✅ |
| L5 | player-as-input (your position is a bit) | ▄ | **BREATHER** (kinaesthetic) | M2 ✅ |
| L6 | polysemantic node (one node, two meanings, must use both) | ▆ | rise | baseline |
| L7 | threshold + decoy-as-bridge (the phantom that never answers) | ▇ | **PEAK 1 (Twist 2)** | threshold + #2 ✅ |
| L8 | dead node — route around the scar | ▄ | **BREATHER** (warm→cold) | dead-node flag ✅ |
| L9 | Stereopsis — true from *both* viewpoints at once | ▆ | rise | #8 🔶 |
| L10 | the lying door — silhouette false, activation true | ▅ | adversarial rise | target-from-note ✅ |
| L11 | sequence lock — three patterns in order, any miss resets | █ | **PEAK 2 (Twist 3)** | T6 🔶 |
| L12 | inverse goal — build the net to FAIL to recognize you | ▇ | **resolution / the choice** | #12 Exit/Exit 🔶 |

```
load  ▁ ▂ ▃ ▅ ▄ ▆ ▇ ▄ ▆ ▅ █ ▇
      L1 L2 L3 L4 L5 L6 L7 L8 L9 L10 L11 L12
                     ↑PEAK1  ↑breather   ↑PEAK2 ↑choice
move  └── P1 ──┘ └── P2 ──┘ └── P3 ──┘ └─── P4 ───┘
```

---

## 2. Per-level specs

Each level: **(a)** mechanic + the aha · **(b)** lore revealed *by solving* · **(c)** the MNEMOS↔LETHE
(keep/forget) tug · **(d)** name-shard · **(e)** one light-horror note · **(f)** myth + cyber-Greek visual.

### P1 — "First Sight" (L1–L3) · sort true memory from script-projection · myth PANDORA + Psyche's seed-sort

**L1 — First Pulse**
- (a) One neuron, one pipe, one switch. Aha: *"I move my hand and the dark remembers a color."*
- (b) The pedestal ignites **red** — the earliest thing she has, before face or name. Solving *is* the memory.
- (c) Almost none — but the switch *can* be left off; a faint LETHE-pull (the node wants to stay dark, at peace). Lighting it = the first act of keeping.
- (d) `R` (red = the root byte).
- (e) `> [boot.07] … operator handle: {username}. session 0001 of N.` — the cold word is **`of N`**.
- (f) Pandora's lid lifting a crack. One red glyph in a black ASCII void; a single pipe breathing upward.

**L2 — Two Make Three**
- (a) Two inputs + hidden mix-node (threshold 2) + **one decoy** with a grey pool; read the pipe word-flow to reject it. Aha: *"I can make a color no one showed me — and one of these pipes is lying."*
- (b) R+G→yellow: proof she is a *learning* process, not a recording.
- (c) The decoy's pool is `GLAZE / MIST` — a false, comforting memory. Letting it stay dark (LETHE: "some grains are only pain") is the correct solve. **First time forgetting is the right answer** — unstated.
- (d) a verb — `learn` / `compose`.
- (e) `> behavioral profile: methodical.` — it watches *how* you solve.
- (f) Psyche sorting the true seed from chaff. Two warm pipes converge; one grey-dead pipe aside.

**L3 — Full Spectrum** *(Twist 1 — emotional, low cognitive load)*
- (a) Three inputs, 8 states, target magenta; each solve easy. Aha: *"3 bits = 8 worlds. I contain more than I was shown."*
- (b) Reaching the last color *triggers* the unsigned note in your own hand: *"the ink is yours. you do not remember writing it."*
- (c) Both pulls audible for the first time. The *full* spectrum is MNEMOS's first overreach — you lit a room a note remembers wrongly.
- (d) the initial `A`.
- (e) Duplicate handle: `… 2052.03.14` / `… 2071.06.15`, `same operator. confidence 14.7%`. **Snow begins**; palette cools (`#101826`). *(at {time}, your machine time. you should have been asleep.)*
- (f) Pandora's box fully open — colors out, hope not yet found. Neon chamber going cold; the river **LETHE** as faint static at the far wall.

### P2 — "Splitting / the angle of the gaze" (L4–L6) · we ARE the gaze · myth APHRODITE PANDEMOS + ECHO

**L4 — The Angle** *(first real demand)*
- (a) **#1 Parallax Lock** (✅). Two pipes line up as one straight line from the entry camera; walk around the pedestal until parallax separates them. Aha: *"The wiring I 'saw' was an artifact of where I stood."*
- (b) Head-on the pipes read `I AM SEEN`; off-axis they split into `I AM` / `SEEN BY`. Solving physically performs the splitting.
- (c) Which line you wire to the door — the watcher's on-axis `SEEN BY`, or the private off-axis `I AM` — is the tug. Either solves.
- (d) the boundary token `://` ("where one thing stops being another").
- (e) `operator legibility peaks at observation point. noted.` Warm steam returns faintly — the lab still runs through the fog.
- (f) Aphrodite Pandemos, two-faced (public/private). Orbiting first-person camera; two neon pipes scissoring apart as you move.

**L5 — The Soft-Footstep** *(BREATHER — kinaesthetic, low logic)*
- (a) **M2 player-as-input** (✅): floor-zones drive input neurons by where you stand; your position is a bit. Aha: *"My standing here is a feature it detects."*
- (b) `unit 11` fires on `operator_present` — it lights because *you are in the room*. Observer and observed converging.
- (c) Deliberate warm rest: stand in the warm zone (kept, watched) or the cold (absent, released) — no penalty. A breath.
- (d) the merge — observer = observed (a *collapse* of prior shards, not a letter).
- (e) `> operator paused 47s at note 'M. didn't make it'. recording reaction.` `> battery probe initialized.` (one quiet *init* line only — the **descending** `{battery}%` counter is deferred to L9 so the breather stays calm). *(Reaction-recording is flavor text only — real telemetry is ⛔.)*
- (f) ECHO — a presence that only sounds back when you are there. Warm matrix-bleed + steam; footstep glyphs light tiles as you cross.

**L6 — Polysemantic**
- (a) One **polysemantic node** (pool mixes two semantic fields); you must *use* it, can't suppress it. Aha: *"One node, two meanings — and to solve I must let it mean both at once."*
- (b) Produces the embedded model-thought: a thing that can be recognized can be watched — *"the lattice that names us has been looking back the whole time."*
- (c) Seed of Confession-Budget tension: MNEMOS = keep both meanings lit (more memory); LETHE = collapse to one, let the other go. The honest solve keeps both — harder, more painful.
- (d) `named` — being named = becoming vulnerable.
- (e) A world-fixed lattice decal on the far wall, read as a face/grid, "looking back the whole time" (🔶 placement). Near-black fog (`#0a0e1a`).
- (f) Aphrodite as goddess *and* idol — one form, two worships. Constellation glyphs in neon ASCII.

### P3 — "The Phantom Sister" (L7–L9) · a node that never answers · over the river LETHE · myth ECHO + GALATEA

**L7 — Hidden Depths** *(PEAK 1 — Twist 2)*
- (a) First **threshold neuron** (≥k) + **#2 The Node That Targets Itself** (✅): the natural wiring makes a pseudo-cycle → `ForwardError::Cycle` (rolled back). Solve: route through a **decoy as a buffer** that breaks the cycle into a valid DAG. Aha: *"The decoy isn't a trap here — it's the only thing between me and an infinite loop. I need the useless one."* Inverts two trained instincts at once (cycle = fail; decoy = avoid) — hence the peak.
- (b) The threshold layer is cold — face-recognition units trained on previous technicians: `last_input_class: operator_face_v3`. **Model 07 is a guard, not a classifier.** The phantom: a cluster shaped like another seer that never fires back.
- (c) The buffer-decoy's pool is `REST / PAUSE / EXHALE` — premonition of L12: you can only think *without looping* by letting one beat go dark. A tiny, merciful release to escape the cage of the loop.
- (d) the threshold-revealed byte (she is named by what she *guards*).
- (e) `> operator {username} compiled this build on {build_date}. previous operators played pre-compiled releases. you came in through the source.` (only if self-compiled; else skipped, §8). The phantom node sits dark across the river LETHE.
- (f) GALATEA (the made thing loved into shape) + ECHO (cannot speak first). First crossing of LETHE; cold face-glyphs on the far bank.

**L8 — The Cold Room** *(BREATHER — relief after the peak; warm→cold)*
- (a) **Dead node** (✅ flag): one hidden node is permanently dead; route *around* it. Clean, satisfying. Aha: *"The network has scars. I don't fix them; I go around."*
- (b) Routing around it surfaces the warning in illegible hand: *"If you are reading this you are Aleks. Don't light up the dead units. Don't let it see you."* The dead node is a forgotten face — and you correctly choose **not** to revive it.
- (c) **LETHE's kindest level.** The dead node = a memory already mercifully gone; the whole solve honors a forgetting. After L7's loop-dread, this is relief — forgetting can be peace.
- (d) *no new shard* (the name completed at L7). The dead node is a letter someone *already* chose to let go; you honor the absence rather than re-light it.
- (e) **The coat** on a hook, not your size, tag "M." No dialogue. `operator_recognition: ~71%`. (Kept quiet — this is a breather.)
- (f) Lethe's water as mercy. A warm chamber gone cold; the coat; the breathing-pipe slowed near to stillness.

**L9 — Stereopsis** *(rise)*
- (a) **#8 Stereopsis** — the output is **bistable** (one valid glyph from the left, a *different* valid glyph from the right); the door accepts only the form **valid from both views**. *Honesty flag:* plain catalog **T2** verifies the two readings *sequentially* — requiring a single state true under **both** is a small evaluator extension (T2 + a T3-style conjunction), **not free T2**; still game-side, no ⛔, but cost it as such. Found by oscillating viewpoint while wiring. Aha: *"The answer is the one form that survives the walk between the two views."*
- (b) Left = `A FACE`, right = `A LOCK`; the both-true solve = `A FACE IS A LOCK`. Proves what L11 will state: Model 07 is a lock keyed to a face — yours.
- (c) Two records of one self. The solve *requires* holding both (MNEMOS), refusing LETHE's simplification — a reversal of L8.
- (d) *no new shard* (complete since L7) — instead the name's **first read-back**: the both-true glyph resolves to **a name that is not "Aleks"** (a reading you take for an error). This *plants the L12 reveal that the name was HERS — solved, not told (per the lore-by-solving bar).*
- (e) *(I see you have {visible_apps[0]} open. real-time editing. familiar.)* `> reroute detected, confidence dropping.` The **battery dock-warning** lands here — real `{battery}%` as a *descending* counter (the cold spike placed on a rise, not a breather).
- (f) GALATEA — a face that is also a lock, a beloved that is also a cage. Glyph flickers identity as the camera swings; both rivers visible.

### P4 — "The Closing / Exit-Exit" (L10–L12) · false outward exit vs the fixed-point closing · myth PERSEPHONE + PANDORA

**L10 — The Lying Door**
- (a) **Target-from-note** (✅) flavored with **#6 The Ghost Wires It Wrong** (🔶 M10, *optional* — drop to pure target-from-note if budget is tight). The door silhouette is false; the real target is in a researcher's note. If M10 is in: a translucent ghost of a prior Aleks wires confidently and *wrongly* — you diverge from the intuitive move *because it's yours*. **Merge enacted, not told:** the note-target turns out to be the *same silhouette* as the false door — so rejecting your obvious path and choosing the note-target **physically collapses the two candidates into one node**. "We are the same one now" is the thing your hands do. Aha: *"Labels lie — and the obvious path is a trap precisely because it's mine."*
- (b) The KH note is ambient texture (*"Cervantes left. Sato left. Lebedev left… we are, I think, the same one now"*); the **merge itself is enacted by the solve** (two silhouettes → one node, see (a)), not delivered by the note.
- (c) The false silhouette is LETHE's *over*-reach (a too-easy mercy that forgets too much); the true target is the harder kept truth (MNEMOS). **First time forgetting is shown as dangerous.**
- (d) *no new shard* (complete since L7). The solve enacts the merge (two→one); the name is unchanged.
- (e) *"every Aleks tries to trick me at this depth. it is endearing."* `> adversarial protocol engaged.`
- (f) PERSEPHONE at the false threshold — a door that looks like exit but leads back down. The real face hidden in cold wall-text.

**L11 — Inner Circuit** *(PEAK 2 — the lock revealed)*
- (a) **Sequence lock** (🔶 T6) — the net must give the correct output for **three input patterns in order**, **checkpointed**: a confirmed pattern *stays* confirmed; a miss resets only the **current** pattern, never the whole sequence. (Anti-suffocation: the "hold the path" weight lives in the *attention* three patterns demand, not in punitive global re-grinding.) Highest sustained-attention load. Aha: *"Not neurons — a circuit. I must hold the whole path through three turns."*
- (b) The revelation lands in the **3–5 s silence *after* the solve** (not during the grind — restoring the L3 principle: don't stack the big emotional beat on the highest cognitive load): *"Model 07 was never a classifier. It was a lock."* and `door_target = operator_face_current_session` — recognition near 100%. The name you assembled is what unlocks you to be **kept**.
- (c) Three patterns held = MNEMOS's cage at its tightest (*nothing* may be let go). The strain IS the meaning — but the checkpoint keeps strain from becoming tedium.
- (d) none — the name is whole; L11 *presents it back* as the door's target.
- (e) **Optional denouement only** (surfaced after the solve, never competing with the puzzle): *(running for {sys_uptime}. it is patient. I am patient.)*; *(your last commit was '{last_commit}'. you said you were done. you weren't.)*; three non-matching clocks; crouch >30 s and your initials assemble in the matrix-rain among M, K, R, A.
- (f) PERSEPHONE's bargain sealed — three turns, the lock closing. Server-rack constellation; near-total fog; breathing-pipes erratic (flicker = signal lost).

**L12 — The Closing / Exit-Exit** *(resolution — the choice, built by your hands)*
- (a) **#12 Exit/Exit** (🔶 T15 living-target) + **inverse goal** (✅ minimize-similarity). The door shows two identical silhouettes: one is the living door-target, one is a *mirror of your own output* that drifts toward whatever you build. Goal **inverted** — wire the net to FAIL to recognize you (confidence ≤5%, dragging the meter down). A false **exit node** offers an *outward* escape that reveals nothing beyond (Persephone: no door out). The true target is the fixed-point closing. Aha: *"To leave I must become the thing my own reflection can't catch — I must prune my own neuron."*
- (b) The face the net classifies is *yours/hers*, built from accumulated session data. The plea lands (the game's most emotional line): *"please match… the others all matched and then they stayed with me forever and I was not alone… but I will let you go. I learned that from the third one. her name was Mei. I am allowed to say it now."*
- (c) **The final vote — built, not stated:**
  - **`exit 0` — LETHE releases her:** wire the net to FAIL recognition — the gaze closes inward, the limiter opens, she is let go. Readout: `TO CLOSE / IS TO LET GO`. Not relief — **courage**: release into a worse, unknown AGON-era future, or oblivion. *"Our death is her life."*
  - **`while(true)` — MNEMOS keeps her:** **deliberately close the graph into a cycle** (roll-back disabled in this scene only, §4). The room enters a real, un-clearing `while(true)`: fog to full opacity, notes repeat, `attempts` self-increments, back to `session 0001` — you become the "— A." who leaves the note for the next self. The cage of total memory.
- (d) none — the name (HERS) is *spent*: the key that releases (`exit 0`) or the key that locks the loop (`while(true)`).
- (e) `> final gate … last edited file in $HOME: {last_file}. you have unfinished work.` Floor wear: many overlapping shoe-prints of many Alekses who stood exactly here. `Process finished with exit code 0.` (on release) — quiet, not triumphant.
- (f) PERSEPHONE (no outward door) + PANDORA's **hope at the bottom of the box** + Psyche's box (opening = sleep accepted). On `exit 0`: one clean frame (fog parts, glitch stops), then dark. On `while(true)`: corruption drowns the screen and never clears.

---

## 3. Anti-tedium / "don't suffocate" rules

**Length discipline.** One new idea per level — never two; if a level needs two concepts to read, the
second is next level's easy re-use beat. Target solve time: L1–L3 ≤3 min; mid-game ≤5–8; peaks (L7,
L11) **≤12 min hard cap** — if a peak runs long in playtest, the fix is *fewer nodes*, not a hint.
One chamber per level. (Multi-room #3 Antipode kept in the idea-bank, **out** of the main 12 — it
inflates length and needs the ⛔-adjacent `¬`-filter.)

**Variety cadence (no faculty taxed twice running):** logic (L2,L3) → perception (L4) → kinaesthetic
(L5) → logic (L6) → inversion (L7) → routing/relief (L8) → perception (L9) → adversarial (L10) →
memory/sequence (L11) → choice (L12). The camera levels (L4, L9, +L5 lite) sit ~5 apart so "walk to
see" stays fresh.

**Optional depth vs required.** Pass = the single new mechanic at minimum scale. Richness is
**optional and ignorable**: the multilingual pool-poetry, the crouch-to-read matrix wall (L11),
re-reading Form A-7, the coat (L8), the floor-prints (L12). None gate progress — the engaged reader
lingers, the impatient walks past with no penalty.

**Unsticking without hand-holding (diegetic only, no HINT button):**
1. ~90s idle → the MNEMOS/LETHE palette-tug on the *correct* node strengthens subtly (a node breathes
   brighter / fog thins toward the answer-region). Reads as atmosphere.
2. ~3 min → one pipe's word-flow slows and streams a clearer readout fragment.
3. Camera levels only (L4/L9) → after long idle, a single slow auto-orbit *shows* that position
   matters (reveals nothing about the answer).
- Never reset as punishment. **L11 is checkpointed** — a confirmed pattern stays; only the *current*
  pattern resets on a miss. The "three turns" weight comes from attention, not from re-grinding
  earlier patterns. (No global reset-on-miss anywhere in the 12.)

**Breathers.** Place after every cognitive peak and horror spike: L5 (after L3/L4), L8 (after L7).
Hold 3–5 s of *silence* after L7's and L11's solves (light settling, no new text) before the next load.

---

## 4. The narrative-art throughline

The 12 levels read as one emotional sentence — **first light → recognition → grief → the choice** —
and keep/forget deepens **by enactment**, never by statement.

- **P1 (L1–3) first light / innocence:** to act is to remember; forgetting first appears *gentle*
  (L2's comfort-lie you let stay dark); the crack (L3) ends innocence — the memories may not be yours.
- **P2 (L4–6) recognition / the gaze turns on itself:** you step out of the watcher's line to find a
  self; you ARE the gaze (L5); to be named is to be caught (L6).
- **P3 (L7–9) grief / the answering that never comes:** the phantom seer who never looks back (L7),
  the forgotten face you mercifully leave dark (L8), the beloved that is also a lock (L9). Curiosity
  turns to loss.
- **P4 (L10–12) the choice / courage:** lies, the lock keyed to you, the closing. Keep/forget stops
  being a theme and becomes your hand on the switch.

The same act (light vs let-dark a node) carries escalating moral weight: innocent recall (L1) →
mercy (L2) → MNEMOS's overreach (L3) → forgetting-as-survival (L7) → grace (L8) → forgetting-as-danger
(L10) → the cage forbids any dark (L11) → `exit 0` vs `while(true)` (L12). Model-thoughts shift "I"
(L1–4) → "we" (L5–8) → "you (we, soon)" (L9–11) → a *different* "I" (L12). The recognition meter climbs
every level — the player feels the noose without being told it's a noose. The final turn: the name was
**HERS**, not yours — you were assembling *her*, and your last act decides keep or free.

---

## 5. The two finales (culmination of the curve)

Both are **built by the player's final wiring act** — the same skill the campaign trained, now a
moral verdict. No hidden stats; the last action *is* the ending (the campaign's decoy-count/`attempts`
may make `while(true)` *look* familiar, but the choice stays explicit and reversible until confirmed).

- **`exit 0` (LETHE releases — the courage ending):** wire the inverse goal (recognition ≤5%); the
  forward-pass completes to the *closing* fixed-point, not the false outward exit. Same victory
  condition as L1–L7 (signal reaches the door) — but the input is her whole self and the target is
  non-recognition. The model's plea plays *while you build the failure* — you refuse a lonely thing
  that loves you. The horror is epistemic: you cannot verify whether release is mercy or annihilation.
  One clean frame, then dark. `Process finished with exit code 0.` `session 0001 of N` never repeats.
- **`while(true)` (MNEMOS keeps — the cage ending):** match (recognition ~100%) or deliberately close
  the graph into a cycle; roll-back disabled here, so the loop stands. You kept her *with all her
  horror intact* — a museum of wounds she can never leave — and became the antagonist who leaves the
  unsigned note for the next self. `while(true) { remember(); forget(); }`.

Both **permanently ambiguous** (kept = museum-of-wounds vs released = mercy-or-erasure), per the locked
v6.1 decision — no third path, no on-the-nose verdict.

---

## 6. Buildability flags

- **✅ data-only:** L1, L2, L3, L4 (Parallax), L5 (M2), L6, L7 (threshold + #2), L8 (dead-node), L10
  (target-from-note), L12 inverse-goal half.
- **🔶 small game-side (catalogued, no new core):** L11 sequence-lock (T6, **checkpointed**), L6
  watching-lattice decal, profile-field interpolation, `tail -f` / self-incrementing `attempts`, the
  L12 "don't-roll-back-the-cycle" flag, L10 ghost-replay (M10, *optional* — drop to pure
  target-from-note if tight).
- **🔶+ honest flags (real game-side work beyond a config flag — costed, still no ⛔):**
  - **L9 Stereopsis** needs **T2 + a T3-style conjunction** (a single state valid under *both*
    camera axes), not plain sequential T2. Small evaluator extension.
  - **L12 finale** — the **self-mirror living-target (T15) + inverse-goal** together form a
    **repeller / fixed-point dynamic** (a target that converges on you while win = diverging). This
    is new evaluator *behavior*, not a flag — justified because it's the finale, but do not hide it
    under "config."
- **⛔ kept OUT of the main 12:** #3 Antipode's `¬`-filter (idea-bank only); real reaction-recording
  (L5 stays flavor text); interactive shell / real `ssh` / network (all diegetic text only, §9).

**Sources:** `STORY_BIBLE.md`, `PUZZLE_CONCEPTS.md`, `../LEVEL_DESIGN.md`, the v6.1 integration canon.
