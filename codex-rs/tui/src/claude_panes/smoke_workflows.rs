//! Workflow implementations for the Claude pane smoke test suite.

use std::path::Path;
use std::path::PathBuf;

use super::execution::run_prepared_claude_turn;
use super::pane::ClaudeCommandMode;
use super::progress::truncate_for_display;
use super::provider::ClaudeProviderProfileKind;
use super::registry::ClaudePaneRegistry;
use super::smoke::ClaudePaneSmokeEntry;
use super::smoke::ClaudePaneWorkflowEntry;
use super::turn_types::ClaudePaneTurnOutput;
pub(crate) async fn run_mock_website_workflow(
    codex_home: &Path,
    fixture_root: &Path,
    provider_name: String,
    profile: ClaudeProviderProfileKind,
) -> ClaudePaneWorkflowEntry {
    let workflow = "mock-website".to_string();
    let fixture_path = workflow_fixture_path(fixture_root, &provider_name, &workflow);
    if let Err(err) = std::fs::create_dir_all(&fixture_path) {
        return workflow_entry_error(
            provider_name,
            Some(profile.profile().title.to_string()),
            workflow,
            None,
            None,
            Some(fixture_path),
            format!("failed to create fixture: {err}"),
        );
    }
    let prompt = concat!(
        "Build a tiny mock website in the current directory for a product named ",
        "PFT Pane Observatory. Create index.html plus either styles.css or script.js. ",
        "The page must include the exact text PFT Pane Observatory and one styled or ",
        "interactive element. After writing files, reply with marker PFT_MOCK_SITE_DONE ",
        "and list the files you created."
    )
    .to_string();
    let mut registry = ClaudePaneRegistry::new();
    let pane_id = match registry.create_pane(profile, fixture_path.clone(), codex_home) {
        Ok(id) => id,
        Err(err) => {
            return workflow_entry_error(
                provider_name,
                Some(profile.profile().title.to_string()),
                workflow,
                None,
                None,
                Some(fixture_path),
                err.to_string(),
            );
        }
    };
    let output = match run_smoke_turn(&mut registry, &pane_id, prompt, codex_home).await {
        Ok(output) => output,
        Err(err) => {
            return workflow_entry_error(
                provider_name,
                Some(profile.profile().title.to_string()),
                workflow,
                None,
                None,
                Some(fixture_path),
                err,
            );
        }
    };
    let index_path = fixture_path.join("index.html");
    let index = std::fs::read_to_string(&index_path).unwrap_or_default();
    let has_asset =
        fixture_path.join("styles.css").exists() || fixture_path.join("script.js").exists();
    if output.status.is_success()
        && output.text.contains("PFT_MOCK_SITE_DONE")
        && index.contains("PFT Pane Observatory")
        && has_asset
    {
        workflow_entry_pass(provider_name, profile, workflow, output, Some(fixture_path))
    } else {
        workflow_entry_from_output(
            provider_name,
            profile,
            workflow,
            output,
            Some(fixture_path),
            "mock website verification failed".to_string(),
        )
    }
}

pub(crate) async fn run_numpy_pandas_benchmark_workflow(
    codex_home: &Path,
    fixture_root: &Path,
    provider_name: String,
    profile: ClaudeProviderProfileKind,
) -> ClaudePaneWorkflowEntry {
    let workflow = "numpy-pandas-benchmark".to_string();
    let fixture_path = workflow_fixture_path(fixture_root, &provider_name, &workflow);
    if let Err(err) = std::fs::create_dir_all(&fixture_path) {
        return workflow_entry_error(
            provider_name,
            Some(profile.profile().title.to_string()),
            workflow,
            None,
            None,
            Some(fixture_path),
            format!("failed to create fixture: {err}"),
        );
    }
    let prompt = concat!(
        "Create and run a Python benchmark comparing NumPy vs Pandas for filtering ",
        "and aggregating numeric rows. Use a deterministic random seed and a data size ",
        "small enough to finish quickly. Output a markdown table with columns ",
        "Implementation, Mean time, Fastest run, and Notes. Include marker ",
        "PFT_NUMPY_PANDAS_BENCH_DONE. If numpy or pandas is missing, report the missing ",
        "dependency clearly instead of hanging."
    )
    .to_string();
    let mut registry = ClaudePaneRegistry::new();
    let pane_id = match registry.create_pane(profile, fixture_path.clone(), codex_home) {
        Ok(id) => id,
        Err(err) => {
            return workflow_entry_error(
                provider_name,
                Some(profile.profile().title.to_string()),
                workflow,
                None,
                None,
                Some(fixture_path),
                err.to_string(),
            );
        }
    };
    let output = match run_smoke_turn(&mut registry, &pane_id, prompt, codex_home).await {
        Ok(output) => output,
        Err(err) => {
            return workflow_entry_error(
                provider_name,
                Some(profile.profile().title.to_string()),
                workflow,
                None,
                None,
                Some(fixture_path),
                err,
            );
        }
    };
    let has_table = output.text.contains('|')
        && output.text.to_lowercase().contains("numpy")
        && output.text.to_lowercase().contains("pandas")
        && output.text.contains("PFT_NUMPY_PANDAS_BENCH_DONE");
    if output.status.is_success() && has_table {
        workflow_entry_pass(provider_name, profile, workflow, output, Some(fixture_path))
    } else {
        workflow_entry_from_output(
            provider_name,
            profile,
            workflow,
            output,
            Some(fixture_path),
            "NumPy vs Pandas benchmark verification failed".to_string(),
        )
    }
}

