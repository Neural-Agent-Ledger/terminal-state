# TERMINAL STATE — Puzzle Solutions / Structural Spec L1–L12

**Date:** 2026-06-18 · **Status:** structural draft (engine-grounded)
**Companion to:** `PUZZLE_PROGRESSION_L1-L12.md` (the *why*: curve, lore, theme). **This doc is the
*what*:** exact node graphs, pipes/switches, thresholds, decoys, door targets, and **the solution**
for every level. No lyricism here.

Grounded in the real engine (the terminal3d `re_puzzle` crate) and the real `levels/01–07.lvl`.

---

## 0. Engine primer (the rules every puzzle obeys)

- **Node** = `{ threshold T, is_input, is_output }`, plus a `pool` (words) for readout. A non-input
  node **fires** (`activation = 1`) iff **Σ(activations of its ON incoming pipes' sources) ≥ T**;
  else 0. An **input node** is a fixed `0/1` from the level's `[inputs]` block (it ignores T).
- **Pipe** `from→to`, `state = ON|OFF`, with **one switch cell** the player walks to and toggles.
  An **OFF** pipe contributes 0 to its target's sum. Toggling is the *only* player action.
- **Forward pass** = topological (Kahn). **Door wins iff EVERY `door_target` node fires** (`==1`).
- **Cycle** (graph loops) ⇒ `ForwardError::Cycle` = unsolvable; the engine **rolls the toggle back**
  (the one exception is the L12 finale flag — see L12).
- **Decoy** = a node/pipe that cannot help reach the door (not a target, or contributes 0, or fires
  but feeds nothing the door wants). Recognised by reading its `pool` (semantically off) or its
  topology (feeds a non-target).
- **Dead node** (used L8) = **no new feature needed**: set `threshold` higher than its maximum
  possible incoming sum ⇒ it can never fire.
- **Readout / name-shard** = when the solution path fires, the target node's `pool` word (at
  `anchor` index) surfaces as that level's shard. Name = 7 shards, complete at L7 (see PROGRESSION
  doc §"Shard map").

**Notation below.** Nodes: `name(kind, T=threshold)`; kinds = `IN` input / `H` hidden / `OUT`
output(door target) / `DECOY`. Pipes: `A→B`. Inputs: `IN=1/0`. **Solution = the set of pipes you
turn ON** (all pipes start OFF unless noted). `✅` = buildable on shipped engine; `🔶 feat:X` =
needs feature X (cost in §13).

---

## L1 — First Pulse  ✅  *(shipped; levels/01.lvl)*
- **Nodes:** `R(IN=1)` · `output(OUT, T=1)`
- **Pipes:** `R→output` (switch at (6,3,2))
- **Door target:** `output`
- **Solution:** turn ON `R→output`. R=1 ≥ T1 ⇒ output fires.
- **Shard:** `R`. **Teaches:** click = a pipe carries a signal to the door.

## L2 — Two Make Three  ✅  *(shipped; levels/02.lvl)*
- **Nodes:** `R(IN=1)` · `G(IN=1)` · `neutral(IN=0, DECOY)` · `mix(H, T=2)` · `output(OUT, T=1)`
- **Pipes:** `R→mix` · `G→mix` · `neutral→mix` (decoy) · `mix→output`
- **Door target:** `output`
- **Solution:** ON = {`R→mix`, `G→mix`, `mix→output`}. `mix` gets 1+1=2 ≥ T2 ⇒ fires ⇒ output fires.
  `neutral→mix` is irrelevant (neutral=0); the *point* is to recognise it's a lie (its pool is
  `STONE/BONE/ASH`, not `ADD/COMBINE`) and not waste a toggle on it.
- **New idea:** combine two real inputs through a threshold-2 node; **reject one external decoy by
  reading its pool.**
- **Shard:** `learn`.

