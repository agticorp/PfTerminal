# Hammer Reduction Process

## Complete

- [x] Captured PFTerminal/Codex session evidence showing repeated high-input
  requests after small user nudges.
- [x] Confirmed PFTerminal inherits Codex's prompt assembly, retry, compaction,
  and edit-tool behavior because it is a Codex fork.
- [x] Downloaded and reviewed local OpenCode, Hermes Agent, Kilo Code, and Cline
  source snapshots.
- [x] Compared provider retry handling, context compaction, edit primitives, and
  loop guards across the studied harnesses.

## To Do

- [ ] Add a provider/model/key cooldown circuit breaker so immediate retries
  after `429` do not send another full request.
- [ ] Add a cross-process request lease so multiple PFTerminal agents do not
  hammer the same provider credential at once.
- [ ] Add request preflight telemetry for input tokens, cached tokens, serialized
  request bytes, and provider cooldown state.
- [ ] Add aggressive third-party-provider compaction/pruning defaults before the
  request reaches provider rate or context limits.
- [ ] Add loop guards for repeated identical tool calls and repeated failed edit
  attempts.

Status: current sprint study and implementation plan.

## Why This Exists

PFTerminal is Codex. It is a fork, so the behavior the user sees is inherited
from the Codex runtime unless PFTerminal changes it deliberately.

The problem is not only that GLM 5.2 can struggle with strict `apply_patch`.
There is a second, separate hammering problem: a tiny follow-up message can
trigger another large provider request because the runtime resends the live
conversation context, tool outputs, and instructions until compaction or a new
session cuts the history down.

The session that exposed this showed small prompts such as `continue` and `wat`
arriving after a `429`, while each follow-up request still carried roughly
35k-37k input tokens. The UI displayed:

```text
Token usage: total=270,204 input=263,887 (+ 624,384 cached) output=6,317
```

That display is non-cached input plus output. The rollout trace for the same
thread showed cumulative input of `888,271`, cached input of `624,384`, and
output of `6,317`. In other words, prompt caching reduced the billable-looking
number shown in the UI, but the harness was still sending a large context shape
to the provider on each turn.

## PFTerminal/Codex Evidence

Observed thread:

```text
thread_id: 019ef259-b3ac-7601-86fd-a3cd6ae9bc56
provider: ambient
model: zai-org/GLM-5.2-FP8
cwd: /home/postfiat/repos
rollout: /home/postfiat/.pfterminal/sessions/2026/06/23/rollout-2026-06-23T02-40-25-019ef259-b3ac-7601-86fd-a3cd6ae9bc56.jsonl
```

The important pattern:

| Event | Evidence |
| --- | --- |
| Small user follow-ups still sent large inputs | Later turns sent about `36,414`, `37,211`, and `37,492` input tokens. |
| The rollout file was not huge | The JSONL trace was about 216 KB, so the issue was repeated accumulation, not one massive stored artifact. |
| Provider requests were already large | Local request logs showed recent serialized request bodies around 110-160 KB. |
| Z.AI and Ambient both returned `429` | Both providers rejected initial attempts. The logs did not show `Retry-After` headers. |
| The runtime did not sit in an internal 429 retry loop | Generic provider `429` handling surfaced an error at attempt 0 because provider retry config has `retry_429: false`. |

The structural issue is therefore not "Codex retries 429 too much." The
structural issue is that every manual follow-up can start a fresh full-context
request, and multiple agents or sessions have no shared provider cooldown.

## Codex Mechanics Inherited By PFTerminal

The inherited mechanics that matter for hammer reduction are:

| Area | Current behavior | Source |
| --- | --- | --- |
| Prompt assembly | Each turn builds model input from cloned live history through `for_prompt(...)`. | `codex-rs/core/src/session/turn.rs` |
| Retry policy | Generic provider config sets `retry_429: false`; transport and 5xx are retryable, 429 generally is not. | `codex-rs/model-provider-info/src/lib.rs`, `codex-rs/codex-client/src/retry.rs` |
| Stream retry | Dropped streams can be retried separately from initial HTTP 429 handling. | `codex-rs/core/src/responses_retry.rs` |
| Error mapping | Non-OpenAI 429 maps to a retry-limit style provider error unless it matches OpenAI usage-limit payloads. | `codex-rs/codex-api/src/api_bridge.rs` |
| Auto compaction | Compaction is tied to token-window pressure, not to provider 429s or repeated small follow-ups. | `codex-rs/core/src/session/turn.rs` |
| Token display | UI total uses non-cached input plus output, while cumulative traces still record cached input separately. | `codex-rs/protocol/src/protocol.rs` |
| Edit tool choice | PFTerminal now has structured edit/write for selected profiles, otherwise Codex-native models can keep strict `apply_patch`. | `codex-rs/core/src/tools/spec_plan.rs` |

