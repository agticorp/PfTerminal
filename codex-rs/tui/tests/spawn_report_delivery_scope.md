# Spawn Report Delivery — Multi-Turn All-Codex Verification Project

## Why this exists

There are consistent, continuous problems with reports not being processed up the
chain: **Nazguls not getting their reports from Trolls, and Trolls not responding to
Orc reports.** The single-turn demo crew ("Troll + 2 Orcs mock website") exercises
TURN 1 → TURN 2 and looks fine, but the real defect only surfaces on multi-turn efforts
where a manager pane is frequently **mid-turn** when a child reports back.

This document defines (1) a deliberately complicated multi-turn project to drive the
edge cases, (2) the exact pane/model mapping (all Codex), and (3) a concrete verification
plan for whether child reports are being processed successfully back up the hierarchy.

## Root-cause summary (for the record, before the fix)

The report-delivery path is `record_spawn_child_report_for_thread` →
`record_spawn_parent_report` → `notify_spawn_parent_report`. For a native Codex parent
thread the trigger is:

```rust
let is_running = agent_navigation.get(&parent_thread_id).map(|e| e.is_running).unwrap_or(false);
if !is_running {
    SubmitSpawnAgentTask { thread_id: parent_thread_id, task: trigger_prompt }
}
```

Two defects for multi-turn:

1. **Reports to a running parent are dropped.** If the parent (Troll/Nazgul) is
   mid-turn when the child (Orc/Troll) reports, only a passive transcript/info cell is
   written — **no turn is triggered to process the report**, and nothing re-surfaces it
   when the parent goes idle. `spawn_parent_reports_by_node` retains the report text for
   the parent's *display context* (capped at 12, shown as "Recent child reports" on the
   parent's next turn), but that context only appears if the parent runs another turn for
   some *other* reason. There is no pending-report queue and no flush-on-idle.

2. **A direct report is not processed like a user query.** A manager receiving a child
   report should run a real turn over it (triage / dispatch / acknowledge), exactly as a
   user prompt would. Today it only becomes a turn if the parent happens to be idle at the
   exact instant of delivery. On a busy multi-turn effort that race is lost constantly.

> The fix (separate from this doc): add a per-parent **pending-report queue**, deliver the
> report as a turn immediately when the parent is idle, and flush the queue on the parent's
   idle transition (`TurnCompleted` → `set_running(false)` in
   `thread_routing.rs` handle_spawn_status_notification). This doc is the verification
   project that proves the fix holds over many turns.

## The project: "Procedural Star Map Explorer"

A single-page interactive website that renders a procedurally generated star map on an
HTML5 canvas, lets the user pan/zoom, select stars, and read generated "system dossiers"
(a name, a type, and a short procedurally-written description) for each star. It is
deliberately **layered** so it cannot be done in two turns and so the work splits cleanly
into two non-overlapping IC assignments that must be reconciled by a Troll, then audited
by a Nazgul over multiple review rounds.

### Required deliverables (concrete, reviewable artifacts)

A directory `star-map-explorer/` containing:

1. `index.html` — loads `style.css`, `stars.js`, `dossier.js`, `app.js`; hosts the
   `<canvas id="sky">` and a `<div id="dossier">` panel. No build step; opens directly
   from the filesystem.
2. `style.css` — dark-space theme; layout uses fl/grid for canvas + side panel; the
   dossier panel is hidden by default and slides in on star select; responsive down to
   mobile width (single column).
3. `stars.js` — a seeded PRNG (mulberry32), a deterministic star generator that produces
   N stars across a virtual coordinate space, each with `{x, y, type, seed}`, and a
   quadtree (or spatial hash) for hit-testing during pan/zoom so selecting a star is
   O(log) not O(n). Exports `StarField` on a global.
4. `dossier.js` — given a star's `{type, seed}`, deterministically synthesizes a system
   name, classification, and a 2–3 sentence description. Names must be reproducible from
   the seed (same seed → same dossier). Exports `generateDossier(star)` on a global.
5. `app.js` — wires `StarField` + `generateDossier` to the canvas: pan (drag), zoom
   (wheel), click-to-select (hit-test via the quadtree), renders stars sized by zoom
   level, and on select calls `generateDossier` and shows the result in the dossier panel
   with a fade/slide animation.
6. `MANIFEST.md` — written by the Troll: the assignment breakdown, which Orc did what,
   the quadtree/dossier interface contract the two Orcs agreed on, a list of edge cases
   tested, and a final risk list.
7. `ACCEPTANCE.md` — written by the Nazgul: the Nazgul's own pass/fail verdict against
   the acceptance criteria below, with evidence (which file/function, what the Nazgul
   observed), remaining risks, and an explicit sign-off or a list of forced rework items.

### Acceptance criteria (Nazgul must verify each against the code, not the Orcs' claims)

