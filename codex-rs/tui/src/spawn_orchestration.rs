use crate::app::App;
use crate::app_event::AppEvent;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::custom_prompt_view::CustomPromptView;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::claude_panes::CODEX_MAIN_PANE_ID;
use crate::collaboration_modes;
use crate::multi_agents::agent_picker_status_dot_spans;
use crate::multi_agents::format_agent_picker_item_name;
use codex_protocol::ThreadId;

const TROLL_ROLE: &str = "troll";
const ORC_ROLE: &str = "orc";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpawnRole {
    Nazgul,
    Troll,
    Orc,
}

impl SpawnRole {
    fn label(self) -> &'static str {
        match self {
            Self::Nazgul => "Nazgul",
            Self::Troll => "Troll",
            Self::Orc => "Orc",
        }
    }

    fn agent_type(self) -> Option<&'static str> {
        match self {
            Self::Nazgul => None,
            Self::Troll => Some(TROLL_ROLE),
            Self::Orc => Some(ORC_ROLE),
        }
    }

    fn task_placeholder(self) -> &'static str {
        match self {
            Self::Nazgul => "",
            Self::Troll => "Describe the supervisory task for the Troll",
            Self::Orc => "Describe the concrete execution task for the Orc",
        }
    }
}

impl App {
    pub(crate) fn open_spawn_role_picker(&mut self) {
        let items = vec![
            self.spawn_role_item(SpawnRole::Nazgul),
            self.spawn_role_item(SpawnRole::Troll),
            self.spawn_role_item(SpawnRole::Orc),
            section_item("Status"),
            SelectionItem {
                name: "Spawn status".to_string(),
                description: Some("Show Nazgul -> Troll -> Orc hierarchy.".to_string()),
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::OpenSpawnStatus);
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        self.chat_widget.show_selection_view(SelectionViewParams {
            title: Some("Spawn".to_string()),
            subtitle: Some("Create supervised native agents or bind a root pane.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            is_searchable: true,
            search_placeholder: Some("Search spawn roles".to_string()),
            ..Default::default()
        });
    }

    pub(crate) fn open_spawn_nazgul_pane_picker(&mut self) {
        let mut items = Vec::new();
        items.push(section_item("Existing User Panes"));
        items.push(self.nazgul_pane_item(
            CODEX_MAIN_PANE_ID.to_string(),
            "Codex - Main".to_string(),
            "Current PFTerminal/Codex session".to_string(),
        ));
        for pane in self.claude_panes.panes() {
            items.push(self.nazgul_pane_item(
                pane.id.clone(),
                pane.title.clone(),
                "Claude Code headless pane".to_string(),
            ));
        }

        self.chat_widget.show_selection_view(SelectionViewParams {
            title: Some("Bind Nazgul Pane".to_string()),
            subtitle: Some("Select an existing user pane to act as the visible root.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            is_searchable: true,
            search_placeholder: Some("Search user panes".to_string()),
            ..Default::default()
        });
    }

    pub(crate) fn bind_spawn_nazgul_pane(&mut self, pane_id: String) {
        self.spawn_nazgul_pane_id = Some(pane_id.clone());
        let title = self.user_pane_title(&pane_id);
        self.chat_widget.add_info_message(
            format!("Bound {title} as Nazgul root."),
            Some("No worker was spawned.".to_string()),
        );
    }

    pub(crate) fn open_spawn_harness_picker(&mut self, role: SpawnRole) {
        self.chat_widget.show_selection_view(SelectionViewParams {
            title: Some(format!("Spawn {}", role.label())),
            subtitle: Some("Choose harness.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items: vec![SelectionItem {
                name: "PFTerminal Agent".to_string(),
                description: Some("Native multi-agent runtime; P0 supported harness.".to_string()),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenSpawnModelPicker { role });
                })],
                dismiss_on_select: true,
                ..Default::default()
            }],
            ..Default::default()
        });
    }

    pub(crate) fn open_spawn_model_picker(&mut self, role: SpawnRole) {
        let model = self.chat_widget.current_model().to_string();
        let effort = self
            .chat_widget
            .current_reasoning_effort()
            .map(|effort| format!("{effort:?}"))
            .unwrap_or_else(|| "default".to_string());
        self.chat_widget.show_selection_view(SelectionViewParams {
            title: Some(format!("Spawn {}", role.label())),
            subtitle: Some("Use current model and effort for the native agent.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items: vec![SelectionItem {
                name: format!("{model} · {effort}"),
                description: Some("Inherited from current PFTerminal model selection.".to_string()),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenSpawnTaskPrompt { role });
                })],
                dismiss_on_select: true,
                ..Default::default()
            }],
            ..Default::default()
        });
    }

    pub(crate) fn open_spawn_task_prompt(&mut self, role: SpawnRole) {
        let tx = self.app_event_tx.clone();
        let view = CustomPromptView::new(
            format!("Spawn {}", role.label()),
            role.task_placeholder().to_string(),
            String::new(),
            Some("Task".to_string()),
            Box::new(move |task| {
                tx.send(AppEvent::SubmitSpawnTask { role, task });
            }),
        );
        self.chat_widget.show_custom_prompt_view(view);
    }

    pub(crate) fn submit_spawn_task(&mut self, role: SpawnRole, task: String) {
        let task = task.trim();
        if task.is_empty() {
            self.chat_widget
                .add_error_message("Spawn task cannot be empty.".to_string());
            return;
        }
        if let Some(error) = self.spawn_role_disabled_reason(role) {
            self.chat_widget.add_error_message(error);
            return;
        }
        let Some(agent_type) = role.agent_type() else {
            self.chat_widget
                .add_error_message("Nazgul is a pane binding, not a spawned worker.".to_string());
            return;
        };
        let model_catalog = self.chat_widget.model_catalog();
        let Some(mask) = collaboration_modes::default_mode_mask(model_catalog.as_ref())
            .or_else(|| collaboration_modes::default_mask(model_catalog.as_ref()))
        else {
            self.chat_widget
                .add_error_message("Default collaboration mode unavailable.".to_string());
            return;
        };
        let instruction = spawn_instruction(role, agent_type, task);
        self.chat_widget
            .submit_user_message_with_mode(instruction, mask);
    }

    pub(crate) fn open_spawn_status(&mut self) {
        self.chat_widget.show_selection_view(SelectionViewParams {
            title: Some("Spawn Status".to_string()),
            subtitle: Some("Nazgul -> Troll -> Orc hierarchy.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items: self.spawn_tree_items(),
            is_searchable: true,
            search_placeholder: Some("Search spawned work".to_string()),
            ..Default::default()
        });
    }

    pub(crate) fn spawn_tree_items(&self) -> Vec<SelectionItem> {
        let mut items = Vec::new();
        items.push(section_item("Nazgul"));
        let bound_pane_id = self
            .spawn_nazgul_pane_id
            .as_deref()
            .unwrap_or(CODEX_MAIN_PANE_ID);
        items.push(SelectionItem {
            name: format!("Nazgul: {}", self.user_pane_title(bound_pane_id)),
            description: Some("Bound root pane; no worker thread.".to_string()),
            is_current: self.claude_panes.active_user_pane_id() == bound_pane_id,
            actions: vec![Box::new({
                let pane_id = bound_pane_id.to_string();
                move |tx| {
                    tx.send(AppEvent::SelectUserPane {
                        pane_id: pane_id.clone(),
                    });
                }
            })],
            dismiss_on_select: true,
            ..Default::default()
        });

        let trolls = self.spawn_troll_threads();
        items.push(section_item("Trolls"));
        if trolls.is_empty() {
            items.push(disabled_item("No Trolls spawned yet"));
        }
        for (troll_thread_id, troll_entry) in trolls {
            items.push(self.spawn_agent_item(troll_thread_id, troll_entry, 0, Some(TROLL_ROLE)));
            let orcs = self.spawn_orc_children(troll_thread_id);
            if orcs.is_empty() {
                items.push(disabled_item("  No Orcs for this Troll yet"));
            }
            for (orc_thread_id, orc_entry) in orcs {
                items.push(self.spawn_agent_item(orc_thread_id, orc_entry, 2, Some(ORC_ROLE)));
            }
        }

        let orphan_orcs = self
            .spawn_threads_with_role(ORC_ROLE)
            .into_iter()
            .filter(|(thread_id, _)| {
                self.spawn_parent_by_thread
                    .get(thread_id)
                    .and_then(|parent| self.agent_navigation.get(parent))
                    .and_then(|entry| entry.agent_role.as_deref())
                    != Some(TROLL_ROLE)
            })
            .collect::<Vec<_>>();
        if !orphan_orcs.is_empty() {
            items.push(section_item("Unlinked Orcs"));
            for (thread_id, entry) in orphan_orcs {
                items.push(self.spawn_agent_item(thread_id, entry, 0, Some(ORC_ROLE)));
            }
        }

        items
    }

    fn spawn_role_item(&self, role: SpawnRole) -> SelectionItem {
        let disabled_reason = self.spawn_role_disabled_reason(role);
        let disabled = disabled_reason.is_some();
        SelectionItem {
            name: role.label().to_string(),
            description: Some(match role {
                SpawnRole::Nazgul => "Bind root pane.".to_string(),
                SpawnRole::Troll => "Supervisor that may spawn and review Orcs.".to_string(),
                SpawnRole::Orc => "Executor under the active Troll.".to_string(),
            }),
            is_disabled: disabled,
            disabled_reason,
            actions: if disabled {
                Vec::new()
            } else if role == SpawnRole::Nazgul {
                vec![Box::new(|tx| {
                    tx.send(AppEvent::OpenSpawnNazgulPanePicker);
                })]
            } else {
                vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenSpawnHarnessPicker { role });
                })]
            },
            dismiss_on_select: true,
            ..Default::default()
        }
    }

    fn spawn_role_disabled_reason(&self, role: SpawnRole) -> Option<String> {
        match role {
            SpawnRole::Nazgul => None,
            SpawnRole::Troll => {
                if self.claude_panes.active_claude_pane_id().is_some() {
                    return Some(
                        "Switch to Codex - Main or a native agent pane before spawning native agents."
                            .to_string(),
                    );
                }
                match self.current_agent_role().as_deref() {
                    Some(TROLL_ROLE) => Some("A Troll may only spawn Orcs.".to_string()),
                    Some(ORC_ROLE) => Some("An Orc cannot spawn child agents.".to_string()),
                    _ => None,
                }
            }
            SpawnRole::Orc => match self.current_agent_role().as_deref() {
                Some(TROLL_ROLE) => None,
                Some(ORC_ROLE) => Some("An Orc cannot spawn child agents.".to_string()),
                _ => Some("Switch to a Troll pane before spawning an Orc.".to_string()),
            },
        }
    }

    fn current_agent_role(&self) -> Option<String> {
        let thread_id = self.active_thread_id?;
        if self.primary_thread_id == Some(thread_id) {
            return None;
        }
        self.agent_navigation
            .get(&thread_id)
            .and_then(|entry| entry.agent_role.clone())
    }

    pub(crate) fn is_spawn_orchestration_thread(&self, thread_id: ThreadId) -> bool {
        self.spawn_status_by_thread.contains_key(&thread_id)
            || self.spawn_parent_by_thread.contains_key(&thread_id)
            || self
                .spawn_parent_by_thread
                .values()
                .any(|parent| *parent == thread_id)
            || self
                .agent_navigation
                .get(&thread_id)
                .and_then(|entry| entry.agent_role.as_deref())
                .is_some_and(|role| role == TROLL_ROLE || role == ORC_ROLE)
    }

    fn nazgul_pane_item(
        &self,
        pane_id: String,
        name: String,
        description: String,
    ) -> SelectionItem {
        let is_bound = self.spawn_nazgul_pane_id.as_deref() == Some(pane_id.as_str());
        SelectionItem {
            name,
            description: Some(description),
            is_current: is_bound,
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::BindSpawnNazgulPane {
                    pane_id: pane_id.clone(),
                });
            })],
            dismiss_on_select: true,
            ..Default::default()
        }
    }

    fn user_pane_title(&self, pane_id: &str) -> String {
        if pane_id == CODEX_MAIN_PANE_ID {
            return "Codex - Main".to_string();
        }
        self.claude_panes
            .panes()
            .iter()
            .find(|pane| pane.id == pane_id)
            .map(|pane| pane.title.clone())
            .unwrap_or_else(|| pane_id.to_string())
    }

    fn spawn_threads_with_role(
        &self,
        role: &str,
    ) -> Vec<(ThreadId, &crate::multi_agents::AgentPickerThreadEntry)> {
        self.agent_navigation
            .ordered_threads()
            .into_iter()
            .filter(|(_, entry)| entry.agent_role.as_deref() == Some(role))
            .collect()
    }

    fn spawn_troll_threads(&self) -> Vec<(ThreadId, &crate::multi_agents::AgentPickerThreadEntry)> {
        self.agent_navigation
            .ordered_threads()
            .into_iter()
            .filter(|(thread_id, entry)| {
                if entry.agent_role.as_deref() == Some(TROLL_ROLE) {
                    return true;
                }
                entry.agent_role.is_none()
                    && self
                        .spawn_parent_by_thread
                        .get(thread_id)
                        .is_some_and(|parent| Some(*parent) == self.primary_thread_id)
            })
            .collect()
    }

    fn spawn_orc_children(
        &self,
        parent_thread_id: ThreadId,
    ) -> Vec<(ThreadId, &crate::multi_agents::AgentPickerThreadEntry)> {
        self.agent_navigation
            .ordered_threads()
            .into_iter()
            .filter(|(thread_id, entry)| {
                self.spawn_parent_by_thread.get(thread_id) == Some(&parent_thread_id)
                    && (entry.agent_role.as_deref() == Some(ORC_ROLE) || entry.agent_role.is_none())
            })
            .collect()
    }

    fn spawn_agent_item(
        &self,
        thread_id: ThreadId,
        entry: &crate::multi_agents::AgentPickerThreadEntry,
        indent: usize,
        fallback_role: Option<&str>,
    ) -> SelectionItem {
        let name = format_agent_picker_item_name(
            entry.agent_nickname.as_deref(),
            entry.agent_role.as_deref().or(fallback_role),
            self.primary_thread_id == Some(thread_id),
        );
        let prefix = " ".repeat(indent);
        let status = if let Some(status) = self.spawn_status_by_thread.get(&thread_id) {
            spawn_status_label(status)
        } else if entry.is_closed {
            "done"
        } else if entry.is_running {
            "running"
        } else {
            "idle"
        };
        SelectionItem {
            name: format!("{prefix}{name}"),
            name_prefix_spans: agent_picker_status_dot_spans(entry.is_closed),
            description: Some(format!("{status}; {}", thread_id)),
            is_current: self.active_thread_id == Some(thread_id),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::SelectAgentThread(thread_id));
            })],
            dismiss_on_select: true,
            search_value: Some(format!("{name} {thread_id}")),
            ..Default::default()
        }
    }
}

