# PFTerminal

PFTerminal is a crypto-native AI services terminal based on the open-source Codex CLI. It defaults to Ambient's GLM 5.2 model and is intended to become one secure interface for crypto-native AI workflows.

## Quick Install

Install the standalone release (preferred). One line:

```bash
curl -fsSL https://github.com/agticorp/PfTerminal/releases/latest/download/install.sh | sh
```

This downloads the verified release artifact for your platform, creates a
`pfterminal` launcher on your `PATH`, and stores state in `~/.pfterminal`
(separate from any stock Codex install). Then run it from the workspace you want
PFTerminal to inspect:

```bash
cd ~/repos/my-project
pfterminal
```

On first launch, pick a provider and enter its API key. The key is stored in the
encrypted vault, never in the chat transcript.

### Verify the install

```bash
pfterminal --version
pfterminal
```

You should see a version string, and inside the TUI:

- `/vault` lists the provider key you added
- `/model` shows Coding Plans and Pay Per API Call sections
- `/providers` includes the OpenAI Codex account plus API-key providers

### Install options at a glance

| Method            | When to use                                              | Command / link                                          |
| ----------------- | -------------------------------------------------------- | ------------------------------------------------------- |
| Release installer | Normal desktop/server install (no Rust toolchain needed) | See the one-liner above                                 |
| npm package       | Node toolchain present, want global `pfterminal`/`codex` | `npm install -g @agticorp/pfterminal`                   |
| Source build      | You want to develop on PFTerminal itself                 | `cargo build -p codex-cli --bin pfterminal` (see below) |

Custom install location override:

```bash
curl -fsSL https://github.com/agticorp/PfTerminal/releases/latest/download/install.sh |
  PFTERMINAL_INSTALL_DIR="$HOME/.local/bin" \
  PFTERMINAL_HOME="$HOME/.pfterminal" \
  sh
```

> The installer requires a published GitHub release. This repo publishes the
> `rust-vX.Y.Z` releases via the manual `pfterminal-release` GitHub Actions
> workflow; the `latest` tag currently points at `rust-v0.0.0`.

## Provider Setup

PFTerminal ships built-in providers. You only need a credential for the provider
you plan to use; you do not have to define providers in `config.toml`.

| Provider     | Provider id  | Key name             | Default model(s)                                                                        |
| ------------ | ------------ | -------------------- | --------------------------------------------------------------------------------------- |
| OpenAI Codex | `openai`     | Codex account login  | `gpt-5.5`                                                                               |
| Ambient      | `ambient`    | `AMBIENT_API_KEY`    | `zai-org/GLM-5.2-FP8`                                                                   |
| Z.AI         | `zai`        | `ZAI_API_KEY`        | `glm-5.2`                                                                               |
| OpenRouter   | `openrouter` | `OPENROUTER_API_KEY` | `z-ai/glm-5.2`, `minimax/minimax-m3`, `openrouter/owl-alpha`, `google/gemini-3.5-flash` |
| Baseten      | `baseten`    | `BASETEN_API_KEY`    | `zai-org/GLM-5.2`                                                                       |
| Vercel       | `vercel`     | `AI_GATEWAY_API_KEY` | `zai/glm-5.2`, `zai/glm-5.2-fast`                                                       |

Keys entered through the first-run provider picker, `/providers`, or `/vault`
are stored encrypted at rest (vault label form `provider/<key_name>`). You can
also export them as env vars for CI/temporary shells:

```bash
export AMBIENT_API_KEY="..."
export ZAI_API_KEY="..."
export OPENROUTER_API_KEY="..."
export BASETEN_API_KEY="..."
export AI_GATEWAY_API_KEY="..."
```

Switch models with `/model` in the TUI or `-m` at startup:

```bash
pfterminal -m zai-org/GLM-5.2-FP8   # Ambient GLM 5.2 (default)
pfterminal -m glm-5.2               # Z.AI GLM 5.2
pfterminal -m z-ai/glm-5.2          # OpenRouter GLM 5.2
```

## Running Locally From Source

```shell
cd codex-rs
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build -p codex-cli --bin pfterminal
```

Launch from the workspace you want PFTerminal to inspect:

```shell
cd ~/repos
/path/to/PfTerminal/codex-rs/target/debug/pfterminal
```

The first source build can take 10-20 minutes on a fresh Mac because Cargo
fetches git dependencies and compiles the full workspace. The
`CARGO_NET_GIT_FETCH_WITH_CLI=true` setting avoids intermittent macOS libgit2
fetch stalls seen with nested git dependencies.

The source-built `pfterminal` binary defaults `CODEX_HOME` to
`$HOME/.pfterminal`; set `CODEX_HOME` only when you need a custom state
directory. This keeps PFTerminal credentials, vault data, sessions, logs,
plugins, and skills separate from a stock Codex install.

## Documentation

Full docs live in [`docs/`](docs):

- [Install And First Run](docs/install.md) — system requirements, release installer internals, source build with the Rust toolchain, npm package, vault setup, model metadata
- [Getting Started](docs/getting-started.md) — first run, provider choices, vault basics, slash commands
- [Authentication And Vault](docs/authentication.md)
- [Configuration](docs/config.md)

## Current Focus

- Ambient API-key onboarding by default
- Ambient GLM 5.2 as the default model
- Z.AI, OpenRouter, Baseten, and Vercel provider choices
- encrypted `/vault` storage for provider API keys and user credentials
- Codex-level coding workflows in a local terminal
- Future crypto-native services such as authentication, Hyperliquid, GPU rentals, staking, borrowing, and related workflows

## Upstream

PFTerminal is based on the open-source Codex CLI project. Keep upstream changes isolated through the `upstream` remote and land PFTerminal changes through this repository.

This repository is licensed under the [Apache-2.0 License](LICENSE).
