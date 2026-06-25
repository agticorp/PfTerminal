# Claude Headless Panes

Status: scope proposal, not implemented.

## Complete

- [x] Verified that current Codex/PFTerminal `/agent` panes are logical agent
  threads, not OS tmux panes.
- [x] Verified local Claude Code supports `--settings`,
  `--dangerously-skip-permissions`, `--permission-mode bypassPermissions`,
  `--session-id`, `--resume`, `--output-format json`, and
  `--output-format stream-json`.
- [x] Live-tested two-turn Claude Code headless continuity:
  first turn created a fixed session id, second turn resumed that id and
  correctly recalled the previous marker.
- [x] Confirmed the second resumed turn reported cache reuse
  (`cache_read_input_tokens`), which is the behavior needed for a pane-like
  session rather than a stateless one-shot call.

## To Do

- [ ] Implement a pane registry for user-owned panes separate from Codex
  subagent thread navigation.
- [ ] Add `/panes` with `User Panes` and `Agent Panes` sections.
- [ ] Add a `ClaudeHeadlessPane` backend that invokes Claude Code through
  headless JSON/stream-json mode.
- [ ] Add a PFTerminal vault auth helper for Claude pane provider credentials.
- [ ] Generate pane-local Claude settings from vault-backed provider profiles.
- [ ] Add per-pane request locking so a Claude session cannot receive two turns
  concurrently.
- [ ] Add tests for pane creation, provider settings generation, resume
  behavior, and redaction boundaries.

## Goal

PFTerminal should let users switch between the main Codex/PFTerminal session,
Claude Code sessions, and Codex subagent threads without embedding a full
terminal multiplexer.

The target UX is:

```text
/panes

User Panes
> Codex - Main - OpenAI
  Claude Code - GLM 5.2 Z.AI
  Claude Code - GLM 5.2 Baseten
  Claude Code - OpenRouter
  Claude Code - Claude Plan
  + New Pane

Agent Panes
  Main [default]
  Linnaeus [explorer]
  Orc [worker]
```

Selecting a user pane changes the active interactive surface. Selecting an
agent pane reuses the existing Codex `/agent` thread switching behavior.

## Decision

Use **Claude Code headless mode**, not embedded tmux, as the first
implementation path.

Embedding Claude's interactive TUI would require PFTerminal to become a
terminal multiplexer: ANSI rendering, resize handling, scrollback, alternate
screen behavior, keyboard forwarding, mouse forwarding, lifecycle management,
and prompt/status detection. That is the wrong first abstraction.

Headless Claude Code gives PFTerminal structured JSON events that can be
rendered into native history cells and later exposed as a Codex tool. The
tradeoff is that PFTerminal owns the UI instead of reusing Claude's terminal
UI.

## Verified Headless Session Behavior

The local test used a fixed Claude session id:

```bash
claude -p \
  --output-format json \
  --session-id 11111111-2222-4333-8444-555555555555 \
  --tools "" \
  --permission-mode bypassPermissions \
  'This is a continuity test. Remember the marker: PFT-PANE-721. Reply with exactly: stored.'
```

The first call succeeded and returned the same session id in JSON output.

A second call using `--session-id` failed with:

```text
Error: Session ID 11111111-2222-4333-8444-555555555555 is already in use.
```

The correct continuation path is:

```bash
claude -p \
  --output-format json \
  --resume 11111111-2222-4333-8444-555555555555 \
  --tools "" \
  --permission-mode bypassPermissions \
  'What marker did I ask you to remember in the previous turn? Reply with only the marker.'
```

That call returned:

```text
PFT-PANE-721
```

and reported cached prior context. Therefore the pane backend should create a
session once and resume it on later turns.

## Architecture

Add a user-pane layer above the existing Codex agent-thread picker.