pub(crate) async fn run_code_review_workflow(
    codex_home: &Path,
    cwd: &Path,
    provider_name: String,
    profile: ClaudeProviderProfileKind,
) -> ClaudePaneWorkflowEntry {
    let workflow = "code-review".to_string();
    let mut registry = ClaudePaneRegistry::new();
    let pane_id = match registry.create_pane(profile, cwd.to_path_buf(), codex_home) {
        Ok(id) => id,
        Err(err) => {
            return workflow_entry_error(
                provider_name,
                Some(profile.profile().title.to_string()),
                workflow,
                None,
                None,
                None,
                err.to_string(),
            );
        }
    };
    let prompt = concat!(
        "Perform a read-only code review of the active implementation diff in this repo. ",
        "You must inspect the actual patch body, not only commit metadata or --stat. ",
        "Start with `git diff --find-renames --find-copies --unified=80`. ",
        "If there is no working-tree diff, review `git show --format=fuller --find-renames --find-copies --unified=80 HEAD` instead. ",
        "If the output is too large, continue with narrower `git diff --patch -- <path>` or `git show --patch HEAD -- <path>` ",
        "commands until you have inspected real diff hunks for the changed files. ",
        "Review that patch as the source of truth and stop reading once the changed diff hunks are understood. ",
        "Return marker PFT_CODE_REVIEW_DONE, include `DIFF_INSPECTED: yes`, and give concrete ",
        "findings with file references or say no findings with a short rationale. ",
        "Do not edit files."
    )
    .to_string();
    let first_output = match run_smoke_turn(&mut registry, &pane_id, prompt, codex_home).await {
        Ok(output) => output,
        Err(err) => {
            return workflow_entry_error(
                provider_name,
                Some(profile.profile().title.to_string()),
                workflow,
                None,
                None,
                None,
                err,
            );
        }
    };
    let has_review = first_output.text.contains("PFT_CODE_REVIEW_DONE")
        && first_output.text.contains("DIFF_INSPECTED: yes")
        && artifact_contains_patch_body(&first_output.artifact_path)
        && shallow_review_rejection_reason(&first_output.text).is_none();
    if !(first_output.status.is_success() && has_review && !first_output.tool_names.is_empty()) {
        let error = shallow_review_rejection_reason(&first_output.text)
            .unwrap_or_else(|| "fresh code review did not prove full diff inspection".to_string());
        return workflow_entry_from_output(
            provider_name,
            profile,
            workflow,
            first_output,
            None,
            error,
        );
    }

    let resume_prompt = concat!(
        "Continue the same read-only code review. Use the context already gathered. ",
        "You may use additional filesystem tools if needed. Return marker ",
        "PFT_CODE_REVIEW_RESUME_DONE and include either one additional concrete finding ",
        "with a file reference or `NO_ADDITIONAL_FINDINGS` with a short rationale. ",
        "Do not edit files."
    )
    .to_string();
    let resume_output =
        match run_smoke_turn(&mut registry, &pane_id, resume_prompt, codex_home).await {
            Ok(output) => output,
            Err(err) => {
                return workflow_entry_error(
                    provider_name,
                    Some(profile.profile().title.to_string()),
                    workflow,
                    None,
                    None,
                    None,
                    err,
                );
            }
        };
    let has_resume_review = resume_output.text.contains("PFT_CODE_REVIEW_RESUME_DONE")
        && shallow_review_rejection_reason(&resume_output.text).is_none();
    if resume_output.status.is_success()
        && has_resume_review
        && matches!(resume_output.command_mode, ClaudeCommandMode::Resume)
    {
        workflow_entry_pass(provider_name, profile, workflow, resume_output, None)
    } else {
        workflow_entry_from_output(
            provider_name,
            profile,
            workflow,
            resume_output,
            None,
            "resumed code review verification failed".to_string(),
        )
    }
}

