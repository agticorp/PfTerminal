//! Parsing of Claude Code stream-json output into structured turn results.

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use serde_json::Value;

use super::pane::ClaudePaneTurnStatus;
use super::progress::dedupe_tool_names;
use super::progress::usage_summary_from_value;
use super::progress_summarize::string_field;
use super::progress_summarize::summarize_reasoning_text;
use super::progress_summarize::summarize_tool_call_input;
use super::turn_types::ClaudePaneReasoningEvent;
use super::turn_types::ClaudePaneToolEvent;
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedClaudeOutput {
    pub(crate) text: String,
    pub(crate) status: ClaudePaneTurnStatus,
    pub(crate) session_id: Option<String>,
    pub(crate) usage_summary: Option<String>,
    pub(crate) terminal_reason: Option<String>,
    pub(crate) error_summary: Option<String>,
    pub(crate) tool_names: Vec<String>,
    pub(crate) tool_events: Vec<ClaudePaneToolEvent>,
    pub(crate) reasoning_events: Vec<ClaudePaneReasoningEvent>,
}

pub(crate) fn parse_claude_output(stdout: &str) -> Result<ParsedClaudeOutput> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Claude returned empty output"));
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return parsed_from_value(&value);
    }

    let mut assistant_chunks = Vec::new();
    let mut final_result = None;
    let mut session_id = None;
    let mut usage_summary = None;
    let mut error_value = None;
    let mut saw_result_event = false;
    let mut tool_names = Vec::new();
    let mut tool_events = Vec::new();
    let mut reasoning_events = Vec::new();
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value = serde_json::from_str(line)
            .with_context(|| format!("Claude stream-json line was not valid JSON: {line}"))?;
        if value.get("is_error").and_then(Value::as_bool) == Some(true) {
            error_value = Some(value.clone());
        }
        collect_text_chunks(&value, &mut assistant_chunks);
        collect_reasoning_events(&value, &mut reasoning_events);
        collect_tool_names(&value, &mut tool_names);
        collect_tool_events(&value, &mut tool_events);
        if let Some(result) = value.get("result").and_then(Value::as_str) {
            saw_result_event = true;
            final_result = Some(result.to_string());
        }
        if session_id.is_none() {
            session_id = value
                .get("session_id")
                .and_then(Value::as_str)
                .map(ToString::to_string);
        }
        if usage_summary.is_none() {
            usage_summary = usage_summary_from_value(&value);
        }
    }

    if let Some(error_value) = error_value {
        let text = assistant_chunks.join("");
        let status = claude_error_status(&error_value);
        return Ok(ParsedClaudeOutput {
            text,
            status,
            session_id,
            usage_summary,
            terminal_reason: error_value
                .get("terminal_reason")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            error_summary: Some(claude_error_summary(&error_value)),
            tool_names: dedupe_tool_names(tool_names),
            tool_events,
            reasoning_events,
        });
    }

    if !saw_result_event {
        return Err(anyhow!(
            "Claude stream ended before a final result event; the turn is incomplete"
        ));
    }

    let text = final_result
        .filter(|result| !result.trim().is_empty())
        .unwrap_or_else(|| assistant_chunks.join(""));
    if text.trim().is_empty() {
        return Err(anyhow!("Claude returned no assistant text"));
    }
    Ok(ParsedClaudeOutput {
        text,
        status: ClaudePaneTurnStatus::Success,
        session_id,
        usage_summary,
        terminal_reason: None,
        error_summary: None,
        tool_names: dedupe_tool_names(tool_names),
        tool_events,
        reasoning_events,
    })
}