## L3 — Full Spectrum  ✅  *(shipped; levels/03.lvl)*
- **Nodes:** `R(IN=1)` · `G(IN=1)` · `B(IN=1)` · `h1(H, T=2)` · `h2(H, T=2)` · `output(OUT, T=1)`
- **Pipes:** `R→h1` · `B→h1` · `G→h2` · `R→h2` · `B→h2` · `h1→output` · `h2→output`
- **Door target:** `output` (target colour = **magenta** = h1's R+B reading)
- **Solution (magenta):** ON = {`R→h1`, `B→h1`, `h1→output`}; leave `h2→output` **OFF**.
  h1 = R+B = 2 ≥ T2 ⇒ fires ⇒ output fires. (A cyan solution exists via h2 = G+B — the level wants
  the magenta one per the note.)
- **New idea (vs L2):** **two competing hidden nodes that share inputs.** You don't reject an
  obvious external lie — you must **discriminate between two plausible *internal* representations**
  (h1 "this room" vs h2 "the room a note remembers wrongly") by *which pipes you route*, not by
  reading a pool. h1 and h2 are both real, both fireable; only one is the door's truth.
- **Shard:** `A`.

> ### ⟦L2 vs L3 — the difference, explicit⟧ *(your note)*
> | | **L2** | **L3** |
> |---|---|---|
> | hidden nodes | **1** (`mix`) | **2 competing** (`h1`,`h2`) |
> | the obstacle | an **external decoy input** (`neutral`) you reject by **pool** | a **second internal hidden node** you reject by **topology** |
> | challenge type | "pick the 2 real inputs of 3" (combinatorial) | "choose which internal path reaches the door" (topological) |
> | failure mode | wasting a toggle on a dead input | lighting the *wrong* hidden node (it fires, but it's the wrong memory) |
>
> They share the colour-additive *skin* but are mechanically distinct. If they still feel samey in
> playtest, push L3 further: give h1/h2 **different thresholds** (h1 T=2, h2 T=1) so h2 fires *too
> easily* and the trap is that the easy answer is the wrong room.

## L4 — The Angle (Parallax)  ✅ *(geometry, no engine feature)*  *(replaces shipped "Shape Detector")*
- **Nodes:** `live(IN=1)` · `dead_src(IN=0, DECOY)` · `output(OUT, T=1)`
- **Pipes:** `live→output` and `dead_src→output` — **routed so that from the spawn camera the two
  pipe-lines visually overlap into one straight line** (authored paths, not an engine feature).
- **Door target:** `output`
- **Solution:** **walk around the pedestal** until parallax separates the two lines on screen; you
  can now see which pipe originates at the live source. Turn ON `live→output`. (Toggling
  `dead_src→output` does nothing — source is 0.)
- **New idea:** the *graph is trivial*; the difficulty is **perceptual** — you must move the camera
  to read the wiring truthfully. Buildable now: it's two normal pipes + careful path geometry.
- **Shard:** `://` (the boundary — "which line is which").

## L5 — The Soft-Footstep (player-as-input)  🔶 feat:floor-trigger-input  *(breather)*
- **Nodes:** `present(IN, driven by floor zone)` · `a(IN=1)` · `b(IN=1)` · `unit11(H, T=2)` · `output(OUT, T=1)`
- **Pipes:** `present→unit11` · `a→unit11` · `b→unit11`(only 2 of these needed) · `unit11→output`
- **Inputs:** `present` = 1 **while you stand in the warm floor-zone**, else 0; `a=1`, `b=1`.
- **Door target:** `output`
- **Solution:** stand in the warm zone (sets `present=1`); ON = {`present→unit11`, `a→unit11`,
  `unit11→output`}. unit11 = present+a = 2 ≥ T2 ⇒ fires. Low logic — a deliberate rest.
- **New idea:** **your body position is a bit.** The node `unit11` literally fires on
  `operator_present`. Needs a small feature: a floor trigger volume setting an input node.
- **Shard:** `merge` (observer = observed: *you* are now one of the inputs).

## L6 — Polysemantic  ✅
- **Nodes:** `s1(IN=1)` · `s2(IN=1)` · `poly(H, T=2, dual-pool)` · `out_face(OUT, T=1)` · `out_grid(OUT, T=1)`
- **Pipes:** `s1→poly` · `s2→poly` · `poly→out_face` · `poly→out_grid`
- **Door target:** **BOTH** `out_face` AND `out_grid` (door wins only if both fire).
- **Solution:** ON = {`s1→poly`, `s2→poly`, `poly→out_face`, `poly→out_grid`}. One fired node
  (`poly`) must feed **two** required outputs at once — you cannot collapse it to one.
- **New idea:** **one node carries two meanings and both are required** — the "keep both / don't
  simplify" beat made mechanical (MNEMOS pull). Data-only (the dual `pool` is just content).
- **Shard:** `named`.

## L7 — Garden  ✅  *(shipped; levels/07.lvl)*
- **Nodes:** `water(IN=1)` · `light(IN=1)` · `seed(IN=1)` · `growth(H, T=3)` · `bloom(OUT, T=1)` · `dormant(H, T=1, DECOY)`
- **Pipes:** `water→growth` · `light→growth` · `seed→growth` · `growth→bloom` · `seed→dormant` (decoy)
- **Door target:** `bloom` (NOT `dormant`)
- **Solution:** ON = {`water→growth`, `light→growth`, `seed→growth`, `growth→bloom`}. growth = 3 ≥ T3.
  `seed→dormant` is the decoy ("a seed left in the dark just sleeps") — fireable but feeds no target.
- **New idea:** **threshold-3 AND-gate** (all three conditions at once) + a fire-but-useless decoy.
- **Shard:** final byte ⇒ **name complete:** `pid://…@<hash>` (= HERS).

> **L8–L12 add NO new shards.** The name is whole; these levels recontextualise, read it back, and
> spend it.

## L8 — The Cold Room (dead node)  ✅ *(dead = unreachable threshold; no feature)*  *(breather)*
- **Nodes:** `a(IN=1)` · `b(IN=1)` · `c(IN=1)` · `X(H, T=99 ⇒ DEAD)` · `H(H, T=2)` · `output(OUT, T=1)`
- **Pipes:** `a→X` · `b→X` · `X→output` (the *obvious* path, through the scar) · `a→H` · `c→H` · `H→output` (the route-around)
- **Door target:** `output`
- **Solution:** the obvious path is dead — `X` can never fire (T=99 > its max sum 2). **Route
  around:** ON = {`a→H`, `c→H`, `H→output`}. Do **not** waste toggles trying to feed `X`.
- **New idea:** **a node that cannot fire; you go around it.** Lore: a forgotten face you mercifully
  leave dark (LETHE's kindest level). Buildable now via the threshold trick.
- **(d) shard:** none (complete since L7); you *honor an absence*.

## L9 — Stereopsis (discrimination → conjunction)  ✅ *(graph; the "two views" is visual skin)*
- **Nodes:** `w(IN=1)` · `x(IN=1)` · `y(IN=1)` · `z(IN=1)` · `h_left(H, T=2)` · `h_right(H, T=2)` · `both(OUT, T=2)`
- **Pipes:** `w→h_left` · `x→h_left` · `y→h_right` · `z→h_right` · `h_left→both` · `h_right→both`
- **Door target:** `both`
- **Solution:** ON = {`w→h_left`, `x→h_left`, `y→h_right`, `z→h_right`, `h_left→both`,
  `h_right→both`}. `both` needs **both** hidden nodes lit (T=2) — the answer is the state true from
  *both sides* at once. (Visually: the sculpture reads `A FACE` from the left, `A LOCK` from the
  right; the solve reads `A FACE IS A LOCK`.)
- **New idea:** **a conjunction node requiring two independent hidden representations** — you must
  satisfy two sub-nets simultaneously, not pick one (the L3 reversal: hold *both*).
- **Plot-by-solving:** `both`'s `pool` reads out **a name that is not "Aleks"** (a feminine readout
  you take for an error) — *plants the L12 "the name is HERS" reveal, solved not told.*

## L10 — The Lying Door (target-from-note + merge)  ✅ *(ghost variant 🔶 feat:M10, optional)*
- **Nodes:** `in1(IN=1)` · `in2(IN=1)` · `J(H, T=1, junction)` · `A_obvious(OUT?, T=1)` · `B_true(OUT, T=1)`
- **Pipes:** `in1→J` · `in2→J` · `J→A_obvious` · `J→B_true`
- **Door target:** **`B_true`** — but the door *silhouette* depicts `A_obvious`. The real target is
  named only in a researcher's `[note]` on the wall.
- **Solution:** **read the note** ⇒ target is `B_true`, not the obvious `A_obvious`. ON = {`in1→J`,
  `in2→J`, `J→B_true`}. Because both candidates hang off the **same junction `J`**, choosing the
  note-target collapses the two apparent options into one path — **the merge, enacted by your hand**
  ("we are the same one now"), not delivered by the note.
- **🔶 optional:** a ghost (M10) pre-wires `J→A_obvious` confidently and wrongly; you must diverge.
  Drop to pure target-from-note if M10 isn't in budget.
- **New idea:** **labels lie — trust the activation/the note, not the silhouette.**

## L11 — Inner Circuit (sequence lock)  🔶 feat:sequence-target (small)
- **Nodes:** ~6 switchable-net nodes feeding one `O(OUT, T=1)`, with enough switches to produce
  three distinct output states.
- **Targets (in order):** `seq = [P1, P2, P3]` — the player must make the net's output match P1,
  then reconfigure to match P2, then P3. **Checkpointed:** a confirmed pattern stays; a miss resets
  **only the current** pattern (never the whole sequence — anti-suffocation).
- **Solution:** find the three switch-configurations that yield P1→P2→P3 and enter them in order.
  (The "hold the whole path through three turns" weight is the *attention*, not punitive resets.)
- **Plot-by-solving (in the 3–5 s silence *after* the solve, not during):** `door_target =
  operator_face_current_session` — the thing you trained is a **lock keyed to your face**; the name
  you assembled is what locks you in to be **kept** (MNEMOS's cage at its tightest).
- **🔶 feature:** a multi-target state machine (`targets = [t1,t2,t3]`, advance on stable match) —
  designed in `LEVEL_DESIGN §7.9`, not yet coded. Small.

## L12 — The Closing / Exit-Exit (the choice)  🔶 feat:inverted-target (small) + feat:cycle-keep (moderate)
- **Nodes:** `self_feed(H, T=1)` … → `REC(OUT?, T=1)` (the recognition node = "she is kept") plus an
  `EXIT(OUT, T=1)` reachable by a different route.
- **The two endings are two different final wirings:**
  - **`exit 0` — LETHE releases (inverted target):** the win condition is **`REC` must read 0** —
    you **prune your own feed** (turn OFF the pipe that lets `REC` reach threshold) while routing
    signal to `EXIT`. "Become the thing your reflection can't catch." Readout `TO CLOSE / IS TO LET
    GO`. → `Process finished with exit code 0.` *(Needs an inverted/"must-be-0" target — small.)*
  - **`while(true)` — MNEMOS keeps (cycle stands):** **deliberately wire a loop** (e.g. `REC→…→REC`).
    Normally the engine rolls a cycle back; in this scene a **`cycle_keep` flag** lets the loop
    stand ⇒ `ForwardError::Cycle` becomes the ending: fog to full, notes repeat, `attempts`
    self-increments, back to `session 0001`. *(Needs the rollback-disable flag — moderate.)*
- **Both** built by the player's last toggle; **both permanently ambiguous**; no third path.
- **🔶 optional post-v1.0:** the full "living-mirror" target that *chases* your output (a
  repeller/fixed-point dynamic). The buildable v1 above (inverted target + cycle-keep) delivers the
  same choice without it.

---

## 13. Engine features needed beyond the shipped engine (honest cost)

The shipped engine already does **L1, L2, L3, L4, L6, L7, L8, L9, L10(base)** (threshold nodes,
pipes/switches, decoys, splines, pools, the dead-node-via-high-threshold trick, target-from-note).
What the rest needs:

| Feature | Levels | Cost | Note |
|---|---|---|---|
| floor-trigger → input node | L5 | small | a trigger volume sets an input bit (player-as-input) |
| ghost-replay (M10) | L10 (optional) | small | a translucent wrong pre-wiring; droppable |
| sequence-target state machine (`targets=[t1,t2,t3]`) | L11 | small | designed `LEVEL_DESIGN §7.9`, not coded |
| inverted / "must-be-0" target | L12 `exit 0` | small | win on a node **not** firing |
| `cycle_keep` flag (don't roll back the loop in this scene) | L12 `while(true)` | moderate | the loop is the ending |
| living-mirror target (chases output) | L12 (optional) | post-v1.0 | not required; nice-to-have |
| camera-angle *engine readout* (true Parallax/Stereopsis as engine state) | — | post-v1.0 | **NOT needed** — L4/L9 above are built as geometry + plain graphs, not engine-evaluated camera state |

**Net:** the whole 12-level campaign is buildable with **four small features + one moderate flag**
(L12 cycle-keep). No level requires the post-v1.0 camera-readout or living-mirror; those are
optional polish. Nothing here is a ⛔ "new core mechanic" that breaks the v0.1 scope contract.

---

## 14. Cross-checks

- L1/L2/L3/L7 graphs above are the **actual shipped `.lvl` files** (verbatim node/threshold/pipe
  structure). L4–L6, L8–L12 are new graphs designed to the same engine rules.
- Solutions are stated as the **set of pipes to turn ON** (all start OFF). Each is verified against
  the firing rule (Σ ON-inputs ≥ T) and the win rule (all door targets fire).
- The L2↔L3 distinction (box under L3) addresses the "they're too similar" note: different hidden-
  node count, different obstacle type, different challenge class.
- Theme/lore per level lives in `PUZZLE_PROGRESSION_L1-L12.md`; this doc is the buildable skeleton.
