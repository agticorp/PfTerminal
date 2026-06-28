//! Progress tracking, display formatting, and time/usage helpers for Claude pane turns.

use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde_json::Value;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

use super::output_parse::claude_error_summary;
use super::output_parse::collect_reasoning_events;
use super::output_parse::collect_text_chunks;
use super::output_parse::collect_tool_events;
use super::pane::ClaudePaneUsageStatus;
use super::progress_summarize::CLAUDE_REASONING_PREFIX;
use super::progress_summarize::CLAUDE_TOOL_CALL_PREFIX;
use super::turn_types::ClaudeCommandPlan;
use super::turn_types::ClaudePaneReasoningEvent;
use super::turn_types::ClaudePaneToolEvent;
use super::turn_types::ClaudePaneTurnProgress;
pub(crate) fn tool_events_from_stdout(stdout: &str) -> Vec<ClaudePaneToolEvent> {
    let mut tool_events = Vec::new();
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            collect_tool_events(&value, &mut tool_events);
        }
    }
    tool_events
}

pub(crate) fn reasoning_events_from_stdout(stdout: &str) -> Vec<ClaudePaneReasoningEvent> {
    let mut reasoning_events = Vec::new();
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            collect_reasoning_events(&value, &mut reasoning_events);
        }
    }
    reasoning_events
}

pub(crate) fn dedupe_tool_names(tool_names: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for name in tool_names {
        if !deduped.iter().any(|existing| existing == &name) {
            deduped.push(name);
        }
    }
    deduped
}

pub(crate) fn usage_summary_from_value(value: &Value) -> Option<String> {
    let usage = value
        .get("usage")
        .or_else(|| value.pointer("/message/usage"))?;
    if !usage.is_object() {
        return None;
    }
    Some(usage.to_string())
}

pub(crate) fn usage_status_from_summary(summary: Option<&str>) -> ClaudePaneUsageStatus {
    let Some(summary) = summary else {
        return ClaudePaneUsageStatus::Missing;
    };
    let Ok(value) = serde_json::from_str::<Value>(summary) else {
        return ClaudePaneUsageStatus::Unknown;
    };
    let Some(object) = value.as_object() else {
        return ClaudePaneUsageStatus::Unknown;
    };
    let mut saw_numeric = false;
    let mut saw_output_metric = false;
    let mut saw_positive_output_metric = false;
    for (key, value) in object {
        if let Some(number) = value.as_u64() {
            saw_numeric = true;
            if matches!(
                key.as_str(),
                "output_tokens" | "completion_tokens" | "completion"
            ) {
                saw_output_metric = true;
                if number > 0 {
                    saw_positive_output_metric = true;
                }
            }
        }
    }
    if saw_positive_output_metric {
        ClaudePaneUsageStatus::Reported
    } else if saw_numeric || saw_output_metric {
        ClaudePaneUsageStatus::Untrusted
    } else {
        ClaudePaneUsageStatus::Unknown
    }
}