pub(crate) fn parsed_from_value(value: &Value) -> Result<ParsedClaudeOutput> {
    if value.get("is_error").and_then(Value::as_bool) == Some(true) {
        let mut tool_names = Vec::new();
        let mut tool_events = Vec::new();
        let mut reasoning_events = Vec::new();
        collect_reasoning_events(value, &mut reasoning_events);
        collect_tool_names(value, &mut tool_names);
        collect_tool_events(value, &mut tool_events);
        return Ok(ParsedClaudeOutput {
            text: String::new(),
            status: claude_error_status(value),
            session_id: value
                .get("session_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            usage_summary: usage_summary_from_value(value),
            terminal_reason: value
                .get("terminal_reason")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            error_summary: Some(claude_error_summary(value)),
            tool_names: dedupe_tool_names(tool_names),
            tool_events,
            reasoning_events,
        });
    }
    let mut assistant_chunks = Vec::new();
    collect_text_chunks(value, &mut assistant_chunks);
    let mut tool_names = Vec::new();
    let mut tool_events = Vec::new();
    let mut reasoning_events = Vec::new();
    collect_reasoning_events(value, &mut reasoning_events);
    collect_tool_names(value, &mut tool_names);
    collect_tool_events(value, &mut tool_events);
    let text = value
        .get("result")
        .and_then(Value::as_str)
        .filter(|result| !result.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| assistant_chunks.join(""));
    if text.trim().is_empty() {
        return Err(anyhow!("Claude returned no assistant text"));
    }
    Ok(ParsedClaudeOutput {
        text,
        status: ClaudePaneTurnStatus::Success,
        session_id: value
            .get("session_id")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        usage_summary: usage_summary_from_value(value),
        terminal_reason: None,
        error_summary: None,
        tool_names: dedupe_tool_names(tool_names),
        tool_events,
        reasoning_events,
    })
}

pub(crate) fn claude_error_status(value: &Value) -> ClaudePaneTurnStatus {
    let subtype = value.get("subtype").and_then(Value::as_str);
    let terminal_reason = value.get("terminal_reason").and_then(Value::as_str);
    if subtype == Some("error_max_turns") || terminal_reason == Some("max_turns") {
        ClaudePaneTurnStatus::MaxTurnsPause
    } else {
        ClaudePaneTurnStatus::ProviderError
    }
}

pub(crate) fn claude_error_summary(value: &Value) -> String {
    let subtype = value
        .get("subtype")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let terminal_reason = value
        .get("terminal_reason")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let errors = value
        .get("errors")
        .and_then(Value::as_array)
        .map(|errors| {
            errors
                .iter()
                .filter_map(Value::as_str)
                .take(3)
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|errors| !errors.is_empty())
        .or_else(|| {
            value
                .get("result")
                .and_then(Value::as_str)
                .filter(|result| !result.trim().is_empty())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "no error details".to_string());
    format!("{subtype}; terminal_reason={terminal_reason}; {errors}")
}

pub(crate) fn collect_text_chunks(value: &Value, chunks: &mut Vec<String>) {
    if let Some(text) = value.get("text").and_then(Value::as_str)
        && value.get("type").and_then(Value::as_str) == Some("text")
    {
        chunks.push(text.to_string());
    }
    if let Some(content) = value.pointer("/message/content").and_then(Value::as_array) {
        for item in content {
            if item.get("type").and_then(Value::as_str) == Some("text")
                && let Some(text) = item.get("text").and_then(Value::as_str)
            {
                chunks.push(text.to_string());
            }
        }
    }
    if let Some(delta_text) = value.pointer("/delta/text").and_then(Value::as_str) {
        chunks.push(delta_text.to_string());
    }
}

pub(crate) fn collect_reasoning_events(value: &Value, events: &mut Vec<ClaudePaneReasoningEvent>) {
    if value.get("type").and_then(Value::as_str) == Some("thinking")
        && let Some(preview) = reasoning_preview_from_value(value)
    {
        events.push(ClaudePaneReasoningEvent { preview });
    }
    if let Some(content) = value.pointer("/message/content").and_then(Value::as_array) {
        for item in content {
            if item.get("type").and_then(Value::as_str) == Some("thinking")
                && let Some(preview) = reasoning_preview_from_value(item)
            {
                events.push(ClaudePaneReasoningEvent { preview });
            }
        }
    }
    if let Some(delta) = value.pointer("/delta/thinking").and_then(Value::as_str) {
        let preview = summarize_reasoning_text(delta);
        if !preview.is_empty() {
            events.push(ClaudePaneReasoningEvent { preview });
        }
    }
}

pub(crate) fn reasoning_preview_from_value(value: &Value) -> Option<String> {
    let text = string_field(value, &["thinking", "text", "content"])?;
    let preview = summarize_reasoning_text(text);
    (!preview.is_empty()).then_some(preview)
}

pub(crate) fn collect_tool_names(value: &Value, tool_names: &mut Vec<String>) {
    if let Some(name) = value.get("name").and_then(Value::as_str)
        && value.get("type").and_then(Value::as_str) == Some("tool_use")
    {
        tool_names.push(name.to_string());
    }
    if let Some(content) = value.pointer("/message/content").and_then(Value::as_array) {
        for item in content {
            if item.get("type").and_then(Value::as_str) == Some("tool_use")
                && let Some(name) = item.get("name").and_then(Value::as_str)
            {
                tool_names.push(name.to_string());
            }
        }
    }
}

pub(crate) fn collect_tool_events(value: &Value, tool_events: &mut Vec<ClaudePaneToolEvent>) {
    if value.get("type").and_then(Value::as_str) == Some("tool_use")
        && let Some(name) = value.get("name").and_then(Value::as_str)
    {
        let preview = value
            .get("input")
            .map(|input| summarize_tool_call_input(name, input))
            .unwrap_or_default();
        tool_events.push(ClaudePaneToolEvent {
            name: name.to_string(),
            preview,
        });
    }
    if let Some(content) = value.pointer("/message/content").and_then(Value::as_array) {
        for item in content {
            if item.get("type").and_then(Value::as_str) == Some("tool_use")
                && let Some(name) = item.get("name").and_then(Value::as_str)
            {
                let preview = item
                    .get("input")
                    .map(|input| summarize_tool_call_input(name, input))
                    .unwrap_or_default();
                tool_events.push(ClaudePaneToolEvent {
                    name: name.to_string(),
                    preview,
                });
            }
        }
    }
}