This explains why the user can have a paid provider plan and still hit `429`.
Plans do not remove per-minute request, token, burst, or concurrency limits.
Repeated full-context calls can exhaust a burst bucket even when total daily or
monthly quota is healthy.

## Harness Comparison

| Harness | Context strategy | Retry/rate-limit behavior | Edit/tool-loop behavior | Lesson for PFTerminal |
| --- | --- | --- | --- | --- |
| PFTerminal/Codex | Sends full live history until compaction thresholds are reached. Prompt caching helps cost but does not eliminate large request bodies. | Generic provider `429` is surfaced to the user, but there is no shared cooldown preventing the next manual request. | Codex-native path uses strict `apply_patch`; PFTerminal added structured edit/write for GLM-style profiles. | Add preflight, cooldown, and cross-process rate state around the inherited Codex loop. |
| OpenCode | Shows context usage, reserves usable context, compacts before overflow, and prunes tool outputs. | Retries with `Retry-After`/`retry-after-ms` support and visible retry status. | Primary edit tool is structured string replacement; `apply_patch` is optional and model-gated. | Copy the model-specific edit primitive and user-visible context/retry state. |
| Hermes Agent | Defaults to compression at 50 percent of context, protects first and recent turns, targets 20 percent summary ratio, and prunes old tool outputs. | Uses jittered backoff, rate-limit header tracking, and concurrency caps for burst control. | `execute_code` keeps intermediate tool results out of LLM context and caps stdout/stderr. | Use aggressive default compaction and artifact old tool output outside the prompt. |
| Kilo Code | Productizes compaction controls: auto compaction, threshold percent, pruning, keep/buffer settings, and request-byte pruning. | Retries 429 with `Retry-After` support and bounded backoff. | Adds doom-loop detection for repeated identical tool calls. | Add settings and hard guards instead of relying only on model discipline. |
| Cline | CLI exposes compaction modes: `basic`, `agentic`, and `off`; VS Code path uses fixed safety buffers and context-window detection. | Retries 429 with `Retry-After`, `x-ratelimit-reset`, and exponential backoff. | Uses one-tool-per-message discipline, repeated-tool-call notices, and provider-specific caps such as Cerebras max-token headroom. | Keep a simple non-LLM compaction fallback and provider-specific output caps. |

## OpenCode Reference

Local source:

```text
path: /home/postfiat/repos/opencode-current
remote: https://github.com/anomalyco/opencode.git
commit: f48f24ec4e1e26cc32c4d4953497fe2734c61ee1
```

OpenCode is relevant because it solves the edit-friction side of the same
problem. It registers `edit`, `write`, and `apply_patch`, then gates patch use
by model family. Non-GPT and OSS-style models can use structured `edit`/`write`
instead of being forced through a strict patch grammar.

Important source areas:

| Source | What it shows |
| --- | --- |
| `packages/opencode/src/tool/registry.ts` | Model-gated exposure of `apply_patch` versus `edit`/`write`. |
| `packages/opencode/src/tool/edit.ts` | JSON edit tool with `filePath`, `oldString`, `newString`, and `replaceAll`. |
| `packages/opencode/src/session/retry.ts` | Retry policy with `Retry-After`, `retry-after-ms`, backoff, and visible retry state. |
| `packages/opencode/src/session/overflow.ts` | Usable-context calculation with reserved output/compaction buffer. |
| `packages/opencode/src/session/compaction.ts` | Recent-turn protection and old tool-output pruning. |

Best practice to import: make simple structured edits the default for
non-Codex-native models, and make context/retry state visible before another
large request is sent.

## Hermes Agent Reference

Local source:

```text
path: /home/postfiat/repos/agent-harness-study/hermes-agent
remote: https://github.com/NousResearch/hermes-agent.git
commit: bb7ff7d
```

