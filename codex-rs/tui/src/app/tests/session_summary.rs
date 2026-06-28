use super::*;
use crate::app::test_support::make_test_app;
use crate::claude_panes::PANE_LAYOUT_VERSION;
use crate::claude_panes::PaneLayoutState;
use crate::claude_panes::persist_pane_layout;
use crate::session_state::ThreadSessionState;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn test_session(
    app: &App,
    thread_id: ThreadId,
    thread_name: Option<String>,
    rollout_path: Option<std::path::PathBuf>,
) -> ThreadSessionState {
    ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name,
        model: "gpt-test".to_string(),
        model_provider_id: app.config.model_provider_id.clone(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: app.config.approvals_reviewer,
        permission_profile: app.config.permissions.permission_profile().clone(),
        active_permission_profile: app.config.permissions.active_permission_profile(),
        cwd: app.config.cwd.clone(),
        runtime_workspace_roots: app.config.workspace_roots.clone(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: None,
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path,
    }
}

#[tokio::test]
async fn session_summary_skips_when_no_usage_or_resume_hint() {
    assert!(
        session_summary(
            TokenUsage::default(),
            /*thread_id*/ None,
            /*thread_name*/ None,
            /*rollout_path*/ None,
        )
        .is_none()
    );
}

#[tokio::test]
async fn exit_resume_hint_falls_back_to_primary_when_active_codex_pane_has_no_rollout() {
    let mut app = make_test_app().await;
    let temp_dir = tempdir().expect("temp dir");
    let primary_rollout_path = temp_dir.path().join("primary-rollout.jsonl");
    std::fs::write(&primary_rollout_path, "{}\n").expect("write primary rollout");
    let primary_thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174001").unwrap();
    let side_thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174002").unwrap();
    let primary_session = test_session(
        &app,
        primary_thread_id,
        None,
        Some(primary_rollout_path.clone()),
    );
    let side_session = test_session(
        &app,
        side_thread_id,
        Some("Codex 1".to_string()),
        /*rollout_path*/ None,
    );

    app.primary_thread_id = Some(primary_thread_id);
    app.primary_session_configured = Some(primary_session);
    app.active_thread_id = Some(side_thread_id);
    app.chat_widget.handle_thread_session(side_session);

    let exit_thread = app.exit_resumable_thread().expect("primary fallback");
    assert_eq!(exit_thread.thread_id, primary_thread_id);
    assert_eq!(
        resume_hint_for_thread(&exit_thread),
        Some("pfterminal resume 123e4567-e89b-12d3-a456-426614174001".to_string())
    );
}

#[tokio::test]
async fn exit_resume_hint_prefers_active_codex_pane_when_it_is_resumable() {
    let mut app = make_test_app().await;
    let temp_dir = tempdir().expect("temp dir");
    let primary_rollout_path = temp_dir.path().join("primary-rollout.jsonl");
    let side_rollout_path = temp_dir.path().join("side-rollout.jsonl");
    std::fs::write(&primary_rollout_path, "{}\n").expect("write primary rollout");
    std::fs::write(&side_rollout_path, "{}\n").expect("write side rollout");
    let primary_thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174003").unwrap();
    let side_thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174004").unwrap();
    let primary_session = test_session(
        &app,
        primary_thread_id,
        None,
        Some(primary_rollout_path.clone()),
    );
    let side_session = test_session(
        &app,
        side_thread_id,
        Some("Codex 1".to_string()),
        Some(side_rollout_path),
    );

    app.primary_thread_id = Some(primary_thread_id);
    app.primary_session_configured = Some(primary_session);
    app.active_thread_id = Some(side_thread_id);
    app.chat_widget.handle_thread_session(side_session);

    let exit_thread = app.exit_resumable_thread().expect("active thread");
    assert_eq!(exit_thread.thread_id, side_thread_id);
    assert_eq!(
        resume_hint_for_thread(&exit_thread),
        Some(
            "pfterminal resume, then select Codex 1 (123e4567-e89b-12d3-a456-426614174004)"
                .to_string()
        )
    );
}

#[tokio::test]
async fn session_summary_skips_resume_hint_until_rollout_exists() {
    let usage = TokenUsage::default();
    let conversation = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let temp_dir = tempdir().expect("temp dir");
    let rollout_path = temp_dir.path().join("rollout.jsonl");

    assert!(
        session_summary(
            usage,
            Some(conversation),
            /*thread_name*/ None,
            Some(&rollout_path),
        )
        .is_none()
    );
}

#[tokio::test]
async fn pane_layout_thread_resume_hint_does_not_require_rollout() {
    let app = make_test_app().await;
    let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174010").unwrap();
    let layout = PaneLayoutState {
        version: PANE_LAYOUT_VERSION,
        codex_thread_id: Some(thread_id.to_string()),
        active_user_pane_id: Some("claude-child".to_string()),
        spawn_nazgul_pane_id: None,
        claude_pane_ids: vec!["claude-child".to_string()],
        spawn_parent_by_node: BTreeMap::new(),
    };
    persist_pane_layout(app.config.codex_home.as_ref(), &layout).expect("persist layout");

    assert_eq!(
        resume_hint_for_pane_layout_thread(app.config.codex_home.as_ref(), Some(thread_id)),
        Some("pfterminal resume 123e4567-e89b-12d3-a456-426614174010".to_string())
    );
}

#[tokio::test]
async fn session_summary_includes_resume_hint_for_persisted_rollout() {
    let usage = TokenUsage {
        input_tokens: 10,
        output_tokens: 2,
        total_tokens: 12,
        ..Default::default()
    };
    let conversation = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let temp_dir = tempdir().expect("temp dir");
    let rollout_path = temp_dir.path().join("rollout.jsonl");
    std::fs::write(&rollout_path, "{}\n").expect("write rollout");

    let summary = session_summary(
        usage,
        Some(conversation),
        /*thread_name*/ None,
        Some(&rollout_path),
    )
    .expect("summary");
    assert_eq!(
        summary.usage_line,
        Some("Token usage: total=12 input=10 output=2".to_string())
    );
    assert_eq!(
        summary.resume_hint,
        Some("pfterminal resume 123e4567-e89b-12d3-a456-426614174000".to_string())
    );
}

#[tokio::test]
async fn session_summary_names_picker_item_when_thread_has_name() {
    let usage = TokenUsage {
        input_tokens: 10,
        output_tokens: 2,
        total_tokens: 12,
        ..Default::default()
    };
    let conversation = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let temp_dir = tempdir().expect("temp dir");
    let rollout_path = temp_dir.path().join("rollout.jsonl");
    std::fs::write(&rollout_path, "{}\n").expect("write rollout");

    let summary = session_summary(
        usage,
        Some(conversation),
        Some("my-session".to_string()),
        Some(&rollout_path),
    )
    .expect("summary");
    assert_eq!(
        summary.resume_hint,
        Some(
            "pfterminal resume, then select my-session (123e4567-e89b-12d3-a456-426614174000)"
                .to_string()
        )
    );
}