pub(crate) async fn run_auditability_workflow(
    codex_home: &Path,
    fixture_root: &Path,
    provider_name: String,
    profile: ClaudeProviderProfileKind,
) -> ClaudePaneWorkflowEntry {
    let workflow = "auditability".to_string();
    let fixture_path = workflow_fixture_path(fixture_root, &provider_name, &workflow);
    if let Err(err) = std::fs::create_dir_all(&fixture_path) {
        return workflow_entry_error(
            provider_name,
            Some(profile.profile().title.to_string()),
            workflow,
            None,
            None,
            Some(fixture_path),
            format!("failed to create fixture: {err}"),
        );
    }
    let mut registry = ClaudePaneRegistry::new();
    let pane_id = match registry.create_pane(profile, fixture_path.clone(), codex_home) {
        Ok(id) => id,
        Err(err) => {
            return workflow_entry_error(
                provider_name,
                Some(profile.profile().title.to_string()),
                workflow,
                None,
                None,
                Some(fixture_path),
                err.to_string(),
            );
        }
    };
    let prompts = [
        "Reply exactly PFT_AUDIT_TURN_1.",
        "Use Bash to run `printf PFT_AUDIT_TURN_2` and then reply with PFT_AUDIT_TURN_2.",
        "Use Bash to run `false`; then explain that the command failed and include marker PFT_AUDIT_FAILURE_PATH.",
    ];
    let mut last_output = None;
    for prompt in prompts {
        let output =
            match run_smoke_turn(&mut registry, &pane_id, prompt.to_string(), codex_home).await {
                Ok(output) => output,
                Err(err) => {
                    return workflow_entry_error(
                        provider_name,
                        Some(profile.profile().title.to_string()),
                        workflow,
                        None,
                        None,
                        Some(fixture_path),
                        err,
                    );
                }
            };
        if !output.audit_path.exists() {
            return workflow_entry_from_output(
                provider_name,
                profile,
                workflow,
                output,
                Some(fixture_path),
                "audit file was not written".to_string(),
            );
        }
        last_output = Some(output);
    }
    let Some(output) = last_output else {
        return workflow_entry_error(
            provider_name,
            Some(profile.profile().title.to_string()),
            workflow,
            None,
            None,
            Some(fixture_path),
            "audit workflow did not run any turns".to_string(),
        );
    };
    if output.status.is_success()
        && output.text.contains("PFT_AUDIT_FAILURE_PATH")
        && !output.tool_names.is_empty()
    {
        workflow_entry_pass(provider_name, profile, workflow, output, Some(fixture_path))
    } else {
        workflow_entry_from_output(
            provider_name,
            profile,
            workflow,
            output,
            Some(fixture_path),
            "auditability workflow verification failed".to_string(),
        )
    }
}

pub(crate) fn artifact_contains_patch_body(path: &Path) -> bool {
    let Ok(artifact) = std::fs::read_to_string(path) else {
        return false;
    };
    artifact.contains("diff --git")
        && artifact.contains("@@")
        && (artifact.contains("+") || artifact.contains("-"))
}

pub(crate) fn shallow_review_rejection_reason(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let rejected = [
        "couldn't pull the full diff",
        "could not pull the full diff",
        "couldn't read the full diff",
        "could not read the full diff",
        "unable to pull the full diff",
        "unable to read the full diff",
        "unable to inspect the full diff",
        "based on the commit metadata",
        "based on the commit message",
        "based on the change description",
        "local tool budget",
        "tool budget was hit",
        "without seeing the full diff",
    ];
    rejected
        .iter()
        .find(|phrase| lower.contains(**phrase))
        .map(|phrase| format!("shallow code review output: `{phrase}`"))
}

pub(crate) fn workflow_fixture_path(
    fixture_root: &Path,
    provider: &str,
    workflow: &str,
) -> PathBuf {
    fixture_root.join(provider).join(workflow)
}

