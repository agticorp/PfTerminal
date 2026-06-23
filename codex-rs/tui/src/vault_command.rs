//! `/vault` slash-command handling for the PFTerminal credential vault.
//!
//! This module parses `/vault` subcommands and renders their output as ratatui `Line`s. It keeps
//! all secret-handling logic in one place so the slash-dispatch path stays small.
//!
//! # Secret entry policy
//!
//! Raw secrets are NEVER accepted as chat text, never enter agent context, prompt history, the
//! transcript, or the normal conversation. The only subcommand that could accept a new secret is
//! `/vault credential add`; the dispatcher routes it to the secure entry flow (a dedicated
//! masked modal/TUI popout) rather than typing the secret inline. The non-secret metadata
//! subcommands (`help`, `status`, `list`, `show`, `delete`) are safe to run inline.

use std::path::Path;

use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

use codex_vault::Vault;
use codex_vault::format_timestamp;

/// The result of parsing a `/vault` invocation, rendered into display lines for the chat history.
pub fn handle_vault_command(codex_home: &Path, args: &str) -> Vec<Line<'static>> {
    handle_vault_command_with_vault(args, &Vault::new(codex_home.to_path_buf()))
}

/// Same as [`handle_vault_command`] but with an explicit vault reference (used by tests and any
/// caller that already holds a vault).
pub fn handle_vault_command_with_vault(args: &str, vault: &Vault) -> Vec<Line<'static>> {
    let mut parts = args.split_whitespace();
    let sub = parts.next().unwrap_or("status");
    match sub {
        "help" | "h" | "?" => help_lines(),
        "status" => status_lines(vault),
        "list" | "ls" => list_lines(vault),
        "show" => {
            let label = parts.next();
            match label {
                Some(label) => show_lines(vault, label),
                None => usage_error("/vault show <label> — a label is required"),
            }
        }
        "delete" | "rm" => {
            let label = parts.next();
            match label {
                Some(label) => delete_lines(vault, label),
                None => usage_error("/vault delete <label> — a label is required"),
            }
        }
        "credential" => {
            let action = parts.next().unwrap_or("help");
            match action {
                "add" => credential_add_hint_lines(),
                "list" => list_lines(vault),
                "show" => {
                    let label = parts.next();
                    match label {
                        Some(label) => show_lines(vault, label),
                        None => usage_error("/vault credential show <label> — a label is required"),
                    }
                }
                "delete" => {
                    let label = parts.next();
                    match label {
                        Some(label) => delete_lines(vault, label),
                        None => {
                            usage_error("/vault credential delete <label> — a label is required")
                        }
                    }
                }
                "reveal" | "export" => {
                    // These surface the raw secret. v0 does not print secrets to chat history
                    // (which would enter the transcript). Route to the secure reveal flow.
                    secure_reveal_hint_lines(action)
                }
                "help" | "h" | "?" => help_lines(),
                other => usage_error(format!(
                    "Unknown `/vault credential {other}` subcommand. Try `/vault help`."
                )),
            }
        }
        "unlock" | "lock" => lock_status_lines(),
        other => usage_error(format!(
            "Unknown `/vault {other}` subcommand. Try `/vault help`."
        )),
    }
}

fn help_lines() -> Vec<Line<'static>> {
    let header = Line::from(vec!["Vault — encrypted credential store".bold().cyan()]);
    let commands = [
        ("/vault", "show vault status (credential count + backend)"),
        (
            "/vault list",
            "list credential labels and metadata (no secrets)",
        ),
        (
            "/vault show <label>",
            "show metadata for one credential (no secret)",
        ),
        (
            "/vault credential add",
            "add a credential via the secure entry modal",
        ),
        ("/vault credential list", "alias for /vault list"),
        (
            "/vault credential reveal <label>",
            "reveal a raw secret (secure popout, not chat)",
        ),
        (
            "/vault credential export <label>",
            "export a raw secret (secure popout, not chat)",
        ),
        ("/vault credential delete <label>", "delete a credential"),
        (
            "/vault delete <label>",
            "alias for /vault credential delete",
        ),
        (
            "/vault unlock",
            "the vault is unlocked via the OS keyring (no password)",
        ),
        (
            "/vault lock",
            "same as unlock — v0 unlock is implicit per session",
        ),
    ];
    let mut lines = vec![header, Line::from(""), Line::from(vec!["Commands".bold()])];
    for (cmd, desc) in commands {
        lines.push(Line::from(vec![
            "  ".into(),
            Span::from(cmd).cyan(),
            "  ".into(),
            Span::from(desc).dim(),
        ]));
    }
    lines
}