fn spawn_instruction(role: SpawnRole, agent_type: &str, task: &str) -> String {
    match role {
        SpawnRole::Troll => format!(
            "Use the native PFTerminal `spawn_agent` tool now.\n\nRole: Troll\nagent_type: \"{agent_type}\"\nTask:\n{task}\n\nDo not do this task yourself first. Call `spawn_agent` directly with `agent_type` set to \"{agent_type}\" and `message` set to the task. After the tool returns, report the spawned Troll and continue supervising as the Nazgul/root pane directs."
        ),
        SpawnRole::Orc => format!(
            "You are the supervising Troll. Use the native PFTerminal `spawn_agent` tool now.\n\nRole: Orc\nagent_type: \"{agent_type}\"\nTask:\n{task}\n\nCall `spawn_agent` directly with `agent_type` set to \"{agent_type}\" and `message` set to the task. Then wait for the Orc to finish, review the Orc output critically, and report evidence and remaining risk to the Nazgul/root pane."
        ),
        SpawnRole::Nazgul => "Nazgul is a pane binding, not a spawned worker.".to_string(),
    }
}

fn section_item(name: &str) -> SelectionItem {
    SelectionItem {
        name: name.to_string(),
        is_disabled: true,
        ..Default::default()
    }
}

fn disabled_item(name: &str) -> SelectionItem {
    SelectionItem {
        name: name.to_string(),
        is_disabled: true,
        ..Default::default()
    }
}