- A1. Opening `index.html` renders stars on the canvas with no console errors.
- A2. Panning and zooming work and do not regress star positions (deterministic).
- A3. Clicking a star opens the dossier panel with a name + type + description.
- A4. Selecting the same star after a reload produces the **identical** dossier
  (seed-determinism). This is the key reproducibility check.
- A5. The quadtree/spatial-hash is actually used for hit-testing (no O(n) scan in the
  click handler) — verifiable by reading `app.js`/`stars.js`.
- A6. Layout is responsive: at a narrow width the dossier stacks below the canvas.
- A7. `MANIFEST.md` exists and matches the shipped code (interface contract is real).
- A8. `ACCEPTANCE.md` exists with a per-criterion verdict and evidence.

### Why this is "somewhat complicated" and genuinely multi-turn

- Two ICs share an **interface contract** (`StarField` hit-test API ↔ `app.js` consumer;
  `generateDossier` signature ↔ `app.js` consumer). The Troll must define and enforce
  it, and a mismatch forces a re-dispatch — this is a natural multi-turn loop.
- **Determinism (A4)** is a cross-cutting property that neither Orc owns alone: the
  `dossier.js` seed path and the `stars.js` seed storage must agree. A bug here typically
  requires the Troll to send one Orc back, wait, and re-review — exactly the "Troll
  responds to Orc report" path that fails today.
- **Hit-test performance (A5)** is a Troll-level architectural concern the Nazgul audits;
  it often generates a rework loop.
- A realistic run is 6–12 turns total across the four panes, with at least 2–3
  child→parent report cycles, at least one forced rework, and at least one moment where a
  parent is mid-turn when a child reports (the failing race).

## Pane / model mapping (all Codex-native)

| Role  | Pane            | Model                          | Provider | Purpose |
|-------|-----------------|--------------------------------|----------|---------|
| Orc 1 | Codex agent pane | `gpt-5.5`         | OpenAI | IC: `stars.js` + spatial index + `app.js` canvas wiring |
| Orc 2 | Codex agent pane | `gpt-5.5`         | OpenAI | IC: `dossier.js` (deterministic name/type/description) + `index.html` + `style.css` |
| Troll | Codex agent pane | `zai-org/GLM-5.2` (Vercel fast) | Vercel | Supervisor: assigns the two Orcs, defines & enforces the interface contract, reviews each Orc's report, forces rework, writes `MANIFEST.md`, reports to Nazgul |
| Nazgul| Codex agent pane (root) | `zai-org/GLM-5.2` (Z.AI)  | Z.AI | CTO/root: kicks off the Troll, audits the final artifacts against A1–A8 by reading the code, writes `ACCEPTANCE.md`, signs off or returns to Troll |

> Nazgul is created with `/spawn nazgul` → "Create Nazgul pane" (Z.AI GLM-5.2), then
> bound as root. Troll is `/spawn troll` under the Nazgul (Vercel-fast GLM-5.2). Each Orc
> is `/spawn orc` under the Troll (`gpt-5.5` via OpenAI). The Troll owns the
> interface contract and the MANIFEST; the Nazgul owns ACCEPTANCE and the final verdict.

## Multi-turn workflow (what should happen, turn by turn)

This is the expected trace if report delivery is **working**. Each "→ report" is a
child→parent report that must trigger a real turn on the parent (not be dropped).

1. **Nazgul turn 1** — receives the objective from Sauron/human; opens `/spawn`,
   creates the Troll, gives the Troll the objective + the A1–A8 criteria and instructs
   it to split work between two `gpt-5.5` Orcs. → dispatch to Troll.
2. **Troll turn 1** — receives the task; designs the interface contract (`StarField`
   hit-test API, `generateDossier` signature); creates Orc 1 and Orc 2; sends each a
   scoped assignment.
3. **Orc 1 turn 1** — implements `stars.js` + spatial index; reports back. **→ report to Troll.**
4. **Orc 2 turn 1** — implements `dossier.js`; reports back. **→ report to Troll.**
   (Orc 1 and Orc 2 run in parallel; the Troll is likely **still mid-turn** when the
   first report lands — this is the prime failing race.)
5. **Troll turn 2** — processes Orc 1's report; reviews against the contract; if the
   hit-test API is wrong, sends a followup_task to Orc 1.
6. **Troll turn 3** — processes Orc 2's report; reviews; may force rework on Orc 2
   (e.g. non-deterministic description).
7. **Orc 1 turn 2** (if rework) — fixes; reports back. **→ report to Troll.**
8. **Orc 2 turn 2** (if rework) — fixes; reports back. **→ report to Troll.**
9. **Troll turn N** — once both Orcs pass review, reconciles the interface, writes
   `MANIFEST.md`, and reports the completed work + evidence + risks to the Nazgul.
   **→ report to Nazgul.**