```rust
enum UserPaneBackend {
    CodexThread(ThreadId),
    ClaudeHeadless(ClaudePaneSession),
}

struct ClaudePaneSession {
    pane_id: Uuid,
    claude_session_id: Option<Uuid>,
    provider_profile: ClaudeProviderProfile,
    cwd: PathBuf,
    settings_path: PathBuf,
    status: PaneStatus,
}
```

The current `/agent` implementation remains responsible for Codex subagent
threads. `/panes` should compose two data sources:

- user panes from the new pane registry;
- agent panes from existing `AgentNavigationState`.

This keeps "user-owned work surfaces" and "Codex-spawned worker threads"
separate instead of overloading `/agent`.

## Runtime Flow

First message to a Claude pane:

```text
PFTerminal
  -> resolve provider profile
  -> read credential from vault
  -> write pane-local Claude settings
  -> run claude -p --output-format stream-json --session-id <pane_uuid>
  -> parse JSON events
  -> render native PFTerminal history cells
```

Later messages to the same pane:

```text
PFTerminal
  -> acquire pane lock
  -> refresh settings/auth helper
  -> run claude -p --output-format stream-json --resume <claude_session_id>
  -> parse JSON events
  -> release pane lock
```

The pane lock is required because Claude rejects a session when another process
is already using the same session id.

## Vault Boundary

Claude Code should not receive direct vault access by default.

PFTerminal owns the vault and exposes credentials to Claude only as a runtime
provider credential for the selected pane. The credential must not be written
to chat history, persisted in global `~/.claude/settings.json`, or included in
the Codex transcript.

Preferred auth path:

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "https://api.z.ai/api/anthropic",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "glm-5.2[1m]"
  },
  "apiKeyHelper": "pfterminal vault auth-helper provider/zai_api_key"
}
```

If `apiKeyHelper` is not reliable across supported Claude Code builds, use a
pane-local settings file plus a child-process environment assembled from the
vault at launch time. That is less ideal, but still avoids mutating global
Claude config.

Do not expose a raw `vault_get_secret` MCP tool to Claude in v0. MCP tool
results enter the model/tool transcript. A later Claude plugin can expose safe
metadata actions such as list labels, describe credentials, or request a
provider profile, but raw secret reads should remain host-owned.

## Provider Profiles

| Pane profile | Claude base URL | Credential source | Notes |
| --- | --- | --- | --- |
| Claude Plan | native Claude auth | Claude's own login/keychain | No PFTerminal provider key required. |
| Z.AI GLM 5.2 | `https://api.z.ai/api/anthropic` | `provider/zai_api_key` | Best first third-party target because Z.AI documents Claude Code usage through an Anthropic-compatible endpoint. |
| Baseten GLM 5.2 | `https://inference.baseten.co` | `provider/baseten_api_key` | Baseten documents Model APIs with Anthropic Messages. Needs live validation with GLM 5.2 model naming and auth headers. |
| OpenRouter | `https://openrouter.ai/api` | `provider/openrouter_api_key` | OpenRouter documents a Claude Code integration through an Anthropic-compatible skin. |
| Ambient GLM 5.2 | unknown | `provider/ambient_api_key` | Only direct if Ambient exposes an Anthropic-compatible Claude endpoint. Otherwise requires a local Anthropic-to-Chat proxy. |

## Rendering Contract

Claude output should be mapped into PFTerminal-native cells, not dumped as raw
terminal text.

| Claude event/source | PFTerminal rendering |
| --- | --- |
| assistant text | assistant history cell |
| tool call start | tool-call history cell |
| tool result | bounded tool-result cell |
| error | error history cell |
| usage | pane footer/status metadata |
| session id | pane registry state |

If a Claude turn produces a large transcript, store the raw JSONL under a pane
artifact path and return/render a bounded summary. Do not inject full Claude
transcripts into the active Codex context unless the user explicitly asks.

## Codex-To-Claude Delegation

After `/panes` works interactively, expose Claude panes to Codex as a bounded
tool:

```json
{
  "pane_id": "claude-zai-1",
  "prompt": "Review this diff for correctness and missing tests.",
  "mode": "review"
}
```

Return a bounded result:

```json
{
  "pane_id": "claude-zai-1",
  "status": "completed",
  "summary": "Found one likely bug in auth fallback.",
  "changed_files": [],
  "artifacts": [
    {
      "type": "transcript",
      "path": ".pfterminal/panes/claude-zai-1/turn-004.jsonl"
    }
  ]
}
```

This lets Codex coordinate with Claude without pretending Claude Code is a
normal Codex model provider.

## Implementation Plan

### P0: Headless Probe Harness

Add a local probe command or test utility that verifies:

- create with `--session-id`;
- resume with `--resume`;
- output parses as JSON/JSONL;
- repeated same-session concurrent use fails and is handled by a lock;
- provider settings can be supplied without editing `~/.claude/settings.json`.

### P1: Vault Auth Helper

Add:

```bash
pfterminal vault auth-helper <label>
```

The helper prints only the requested secret to stdout for Claude's auth helper
contract and logs no secret material. It should fail closed when the vault is
locked, the label is missing, or the label is not an allowed provider
credential for the pane profile.

### P1: Pane Registry And `/panes`

Add a registry for user panes:

- stable pane id;
- title;
- backend type;
- cwd;
- provider profile;
- Claude session id, when known;
- status;
- latest usage summary;
- artifact path.

Add `SlashCommand::Panes`, `AppEvent::OpenPanePicker`, and `AppEvent` variants
for selecting and creating panes. Reuse the existing `ListSelectionView`
surface, but render grouped rows with `User Panes` and `Agent Panes`.

### P1: Claude Headless Backend

Implement the backend that:

- writes pane-local settings;
- resolves provider credentials through the vault;
- invokes Claude Code headless;
- parses JSON/stream-json;
- maps events to PFTerminal history cells;
- stores raw JSONL artifacts outside the active chat context.

### P2: Codex Tool Bridge

Expose a bounded `claude_code_exec` tool after interactive panes are stable.
The tool should return summary and artifact references, not raw full
transcripts.

### P3: Claude Plugin

Build a Claude plugin only after the host-owned vault/helper path works. The
plugin can provide metadata and user-approved actions, but should not expose
raw secrets to Claude model context.

## Acceptance Criteria

- `/panes` opens a picker with `User Panes` and `Agent Panes`.
- A Z.AI Claude pane can be created from a vault-stored `provider/zai_api_key`
  without editing `~/.claude/settings.json`.
- A two-turn Claude pane remembers prior context through `--resume`.
- The pane backend prevents concurrent sends to the same Claude session.
- Raw provider keys do not appear in PFTerminal chat history, Claude transcript
  rendering, logs, or command argv.
- Agent panes continue to use existing `/agent` behavior.
- The implementation can be disabled without changing core Codex subagent
  behavior.

## Risks

- Claude Code CLI flags and settings behavior can change. The backend must
  fail with a clear diagnostic when a required flag is missing.
- Provider-specific Anthropic-compatible endpoints may differ in header,
  model-name, and caching behavior.
- `apiKeyHelper` support must be validated on every supported Claude Code
  version; otherwise use child-process env injection as a fallback.
- Raw Claude stream-json may include large tool outputs. Store artifacts and
  render bounded summaries.
- The first implementation must not become a hidden terminal multiplexer. If
  full interactive Claude TUI embedding is needed later, treat it as a separate
  PTY/tmux feature.

## References

- Claude Code settings: <https://code.claude.com/docs/en/settings>
- Claude Code CLI reference: <https://code.claude.com/docs/en/cli-reference>
- Z.AI Claude Code integration: <https://docs.z.ai/devpack/tool/claude>
- Baseten Model APIs: <https://docs.baseten.co/inference/model-apis/overview>
- OpenRouter Claude Code integration:
  <https://openrouter.ai/docs/cookbook/coding-agents/claude-code-integration>