pub(crate) fn truncate_for_display(value: &str, max_chars: usize) -> String {
    let mut out = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

pub(crate) fn elapsed_ms(started_at: &Instant) -> i64 {
    i64::try_from(started_at.elapsed().as_millis()).unwrap_or(i64::MAX)
}

pub(crate) fn format_elapsed_ms(elapsed_ms: i64) -> String {
    let total_seconds = (elapsed_ms.max(0) / 1_000).max(0);
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    if minutes > 0 {
        format!("{minutes}m{seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

pub(crate) fn tool_blurb_from_progress(progress: &ClaudePaneTurnProgress) -> String {
    progress
        .summary
        .strip_prefix(CLAUDE_TOOL_CALL_PREFIX)
        .unwrap_or(progress.summary.as_str())
        .trim()
        .to_string()
}

pub(crate) fn reasoning_blurb_from_progress(progress: &ClaudePaneTurnProgress) -> String {
    progress
        .summary
        .strip_prefix(CLAUDE_REASONING_PREFIX)
        .unwrap_or(progress.summary.as_str())
        .trim()
        .to_string()
}

pub(crate) fn progress_status_text(progress: &ClaudePaneTurnProgress) -> String {
    match progress.phase.as_str() {
        "system" => "session initialized".to_string(),
        "assistant-result" => "finalizing result".to_string(),
        "waiting" => "waiting for Claude".to_string(),
        "error" => progress.summary.trim().to_string(),
        "tool-call" => tool_blurb_from_progress(progress),
        "reasoning" | "reasoning-tokens" => reasoning_blurb_from_progress(progress),
        _ => progress.summary.trim().to_string(),
    }
}

pub(crate) fn thinking_tokens_progress(
    plan: &ClaudeCommandPlan,
    started_at: &Instant,
    value: &Value,
) -> Option<ClaudePaneTurnProgress> {
    let estimated_tokens = value.get("estimated_tokens").and_then(Value::as_u64)?;
    let bucket = thinking_tokens_progress_bucket(estimated_tokens);
    Some(ClaudePaneTurnProgress {
        pane_id: plan.pane_id.clone(),
        phase: "reasoning-tokens".to_string(),
        summary: format!(
            "{}thinking: {} reasoning tokens",
            CLAUDE_REASONING_PREFIX,
            format_reasoning_token_count(estimated_tokens)
        ),
        assistant_text_delta: None,
        hint: Some(format!("thinking-token-bucket:{bucket}")),
        elapsed_ms: elapsed_ms(started_at),
        artifact_path: plan.artifact_path.clone(),
        audit_path: plan.audit_path.clone(),
    })
}

pub(crate) fn thinking_tokens_progress_bucket(tokens: u64) -> u64 {
    if tokens < 100 {
        tokens / 10
    } else {
        tokens / 100
    }
}

pub(crate) fn format_reasoning_token_count(tokens: u64) -> String {
    if tokens < 1_000 {
        tokens.to_string()
    } else {
        let tenths = tokens / 100;
        format!("{}.{}K", tenths / 10, tenths % 10)
    }
}

pub(crate) fn unix_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub(crate) fn emit_claude_progress(
    progress_tx: &Option<AppEventSender>,
    plan: &ClaudeCommandPlan,
    started_at: &Instant,
    phase: &str,
    summary: &str,
    hint: Option<String>,
) {
    if let Some(tx) = progress_tx.as_ref() {
        tx.send(AppEvent::ClaudePaneTurnProgress {
            progress: ClaudePaneTurnProgress {
                pane_id: plan.pane_id.clone(),
                phase: phase.to_string(),
                summary: summary.to_string(),
                assistant_text_delta: None,
                hint,
                elapsed_ms: elapsed_ms(started_at),
                artifact_path: plan.artifact_path.clone(),
                audit_path: plan.audit_path.clone(),
            },
        });
    }
}

#[cfg(test)]
pub(crate) fn progress_from_claude_value(
    plan: &ClaudeCommandPlan,
    started_at: &Instant,
    value: &Value,
) -> Option<ClaudePaneTurnProgress> {
    progresses_from_claude_value(plan, started_at, value)
        .into_iter()
        .next()
}

pub(crate) fn progresses_from_claude_value(
    plan: &ClaudeCommandPlan,
    started_at: &Instant,
    value: &Value,
) -> Vec<ClaudePaneTurnProgress> {
    let mut progresses = Vec::new();
    let value_type = value.get("type").and_then(Value::as_str).unwrap_or("event");
    if value_type == "system"
        && value.get("subtype").and_then(Value::as_str) == Some("thinking_tokens")
    {
        if let Some(progress) = thinking_tokens_progress(plan, started_at, value) {
            progresses.push(progress);
        }
        return progresses;
    }
    let mut text_chunks = Vec::new();
    collect_text_chunks(value, &mut text_chunks);
    for chunk in text_chunks {
        progresses.push(ClaudePaneTurnProgress {
            pane_id: plan.pane_id.clone(),
            phase: "assistant-text".to_string(),
            summary: "Claude assistant text.".to_string(),
            assistant_text_delta: Some(chunk),
            hint: None,
            elapsed_ms: elapsed_ms(started_at),
            artifact_path: plan.artifact_path.clone(),
            audit_path: plan.audit_path.clone(),
        });
    }
    let mut reasoning_events = Vec::new();
    collect_reasoning_events(value, &mut reasoning_events);
    for event in reasoning_events {
        progresses.push(ClaudePaneTurnProgress {
            pane_id: plan.pane_id.clone(),
            phase: "reasoning".to_string(),
            summary: format!("{}{}", CLAUDE_REASONING_PREFIX, event.preview.trim()),
            assistant_text_delta: None,
            hint: None,
            elapsed_ms: elapsed_ms(started_at),
            artifact_path: plan.artifact_path.clone(),
            audit_path: plan.audit_path.clone(),
        });
    }
    let mut tool_events = Vec::new();
    collect_tool_events(value, &mut tool_events);
    for event in tool_events {
        let summary = if event.preview.trim().is_empty() {
            format!("{}{}", CLAUDE_TOOL_CALL_PREFIX, event.name)
        } else {
            format!(
                "{}{}: {}",
                CLAUDE_TOOL_CALL_PREFIX,
                event.name,
                event.preview.trim()
            )
        };
        progresses.push(ClaudePaneTurnProgress {
            pane_id: plan.pane_id.clone(),
            phase: "tool-call".to_string(),
            summary,
            assistant_text_delta: None,
            hint: None,
            elapsed_ms: elapsed_ms(started_at),
            artifact_path: plan.artifact_path.clone(),
            audit_path: plan.audit_path.clone(),
        });
    }
    if value.get("is_error").and_then(Value::as_bool) == Some(true) {
        progresses.push(ClaudePaneTurnProgress {
            pane_id: plan.pane_id.clone(),
            phase: "error".to_string(),
            summary: "Claude reported an error result.".to_string(),
            assistant_text_delta: None,
            hint: Some(claude_error_summary(value)),
            elapsed_ms: elapsed_ms(started_at),
            artifact_path: plan.artifact_path.clone(),
            audit_path: plan.audit_path.clone(),
        });
        return progresses;
    }
    if value.get("result").and_then(Value::as_str).is_some() {
        progresses.push(ClaudePaneTurnProgress {
            pane_id: plan.pane_id.clone(),
            phase: "assistant-result".to_string(),
            summary: "Claude returned a result.".to_string(),
            assistant_text_delta: None,
            hint: None,
            elapsed_ms: elapsed_ms(started_at),
            artifact_path: plan.artifact_path.clone(),
            audit_path: plan.audit_path.clone(),
        });
        return progresses;
    }
    if value_type == "system" && value.get("subtype").and_then(Value::as_str) == Some("init") {
        progresses.push(ClaudePaneTurnProgress {
            pane_id: plan.pane_id.clone(),
            phase: "system".to_string(),
            summary: "Claude session initialized.".to_string(),
            assistant_text_delta: None,
            hint: value
                .get("session_id")
                .and_then(Value::as_str)
                .map(|session_id| format!("session_id: {session_id}")),
            elapsed_ms: elapsed_ms(started_at),
            artifact_path: plan.artifact_path.clone(),
            audit_path: plan.audit_path.clone(),
        });
    }
    progresses
}

pub(crate) fn progress_key(progress: &ClaudePaneTurnProgress) -> String {
    if progress.phase == "reasoning-tokens" {
        return format!(
            "{}\n{}",
            progress.phase,
            progress
                .hint
                .as_deref()
                .unwrap_or(progress.summary.as_str())
        );
    }
    format!(
        "{}\n{}\n{}",
        progress.phase,
        progress.summary,
        progress.hint.as_deref().unwrap_or_default()
    )
}