fn status_lines(vault: &Vault) -> Vec<Line<'static>> {
    match vault.list() {
        Ok(entries) => {
            let count = entries.len();
            vec![
                Line::from(vec!["Vault status".bold().cyan()]),
                Line::from(vec!["  state:      ".dim(), "unlocked (OS keyring)".into()]),
                Line::from(vec![
                    "  backend:    ".dim(),
                    "encrypted (age + keyring passphrase)".into(),
                ]),
                Line::from(vec!["  credentials: ".dim(), count.to_string().into()]),
            ]
        }
        Err(err) => error_lines(format!("Failed to read vault: {err}")),
    }
}

fn list_lines(vault: &Vault) -> Vec<Line<'static>> {
    match vault.list() {
        Ok(entries) if entries.is_empty() => {
            vec![Line::from(vec![
                "Vault is empty. Use ".dim(),
                "/vault credential add".cyan(),
                " to store a credential.".dim(),
            ])]
        }
        Ok(entries) => {
            let mut lines = vec![Line::from(vec![
                "Credentials (".into(),
                entries.len().to_string().into(),
                ")".into(),
            ])];
            for meta in entries {
                let provider = meta.provider.unwrap_or_else(|| "—".to_string());
                lines.push(Line::from(vec![
                    "  • ".into(),
                    Span::from(meta.label).cyan(),
                    "  ".into(),
                    Span::from(format!("[{}]", meta.credential_type.as_ref())).dim(),
                    "  ".into(),
                    Span::from(provider).dim(),
                ]));
                lines.push(Line::from(vec![
                    "      created ".dim(),
                    Span::from(format_timestamp(meta.created_at)).dim(),
                    "  updated ".dim(),
                    Span::from(format_timestamp(meta.updated_at)).dim(),
                ]));
            }
            lines
        }
        Err(err) => error_lines(format!("Failed to list vault: {err}")),
    }
}

fn show_lines(vault: &Vault, label: &str) -> Vec<Line<'static>> {
    match vault.show(label) {
        Ok(meta) => {
            let rows = [
                ("label", meta.label.clone()),
                ("type", meta.credential_type.as_ref().to_string()),
                ("provider", meta.provider.unwrap_or_else(|| "—".to_string())),
                ("notes", meta.notes.unwrap_or_else(|| "—".to_string())),
                (
                    "revocation",
                    meta.revocation_notes.unwrap_or_else(|| "—".to_string()),
                ),
                ("backend", format!("{:?}", meta.storage_backend)),
                ("created", format_timestamp(meta.created_at)),
                ("updated", format_timestamp(meta.updated_at)),
            ];
            let mut lines = vec![Line::from(vec!["Credential".bold().cyan()])];
            for (key, value) in rows {
                lines.push(Line::from(vec![
                    Span::from(format!("  {key:<10} ")).dim(),
                    value.into(),
                ]));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                "Raw secret hidden. Use ".dim(),
                "/vault credential reveal <label>".cyan(),
                " (secure popout) to view it.".dim(),
            ]));
            lines
        }
        Err(err) => error_lines(format!("No credential labeled {label:?}: {err}")),
    }
}

fn delete_lines(vault: &Vault, label: &str) -> Vec<Line<'static>> {
    let label_owned = label.to_string();
    match vault.delete(&label_owned) {
        Ok(true) => vec![Line::from(vec![
            "Deleted credential ".into(),
            Span::from(label_owned).cyan(),
            ".".into(),
        ])],
        Ok(false) => error_lines(format!("No credential labeled {label_owned:?} to delete.")),
        Err(err) => error_lines(format!("Failed to delete {label_owned:?}: {err}")),
    }
}

fn credential_add_hint_lines() -> Vec<Line<'static>> {
    // SECURITY: raw secrets must not be typed into chat. The dispatcher opens the secure-entry
    // modal for live `/vault credential add`; this hint remains for non-live/direct parser calls.
    vec![
        Line::from(vec!["Add credential".bold().cyan()]),
        Line::from(""),
        Line::from(vec![
            "Secrets are never typed into chat. ".dim(),
            "They must not enter agent context, prompt history, or the transcript.".dim(),
        ]),
        Line::from(""),
        Line::from(vec![
            "Run ".dim(),
            "/vault credential add".cyan(),
            " with no arguments to open a dedicated secure-entry modal for ".dim(),
            "label + secret".cyan(),
            " without submitting them as chat text.".dim(),
        ]),
    ]
}

fn secure_reveal_hint_lines(action: &str) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![format!("Vault credential {action}").bold().cyan()]),
        Line::from(""),
        Line::from(vec![
            "Raw secrets are only revealed through a secure popout, never printed into chat ".dim(),
            "history (which is part of the transcript).".dim(),
        ]),
        Line::from(""),
        Line::from(vec![
            "TODO(vault): open a secure reveal/export modal for the requested label.".dim(),
        ]),
    ]
}

