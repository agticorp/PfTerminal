//! Vault action menu and secret-copy helpers.

use super::*;
use crate::bottom_pane::SelectionTab;

const VAULT_MENU_VIEW_ID: &str = "vault-menu";
const VAULT_CREDENTIAL_ACTIONS_VIEW_ID: &str = "vault-credential-actions";
const VAULT_ACTIONS_TAB_ID: &str = "actions";
const VAULT_CREDENTIALS_TAB_ID: &str = "credentials";

impl ChatWidget {
    pub(crate) fn open_vault_menu(&mut self) {
        let codex_home = self.config.codex_home.as_path().to_path_buf();
        let credential_result = sorted_vault_credentials(&codex_home);
        let credential_count = credential_result.as_ref().ok().map(Vec::len);
        let tabs = vec![
            SelectionTab {
                id: VAULT_ACTIONS_TAB_ID.to_string(),
                label: "Actions".to_string(),
                header: vault_header(credential_count),
                items: vault_action_items(codex_home),
            },
            SelectionTab {
                id: VAULT_CREDENTIALS_TAB_ID.to_string(),
                label: "View credentials".to_string(),
                header: vault_credentials_header(credential_count),
                items: vault_credential_items(credential_result),
            },
        ];

        self.show_selection_view(SelectionViewParams {
            view_id: Some(VAULT_MENU_VIEW_ID),
            footer_hint: Some(standard_popup_hint_line()),
            is_searchable: true,
            search_placeholder: Some("Search vault".to_string()),
            tabs,
            initial_tab_id: Some(VAULT_ACTIONS_TAB_ID.to_string()),
            header: Box::new(()),
            ..Default::default()
        });
    }

    pub(crate) fn open_vault_credential_actions(&mut self, label: String) {
        let codex_home = self.config.codex_home.as_path().to_path_buf();
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Vault credential".bold()));
        header.push(Line::from(label.clone().cyan()));
        header.push(Line::from(
            "Choose an action. Secrets are never printed to chat.".dim(),
        ));

        self.show_selection_view(SelectionViewParams {
            view_id: Some(VAULT_CREDENTIAL_ACTIONS_VIEW_ID),
            footer_hint: Some(standard_popup_hint_line()),
            is_searchable: false,
            items: vault_credential_action_items(codex_home, label),
            header: Box::new(header),
            ..Default::default()
        });
    }

    pub(crate) fn copy_vault_secret_to_clipboard(&mut self, label: String) {
        self.copy_vault_secret_to_clipboard_with(label, crate::clipboard_copy::copy_to_clipboard);
    }

    fn copy_vault_secret_to_clipboard_with(
        &mut self,
        label: String,
        copy_fn: impl FnOnce(&str) -> Result<Option<crate::clipboard_copy::ClipboardLease>, String>,
    ) {
        let vault = codex_vault::Vault::new(self.config.codex_home.as_path().to_path_buf());
        match vault.reveal(&label) {
            Ok(secret) => match copy_fn(&secret) {
                Ok(lease) => {
                    self.clipboard_lease = lease;
                    self.add_info_message(
                        format!("Copied vault credential {label:?} to clipboard."),
                        /*hint*/ None,
                    );
                }
                Err(err) => {
                    self.add_error_message(format!(
                        "Failed to copy vault credential {label:?}: {err}"
                    ));
                }
            },
            Err(err) => {
                self.add_error_message(format!("Failed to read vault credential {label:?}: {err}"));
            }
        }
    }
}

fn sorted_vault_credentials(
    codex_home: &Path,
) -> Result<Vec<codex_vault::VaultCredentialMeta>, codex_vault::VaultError> {
    let mut credentials = codex_vault::Vault::new(codex_home.to_path_buf()).list()?;
    credentials.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(credentials)
}

fn vault_header(credential_count: Option<usize>) -> Box<dyn Renderable> {
    let mut header = ColumnRenderable::new();
    header.push(Line::from("Vault".bold()));
    header.push(Line::from(
        "Add credentials, inspect metadata, or copy secrets without sending them to chat.".dim(),
    ));
    if let Some(count) = credential_count {
        header.push(Line::from(format!("{count} credential(s) stored").dim()));
    }
    Box::new(header)
}

fn vault_credentials_header(credential_count: Option<usize>) -> Box<dyn Renderable> {
    let mut header = ColumnRenderable::new();
    header.push(Line::from("View credentials".bold()));
    header.push(Line::from(
        "Select a credential to inspect metadata or copy its secret.".dim(),
    ));
    if let Some(count) = credential_count {
        header.push(Line::from(format!("{count} credential(s) stored").dim()));
    }
    Box::new(header)
}