10. **Nazgul turn 2** — receives the Troll's report; opens the files and audits against
    A1–A8 itself (does not trust the Troll's claims). If any criterion fails, returns the
    work to the Troll with specific rework. → dispatch back to Troll.
11. **Troll turn N+1** (if Nazgul returned work) — triages the Nazgul's findings, sends
    the responsible Orc a targeted followup, waits, re-reviews, re-reports to Nazgul.
    **→ report to Nazgul.**
12. **Nazgul turn 3** — re-audits only the fixed criteria; writes `ACCEPTANCE.md` with
    per-criterion verdicts + evidence; signs off.

A successful run is **≥ 6 turns with ≥ 3 child→parent report cycles and ≥ 1 forced
rework loop**, and at least one report that arrives while its parent is mid-turn.

## Verification plan: "are child reports going successfully back?"

The verification answers three questions, each with a concrete observable.

### Q1. Did every child→parent report trigger a real turn on the parent?

**Mechanism (post-fix):** a pending-report queue per parent. On `TurnCompleted` of a
child, the report is enqueued for the parent; if the parent is idle it runs immediately,
otherwise it is flushed on the parent's next idle transition.

**Observables to record per report cycle:**
- The parent pane's transcript shows an actual **assistant turn** that begins with the
  "A child pane has reported back. Review the child report below …" prompt (i.e. the
  report became a turn, not just a passive "Child report delivered." info line).
- The child's `TurnCompleted` and the parent's subsequent `TurnStarted` are paired in the
  event log, even when the parent was running at report time (verify via the parent's
  `spawn_parent_reports_by_node` being drained / the parent's transcript showing the
  report text as a user-style input).

**Pass:** every report in the run (Orc→Troll and Troll→Nazgul) has a corresponding
parent turn. **Fail:** any report that only produced a "Child report delivered." info
cell with no following parent turn, and no later flush.

### Q2. Did managers actually act on reports (triage / dispatch / acknowledge), not ignore them?

**Observables:**
- After processing an Orc report, the Troll's turn produces one of: a followup_task /
  send_input to an Orc (rework), an explicit acknowledgement to the Orc, or a note in
  `MANIFEST.md`. A turn that ends with no tool call and no written artifact after a report
  is a "drop" even if the turn ran.
- After the Troll's report, the Nazgul's turn reads files (exec/read) and/or dispatches
  back to the Troll — not a no-op acknowledgement.

**Pass:** ≥ 1 forced-rework loop (Troll sends an Orc back) and the Nazgul performs a
real code audit (file reads) before ACCEPTANCE. **Fail:** any report followed by an empty
parent turn, or the Nazgul signing off without reading the code.

### Q3. Does it hold across multiple turns and the mid-turn race?

**Observable:** repeat the run and specifically check the cycle where Orc 1 and Orc 2
report while the Troll is still in Troll-turn-1. Pre-fix this is where reports die.
Post-fix the Troll must, after its own turn-1 completes, run a turn that consumes both
queued reports and then dispatches followups/reviews.

**Pass:** both parallel Orc reports are processed (no report lost to the race), and the
total turn count matches the expected ≥6-turn trace. **Fail:** the Troll's turn-1
completes and no follow-up turn consumes a report that arrived during it.

### How verification is recorded

A run produces, in `star-map-explorer/`:
- `RUN-LOG.md` — a chronological list of every turn across all four panes with:
  `{pane, turn#, trigger (initial task | report | rework dispatch | audit), outcome}`.
  This is the human-readable trace used to answer Q1–Q3.
- The four panes' transcripts (already persisted by PFTerminal per-pane) are the source of
  truth for "did a report become a turn".
- `ACCEPTANCE.md` (Nazgul) and `MANIFEST.md` (Troll) are the product artifacts that prove
  the work itself shipped; the run-log + transcripts prove the *report-delivery mechanism*
  held.

A run is a **green** run iff: all of A1–A8 pass **and** Q1, Q2, Q3 each pass. A run where
A1–A8 pass but any of Q1–Q3 fail is a **report-delivery regression** — the product shipped
but the orchestration mechanism is broken (the exact historical symptom).

## Harness

A `/spawn`-driven scripted run (TUI automation or a `claude-pane-workflow-suite`-style
harness adapted for native Codex panes) that:
1. Creates the Nazgul (Z.AI), Troll (Vercel fast), and two `gpt-5.5` OpenAI Orcs via `/spawn`.
2. Seeds the Nazgul with the objective + A1–A8 + the workflow above.
3. Lets it run to completion (no human prompts after kickoff) and collects the
   transcripts + run-log.
4. Emits a machine-readable report: per-cycle report-delivery pairing (Q1), per-report
   parent-turn classification (Q2), and the mid-turn-race check (Q3).

> The harness is implemented after the report-delivery fix (pending-report queue +
> flush-on-idle). This document is the **scope** for that work.