Hermes Agent is the strongest reference for hammer reduction itself.

Important source areas:

| Source | What it shows |
| --- | --- |
| `cli-config.yaml.example` | Compression defaults: enabled, threshold `0.50`, target ratio `0.20`, protect first 3 turns, protect recent turns, and session-search concurrency caps. |
| `agent/context_compressor.py` | Head/tail protection, old tool-output pruning, structured summaries, iterative updates, and failure cooldown. |
| `agent/conversation_compression.py` | Compression locks and threshold validation against the auxiliary model context window. |
| `agent/retry_utils.py` | Jittered backoff to avoid synchronized retry spikes. |
| `agent/rate_limit_tracker.py` | Provider rate-limit header parsing and `/usage` style visibility. |
| `tools/code_execution_tool.py` | Tool-result isolation and stdout/stderr caps so intermediate execution chatter does not all enter the LLM context. |

Best practice to import: compact much earlier for third-party providers,
deduplicate/prune old tool output, cap burst concurrency, and show rate-limit
state explicitly.

## Kilo Code Reference

Local source:

```text
path: /home/postfiat/repos/agent-harness-study/kilocode
remote: https://github.com/Kilo-Org/kilocode.git
commit: 0f55066
```

Kilo Code is useful because it packages these controls as user-facing product
settings instead of burying them in the harness.

Important source areas:

| Source | What it shows |
| --- | --- |
| `packages/kilo-vscode/webview-ui/src/components/settings/ContextTab.tsx` | Auto compaction, compaction threshold, and prune-old-outputs settings. |
| `packages/core/src/config/compaction.ts` | Compaction config fields: auto, prune, keep, and buffer. |
| `packages/opencode/src/session/prompt.ts` | Request-byte pruning, compaction attempt caps, media stripping, and post-summary trimming. |
| `packages/opencode/src/session/compaction.ts` | Payload-limit pruning and post-compaction tool-output pruning. |
| `packages/opencode/src/session/processor.ts` | Doom-loop detection for repeated identical tool calls. |
| `packages/kilo-vscode/src/util/retry.ts` | Retry-After parsing and bounded retry schedule. |

Best practice to import: expose hammer-reduction controls in settings and add
request-size gates, not just token-window gates.

## Cline Reference

Local source:

```text
path: /home/postfiat/repos/agent-harness-study/cline
remote: https://github.com/cline/cline.git
commit: 19d4248
```

Cline is open source for the CLI and VS Code extension. Its JetBrains plugin is
not part of the open-source study.

Important source areas:

| Source | What it shows |
| --- | --- |
| `apps/cli/README.md` | CLI compaction modes: `basic`, `agentic`, and `off`; default is `basic`. |
| `apps/cli/src/utils/compaction-mode.ts` | CLI compaction config construction. |
| `apps/vscode/src/core/context/context-management/ContextManager.ts` | Compaction decisions based on previous API token usage and context-window info. |
| `apps/vscode/src/core/context/context-management/context-window-utils.ts` | Fixed safety buffers below advertised context length. |
| `apps/vscode/src/core/api/retry.ts` | Retry-After and rate-limit reset parsing. |
| `apps/vscode/src/core/api/providers/cerebras.ts` | Provider-specific max-token cap to preserve rate-limit headroom. |
| `apps/vscode/src/core/prompts/responses.ts` | Duplicate-read notices, repeated-tool-call notices, and guidance to break large edits into smaller chunks. |

Best practice to import: provide a cheap basic compaction fallback, leave a
large safety buffer below advertised context, and tune per-provider max output
so reserved tokens do not exhaust rate buckets.

## Provider Best Practices

| Provider class | Practice |
| --- | --- |
| OpenAI/Codex-native | Keep strict `apply_patch` available for models trained to use it. Still track serialized bytes and non-cached/cached input so prompt caching does not hide oversized turns. |
| Z.AI/GLM 5.2 | Prefer structured edit/write. Do not immediately retry `429` without `Retry-After`; if no header exists, apply local cooldown. Use smaller smoke-test prompts and compact before large context turns. |
| Ambient GLM | Treat as GLM-class for edit tools and third-party-provider cooldowns. Add provider/model/key cooldown shared across tmux workers. |
| OpenRouter and generic OpenAI-compatible gateways | Assume rate-limit headers may be missing or provider-specific. Strip unsupported tool types, cap context, and set local exponential cooldown on 429. |
| Fast inference providers such as Cerebras/Groq | Cap max output tokens because some providers reserve quota based on requested maximum output, not only actual output. |
| Local providers such as Ollama/LM Studio | They avoid paid API 429s, but context bloat still hurts latency. Keep pruning and loop guards enabled. |

