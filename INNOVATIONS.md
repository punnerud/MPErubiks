# Innovations

What MPErubiks invented, what it borrowed, and what it moved somewhere it is
not normally used — in the spirit (and format) of
[mpedb's INNOVATIONS.md](https://github.com/punnerud/mpedb/blob/main/INNOVATIONS.md).
Each entry states the *problem* before the mechanism. Numbers are measured,
not estimated; where something was tried and lost, it is in §7, which is
deliberately not short.

| tag | meaning |
|---|---|
| **[I]** | invented here |
| **[T]** | textbook / standard practice — listed for completeness, not credit |
| **[X]** | transferred from another field (mostly: route optimization / MPEE, database / MPEdb) |
| **[C]** | standard parts, unusual combination |

The through-line: **cost models + orderings**. A Rubik's solve, a camera scan,
a compressed table and a SQL plan are the same shape once you ask *"what is
the cheapest order?"* — MPEE asks it about routes, MPEdb about query plans,
and every section below asks it about something on a cube.

---

## 1. Route compression of the solver table [X]

**Problem.** The Kociemba solver ships 6.8 MB of move/pruning tables; a phone
downloads that before the first solve.

**Mechanism.** Three distribution-shaping transforms before deflate, chosen by
measurement: (a) pruning depths are 0..13 → two per byte; (b) u16 move-table
coordinates split into lo/hi byte planes; (c) — the transfer — the move
tables are stored as **per-column deltas along the coordinate order**.
The reordering question is formally an open TSP: find the row permutation π
minimizing `Σᵢ H(row_π(i+1) ⊖ row_π(i))`. Measured on the 40 320-row corner
permutation table: the **identity permutation wins** — kewb's coordinates are
positional/factorial number systems (`co = Σ oₖ·3ᵏ`, `cp = Σ lₖ·k!`), so
neighboring rows differ in few digits and the enumeration *is* the optimal
route. Deltas along it: 517 KB → 46 KB (11×). Total asset: **6.8 MB → 0.9 MB
(13 %)**, bit-exact roundtrip, ~15 ms to unpack.

**Provenance.** Delta coding is [T]; recognizing table-row ordering as a
solved TSP whose optimal tour is the index itself, and measuring competitors
against it, is the MPEE transfer. See §7.1–7.2 for what lost.

## 2. Algorithms as route segments: the guided-solution engine [X]

**Problem.** The move-optimal solution is pedagogically worthless to a human:
every move must be read. A human executes *known algorithms* from muscle
memory — so the right objective is predicted *human* time, not move count.

**Mechanism.** A bounded best-first search over chains of recognized,
user-trained algorithms with kewb filling the gaps. Pure VRP cost modeling:
per-segment ergonomic cost, **junction costs** between segments (move
cancellation bonus, regrip penalty, recognition pause), a hard user-set
budget over the optimal length, and a lexicographic objective
`min (−algorithms_used, −user_priority, first_use_ergo, total_ergo)` — show
the most knowledge, break ties by the user's own ranking. Expensive tail
solves are memoized by canonical state ("buy the cell once" — the MPEdb
discipline). The in-progress *Min vei* mode completes the transfer: edge
costs become the user's **measured** per-algorithm times from the database,
plus a micro-IDA* cross solver for the one stage that has no named
algorithms.

## 3. The scan constraint resolver: analyze your way to correct [I]

**Problem.** A phone camera cannot classify all 54 stickers reliably —
glare, warm light, lime-vs-yellow. Better optics is the wrong fix.

**Mechanism.** The camera outputs *evidence* (per-cell vote histograms, kept
soft by hue-softmax so near-boundary pixels honestly split), and the cube
itself is the decider: a backtracking search over **evidence-plausible
candidates only** (a cell with zero red votes can never become red), pruned
by the hard invariants — nine stickers per color, all 26 pieces exactly once,
solvability. Cells with no evidence are fully free; when even plausible
readings admit no legal cube, the lowest-margin cells are widened, cells of
*oversubscribed* colors first. Search is MRV-ordered, node-budgeted (never
blocks a UI thread), and full validation runs only on complete assignments.
Field result: real six-side scans resolve with 0–3 corrections.

**Provenance.** CSP techniques are [T]; treating a *camera* as an evidence
source whose output is completed by group-theoretic invariants — and wiring
negative evidence into the candidate sets — we have not seen elsewhere.

## 4. Recognition patterns derived from the algorithms themselves [I]

**Problem.** 127 algorithm cases need recognition patterns; hand-coded masks
rot and typos become silent misteaching.

**Mechanism.** A case is defined *only* by its algorithm. At startup the
recognizer applies `alg.inverse()` to a solved cube and derives the pattern:
OLL as a 21-bit orientation mask, PLL as a y-invariant relative-color cycle
key, F2L as piece-location keys — each inserted under all 4 AUFs (× 4
y-frames where relevant). Data errors are structurally impossible to ship:
an algorithm that doesn't produce a state of its claimed kind, or two cases
that collide, **fail the build**. Execution folds pre-AUF and frame
conjugation (`in_y_frame`) back in mechanically.

## 5. Camera discipline: one source of truth, then geometry [C]

Three combinations that made the scanner work on real phones:

- **Single-source canvas** — the same 2D canvas feeds the GPU preview *and*
  the sampler, so what you see is exactly what is measured (raw GPU copies
  from the `<video>` element silently bypass iOS's orientation).
- **Grid auto-fit as two 1-D problems** — the sticker grid's offset *and
  scale* are found by projecting chroma mass (plus a brightness floor so
  white faces work) onto each axis and scoring span/gap alignment;
  confidence = retained-mass fraction. The user just holds the cube "roughly
  there"; the grid turns green when locked, and sampling follows invisibly
  while the drawn grid stays still.
- **Center identity as a 6×6 assignment problem** — which capture shows
  which color is solved by brute-forcing all 720 permutations over center
  evidence, so duplicate or unreadable center votes cannot corrupt the
  mapping. The scanned cube is finally rotated + relabeled so every color
  sits on its standard face — the display speaks the physical cube's colors.

## 6. Fonts sized to the text, fetched when chosen [C]

**Problem.** Supporting Chinese, Japanese and Korean means glyphs the UI
font lacks. A full Noto CJK is ~10 MB — more than the whole app — and
bundling it taxes every user, including the ones who only ever read Norwegian.

**Mechanism.** The translations are the input to the font build: for each
CJK language, `tools/fetch_font_subsets.py` collects the ~230 distinct
characters that language's UI actually uses and asks Google's font API for a
subset containing exactly those (`text=`), served as TTF to a legacy
user-agent because egui reads TTF, not woff2. Result: 38–94 KB per language
(253 KB for all four), redistributable under Noto's OFL. At runtime the
subset is fetched only when the user opens the language picker, and
installed as the LOWEST-priority fallback so Latin text keeps the default
font's shapes. Adding a script costs other users nothing.

**Provenance.** Font subsetting is [T]; deriving the subset from the app's
own translation files, and treating "which glyphs does this product need"
as a build-time question, is the combination.

## 7. Small disciplines worth naming

- **Derived, never stored, UI gates [I-ish].** The practice-stop gate is a
  pure function of `(mode, cursor, segment_ids, revealed)` — no stored flag
  can go stale. The same discipline killed a class of bugs elsewhere
  (auto-snap re-arming, guide done-state).
- **Live screenshots as documentation [C].** The README's images *are* the
  screenshot tests; they regenerate with the code and cannot rot.
- **Translation as a fan-out, verification as a test [C].** 30 agents each
  own one language and write one CSV; correctness is enforced mechanically
  afterwards (every key present in every language, RFC 4180 round-trip,
  unique codes, English fallback never empty). One agent noticed the English
  source itself had a duplicated scan instruction — a bug the fan-out
  surfaced for free.
- **Field-data eval as a test [C].** Real uploaded captures with hand-labeled
  ground truth run as a gated cargo test with a no-regression assert — the
  classifier's accuracy (87/90) is a number in CI, not a feeling. The night
  run's labels were validated by an independent invariant: they sum to
  exactly nine stickers per class and form a legal cube through the grid
  tables.

## 8. Negative results

Measured, then rejected — kept because the numbers are the point.

1. **Lexicographic row reordering of move tables.** 7× *worse* than the
   natural order after delta (313 KB vs 46 KB) and must additionally ship a
   79 KB permutation. The coordinate enumeration is already the tour.
2. **Delta coding on pruning tables.** Depths vary locally without row
   adjacency; deltas *raise* entropy (237 KB → 282 KB). Nibble+deflate stays.
3. **MPEE as the cube solver.** Rejected on day one: cube solving is
   group-theoretic search with perfect structure (Kociemba two-phase gives
   ≤21 in milliseconds); VRP metaheuristics have nothing to grip. MPEE's
   *thinking* — measured cost matrices, orderings, buy-once caching — is all
   over this repo; its solver is not.
4. **Hard 21-move bound on one wasm thread.** 10–15 s solve times in the
   field. A 23 bound returns near-instantly and is pedagogically identical;
   a shallow IDA* pre-pass (≤4 moves) keeps nearly-solved cubes honest.
   Corollary: the engine's hidden `.max(24)` budget floor silently violated
   small user caps — floors and caps don't compose.
5. **All-nine-cells-stable auto-capture.** Demanding nine identical
   classifications for 15 straight frames never fired hand-held; one
   flickering cell reset the window. Tolerating two flickers per frame (the
   averaged evidence absorbs them) made capture calm *and* reliable.
6. **The moving guide square.** Auto-alignment that visibly dragged the grid
   read as jitter and made users lose the cube. The fix that shipped:
   alignment steers *sampling* silently; the drawn grid only changes color.
7. **Whole-file generic compression.** gzip -9 on the raw table: 2.74 MB.
   The shaped format is a third of that — the win was never the entropy
   coder, it was giving it the right distributions.
