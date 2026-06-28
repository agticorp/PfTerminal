//! Claude Code headless pane management.
//!
//! This module manages Claude Code headless panes — terminal panes that run
//! Claude Code as a subprocess with provider-specific routing (Ambient, Z.AI,
//! OpenRouter, Baseten, Vercel, or Claude Plan). It provides:
//!
//! - Provider profile definitions ([`provider`])
//! - Pane state and live turn tracking ([`pane`])
//! - Pane registry and layout persistence ([`registry`])
//! - Disk persistence and restoration ([`persistence`])
//! - Turn types (output, progress, audit, command plan) ([`turn_types`])
//! - Command plan building and vault integration ([`command_plan`])
//! - Turn execution and process management ([`execution`])
//! - Local HTTP bridge for provider routing ([`bridge`], [`bridge_translate`])
//! - Output parsing from stream-json ([`output_parse`])
//! - Progress tracking and text summarization ([`progress`])
//! - Smoke test and workflow suites ([`smoke`])
//! - App integration (pickers, turn submission, display sync) ([`app_integration`])

pub(crate) mod app_integration;
pub(crate) mod bridge;
pub(crate) mod bridge_translate;
pub(crate) mod command_plan;
pub(crate) mod execution;
pub(crate) mod output_parse;
pub(crate) mod pane;
pub(crate) mod persistence;
pub(crate) mod progress;
pub(crate) mod progress_summarize;
pub(crate) mod provider;
pub(crate) mod registry;
pub mod smoke;
pub(crate) mod smoke_workflows;
pub(crate) mod turn_types;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

// Re-export the public API surface that other modules in the crate consume.
// These are the symbols referenced as `crate::claude_panes::*` elsewhere.

#[allow(unused_imports)] // used in tests
pub(crate) use pane::ClaudeCommandMode;
pub(crate) use pane::ClaudePane;
pub(crate) use pane::ClaudePaneStatus;
#[allow(unused_imports)] // used in tests
pub(crate) use pane::ClaudePaneTurnStatus;
#[allow(unused_imports)] // used in tests
pub(crate) use pane::ClaudePaneUsageStatus;
#[allow(unused_imports)] // used in tests
pub(crate) use pane::PaneLayoutState;
pub(crate) use provider::ClaudeProviderProfileKind;
pub(crate) use registry::CODEX_MAIN_PANE_ID;
pub(crate) use registry::ClaudePaneRegistry;
#[allow(unused_imports)] // used in tests
pub(crate) use registry::PANE_LAYOUT_VERSION;
pub(crate) use registry::load_pane_layout;
#[allow(unused_imports)] // used in tests
pub(crate) use registry::persist_pane_layout;
pub(crate) use turn_types::ClaudePaneTurnOutput;
pub(crate) use turn_types::ClaudePaneTurnProgress;

// Re-export smoke test entry points for the CLI binary.
pub use smoke::ClaudePaneSmokeEntry;
pub use smoke::ClaudePaneSmokeOptions;
pub use smoke::ClaudePaneSmokeReport;
pub use smoke::ClaudePaneWorkflowEntry;
pub use smoke::ClaudePaneWorkflowOptions;
pub use smoke::ClaudePaneWorkflowReport;
pub use smoke::run_claude_pane_smoke;
pub use smoke::run_claude_pane_workflow_suite;