pub(crate) fn workflow_entry_pass(
    provider: String,
    profile: ClaudeProviderProfileKind,
    workflow: String,
    output: ClaudePaneTurnOutput,
    fixture_path: Option<PathBuf>,
) -> ClaudePaneWorkflowEntry {
    ClaudePaneWorkflowEntry {
        provider,
        profile: Some(profile.profile().title.to_string()),
        workflow,
        status: "passed".to_string(),
        artifact_path: Some(output.artifact_path),
        audit_path: Some(output.audit_path),
        fixture_path,
        error: None,
        output_excerpt: Some(truncate_for_display(&output.text, 1_000)),
    }
}

pub(crate) fn workflow_entry_from_output(
    provider: String,
    profile: ClaudeProviderProfileKind,
    workflow: String,
    output: ClaudePaneTurnOutput,
    fixture_path: Option<PathBuf>,
    error: String,
) -> ClaudePaneWorkflowEntry {
    let failure = output.failure_message();
    let excerpt = truncate_for_display(&output.text, 1_000);
    ClaudePaneWorkflowEntry {
        provider,
        profile: Some(profile.profile().title.to_string()),
        workflow,
        status: "failed".to_string(),
        artifact_path: Some(output.artifact_path),
        audit_path: Some(output.audit_path),
        fixture_path,
        error: Some(format!("{error}: {failure}")),
        output_excerpt: Some(excerpt),
    }
}

pub(crate) fn workflow_entry_error(
    provider: String,
    profile: Option<String>,
    workflow: String,
    artifact_path: Option<PathBuf>,
    audit_path: Option<PathBuf>,
    fixture_path: Option<PathBuf>,
    error: String,
) -> ClaudePaneWorkflowEntry {
    ClaudePaneWorkflowEntry {
        provider,
        profile,
        workflow,
        status: "failed".to_string(),
        artifact_path,
        audit_path,
        fixture_path,
        error: Some(error),
        output_excerpt: None,
    }
}

pub(crate) async fn run_single_smoke_provider(
    codex_home: &Path,
    cwd: &Path,
    provider_name: String,
) -> ClaudePaneSmokeEntry {
    let Some(profile) = smoke_provider_profile(&provider_name) else {
        return ClaudePaneSmokeEntry {
            provider: provider_name,
            profile: None,
            status: "unknown-provider".to_string(),
            first_turn_status: None,
            second_turn_status: None,
            first_turn_duration_ms: None,
            second_turn_duration_ms: None,
            first_artifact_path: None,
            first_audit_path: None,
            artifact_path: None,
            audit_path: None,
            error: Some("unknown Claude pane smoke provider".to_string()),
        };
    };
    let profile_config = profile.profile();
    let mut registry = ClaudePaneRegistry::new();
    let pane_id = match registry.create_pane(profile, cwd.to_path_buf(), codex_home) {
        Ok(pane_id) => pane_id,
        Err(err) => {
            return ClaudePaneSmokeEntry {
                provider: provider_name,
                profile: Some(profile_config.title.to_string()),
                status: "unavailable".to_string(),
                first_turn_status: None,
                second_turn_status: None,
                first_turn_duration_ms: None,
                second_turn_duration_ms: None,
                first_artifact_path: None,
                first_audit_path: None,
                artifact_path: None,
                audit_path: None,
                error: Some(err.to_string()),
            };
        }
    };

    let first_result = run_smoke_turn(
        &mut registry,
        &pane_id,
        smoke_first_turn_prompt(),
        codex_home,
    )
    .await;
    let first_output = match first_result {
        Ok(output) => output,
        Err(err) => {
            return ClaudePaneSmokeEntry {
                provider: provider_name,
                profile: Some(profile_config.title.to_string()),
                status: "failed".to_string(),
                first_turn_status: None,
                second_turn_status: None,
                first_turn_duration_ms: None,
                second_turn_duration_ms: None,
                first_artifact_path: None,
                first_audit_path: None,
                artifact_path: None,
                audit_path: None,
                error: Some(err),
            };
        }
    };
    let artifact_path = Some(first_output.artifact_path.clone());
    let audit_path = Some(first_output.audit_path.clone());
    let first_artifact_path = Some(first_output.artifact_path.clone());
    let first_audit_path = Some(first_output.audit_path.clone());
    let first_turn_duration_ms = Some(first_output.duration_ms);
    if !first_output.status.is_success() {
        return ClaudePaneSmokeEntry {
            provider: provider_name,
            profile: Some(profile_config.title.to_string()),
            status: "failed".to_string(),
            first_turn_status: Some(first_output.status),
            second_turn_status: None,
            first_turn_duration_ms,
            second_turn_duration_ms: None,
            first_artifact_path,
            first_audit_path,
            artifact_path,
            audit_path,
            error: Some(first_output.failure_message()),
        };
    }

    let second_result = run_smoke_turn(
        &mut registry,
        &pane_id,
        "Continue from the same Claude pane session. Reply with exactly: PFT_CLAUDE_SMOKE_RESUME_OK"
            .to_string(),
        codex_home,
    )
    .await;
    match second_result {
        Ok(second_output) if second_output.status.is_success() => ClaudePaneSmokeEntry {
            provider: provider_name,
            profile: Some(profile_config.title.to_string()),
            status: "passed".to_string(),
            first_turn_status: Some(first_output.status),
            second_turn_status: Some(second_output.status),
            first_turn_duration_ms,
            second_turn_duration_ms: Some(second_output.duration_ms),
            first_artifact_path,
            first_audit_path,
            artifact_path: Some(second_output.artifact_path),
            audit_path: Some(second_output.audit_path),
            error: None,
        },
        Ok(second_output) => {
            let error = second_output.failure_message();
            ClaudePaneSmokeEntry {
                provider: provider_name,
                profile: Some(profile_config.title.to_string()),
                status: "failed".to_string(),
                first_turn_status: Some(first_output.status),
                second_turn_status: Some(second_output.status),
                first_turn_duration_ms,
                second_turn_duration_ms: Some(second_output.duration_ms),
                first_artifact_path,
                first_audit_path,
                artifact_path: Some(second_output.artifact_path),
                audit_path: Some(second_output.audit_path),
                error: Some(error),
            }
        }
        Err(err) => ClaudePaneSmokeEntry {
            provider: provider_name,
            profile: Some(profile_config.title.to_string()),
            status: "failed".to_string(),
            first_turn_status: Some(first_output.status),
            second_turn_status: None,
            first_turn_duration_ms,
            second_turn_duration_ms: None,
            first_artifact_path,
            first_audit_path,
            artifact_path,
            audit_path,
            error: Some(err),
        },
    }
}

