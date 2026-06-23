#![cfg(not(target_os = "windows"))]

use core_test_support::test_codex::local_selections;
use std::fs;
use std::future::Future;

use assert_matches::assert_matches;
use codex_models_manager::bundled_models_response;
use codex_protocol::items::TurnItem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::TempDirExt;
use core_test_support::assert_regex_match;
use core_test_support::responses;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use serde_json::Value;
use serde_json::json;
fn call_output(req: &ResponsesRequest, call_id: &str) -> (String, Option<bool>) {
    let raw = req.function_call_output(call_id);
    assert_eq!(
        raw.get("call_id").and_then(Value::as_str),
        Some(call_id),
        "mismatched call_id in function_call_output"
    );
    let (content_opt, success) = req
        .function_call_output_content_and_success(call_id)
        .expect("function_call_output present");
    let content = content_opt.expect("function_call_output content present");
    (content, success)
}

fn custom_call_output(req: &ResponsesRequest, call_id: &str) -> (String, Option<bool>) {
    let raw = req.custom_tool_call_output(call_id);
    assert_eq!(
        raw.get("call_id").and_then(Value::as_str),
        Some(call_id),
        "mismatched call_id in custom_tool_call_output"
    );
    let (content_opt, success) = req
        .custom_tool_call_output_content_and_success(call_id)
        .expect("custom_tool_call_output present");
    let content = content_opt.expect("custom_tool_call_output content present");
    (content, success)
}

