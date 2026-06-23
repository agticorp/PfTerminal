use super::*;
use codex_extension_api::ExtensionData;
use codex_extension_api::TurnItemContributor;
use codex_protocol::items::AgentMessageContent;
use pretty_assertions::assert_eq;
use std::sync::Arc;

struct RewriteAgentMessageContributor;

impl TurnItemContributor for RewriteAgentMessageContributor {
    fn contribute<'a>(
        &'a self,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
        item: &'a mut TurnItem,
    ) -> codex_extension_api::ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            if let TurnItem::AgentMessage(agent_message) = item {
                agent_message.content = vec![AgentMessageContent::Text {
                    text: "plan contributed assistant text".to_string(),
                }];
            }
            Ok(())
        })
    }
}

fn assistant_output_text(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: Some("msg-1".to_string()),
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        phase: None,
        metadata: None,
    }
}

#[test]
fn explicit_shell_command_budget_parses_common_phrasings() {
    let cases = [
        ("Use at most 5 shell commands.", Some(5)),
        ("Run no more than 4 commands, then answer.", Some(4)),
        ("Maximum of 3 shell commands for this review.", Some(3)),
        ("Max 2 commands.", Some(2)),
        ("Use 6 or fewer shell commands.", Some(6)),
        ("Use at most 0 shell commands.", None),
    ];

    for (text, expected) in cases {
        assert_eq!(
            explicit_shell_command_budget_from_text(text),
            expected,
            "{text}"
        );
    }
}

#[test]
fn explicit_shell_command_budget_ignores_non_command_numbers() {
    assert_eq!(
        explicit_shell_command_budget_from_text(
            "Review the 5 largest files and finish within 300 seconds."
        ),
        None
    );
    assert_eq!(
        explicit_shell_command_budget_from_text(
            "Use at most 7 shell commands, but no more than 5 commands if possible."
        ),
        Some(5)
    );
}

#[tokio::test]
async fn plan_mode_uses_contributed_turn_item_for_last_agent_message() {
    let (mut session, turn_context) = crate::session::tests::make_session_and_context().await;
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_item_contributor(Arc::new(RewriteAgentMessageContributor));
    session.services.extensions = Arc::new(builder.build());
    let turn_store = ExtensionData::new(turn_context.sub_id.clone());
    let mut state = PlanModeStreamState::new(&turn_context.sub_id);
    let mut last_agent_message = None;
    let item = assistant_output_text("original assistant text");

    let handled = handle_assistant_item_done_in_plan_mode(
        &session,
        &turn_context,
        &turn_store,
        &item,
        &mut state,
        /*previously_active_item*/ None,
        &mut last_agent_message,
    )
    .await;

    assert!(handled);
    assert_eq!(
        last_agent_message.as_deref(),
        Some("plan contributed assistant text")
    );
}
