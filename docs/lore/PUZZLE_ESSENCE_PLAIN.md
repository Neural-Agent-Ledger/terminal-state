# TERMINAL STATE — what the puzzles ACTUALLY are (plain language)

No lore, no lyric, no file fragments, no node tables. Just: **what is the puzzle, and what does the
player do.** (For the lore → `PUZZLE_PROGRESSION_L1-L12.md`. For the exact graphs/solutions →
`PUZZLE_SOLUTIONS_L1-L12.md`. For the real shipped puzzles → `levels/01–07.lvl`.)

---

## The core loop (the whole game in five sentences)

You walk, first-person, through a dark ASCII room. Glowing **wires** ("pipes") run between glowing
**dots** ("neurons"); every wire has a **switch** you step up to and flip on/off. A neuron **lights
up** when enough of its incoming switched-on wires are lit — that "enough" is the neuron's
**threshold** (e.g. "needs 2 of its wires on"). Some neurons are fixed **inputs** (always on or
off); one neuron is the **door**. **Your job: flip the right switches so the signal flows from the
inputs, through the middle neurons, to the door — and it opens.**

So yes — you are **wiring a tiny neural net**, but as a *place you walk* and *switches you flip*,
not a settings screen. One rule to remember: if you ever wire a **loop** (a wire-path that feeds
back into itself), the signal can never finish — the game treats that as unsolvable and snaps your
switch back. (The very last level is the one place a loop is *allowed* to stand — on purpose.)

Each level below = **the core loop + exactly one new twist.**

---

## The 12 puzzles — the essence of each

**L1 — the basics.** One input, one wire, one door. Flip the switch so the input reaches the door.
*That's it* — it teaches you that flipping a switch sends a signal. (≈30 seconds.)

**L2 — spot the fake wire.** Two real inputs plus one **fake** input. The middle neuron needs two
wires on to light. Connect the two real ones, **ignore the fake** (you can tell it's fake — the
words flowing on it are unrelated), then send it to the door. *Twist: not every wire is worth using.*

**L3 — pick the right inner path.** Three inputs feed **two competing middle neurons.** Both *can*
light — but only one is the "right" one; the other is a plausible trap. You must wire the inputs to
light the **correct** middle neuron and send only that one onward. *Twist: it's not "combine
everything" (that was L2) — it's "choose which internal route is true."* (This is what makes L3
genuinely different from L2.)

**L4 — move to see the truth.** From where you spawn, the wires line up and look like **one wire** —
you literally can't tell which is which. **Walk around the room** until your angle separates them,
*then* you can see which one comes from a live source, and flip that. *Twist: the puzzle is hidden
by your viewpoint; you solve it by moving.*

**L5 — you are part of the circuit (breather).** One neuron only lights **when you're standing on a
certain spot.** Stand there, flip the rest. Easy on purpose — a rest level. *Twist: your own
position is a switch.*

**L6 — one neuron, two jobs.** A single middle neuron must feed **two doors at once** — you can't
cheat by using it for only one. Light it, branch it to both. *Twist: one thing has to serve two ends
at the same time.*

**L7 — all three, or nothing.** The door only opens when **all three conditions are on
simultaneously** (a strict "AND"), and there's one tempting wire that just lights a dead end. Turn
on all three, ignore the dead end. *Twist: total agreement required.*

**L8 — the broken neuron (breather).** One neuron is **dead — it can never light, no matter what you
do.** Stop trying to fix it; **route around it** through another path. *Twist: accept the broken
part and go around it.*

**L9 — two truths at once.** The final neuron needs **two separate mini-circuits both satisfied at
the same time.** You can't solve one and break it while solving the other — you have to hold both.
(Visually the sculpture even reads as one thing from the left, another from the right; the answer is
the form that's true from both.) *Twist: satisfy two independent halves together.*

**L10 — the door is lying.** The picture **on** the door tells you the wrong target. The **real**
target is written on a note on the wall — read it, aim there instead. (And the real target sits on
the same junction as the fake, so picking it quietly merges the two into one path.) *Twist: trust
the signal/the note, not the label.*

**L11 — a combination, in order (hardest).** Not one answer — a **sequence**. Set the circuit to
make answer 1, lock it, reconfigure for answer 2, then answer 3 — **in order.** A slip only resets
the *current* step (not the whole thing), so it's demanding, not punishing. *Twist: hold a routine
across three turns.*

**L12 — the choice (the ending).** The last room. The thing the circuit has been trained to
recognize is **you.** Two ways to finish, and your **last switch decides which:**
- Wire it so the circuit **fails to recognize you** → you slip free and leave (one ending).
- Deliberately wire a **loop that never resolves** → you stay, repeating, forever (the other ending).
*Twist: the final puzzle isn't "win" — it's "which way do you end."* Both are deliberately left
ambiguous; there's no third option.

---

## Quick reference — each twist in one line

| Lvl | The one new idea |
|---|---|
| L1 | flip a switch, signal flows |
| L2 | ignore the fake wire |
| L3 | choose the correct of two inner neurons |
| L4 | walk to see which wire is which |
| L5 | your position is a switch (rest) |
| L6 | one neuron must feed two doors |
| L7 | all three conditions at once |
| L8 | route around a dead neuron (rest) |
| L9 | satisfy two mini-circuits together |
| L10 | the door's label lies — read the note |
| L11 | a sequence of answers, in order |
| L12 | free yourself, or loop forever (the choice) |

---

*Reality note: only L1–L7 exist as real, playable files today (`levels/01–07.lvl`). L8–L12 are
designed here and in the two companion docs but are not built yet — building them means writing
`levels/08–12.lvl` in the same format.*
