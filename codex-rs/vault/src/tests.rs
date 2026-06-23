use std::sync::Arc;

use chrono::Utc;
use codex_keyring_store::tests::MockKeyringStore;
use pretty_assertions::assert_eq;

use super::*;

fn test_vault() -> (tempfile::TempDir, Arc<MockKeyringStore>, Vault) {
    let dir = tempfile::tempdir().expect("tempdir");
    let keyring = Arc::new(MockKeyringStore::default());
    let vault = Vault::new_with_keyring_store(dir.path().to_path_buf(), keyring.clone());
    (dir, keyring, vault)
}

fn api_key_entry(label: &str, secret: &str) -> AddCredential {
    AddCredential {
        label: label.to_string(),
        credential_type: CredentialType::ApiKey,
        provider: Some("ambient".to_string()),
        notes: Some("primary key".to_string()),
        revocation_notes: Some("rotate at https://console.example.com".to_string()),
        secret: secret.to_string(),
    }
}

#[test]
fn add_then_reveal_round_trips() {
    let (_dir, _keyring, vault) = test_vault();
    vault
        .add(api_key_entry("ambient/prod", "sk-secret-123"))
        .unwrap();

    assert!(vault.exists("ambient/prod").unwrap());
    let meta = vault.show("ambient/prod").unwrap();
    assert_eq!(meta.label, "ambient/prod");
    assert_eq!(meta.credential_type, CredentialType::ApiKey);
    assert_eq!(meta.provider.as_deref(), Some("ambient"));
    assert_eq!(meta.storage_backend, StorageBackend::EncryptedSecrets);

    // Listing must NOT include the raw secret.
    let listed = vault.list().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0], meta);
    let listed_json = serde_json::to_string(&listed).unwrap();
    assert!(!listed_json.contains("sk-secret-123"));

    // Reveal returns the raw value.
    assert_eq!(vault.reveal("ambient/prod").unwrap(), "sk-secret-123");
}

#[test]
fn add_duplicate_label_is_rejected() {
    let (_dir, _keyring, vault) = test_vault();
    vault.add(api_key_entry("dupe", "one")).unwrap();
    let err = vault
        .add(api_key_entry("dupe", "two"))
        .expect_err("duplicate label should be rejected");
    assert!(
        matches!(err, VaultError::CredentialExists { ref label } if label == "dupe"),
        "unexpected error: {err:?}"
    );
    // Original secret is preserved.
    assert_eq!(vault.reveal("dupe").unwrap(), "one");
}

#[test]
fn add_empty_secret_is_rejected() {
    let (_dir, _keyring, vault) = test_vault();
    let mut entry = api_key_entry("empty", "x");
    entry.secret = "   ".to_string();
    let err = vault
        .add(entry)
        .expect_err("empty secret should be rejected");
    assert!(
        matches!(err, VaultError::EmptySecret),
        "unexpected: {err:?}"
    );
}

#[test]
fn invalid_labels_are_rejected() {
    let (_dir, _keyring, vault) = test_vault();
    for bad in ["", "    ", "has space", "has@symbol", &"x".repeat(129)] {
        let err = vault
            .add(api_key_entry(bad, "secret"))
            .err()
            .unwrap_or_else(|| panic!("expected label {bad:?} to be rejected"));
        assert!(
            matches!(err, VaultError::InvalidLabel(_)),
            "unexpected: {err:?}"
        );
    }
}

#[test]
fn label_is_trimmed_and_case_preserved() {
    let (_dir, _keyring, vault) = test_vault();
    vault.add(api_key_entry("  MyLabel  ", "secret")).unwrap();
    assert!(vault.exists("  MyLabel  ").unwrap());
    // Lookup uses the normalized (trimmed) form.
    assert!(vault.exists("MyLabel").unwrap());
    assert_eq!(vault.reveal("MyLabel").unwrap(), "secret");
}

#[test]
fn update_changes_secret_and_metadata() {
    let (_dir, _keyring, vault) = test_vault();
    vault.add(api_key_entry("k1", "old-secret")).unwrap();
    let before = vault.show("k1").unwrap();
    let updated = vault
        .update(
            "k1",
            Some("new-secret".to_string()),
            Some(Some("openrouter".to_string())),
            None,
            None,
        )
        .unwrap();
    assert_eq!(vault.reveal("k1").unwrap(), "new-secret");
    assert_eq!(updated.provider.as_deref(), Some("openrouter"));
    // created_at preserved, updated_at advanced (or equal if same second).
    assert_eq!(updated.created_at, before.created_at);
    assert!(updated.updated_at >= before.updated_at);
}

#[test]
fn update_missing_label_errors() {
    let (_dir, _keyring, vault) = test_vault();
    let err = vault
        .update("nope", Some("x".to_string()), None, None, None)
        .expect_err("missing label should error");
    assert!(
        matches!(err, VaultError::NotFound { ref label } if label == "nope"),
        "unexpected: {err:?}"
    );
}