fn spawn_status_label(status: &codex_app_server_protocol::CollabAgentState) -> &'static str {
    match status.status {
        codex_app_server_protocol::CollabAgentStatus::PendingInit => "pending",
        codex_app_server_protocol::CollabAgentStatus::Running => "running",
        codex_app_server_protocol::CollabAgentStatus::Interrupted => "interrupted",
        codex_app_server_protocol::CollabAgentStatus::Completed => "done",
        codex_app_server_protocol::CollabAgentStatus::Errored => "error",
        codex_app_server_protocol::CollabAgentStatus::Shutdown => "closed",
        codex_app_server_protocol::CollabAgentStatus::NotFound => "not found",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn troll_instruction_requires_native_spawn_tool() {
        let instruction = spawn_instruction(SpawnRole::Troll, "troll", "review the repo");

        assert!(instruction.contains("spawn_agent"));
        assert!(instruction.contains("agent_type: \"troll\""));
        assert!(instruction.contains("Do not do this task yourself first"));
        assert!(instruction.contains("review the repo"));
    }

    #[test]
    fn orc_instruction_requires_wait_and_review() {
        let instruction = spawn_instruction(SpawnRole::Orc, "orc", "run cargo test");

        assert!(instruction.contains("agent_type: \"orc\""));
        assert!(instruction.contains("Then wait for the Orc to finish"));
        assert!(instruction.contains("review the Orc output critically"));
        assert!(instruction.contains("run cargo test"));
    }

    #[test]
    fn nazgul_instruction_does_not_spawn_worker() {
        let instruction = spawn_instruction(SpawnRole::Nazgul, "nazgul", "anything");

        assert!(instruction.contains("pane binding"));
        assert!(instruction.contains("not a spawned worker"));
    }
}
