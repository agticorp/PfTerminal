# Claude Code Integration Completion Spec

Status: implemented and verified for the current PFTerminal pane path.

This document defines the completion bar for the Claude Code integration in
PFTerminal. The current Ambient-backed headless pane proves that `claude -p`
can be launched and that one local bridge path can carry a simple tool loop.
That is not enough to call the feature complete.

## Problem Statement

The earlier implementation failed the product expectation in four concrete
ways:

- It has not been tested across the provider set exposed in the UI.
- It can hit `error_max_turns` after one apparently simple request, which means
  it cannot yet be trusted for substantive Claude Code work.
- It does not give the user a useful per-turn audit trail inside the UX.
- It was shipped with too little cross-sectional testing, so a user found a
  runtime failure immediately after login.

Completion means a user can open `/panes`, create or select a Claude Code pane,
send normal coding/review prompts, see what happened, and continue the
conversation without debugging provider plumbing.

## Completion Gates

The feature is not complete unless every gate below has current evidence.

- [x] Provider matrix tested through the same pane backend used by `/panes`,
  not only through isolated curls.
- [x] Multi-turn pane sessions work for substantive tasks, not only marker
  recall.
- [x] Claude Code tool loops can perform real repo work without terminating at
  `error_max_turns` under normal prompts.
- [x] Turn artifacts are visible and inspectable from the UI.
- [x] Every turn has a bounded audit record: provider, model, pane id, session
  id, command shape, artifact path, duration, usage, terminal reason, and
  whether tools were used.
- [x] Provider errors render as actionable pane messages, not raw or misleading
  success text.
- [x] Failed turns preserve enough diagnostic evidence to debug without leaking
  secrets into chat history or artifacts.
- [x] Regression tests cover success, provider failure, max-turn failure,
  multi-turn resume, and artifact parsing.
- [x] A live smoke script can be run by maintainers before release and records
  its results in one place.

## Provider Matrix

Each provider profile exposed in `/panes` must be tested through the actual
PFTerminal pane path.

| Provider pane | Required credential | Required result |
| --- | --- | --- |
| Claude Code - GLM 5.2 Ambient | `provider/ambient_api_key` | Create pane, run a tool loop, continue the same session, render audit. |
| Claude Code - GLM 5.2 Z.AI | `provider/zai_api_key` | Create pane or fail with a precise provider error; no hang, no false success. |
| Claude Code - GLM 5.2 Baseten | `provider/baseten_api_key` | Create pane or fail with a precise provider error; no hang, no false success. |
| Claude Code - OpenRouter | `provider/openrouter_api_key` | Create pane or fail with a precise provider error; no hang, no false success. |
| Claude Code - Claude Plan | Claude's native auth | Create pane using native Claude auth, or clearly explain missing native auth. |

If a provider cannot support Claude Code headless today, the UI must mark that
profile unavailable or experimental instead of presenting it as working.

Current provider smoke result:

```text
Claude pane smoke: 5 passed, 5 checked
report: /home/postfiat/.pfterminal/panes/smoke-reports/claude-pane-smoke-1782391716.json

ambient: passed
zai: passed
baseten: passed
openrouter: passed
claude-plan: passed
```

## Substantive Work Tests

Marker prompts are useful as a smoke test, but they do not prove the feature.
The completion suite must include prompts that force Claude Code to perform
real work:

- Read-only code review of a target file or directory.
- Repository survey that uses at least one filesystem tool and returns a
  structured summary.
- Small edit task in a disposable fixture repo, followed by a diff assertion.
- Multi-turn continuation where the second prompt depends on files or facts
  discovered in the first turn.
- Failure-path prompt that intentionally exceeds a turn budget and verifies the
  UX reports max-turn termination as incomplete work.

The observed failure:

```text
Claude returned an error result: error_max_turns;
terminal_reason=max_turns; Reached maximum number of turns (8)
```

is a blocking bug until the pane can either finish normal coding prompts or
clearly ask the user to continue the same pane without losing context.

Current result:

- Claude pane max turns were raised from 8 to 24 for normal pane work.
- `error_max_turns` is parsed as `max-turn-pause`, not a fake success and not
  an opaque string error.
- The pane preserves the Claude session id and tells the user to type
  `continue` in the same pane to resume.
- A live Ambient code-review test passed against
  `codex-rs/tui/src/claude_panes.rs`.
- A live Ambient disposable edit test passed and asserted the fixture file was
  actually changed.

## Max-Turn Policy

