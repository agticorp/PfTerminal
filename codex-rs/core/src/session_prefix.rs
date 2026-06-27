use codex_protocol::AgentPath;
use codex_protocol::protocol::AgentStatus;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;

use crate::context::ContextualUserFragment;
use crate::context::InterAgentCompletionMessage;
use crate::context::SubagentNotification;

const COMPLETION_MESSAGE_MAX_TOKENS: usize = 1_000;
const COMPLETION_MESSAGE_ENVELOPE_TOKEN_RESERVE: usize = 100;
const ERROR_MAX_TOKENS: usize =
    COMPLETION_MESSAGE_MAX_TOKENS - COMPLETION_MESSAGE_ENVELOPE_TOKEN_RESERVE;
const ERROR_NEXT_ACTION: &str = "This agent's turn failed. If you still need this agent, use the available collaboration tools to give it another task.";

// Helpers for model-visible session state markers that are stored in user-role
// messages but are not user intent.

// TODO(jif) unify with structured schema
pub(crate) fn format_subagent_notification_message(
    agent_reference: &str,
    status: &AgentStatus,
) -> String {
    SubagentNotification::new(agent_reference, status.clone()).render()
}

pub(crate) fn format_inter_agent_completion_message(
    task_name: AgentPath,
    sender: AgentPath,
    status: &AgentStatus,
) -> Option<String> {
    let payload = match status {
        AgentStatus::Completed(Some(message)) => message.clone(),
        AgentStatus::Completed(None) => String::new(),
        AgentStatus::Errored(error) => {
            let error = truncate_text(error, TruncationPolicy::Tokens(ERROR_MAX_TOKENS));
            format!("Agent errored: {error}\n\n{ERROR_NEXT_ACTION}")
        }
        AgentStatus::Shutdown => "Agent shut down.".to_string(),
        AgentStatus::NotFound => "Agent was not found.".to_string(),
        AgentStatus::PendingInit | AgentStatus::Running | AgentStatus::Interrupted => return None,
    };
    Some(InterAgentCompletionMessage::new(task_name, sender, payload).render())
}

#[cfg(test)]
#[path = "session_prefix_tests.rs"]
mod tests;

pub(crate) fn format_subagent_context_line(
    agent_reference: &str,
    agent_nickname: Option<&str>,
    agent_role: Option<&str>,
    agent_status: Option<&AgentStatus>,
    last_task_message: Option<&str>,
    last_result_message: Option<&str>,
) -> String {
    let mut line = match agent_nickname.filter(|nickname| !nickname.is_empty()) {
        Some(agent_nickname) => format!("- {agent_reference}: {agent_nickname}"),
        None => format!("- {agent_reference}"),
    };

    let mut details = Vec::new();
    if let Some(agent_role) = agent_role.filter(|role| !role.is_empty()) {
        details.push(format!("role={agent_role}"));
    }
    if let Some(agent_status) = agent_status {
        details.push(format!("status={}", subagent_status_label(agent_status)));
    }
    if let Some(last_task_message) = last_task_message.filter(|message| !message.trim().is_empty())
    {
        details.push(format!("task={last_task_message}"));
    }
    if let Some(last_result_message) =
        last_result_message.filter(|message| !message.trim().is_empty())
    {
        details.push(format!("result={last_result_message}"));
    }
    if !details.is_empty() {
        line.push_str(" (");
        line.push_str(&details.join("; "));
        line.push(')');
    }
    line
}

fn subagent_status_label(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::PendingInit => "pending",
        AgentStatus::Running => "running",
        AgentStatus::Interrupted => "interrupted",
        AgentStatus::Completed(_) => "done",
        AgentStatus::Errored(_) => "error",
        AgentStatus::Shutdown => "closed",
        AgentStatus::NotFound => "not found",
    }
}
