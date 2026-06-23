//! Vault action menu and secret-copy helpers.

use super::*;

const VAULT_MENU_VIEW_ID: &str = "vault-menu";

impl ChatWidget {
    pub(crate) fn open_vault_menu(&mut self) {
        let codex_home = self.config.codex_home.as_path().to_path_buf();
        let mut items = vec![
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
                "List credentials",
                "Show stored labels and metadata; never prints secrets",
                codex_home.clone(),
                "list".to_string(),
            ),
            vault_history_item(
                "Vault status",
                "Show lock state, backend, and credential count",
                codex_home.clone(),
                "status".to_string(),
            ),
        ];

        let mut credential_count: Option<usize> = None;
        match codex_vault::Vault::new(codex_home.clone()).list() {
            Ok(mut credentials) => {
                credentials.sort_by(|left, right| left.label.cmp(&right.label));
                credential_count = Some(credentials.len());
                for credential in credentials {
                    let label = credential.label;
                    items.push(vault_history_item(
                        format!("Show {label}"),
                        "Inspect metadata only; secret remains hidden",
                        codex_home.clone(),
                        format!("show {label}"),
                    ));
                    items.push(SelectionItem {
                        name: format!("Copy secret: {label}"),
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
                    });
                }
            }
            Err(err) => {
                items.push(SelectionItem {
                    name: "Credential list unavailable".to_string(),
                    description: Some(err.to_string()),
                    is_disabled: true,
                    dismiss_on_select: false,
                    ..Default::default()
                });
            }
        }

        let mut header = ColumnRenderable::new();
        header.push(Line::from("Vault".bold()));
        header.push(Line::from(
            "Add credentials, inspect metadata, or copy secrets without sending them to chat."
                .dim(),
        ));
        if let Some(count) = credential_count {
            header.push(Line::from(format!("{count} credential(s) stored").dim()));
        }

        self.show_selection_view(SelectionViewParams {
            view_id: Some(VAULT_MENU_VIEW_ID),
            footer_hint: Some(standard_popup_hint_line()),
            is_searchable: true,
            search_placeholder: Some("Search vault actions".to_string()),
            items,
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

    #[test]
    fn vault_menu_view_id_is_stable() {
        assert_eq!(VAULT_MENU_VIEW_ID, "vault-menu");
    }
}
