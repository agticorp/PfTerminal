# Authentication

PFTerminal currently has two authentication surfaces:

1. inherited Codex CLI authentication for model-provider access; and
2. the current sprint design for `/vault credential`, an encrypted store for
   crypto keys and spend-bearing API keys.

## Current Sprint

The active credential-store sprint is documented here:

- [Current Sprint: Credential Store](current-sprint/authentication.md)

That sprint page is the product direction for agent-safe provider credentials,
crypto keys, raw-secret boundaries, and encrypted storage.

## Codex CLI Auth

The inherited Codex CLI auth behavior remains relevant for OpenAI/ChatGPT
flows, API-key storage, and provider auth compatibility.

For upstream Codex CLI authentication behavior, see the official Codex auth
documentation:

```text
https://developers.openai.com/codex/auth
```