#[test]
fn delete_removes_credential_and_secret() {
    let (_dir, _keyring, vault) = test_vault();
    vault.add(api_key_entry("to-remove", "secret")).unwrap();
    assert!(vault.delete("to-remove").unwrap());
    assert!(!vault.exists("to-remove").unwrap());
    // Deleting again reports false (already gone).
    assert!(!vault.delete("to-remove").unwrap());
}

#[test]
fn reveal_missing_label_errors() {
    let (_dir, _keyring, vault) = test_vault();
    let err = vault
        .reveal("ghost")
        .expect_err("missing label should error");
    assert!(
        matches!(err, VaultError::NotFound { ref label } if label == "ghost"),
        "unexpected: {err:?}"
    );
}

#[test]
fn multiple_credential_types_are_supported() {
    let (_dir, _keyring, vault) = test_vault();
    vault
        .add(AddCredential {
            label: "bearer".to_string(),
            credential_type: CredentialType::BearerToken,
            provider: None,
            notes: None,
            revocation_notes: None,
            secret: "token-abc".to_string(),
        })
        .unwrap();
    vault
        .add(AddCredential {
            label: "seed".to_string(),
            credential_type: CredentialType::SeedPhrase,
            provider: None,
            notes: None,
            revocation_notes: None,
            secret: "abandon amount bridge".to_string(),
        })
        .unwrap();
    let listed = vault.list().unwrap();
    assert_eq!(listed.len(), 2);
    // Sorted by label.
    assert_eq!(listed[0].label, "bearer");
    assert_eq!(listed[1].label, "seed");
    assert_eq!(vault.reveal("seed").unwrap(), "abandon amount bridge");
}

#[test]
fn timestamp_format_is_iso8601() {
    let now = Utc::now().timestamp();
    let formatted = format_timestamp(now);
    assert!(
        formatted.ends_with('Z'),
        "expected UTC 'Z' suffix: {formatted}"
    );
    assert!(
        formatted.contains('T'),
        "expected ISO-8601 'T' separator: {formatted}"
    );
}

#[test]
fn vault_index_is_encrypted_at_rest() {
    let (dir, _keyring, vault) = test_vault();
    vault
        .add(api_key_entry("secret-label", "plaintext-should-not-appear"))
        .unwrap();

    // The managed-secrets file must exist and must NOT contain the plaintext secret or label.
    let secrets_file = dir.path().join("secrets").join("local.age");
    assert!(
        secrets_file.exists(),
        "expected encrypted secrets file to exist"
    );
    let bytes = std::fs::read(&secrets_file).unwrap();
    let contents = String::from_utf8_lossy(&bytes);
    assert!(
        !contents.contains("plaintext-should-not-appear"),
        "raw secret leaked into the secrets file"
    );
}

#[test]
fn persistence_across_vault_instances() {
    // A vault persists into the age-encrypted secrets file. A second Vault instance
    // pointed at the same codex_home AND the same OS keyring store reads it back.
    // (The keyring holds the encryption passphrase, so it must be shared.)
    let (dir, keyring, vault) = test_vault();
    vault.add(api_key_entry("persist", "value-1")).unwrap();

    let vault2 = Vault::new_with_keyring_store(dir.path().to_path_buf(), keyring);
    assert_eq!(vault2.reveal("persist").unwrap(), "value-1");
}

#[test]
fn distinct_labels_do_not_collide_at_secret_layer() {
    // Regression: `a/b`, `a.b`, `a_b`, `a-b`, and case variants must map to DISTINCT secret
    // records so adding one never overwrites another's stored secret.
    let (_dir, _keyring, vault) = test_vault();
    for (label, secret) in [
        ("a/b", "slash"),
        ("a.b", "dot"),
        ("a_b", "underscore"),
        ("a-b", "hyphen"),
        ("MyLabel", "mixed"),
        ("mylabel", "lower"),
    ] {
        vault
            .add(api_key_entry(label, secret))
            .unwrap_or_else(|e| panic!("adding {label:?} failed: {e:?}"));
    }

    // Each label independently resolves to its own secret — no clobbering.
    assert_eq!(vault.reveal("a/b").unwrap(), "slash");
    assert_eq!(vault.reveal("a.b").unwrap(), "dot");
    assert_eq!(vault.reveal("a_b").unwrap(), "underscore");
    assert_eq!(vault.reveal("a-b").unwrap(), "hyphen");
    assert_eq!(vault.reveal("MyLabel").unwrap(), "mixed");
    assert_eq!(vault.reveal("mylabel").unwrap(), "lower");

    // All six metadata entries survive.
    assert_eq!(vault.list().unwrap().len(), 6);
}
