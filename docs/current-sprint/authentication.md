# Credential Store

Status: current sprint, v0 design.

The sprint target is PFTerminal `/vault`: a native credential store for crypto
keys and spend-bearing API keys.

## Sprint Decision

Build `/vault` as a credential store:

- users can save credentials under labels;
- the vault is created or initialized automatically when a user logs in or
  starts PFTerminal for the first time;
- credentials are encrypted at rest;
- Ambient, Z.AI GLM 5.2, OpenRouter, and similar provider API keys use the
  same encrypted vault path instead of ad hoc plaintext storage;
- credentials can represent API keys, bearer tokens, crypto private keys, seed
  phrases, keystore JSON, RPC keys, exchange keys, deployment keys, or manual
  secrets;
- raw secret values are hidden by default;
- raw secret reveal/export is explicit.

The core invariant:

> PFTerminal gives users one place to store sensitive credentials instead of
> scattering them through shell history, text files, and ad hoc environment
> variables.

## Why This Exists

Users and agents are already handling real secrets:

- model provider keys;
- coding-plan keys;
- search provider keys;
- GPU cloud keys;
- RPC vendor accounts;
- data APIs, exchanges, observability tools, deployment platforms, and custom
  internal services;
- crypto private keys, seed phrases, and keystore files.

The current failure mode is plaintext operational drift: keys in text files,
shell history risk, unclear labels, and no consistent encrypted storage surface.

## V0 Shape

```text
PFTerminal /vault UX
        |
        v
secure input popout
secret never enters agent context
        |
        v
credential metadata
        |
        v
encrypted credential store
        |
        v
Codex secrets / OS keyring substrate
```

PFTerminal owns the user-facing credential UX. The existing Codex-derived
secrets and keyring code should be reused where it fits.

## Storage Boundary

What v0 requires is:

- a slash-command surface;
- encrypted local storage;
- credential labels and types;
- explicit reveal/export behavior.

The sprint is complete when credentials can be saved, listed, revealed,
exported, and deleted through PFTerminal without using plaintext files as the
primary storage path.

## User Stories

### Startup And Login

- As a PFTerminal user, when I start PFTerminal and need to configure Ambient,
  the Ambient API key should be stored through the encrypted vault path backed
  by Codex secrets and the OS keyring.
- As a PFTerminal user, when I start PFTerminal and need to configure Z.AI GLM
  5.2, the Z.AI API key should be stored through the same encrypted vault path.
- As a PFTerminal user, when I log in or start PFTerminal for the first time,
  the vault should be created or initialized automatically.

### Secure Entry

- As a PFTerminal user, when I type `/vault`, I should get a secure credential
  entry flow.
- As a PFTerminal user, I should be able to enter a password, API key, private
  key, or other secret with a human-readable label such as `openrouter-main`,
  `ambient-main`, or `zai-plan`.
- As a PFTerminal user, the credential-entry UI should behave like a secure
  popout or modal instead of a normal chat message.
- As a PFTerminal user, the raw secret I type into the vault UI must not be
  inserted into the agent context window, transcript, prompt history, or normal
  chat stream.

### Persistent Storage

- As a PFTerminal user, once I store an OpenRouter key, Ambient key, Z.AI key,
  or crypto key, it should persist securely across sessions.
- As a PFTerminal user, stored credentials should be encrypted at rest using the
  Codex encrypted secrets and OS keyring mechanism.
- As a PFTerminal user, each stored credential should have at least a label and
  credential type so I can identify it later without revealing the raw secret.

### Agent Use While Unlocked

- As a PFTerminal user, I should be able to tell the agent to use a particular
  stored key by label, for example `openrouter-main`.
- As a PFTerminal user, when the vault is unlocked, the agent should be able to
  request use of that labeled credential without asking me to paste the key
  again.
- As a PFTerminal user, the agent should be able to use the credential through
  the vault, but the raw secret should not be exposed in the conversation
  context.
- As a PFTerminal user, I should not have to re-enter the same key repeatedly
  during a normal unlocked session.

### Locking

- As a PFTerminal user, unlocking the vault should make stored credentials
  available for the current PFTerminal session.
- As a PFTerminal user, locking or ending the session should prevent further use
  of credentials until the vault is unlocked again.

## Non-Goals

Do not build these in v0:

- automatic transaction execution;
- hosted custody service;
- StakeHub replacement;
- MPC or multisig custody layer;
- policy engine for crypto transactions.

## User Commands

The intended human-facing command surface:

```text
/vault
/vault unlock
/vault lock
/vault credential add
/vault credential list
/vault credential show <label>
/vault credential reveal <label>
/vault credential export <label>
/vault credential delete <label>
```

## Credential Metadata

Each imported credential should have:

- label;
- credential type;
- optional provider name;
- optional notes;
- created timestamp;
- updated timestamp;
- storage backend;
- revocation or recovery notes, when relevant.

Initial credential types:

```text
api-key
bearer-token
basic-auth
oauth-client
crypto-private-key
seed-phrase
keystore-json
rpc-key
exchange-key
deployment-key
manual-secret
```

## Existing Codex Storage

Current Codex already has storage components:

- `codex-rs/secrets` stores named secrets in encrypted local files.
- `codex-rs/keyring-store` wraps OS keyring storage.
- primary Codex auth can use encrypted auth storage.
- provider API keys currently use the thinner provider auth path.

V0 should expose a user-facing credential command surface around the storage
substrate. It is not a replacement for Codex login.

## Implementation Phases

| Phase | Deliverable | Exit Criteria |
| --- | --- | --- |
| 0 | Automatic vault initialization on login/startup. | A new user has a vault available before adding the first credential. |
| 1 | Secure `/vault credential add` input flow. | A user can add a labeled secret without the raw value entering the agent context or transcript. |
| 2 | Local encrypted storage backend. | Credentials persist across sessions using Codex encrypted secrets and OS keyring support. |
| 3 | Lock/unlock session behavior. | The agent can use labeled credentials while the vault is unlocked, without repeated secret entry. |
| 4 | Provider-key integration. | Ambient, Z.AI GLM 5.2, and OpenRouter-style API keys can be stored and used from the vault path. |

## Source

- current sprint pasted specification, revised to use `/vault` terminology
- `codex-rs/secrets`
- `codex-rs/keyring-store`
- `codex-rs/tui/src/bottom_pane/approval_overlay.rs`
