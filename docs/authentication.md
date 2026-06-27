# Authentication And Vault

PFTerminal has three credential surfaces:

1. OpenAI Codex account login for the `openai` provider;
2. provider API keys for Ambient, Z.AI, OpenRouter, Baseten, Vercel, and similar
   providers; and
3. the encrypted `/vault` credential store for provider keys and other
   user-managed secrets.

OpenAI Codex account login uses device auth from `/providers` or the inherited
`pfterminal login` command. Provider keys entered through PFTerminal onboarding
or `/providers` are written to the vault.

## OpenAI Codex Account

Use `/providers` and select:

```text
Provider: OpenAI Codex Account
```

PFTerminal starts a device-code login, shows the verification URL and one-time
code, and stores the resulting Codex/OpenAI account auth in the configured
PFTerminal home.

Installed `pfterminal` launchers and the source-built `pfterminal` binary
default `CODEX_HOME` to `$HOME/.pfterminal`. To override that location, set:

```bash
export CODEX_HOME="${PFTERMINAL_HOME:-$HOME/.pfterminal}"
```

That keeps PFTerminal account auth, vault data, sessions, and logs separate
from a stock Codex install that uses `$HOME/.codex`.

## Provider Keys

Built-in providers use these key names:

| Provider   | Provider id  | Key name             | Vault label                   |
| ---------- | ------------ | -------------------- | ----------------------------- |
| Ambient    | `ambient`    | `AMBIENT_API_KEY`    | `provider/ambient_api_key`    |
| Z.AI       | `zai`        | `ZAI_API_KEY`        | `provider/zai_api_key`        |
| OpenRouter | `openrouter` | `OPENROUTER_API_KEY` | `provider/openrouter_api_key` |
| Baseten    | `baseten`    | `BASETEN_API_KEY`    | `provider/baseten_api_key`    |
| Vercel     | `vercel`     | `AI_GATEWAY_API_KEY` | `provider/ai_gateway_api_key` |

Provider key resolution checks the encrypted vault first. Legacy
`provider_auth.json` is still read for migration compatibility, and a successful
vault write removes the migrated plaintext key when possible.

Environment variables are still supported for temporary shells and automation:

```bash
export AMBIENT_API_KEY="..."
export ZAI_API_KEY="..."
export OPENROUTER_API_KEY="..."
export BASETEN_API_KEY="..."
export AI_GATEWAY_API_KEY="..."
```

For normal interactive use, store keys through onboarding or `/vault` so they
are encrypted at rest.

## Vault Storage

The vault is backed by the Codex managed-secrets store:

- encrypted file: `$CODEX_HOME/secrets/local.age`;
- passphrase storage: OS keyring when available;
- fallback: local `0600` keyring fallback file only for the vault passphrase on
  keyring-less hosts;
- metadata: labels, types, providers, and timestamps are listable without
  revealing raw secrets.

The vault is global to the PFTerminal home directory, so stored credentials are
available from any working directory that uses the same `CODEX_HOME`.

## Using `/vault`

Open the vault action menu:

```text
/vault
```

Useful commands:

```text
/vault list
/vault show provider/zai_api_key
/vault credential add
/vault credential delete provider/openrouter_api_key
```

`/vault credential add` opens a masked entry view. Do not type raw secrets as
chat text. The secure entry path keeps secrets out of prompt history, transcript
history, and model context.

`/vault show <label>` displays metadata only. Raw reveal/export is intentionally
handled through secure UI, not chat output.

## Login And Logout Commands

PFTerminal still includes inherited Codex login commands:

```bash
pfterminal login
pfterminal login --with-api-key
pfterminal login status
pfterminal logout
pfterminal logout --all
```

`pfterminal logout` removes Codex/OpenAI account auth and preserves provider
API keys in the vault. Use `pfterminal logout --all` only when you also want to
remove provider API keys from the vault and legacy provider auth storage.

For Ambient, Z.AI, OpenRouter, Baseten, and Vercel, use the provider
onboarding picker, `/providers`, `/vault`, or the provider env vars above.