pub(crate) async fn run_smoke_turn(
    registry: &mut ClaudePaneRegistry,
    pane_id: &str,
    prompt: String,
    codex_home: &Path,
) -> Result<ClaudePaneTurnOutput, String> {
    let prepared = registry
        .prepare_turn(pane_id, prompt, codex_home)
        .map_err(|err| err.to_string())?;
    let result = run_prepared_claude_turn(prepared, None).await;
    registry.finish_turn(pane_id, &result);
    result
}

pub(crate) fn smoke_provider_profile(provider_name: &str) -> Option<ClaudeProviderProfileKind> {
    match provider_name {
        "ambient" | "ambient-glm-52" => Some(ClaudeProviderProfileKind::AmbientGlm52),
        "ambient-kimi" | "ambient-kimi-k27" | "ambient-kimi-k2-7" | "kimi-k27" | "kimi-k2-7" => {
            Some(ClaudeProviderProfileKind::AmbientKimiK27)
        }
        "zai" | "zai-glm-52" => Some(ClaudeProviderProfileKind::ZaiGlm52),
        "baseten" | "baseten-glm-52" => Some(ClaudeProviderProfileKind::BasetenGlm52),
        "openrouter" | "openrouter-glm-52" => Some(ClaudeProviderProfileKind::OpenRouterGlm52),
        "vercel" | "vercel-glm-52" => Some(ClaudeProviderProfileKind::VercelGlm52),
        "vercel-fast" | "vercel-glm-52-fast" => Some(ClaudeProviderProfileKind::VercelGlm52Fast),
        "claude-plan" | "claude" => Some(ClaudeProviderProfileKind::ClaudePlan),
        _ => None,
    }
}

pub(crate) fn smoke_first_turn_prompt() -> String {
    concat!(
        "Perform a read-only PFTerminal Claude pane smoke test. ",
        "Use Claude Code filesystem tools to inspect Cargo.toml, ",
        "codex-rs/tui/src/claude_panes.rs, and ",
        "docs/current-sprint/claude-code-integration-completion-spec.md. ",
        "Then reply with a compact JSON object containing marker ",
        "PFT_CLAUDE_SMOKE_OK, files_checked, tools_used, and two concrete ",
        "code-review observations about the Claude pane implementation. ",
        "Do not edit files."
    )
    .to_string()
}
