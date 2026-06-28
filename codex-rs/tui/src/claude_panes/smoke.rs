//! Smoke test and workflow suite runners for Claude pane providers.

use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use super::pane::ClaudePaneTurnStatus;

use super::smoke_workflows::run_auditability_workflow;
use super::smoke_workflows::run_code_review_workflow;
use super::smoke_workflows::run_mock_website_workflow;
use super::smoke_workflows::run_numpy_pandas_benchmark_workflow;
use super::smoke_workflows::run_single_smoke_provider;
use super::smoke_workflows::smoke_provider_profile;
use super::smoke_workflows::workflow_entry_error;
#[derive(Debug, Clone)]
pub struct ClaudePaneSmokeOptions {
    pub codex_home: PathBuf,
    pub cwd: PathBuf,
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudePaneSmokeReport {
    pub report_path: PathBuf,
    pub passed: bool,
    pub summary: String,
    pub entries: Vec<ClaudePaneSmokeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudePaneSmokeEntry {
    pub provider: String,
    pub profile: Option<String>,
    pub status: String,
    pub first_turn_status: Option<ClaudePaneTurnStatus>,
    pub second_turn_status: Option<ClaudePaneTurnStatus>,
    pub first_turn_duration_ms: Option<i64>,
    pub second_turn_duration_ms: Option<i64>,
    pub first_artifact_path: Option<PathBuf>,
    pub first_audit_path: Option<PathBuf>,
    pub artifact_path: Option<PathBuf>,
    pub audit_path: Option<PathBuf>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClaudePaneWorkflowOptions {
    pub codex_home: PathBuf,
    pub cwd: PathBuf,
    pub providers: Vec<String>,
    pub workflows: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudePaneWorkflowReport {
    pub report_path: PathBuf,
    pub passed: bool,
    pub summary: String,
    pub entries: Vec<ClaudePaneWorkflowEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudePaneWorkflowEntry {
    pub provider: String,
    pub profile: Option<String>,
    pub workflow: String,
    pub status: String,
    pub artifact_path: Option<PathBuf>,
    pub audit_path: Option<PathBuf>,
    pub fixture_path: Option<PathBuf>,
    pub error: Option<String>,
    pub output_excerpt: Option<String>,
}

pub async fn run_claude_pane_smoke(
    options: ClaudePaneSmokeOptions,
) -> Result<ClaudePaneSmokeReport> {
    let uses_default_baseline = options.providers.is_empty();
    let provider_names = if uses_default_baseline {
        vec![
            "ambient".to_string(),
            "zai".to_string(),
            "baseten".to_string(),
            "openrouter".to_string(),
            "claude-plan".to_string(),
        ]
    } else {
        options.providers
    };
    let mut entries = Vec::new();
    for provider_name in provider_names {
        entries.push(
            run_single_smoke_provider(
                &options.codex_home,
                &options.cwd,
                provider_name.trim().to_string(),
            )
            .await,
        );
    }

    let passed = if uses_default_baseline {
        entries
            .iter()
            .any(|entry| entry.status == "passed" && entry.provider == "ambient")
    } else {
        !entries.is_empty() && entries.iter().all(|entry| entry.status == "passed")
    };
    let report_dir = options.codex_home.join("panes").join("smoke-reports");
    std::fs::create_dir_all(&report_dir).with_context(|| {
        format!(
            "failed to create Claude pane smoke report directory `{}`",
            report_dir.display()
        )
    })?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let report_path = report_dir.join(format!("claude-pane-smoke-{timestamp}.json"));
    let summary = format!(
        "Claude pane smoke: {} passed, {} checked; report: {}",
        entries
            .iter()
            .filter(|entry| entry.status == "passed")
            .count(),
        entries.len(),
        report_path.display()
    );
    let report = ClaudePaneSmokeReport {
        report_path: report_path.clone(),
        passed,
        summary,
        entries,
    };
    let bytes = serde_json::to_vec_pretty(&report).context("failed to serialize smoke report")?;
    std::fs::write(&report_path, bytes).with_context(|| {
        format!(
            "failed to write Claude pane smoke report `{}`",
            report_path.display()
        )
    })?;
    Ok(report)
}

pub async fn run_claude_pane_workflow_suite(
    options: ClaudePaneWorkflowOptions,
) -> Result<ClaudePaneWorkflowReport> {
    let provider_names = if options.providers.is_empty() {
        vec!["ambient".to_string()]
    } else {
        options.providers
    };
    let workflow_names = if options.workflows.is_empty() {
        vec![
            "mock-website".to_string(),
            "numpy-pandas-benchmark".to_string(),
            "code-review".to_string(),
            "auditability".to_string(),
        ]
    } else {
        options.workflows
    };
    let report_root = options.codex_home.join("panes").join("workflow-reports");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let fixture_root = report_root.join(format!("fixtures-{timestamp}"));
    std::fs::create_dir_all(&fixture_root).with_context(|| {
        format!(
            "failed to create Claude pane workflow fixture directory `{}`",
            fixture_root.display()
        )
    })?;

    let mut entries = Vec::new();
    for provider_name in provider_names {
        for workflow_name in &workflow_names {
            entries.push(
                run_single_workflow(
                    &options.codex_home,
                    &options.cwd,
                    &fixture_root,
                    provider_name.trim().to_string(),
                    workflow_name.trim().to_string(),
                )
                .await,
            );
        }
    }
    let passed = entries.iter().all(|entry| entry.status == "passed");
    let report_path = report_root.join(format!("claude-pane-workflow-suite-{timestamp}.json"));
    let summary = format!(
        "Claude pane workflow suite: {} passed, {} checked; report: {}",
        entries
            .iter()
            .filter(|entry| entry.status == "passed")
            .count(),
        entries.len(),
        report_path.display()
    );
    let report = ClaudePaneWorkflowReport {
        report_path: report_path.clone(),
        passed,
        summary,
        entries,
    };
    let bytes =
        serde_json::to_vec_pretty(&report).context("failed to serialize workflow report")?;
    std::fs::write(&report_path, bytes).with_context(|| {
        format!(
            "failed to write Claude pane workflow report `{}`",
            report_path.display()
        )
    })?;
    Ok(report)
}

pub(crate) async fn run_single_workflow(
    codex_home: &Path,
    cwd: &Path,
    fixture_root: &Path,
    provider_name: String,
    workflow_name: String,
) -> ClaudePaneWorkflowEntry {
    let Some(profile) = smoke_provider_profile(&provider_name) else {
        return workflow_entry_error(
            provider_name,
            None,
            workflow_name,
            None,
            None,
            None,
            "unknown workflow provider".to_string(),
        );
    };
    let profile_title = Some(profile.profile().title.to_string());
    match workflow_name.as_str() {
        "mock-website" => {
            run_mock_website_workflow(codex_home, fixture_root, provider_name, profile).await
        }
        "numpy-pandas-benchmark" => {
            run_numpy_pandas_benchmark_workflow(codex_home, fixture_root, provider_name, profile)
                .await
        }
        "code-review" => run_code_review_workflow(codex_home, cwd, provider_name, profile).await,
        "auditability" => {
            run_auditability_workflow(codex_home, fixture_root, provider_name, profile).await
        }
        _ => workflow_entry_error(
            provider_name,
            profile_title,
            workflow_name,
            None,
            None,
            None,
            "unknown workflow".to_string(),
        ),
    }
}