fn vault_action_items(codex_home: PathBuf) -> Vec<SelectionItem> {
    vec![
        SelectionItem {
            name: "Add credential".to_string(),
            description: Some("Open masked label and secret entry".to_string()),
            actions: vec![Box::new(|tx| {
                tx.send(AppEvent::OpenVaultCredentialAdd);
            })],
            dismiss_on_select: true,
            ..Default::default()
        },
        vault_history_item(
            "Vault status",
            "Show lock state, backend, and credential count",
            codex_home,
            "status".to_string(),
        ),
    ]
}

fn vault_credential_items(
    credential_result: Result<Vec<codex_vault::VaultCredentialMeta>, codex_vault::VaultError>,
) -> Vec<SelectionItem> {
    match credential_result {
        Ok(credentials) if credentials.is_empty() => vec![SelectionItem {
            name: "No credentials stored".to_string(),
            description: Some("Use the Actions tab to add one.".to_string()),
            is_disabled: true,
            dismiss_on_select: false,
            ..Default::default()
        }],
        Ok(credentials) => credentials.into_iter().map(vault_credential_item).collect(),
        Err(err) => vec![SelectionItem {
            name: "Credential list unavailable".to_string(),
            description: Some(err.to_string()),
            is_disabled: true,
            dismiss_on_select: false,
            ..Default::default()
        }],
    }
}

fn vault_credential_item(credential: codex_vault::VaultCredentialMeta) -> SelectionItem {
    let label = credential.label;
    let credential_type = credential.credential_type.description();
    let description = match credential.provider {
        Some(provider) => format!("{credential_type}; provider {provider}"),
        None => credential_type.to_string(),
    };
    SelectionItem {
        name: label.clone(),
        description: Some(description),
        actions: vec![Box::new(move |tx| {
            tx.send(AppEvent::OpenVaultCredentialActions {
                label: label.clone(),
            });
        })],
        dismiss_on_select: false,
        ..Default::default()
    }
}

fn vault_credential_action_items(codex_home: PathBuf, label: String) -> Vec<SelectionItem> {
    vec![
        vault_history_item(
            "Show metadata",
            "Inspect metadata only; secret remains hidden",
            codex_home,
            format!("show {label}"),
        ),
        SelectionItem {
            name: "Copy secret".to_string(),
            description: Some(
                "Copy raw secret to clipboard; it is not printed to chat".to_string(),
            ),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::OpenVaultCopySecret {
                    label: label.clone(),
                });
            })],
            dismiss_on_select: true,
            ..Default::default()
        },
    ]
}

fn vault_history_item(
    name: impl Into<String>,
    description: impl Into<String>,
    codex_home: PathBuf,
    args: String,
) -> SelectionItem {
    SelectionItem {
        name: name.into(),
        description: Some(description.into()),
        actions: vec![Box::new(move |tx| {
            let lines = crate::vault_command::handle_vault_command(&codex_home, &args);
            tx.send(AppEvent::InsertHistoryCell(Box::new(
                PlainHistoryCell::new(lines),
            )));
        })],
        dismiss_on_select: true,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_vault::CredentialType;
    use codex_vault::StorageBackend;
    use codex_vault::VaultCredentialMeta;

    #[test]
    fn vault_menu_view_id_is_stable() {
        assert_eq!(VAULT_MENU_VIEW_ID, "vault-menu");
    }

    #[test]
    fn top_level_vault_actions_do_not_include_per_credential_actions() {
        let items = vault_action_items(PathBuf::from("/tmp/codex-home"));
        let names = items
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["Add credential", "Vault status"]);
        assert!(!names.iter().any(|name| name.starts_with("Copy secret")));
        assert!(!names.iter().any(|name| name.starts_with("Show provider/")));
    }

    #[test]
    fn credential_tab_shows_one_row_per_credential() {
        let items = vault_credential_items(Ok(vec![VaultCredentialMeta {
            label: "provider/ambient_api_key".to_string(),
            credential_type: CredentialType::ApiKey,
            provider: Some("AMBIENT_API_KEY".to_string()),
            notes: None,
            revocation_notes: None,
            created_at: 1,
            updated_at: 1,
            storage_backend: StorageBackend::EncryptedSecrets,
        }]));

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "provider/ambient_api_key");
        assert!(
            items[0]
                .description
                .as_deref()
                .is_some_and(|description| description.contains("API key"))
        );
    }

    #[test]
    fn credential_actions_are_scoped_to_selected_credential() {
        let items = vault_credential_action_items(
            PathBuf::from("/tmp/codex-home"),
            "provider/zai_api_key".to_string(),
        );
        let names = items
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["Show metadata", "Copy secret"]);
    }
}
