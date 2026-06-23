# Codex Fork Modifications

PFTerminal is a product fork of the open-source Codex CLI. The current goal is to preserve Codex's local coding-agent runtime while changing the product defaults, packaging, branding, and provider integrations.

## Runtime Lineage

The Rust workspace remains under `codex-rs/` and keeps the major Codex subsystems:

- TUI and slash commands.
- Model/provider client runtime.
- Tool registry and execution.
- Sandbox and approval flows.
- MCP support.
- Session, rollout, and thread storage.
- Exec and review modes.

The repository README and `codex-rs/README.md` document this as a Codex-derived Rust workspace.

## Product Command Names

PFTerminal keeps upstream-compatible `codex` paths while adding product-facing command names:

- `codex-rs/cli/Cargo.toml` defines both `codex` and `pfterminal` binaries.
- `codex-rs/cli/src/pfterminal_main.rs` currently includes the same implementation as `main.rs`.
- `codex-cli/package.json` publishes `@agticorp/pfterminal` with both `pfterminal` and `codex` bin aliases.
- `codex-cli/bin/codex.js` resolves platform packages named `@agticorp/pfterminal-*`.

This keeps existing Codex workflows usable while making `pfterminal` the product-facing command.

## Packaging And Installers

The npm packaging has been renamed around `@agticorp/pfterminal`:

- Main package: `@agticorp/pfterminal`.
- Platform packages: `@agticorp/pfterminal-linux-x64`, `@agticorp/pfterminal-darwin-arm64`, and related target variants.
- TypeScript SDK package: `@agticorp/pfterminal-sdk`.

Standalone installer scripts in `scripts/install/` use PFTerminal messaging and install locations.

## Branding Changes

The TUI and login surfaces have PFTerminal branding:

- Device-code prompt: `Welcome to PFTerminal` and `Post Fiat's command-line coding agent`.
- Session cards render `PFTerminal`.
- Composer placeholder text uses `Ask PFTerminal to do anything`.
- Status surfaces include `Post Fiat Terminal`.
- Tooltips and resume guidance reference `pfterminal`.

The status line can therefore show a session such as:

```text
zai-org/GLM-5.2-FP8 standard ... Post Fiat Terminal
```

## Model Picker Changes

The model picker is intentionally narrowed for the current product:

- Ambient and Z.AI GLM models are shown by default.
- Hidden or non-product models can still be selected by config or command-line model override.
- GLM reasoning is presented as `Standard` and `Deep` instead of raw OpenAI-style effort labels.

Key path: `codex-rs/tui/src/chatwidget/model_popups.rs`.

## Prompt And Base Instructions

Bundled Ambient and Z.AI model metadata use PFTerminal base instructions:

```text
You are PFTerminal, a coding agent.
```

The instructions preserve the Codex engineering posture: inspect code first, keep edits scoped, use `rg`/`rg --files`, and verify work when practical.

## Upstream Isolation

The fork should keep upstream Codex changes easy to reason about:

- Product-specific provider constants should stay in provider metadata modules.
- Product model choices should stay in bundled model metadata.
- UI branding should stay in TUI-facing modules.
- Request-compatibility shims should stay close to API serialization.

That boundary makes it easier to merge upstream Codex changes without hiding PFTerminal product behavior in scattered prompt text.

## Source

- `README.md`
- `codex-rs/README.md`
- `codex-rs/cli/Cargo.toml`
- `codex-rs/cli/src/pfterminal_main.rs`
- `codex-cli/package.json`
- `codex-cli/bin/codex.js`
- `scripts/install/`
- `codex-rs/tui/src/chatwidget/model_popups.rs`
- `codex-rs/models-manager/models.json`