fn tool_names(req: &ResponsesRequest) -> Vec<String> {
    req.body_json()
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| {
            tool.get("name")
                .or_else(|| tool.get("type"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

fn configure_glm_model_slug(config: &mut codex_core::config::Config) {
    let mut catalog = bundled_models_response().expect("bundled models.json should parse");
    let mut model = catalog
        .models
        .iter()
        .find(|model| model.slug == "gpt-5.2")
        .cloned()
        .expect("bundled gpt-5.2 model exists");
    model.slug = "glm-5.2".to_string();
    model.display_name = "GLM 5.2".to_string();
    model.apply_patch_tool_type = None;
    catalog.models.push(model);
    config.model_catalog = Some(catalog);
    config.model = Some("glm-5.2".to_string());
}

fn run_tool_harness_test<F, Fut>(name: &'static str, make_future: F) -> anyhow::Result<()>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<()>> + 'static,
{
    std::thread::Builder::new()
        .name(name.to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime should build");
            runtime.block_on(make_future())
        })
        .expect("tool harness test thread should spawn")
        .join()
        .expect("tool harness test thread should not panic")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_tool_executes_command_and_streams_output() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_model("test-gpt-5-codex");
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "shell-command-tool-call";
    let command_args = json!({
        "command": "echo tool harness",
        "login": false,
    })
    .to_string();
    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "shell_command", &command_args),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "all done"),
        ev_completed("resp-2"),
    ]);
    let second_mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();
    let cwd_path = cwd.abs();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd_path.as_path());

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "please run the shell command".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(cwd_path)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let req = second_mock.single_request();
    let (output_text, _) = call_output(&req, call_id);
    assert_regex_match(
        r"(?s)^Exit code: 0\nWall time: [0-9]+(?:\.[0-9]+)? seconds\nOutput:\ntool harness\n?$",
        &output_text,
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_plan_tool_emits_plan_update_event() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex();
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "plan-tool-call";
    let plan_args = json!({
        "explanation": "Tool harness check",
        "plan": [
            {"step": "Inspect workspace", "status": "in_progress"},
            {"step": "Report results", "status": "pending"},
        ],
    })
    .to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "update_plan", &plan_args),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "plan acknowledged"),
        ev_completed("resp-2"),
    ]);
    let second_mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();
    let cwd_path = cwd.abs();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd_path.as_path());

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "please update the plan".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(cwd_path)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    let mut saw_plan_update = false;
    wait_for_event(&codex, |event| match event {
        EventMsg::PlanUpdate(update) => {
            saw_plan_update = true;
            assert_eq!(update.explanation.as_deref(), Some("Tool harness check"));
            assert_eq!(update.plan.len(), 2);
            assert_eq!(update.plan[0].step, "Inspect workspace");
            assert_matches!(update.plan[0].status, StepStatus::InProgress);
            assert_eq!(update.plan[1].step, "Report results");
            assert_matches!(update.plan[1].status, StepStatus::Pending);
            false
        }
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    assert!(saw_plan_update, "expected PlanUpdate event");

    let req = second_mock.single_request();
    let (output_text, _success_flag) = call_output(&req, call_id);
    assert_eq!(output_text, "Plan updated");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_plan_tool_rejects_malformed_payload() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex();
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "plan-tool-invalid";
    let invalid_args = json!({
        "explanation": "Missing plan data"
    })
    .to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "update_plan", &invalid_args),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "malformed plan payload"),
        ev_completed("resp-2"),
    ]);
    let second_mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();
    let cwd_path = cwd.abs();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd_path.as_path());

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "please update the plan".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(cwd_path)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    let mut saw_plan_update = false;
    wait_for_event(&codex, |event| match event {
        EventMsg::PlanUpdate(_) => {
            saw_plan_update = true;
            false
        }
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    assert!(
        !saw_plan_update,
        "did not expect PlanUpdate event for malformed payload"
    );

    let req = second_mock.single_request();
    let (output_text, success_flag) = call_output(&req, call_id);
    assert!(
        output_text.contains("failed to parse function arguments"),
        "expected parse error message in output text, got {output_text:?}"
    );
    if let Some(success_flag) = success_flag {
        assert!(
            !success_flag,
            "expected tool output to mark success=false for malformed payload"
        );
    }

    Ok(())
}

#[test]
fn apply_patch_tool_executes_and_emits_patch_events() -> anyhow::Result<()> {
    run_tool_harness_test(
        "apply-patch-tool-harness",
        apply_patch_tool_executes_and_emits_patch_events_inner,
    )
}

async fn apply_patch_tool_executes_and_emits_patch_events_inner() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_model("gpt-5.2");
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let file_name = "notes.txt";
    let file_path = cwd.path().join(file_name);
    let call_id = "apply-patch-call";
    let patch_content = format!(
        r#"*** Begin Patch
*** Add File: {file_name}
+Tool harness apply patch
*** End Patch"#
    );

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_apply_patch_custom_tool_call(call_id, &patch_content),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "patch complete"),
        ev_completed("resp-2"),
    ]);
    let second_mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();
    let cwd_path = cwd.abs();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd_path.as_path());

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "please apply a patch".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(cwd_path)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    let mut saw_file_change_started = false;
    let mut saw_file_change_completed = false;
    let mut saw_patch_begin = false;
    let mut patch_end_success = None;
    wait_for_event(&codex, |event| match event {
        EventMsg::ItemStarted(started) => {
            if let TurnItem::FileChange(item) = &started.item {
                saw_file_change_started = true;
                assert_eq!(item.id, call_id);
                assert_eq!(item.status, None);
            }
            false
        }
        EventMsg::ItemCompleted(completed) => {
            if let TurnItem::FileChange(item) = &completed.item {
                saw_file_change_completed = true;
                assert_eq!(item.id, call_id);
                assert_eq!(
                    item.status,
                    Some(codex_protocol::protocol::PatchApplyStatus::Completed)
                );
            }
            false
        }
        EventMsg::PatchApplyBegin(begin) => {
            saw_patch_begin = true;
            assert_eq!(begin.call_id, call_id);
            false
        }
        EventMsg::PatchApplyEnd(end) => {
            assert_eq!(end.call_id, call_id);
            patch_end_success = Some(end.success);
            false
        }
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    assert!(
        saw_file_change_started,
        "expected ItemStarted for TurnItem::FileChange"
    );
    assert!(
        saw_file_change_completed,
        "expected ItemCompleted for TurnItem::FileChange"
    );
    assert!(saw_patch_begin, "expected PatchApplyBegin event");
    let patch_end_success =
        patch_end_success.expect("expected PatchApplyEnd event to capture success flag");
    assert!(patch_end_success);

    let req = second_mock.single_request();
    let (output_text, _success_flag) = custom_call_output(&req, call_id);

    let expected_pattern = format!(
        r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
A {file_name}
?$"
    );
    assert_regex_match(&expected_pattern, &output_text);

    let updated_contents = fs::read_to_string(file_path)?;
    assert_eq!(
        updated_contents, "Tool harness apply patch\n",
        "expected updated file content"
    );

    Ok(())
}

#[test]
fn structured_edit_tool_executes_via_apply_patch_events() -> anyhow::Result<()> {
    run_tool_harness_test(
        "structured-edit-tool-harness",
        structured_edit_tool_executes_via_apply_patch_events_inner,
    )
}

async fn structured_edit_tool_executes_via_apply_patch_events_inner() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(configure_glm_model_slug);
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let file_name = "structured-notes.txt";
    let file_path = cwd.path().join(file_name);
    fs::write(&file_path, "alpha\nbeta\ngamma\n")?;
    let call_id = "structured-edit-call";
    let edit_args = json!({
        "path": file_name,
        "old_string": "beta\n",
        "new_string": "BETA\n",
        "replace_all": false,
    })
    .to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "structured_edit", &edit_args),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "structured edit complete"),
        ev_completed("resp-2"),
    ]);
    let second_mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();
    let cwd_path = cwd.abs();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd_path.as_path());

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "please apply a structured edit".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(cwd_path)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    let mut saw_file_change_started = false;
    let mut saw_file_change_completed = false;
    let mut saw_patch_begin = false;
    let mut patch_end_success = None;
    wait_for_event(&codex, |event| match event {
        EventMsg::ItemStarted(started) => {
            if let TurnItem::FileChange(item) = &started.item {
                saw_file_change_started = true;
                assert_eq!(item.id, call_id);
            }
            false
        }
        EventMsg::ItemCompleted(completed) => {
            if let TurnItem::FileChange(item) = &completed.item {
                saw_file_change_completed = true;
                assert_eq!(item.id, call_id);
                assert_eq!(
                    item.status,
                    Some(codex_protocol::protocol::PatchApplyStatus::Completed)
                );
            }
            false
        }
        EventMsg::PatchApplyBegin(begin) => {
            saw_patch_begin = true;
            assert_eq!(begin.call_id, call_id);
            false
        }
        EventMsg::PatchApplyEnd(end) => {
            assert_eq!(end.call_id, call_id);
            patch_end_success = Some(end.success);
            false
        }
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    let req = second_mock.single_request();
    let (output_text, success_flag) = call_output(&req, call_id);
    if let Some(success_flag) = success_flag {
        assert!(success_flag);
    }

    assert!(
        saw_file_change_started,
        "expected ItemStarted for TurnItem::FileChange; output was {output_text:?}"
    );
    assert!(
        saw_file_change_completed,
        "expected ItemCompleted for TurnItem::FileChange; output was {output_text:?}"
    );
    assert!(
        saw_patch_begin,
        "expected PatchApplyBegin event; output was {output_text:?}"
    );
    assert_eq!(patch_end_success, Some(true));

    let expected_pattern = format!(
        r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
M {file_name}
?$"
    );
    assert_regex_match(&expected_pattern, &output_text);

    let updated_contents = fs::read_to_string(file_path)?;
    assert_eq!(updated_contents, "alpha\nBETA\ngamma\n");

    Ok(())
}

#[test]
fn structured_write_tool_creates_file_via_apply_patch_events() -> anyhow::Result<()> {
    run_tool_harness_test(
        "structured-write-tool-harness",
        structured_write_tool_creates_file_via_apply_patch_events_inner,
    )
}

async fn structured_write_tool_creates_file_via_apply_patch_events_inner() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(configure_glm_model_slug);
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let file_name = "structured-created.txt";
    let file_path = cwd.path().join(file_name);
    let call_id = "structured-write-call";
    let write_args = json!({
        "path": file_name,
        "content": "created by structured_write\n",
        "mode": "create_only",
    })
    .to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "structured_write", &write_args),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "structured write complete"),
        ev_completed("resp-2"),
    ]);
    let second_mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();
    let cwd_path = cwd.abs();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd_path.as_path());

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "please create a file with structured_write".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(cwd_path)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    let mut saw_file_change_started = false;
    let mut saw_file_change_completed = false;
    let mut saw_patch_begin = false;
    let mut patch_end_success = None;
    wait_for_event(&codex, |event| match event {
        EventMsg::ItemStarted(started) => {
            if let TurnItem::FileChange(item) = &started.item {
                saw_file_change_started = true;
                assert_eq!(item.id, call_id);
            }
            false
        }
        EventMsg::ItemCompleted(completed) => {
            if let TurnItem::FileChange(item) = &completed.item {
                saw_file_change_completed = true;
                assert_eq!(item.id, call_id);
                assert_eq!(
                    item.status,
                    Some(codex_protocol::protocol::PatchApplyStatus::Completed)
                );
            }
            false
        }
        EventMsg::PatchApplyBegin(begin) => {
            saw_patch_begin = true;
            assert_eq!(begin.call_id, call_id);
            false
        }
        EventMsg::PatchApplyEnd(end) => {
            assert_eq!(end.call_id, call_id);
            patch_end_success = Some(end.success);
            false
        }
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    let req = second_mock.single_request();
    let (output_text, success_flag) = call_output(&req, call_id);
    if let Some(success_flag) = success_flag {
        assert!(success_flag);
    }

    assert!(
        saw_file_change_started,
        "expected ItemStarted for TurnItem::FileChange; output was {output_text:?}"
    );
    assert!(
        saw_file_change_completed,
        "expected ItemCompleted for TurnItem::FileChange; output was {output_text:?}"
    );
    assert!(
        saw_patch_begin,
        "expected PatchApplyBegin event; output was {output_text:?}"
    );
    assert_eq!(patch_end_success, Some(true));

    let expected_pattern = format!(
        r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
A {file_name}
?$"
    );
    assert_regex_match(&expected_pattern, &output_text);

    let updated_contents = fs::read_to_string(file_path)?;
    assert_eq!(updated_contents, "created by structured_write\n");

    Ok(())
}

#[test]
fn apply_patch_reports_parse_diagnostics() -> anyhow::Result<()> {
    run_tool_harness_test(
        "apply-patch-parse-diagnostics",
        apply_patch_reports_parse_diagnostics_inner,
    )
}

async fn apply_patch_reports_parse_diagnostics_inner() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_model("gpt-5.2");
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "apply-patch-parse-error";
    let second_call_id = "apply-patch-parse-error-2";
    let patch_content = r"*** Begin Patch
*** Update File: broken.txt
*** End Patch";

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_apply_patch_custom_tool_call(call_id, patch_content),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_response_created("resp-2"),
        ev_apply_patch_custom_tool_call(second_call_id, patch_content),
        ev_completed("resp-2"),
    ]);
    let second_mock = responses::mount_sse_once(&server, second_response).await;

    let third_response = sse(vec![
        ev_response_created("resp-3"),
        ev_assistant_message("msg-1", "failed"),
        ev_completed("resp-3"),
    ]);
    let third_mock = responses::mount_sse_once(&server, third_response).await;

    let session_model = session_configured.model.clone();
    let cwd_path = cwd.abs();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd_path.as_path());

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "please apply a patch".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(cwd_path)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let req = second_mock.single_request();
    let (output_text, success_flag) = custom_call_output(&req, call_id);
    let second_request_tools = tool_names(&req);

    assert!(
        output_text.contains("apply_patch verification failed"),
        "expected apply_patch verification failure message, got {output_text:?}"
    );
    assert!(
        output_text.contains("invalid hunk"),
        "expected parse diagnostics in output text, got {output_text:?}"
    );

    if let Some(success_flag) = success_flag {
        assert!(
            !success_flag,
            "expected tool output to mark success=false for parse failures"
        );
    }

    assert!(
        second_request_tools.contains(&"apply_patch".to_string()),
        "first parse failure should keep strict apply_patch available for one repair attempt; tools were {second_request_tools:?}"
    );
    assert!(
        !second_request_tools.contains(&"structured_edit".to_string()),
        "structured_edit should not be visible until the repeated-failure threshold; tools were {second_request_tools:?}"
    );

    let req = third_mock.single_request();
    let (output_text, success_flag) = custom_call_output(&req, second_call_id);
    let third_request_tools = tool_names(&req);

    assert!(
        output_text.contains("Repeated apply_patch grammar failures detected (2)"),
        "expected structured-edit fallback guidance after repeated parse failures, got {output_text:?}"
    );
    if let Some(success_flag) = success_flag {
        assert!(
            !success_flag,
            "expected second parse failure output to mark success=false"
        );
    }
    assert!(
        third_request_tools.contains(&"structured_edit".to_string()),
        "repeated parse failures should expose structured_edit; tools were {third_request_tools:?}"
    );
    assert!(
        third_request_tools.contains(&"structured_write".to_string()),
        "repeated parse failures should expose structured_write; tools were {third_request_tools:?}"
    );
    assert!(
        !third_request_tools.contains(&"apply_patch".to_string()),
        "repeated parse failures should hide apply_patch; tools were {third_request_tools:?}"
    );

    Ok(())
}