## Target PFTerminal Mechanics

### Provider Cooldown

Add a local provider-state table or equivalent durable store keyed by provider,
model, and credential fingerprint:

```text
provider_request_state(
  provider_id,
  model,
  key_fingerprint,
  cooldown_until,
  last_status,
  last_request_id,
  last_input_tokens,
  last_cached_input_tokens,
  last_request_bytes,
  updated_at
)
```

Before a request, PFTerminal should check this state. If a cooldown is active,
it should not send the request. It should show the user the wait time and offer
clear options: wait, compact, switch model/provider, or start a fresh thread.

On `429`, PFTerminal should parse `Retry-After`, `retry-after-ms`, and
provider reset headers. If no usable header exists, set a local exponential
cooldown such as 30s, 60s, 120s, capped at 5m.

### Cross-Process Lease

Multiple PFTerminal workers can run from tmux or separate shells. They need a
shared request lease keyed by provider/model/key so two agents do not send
large requests through the same credential at the same time.

V0 should default to one active request per provider/model/key. More aggressive
parallelism can be a config option after telemetry proves it is safe.

### Request Preflight

Before sending to the provider, compute and display:

- last user message size;
- estimated input tokens;
- cached input tokens from the previous response, when known;
- serialized request bytes;
- provider/model/key fingerprint;
- active cooldown or lease holder;
- whether compaction is recommended or required.

If the user sends a tiny follow-up while the current context is still tens of
thousands of tokens, the UI should make that obvious before another provider
call goes out.

### Compaction And Pruning

For third-party providers, default to earlier compaction than the Codex-native
path:

- compact around 50-60 percent of usable model context;
- protect the initial task/instructions;
- protect recent turns by token budget, not only message count;
- prune or summarize older tool outputs before asking an LLM to compact;
- store raw old outputs outside prompt context with file path and hash;
- use request-byte pruning if the serialized body crosses a configured limit;
- cap compaction attempts per turn.

This should be a provider profile setting, not a product decision about which
model is "better." Some providers can handle the larger prompt shape; some
cannot.

### Loop Guards

Add hard guards for:

- repeated identical tool calls;
- repeated failed edit attempts against the same file and same target text;
- repeated read-only shell commands when the user set a review cap;
- repeated immediate provider calls after a provider-side 429.

When a guard trips, the runtime should stop and report the specific loop, not
ask the provider to reason through another full context copy.

## Acceptance Criteria

- After a provider `429`, typing `continue` or `wat` does not send another
  provider request until cooldown expires or the user explicitly overrides.
- Two PFTerminal processes using the same provider/model/key share cooldown and
  active-request state.
- A tiny user message with a 35k-token live context triggers preflight
  visibility and optional compaction before request dispatch.
- Old tool outputs are pruned or summarized before they dominate the prompt.
- GLM-class models use structured edit/write by default; strict `apply_patch`
  remains available for Codex-native models.
- The UI can answer: what provider was hit, how large the request was, how much
  was cached, why it was blocked or sent, and when it is safe to retry.

## Source Snapshot Register

| Harness | Local path | Remote | Commit |
| --- | --- | --- | --- |
| OpenCode | `/home/postfiat/repos/opencode-current` | `https://github.com/anomalyco/opencode.git` | `f48f24ec4e1e26cc32c4d4953497fe2734c61ee1` |
| Hermes Agent | `/home/postfiat/repos/agent-harness-study/hermes-agent` | `https://github.com/NousResearch/hermes-agent.git` | `bb7ff7d` |
| Kilo Code | `/home/postfiat/repos/agent-harness-study/kilocode` | `https://github.com/Kilo-Org/kilocode.git` | `0f55066` |
| Cline | `/home/postfiat/repos/agent-harness-study/cline` | `https://github.com/cline/cline.git` | `19d4248` |