`--max-turns 8` is a guardrail, not a product answer.

The pane backend must distinguish:

- successful completion;
- provider failure;
- permission/tool failure;
- max-turn pause with resumable state;
- max-turn failure where no useful answer was produced.

For max-turn pause, the UI should keep the pane active and offer a visible
continue action. It must not render the turn as successful if Claude returned
`error_max_turns`.

## Auditability Requirements

Every Claude pane turn must produce an audit card or equivalent inspectable
record in the PFTerminal UI.

Minimum fields:

- pane name and pane id;
- provider profile and model;
- Claude session id;
- turn number;
- start time, end time, duration;
- command mode (`--session-id` or `--resume`);
- max-turn setting;
- artifact path;
- stream-json parse status;
- tool-use count and tool names;
- usage fields reported by Claude;
- terminal reason;
- final result status: success, provider error, max-turn pause, max-turn
  failure, or parse failure.

The raw JSONL artifact may remain on disk, but the user must not have to hunt
for it manually after an error. The UI should expose the artifact path and a
short summary.

Current result: every Claude pane turn writes `turn-NNNN.jsonl` plus
`turn-NNNN.audit.json`, and the UI completion/error message includes both
paths. The latest audit path is also included in the `/panes` picker metadata.

## Secret Handling

Provider secrets must remain vault-owned.

- Pane settings may use `apiKeyHelper`.
- Raw provider keys must not be written into pane settings, command arguments,
  chat history, JSONL artifacts, logs, or docs.
- Tests must assert that a revealed provider key is absent from pane settings
  and turn artifacts.

## UX Requirements

The user-facing behavior must feel like a PFTerminal pane, not a pasted subprocess
log.

- `/panes` shows user panes and agent panes separately.
- Selecting a Claude pane changes the active footer label.
- User prompts route to the active Claude pane until the user switches away.
- Successful turns render assistant output in normal history.
- Failed turns render a concise error with provider, terminal reason, and next
  action.
- Audit details are accessible from the turn, not buried only in files.
- The user can continue a paused/max-turn pane without reselecting credentials.

## Required Test Commands

Before claiming completion, run and record:

```bash
cargo test -p codex-tui claude_panes

PFTERMINAL_LIVE_CODEX_HOME=/home/postfiat/.pfterminal \
  cargo test -p codex-tui claude_panes::tests::live_ambient_bridge_runs_claude_headless_for_two_turns -- --ignored --nocapture

PFTERMINAL_LIVE_CODEX_HOME=/home/postfiat/.pfterminal \
  cargo test -p codex-tui claude_panes::tests::live_ambient_bridge_runs_claude_tool_loop -- --ignored --nocapture

cargo clippy -p codex-tui -p codex-cli --tests
cargo build -p codex-cli --bin pfterminal
.venv-docs/bin/mkdocs build --strict
```

Add a live matrix runner before release:

```bash
pfterminal claude-pane-smoke --providers ambient,zai,baseten,openrouter,claude-plan
```

That runner should create a machine-readable report under
`$CODEX_HOME/panes/smoke-reports/` and a human-readable summary that can be
pasted into the sprint doc.

Additional live tests now required and passing:

```bash
PFTERMINAL_LIVE_CODEX_HOME=/home/postfiat/.pfterminal \
  cargo test -p codex-tui claude_panes::tests::live_ambient_bridge_runs_substantive_code_review -- --ignored --nocapture

PFTERMINAL_LIVE_CODEX_HOME=/home/postfiat/.pfterminal \
  cargo test -p codex-tui claude_panes::tests::live_ambient_bridge_runs_disposable_edit_task -- --ignored --nocapture
```

## Release Rule

Do not call the Claude Code integration complete unless:

- every supported provider profile is either verified working or explicitly
  marked unavailable/experimental in the UI;
- at least one provider completes a substantive repo task through `/panes`;
- max-turn behavior is resumable and accurately rendered;
- turn audit records are visible from the UX;
- all required tests and live smoke checks have fresh passing output.

Current completion evidence:

- `cargo test -p codex-tui claude_panes --no-fail-fast`: 16 passed, 4 ignored.
- Live Ambient two-turn resume test: passed.
- Live Ambient tool-loop test: passed.
- Live Ambient substantive code-review test: passed.
- Live Ambient disposable edit test: passed.
- `pfterminal claude-pane-smoke --providers ambient,zai,baseten,openrouter,claude-plan --cwd /home/postfiat/repos/PfTerminal`: 5 passed, 5 checked.
