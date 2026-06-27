use codex_protocol::AgentPath;
use codex_protocol::protocol::AgentStatus;
use codex_utils_output_truncation::approx_token_count;

use super::COMPLETION_MESSAGE_MAX_TOKENS;
use super::ERROR_NEXT_ACTION;
use super::format_inter_agent_completion_message;
use super::format_subagent_context_line;

#[test]
fn error_completion_message_stays_below_manual_review_threshold() {
    let message = format_inter_agent_completion_message(
        AgentPath::root(),
        AgentPath::try_from("/root/worker").expect("valid agent path"),
        &AgentStatus::Errored("stream disconnected ".repeat(1_000)),
    )
    .expect("error status should produce a completion message");

    assert!(approx_token_count(&message) < COMPLETION_MESSAGE_MAX_TOKENS);
    assert!(message.contains(ERROR_NEXT_ACTION));
}

#[test]
fn subagent_context_line_includes_management_fields() {
    let line = format_subagent_context_line(
        "orc_snaga",
        Some("Snaga"),
        Some("orc"),
        Some(&AgentStatus::Completed(Some("built animation".to_string()))),
        Some("wire animated website"),
        Some("created index.html and ran checks"),
    );

    assert_eq!(
        line,
        "- orc_snaga: Snaga (role=orc; status=done; task=wire animated website; result=created index.html and ran checks)"
    );
}