fn lock_status_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(vec!["Vault lock state".bold().cyan()]),
        Line::from(""),
        Line::from(vec![
            "v0 unlock is implicit per session: the OS keyring passphrase gates decryption, ".dim(),
            "so there is no separate password to type.".dim(),
        ]),
        Line::from(""),
        Line::from(vec![
            "While the keyring is available, the vault stays unlocked for the session ".dim(),
            "and credentials resolve by label without re-entry.".dim(),
        ]),
    ]
}

fn usage_error(message: impl Into<String>) -> Vec<Line<'static>> {
    error_lines(message.into())
}

fn error_lines(message: String) -> Vec<Line<'static>> {
    vec![Line::from(vec![Span::from(message).red()])]
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_keyring_store::tests::MockKeyringStore;
    use codex_vault::AddCredential;
    use codex_vault::CredentialType;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn vault_with_entry(codex_home: &Path) -> Vault {
        let keyring = Arc::new(MockKeyringStore::default());
        let vault = Vault::new_with_keyring_store(codex_home.to_path_buf(), keyring);
        vault
            .add(AddCredential {
                label: "ambient/prod".to_string(),
                credential_type: CredentialType::ApiKey,
                provider: Some("ambient".to_string()),
                notes: Some("primary".to_string()),
                revocation_notes: None,
                secret: "sk-secret".to_string(),
            })
            .expect("add credential");
        vault
    }

    fn assert_contains(lines: &[Line], needle: &str) {
        let joined: String = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<Vec<&str>>()
            .join("");
        assert!(
            joined.contains(needle),
            "expected rendered output to contain {needle:?}; got: {joined:?}"
        );
    }

    #[test]
    fn help_lists_subcommands() {
        let lines = handle_vault_command(Path::new("/tmp"), "help");
        assert_contains(&lines, "encrypted credential store");
        assert_contains(&lines, "/vault credential add");
    }

    #[test]
    fn status_reports_unlocked_and_count() {
        let dir = tempdir().unwrap();
        let vault = vault_with_entry(dir.path());
        let lines = handle_vault_command_with_vault("status", &vault);
        assert_contains(&lines, "unlocked (OS keyring)");
        assert_contains(&lines, "credentials");
    }

    #[test]
    fn list_shows_label_without_secret() {
        let dir = tempdir().unwrap();
        let vault = vault_with_entry(dir.path());
        let lines = handle_vault_command_with_vault("list", &vault);
        assert_contains(&lines, "ambient/prod");
        let joined: String = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<Vec<&str>>()
            .join("");
        assert!(
            !joined.contains("sk-secret"),
            "list must never include the raw secret"
        );
    }

    #[test]
    fn show_renders_metadata_not_secret() {
        let dir = tempdir().unwrap();
        let vault = vault_with_entry(dir.path());
        let lines = handle_vault_command_with_vault("show ambient/prod", &vault);
        assert_contains(&lines, "ambient/prod");
        assert_contains(&lines, "Raw secret hidden");
        let joined: String = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<Vec<&str>>()
            .join("");
        assert!(
            !joined.contains("sk-secret"),
            "show must not reveal the secret"
        );
    }

    #[test]
    fn delete_removes_credential() {
        let dir = tempdir().unwrap();
        let vault = vault_with_entry(dir.path());
        assert!(vault.exists("ambient/prod").unwrap());

        let lines = handle_vault_command_with_vault("delete ambient/prod", &vault);
        assert_contains(&lines, "Deleted credential");
        assert!(!vault.exists("ambient/prod").unwrap());
    }

    #[test]
    fn show_missing_label_errors() {
        let dir = tempdir().unwrap();
        let keyring = Arc::new(MockKeyringStore::default());
        let vault = Vault::new_with_keyring_store(dir.path().to_path_buf(), keyring);
        let lines = handle_vault_command_with_vault("show nope", &vault);
        assert_contains(&lines, "No credential labeled");
    }

    #[test]
    fn credential_add_does_not_accept_inline_secret() {
        let lines = handle_vault_command(Path::new("/tmp"), "credential add");
        // Must instruct secure entry, never reference an inline secret argument.
        assert_contains(&lines, "never typed into chat");
        assert_contains(&lines, "/vault credential add");
        assert_contains(&lines, "with no arguments");
    }

    #[test]
    fn reveal_and_export_route_to_secure_flow() {
        for action in ["reveal", "export"] {
            let lines = handle_vault_command(Path::new("/tmp"), &format!("credential {action} x"));
            assert_contains(&lines, "secure popout");
            assert_contains(&lines, "TODO(vault)");
        }
    }

    #[test]
    fn unknown_subcommand_errors() {
        let lines = handle_vault_command(Path::new("/tmp"), "frobnicate");
        assert_contains(&lines, "Unknown");
    }

    #[test]
    fn empty_args_defaults_to_status() {
        let dir = tempdir().unwrap();
        let vault = vault_with_entry(dir.path());
        let lines = handle_vault_command_with_vault("", &vault);
        assert_contains(&lines, "Vault status");
    }
}
