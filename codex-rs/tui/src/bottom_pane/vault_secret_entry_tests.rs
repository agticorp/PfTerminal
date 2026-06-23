use std::sync::Arc;
use std::sync::Mutex;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

use super::Field;
use super::VaultSecretEntryView;

use crate::bottom_pane::BottomPaneView;

fn char_event(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn enter_event() -> KeyEvent {
    KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
}

/// Captures the (label, secret) the view ultimately submits.
#[derive(Default, Clone)]
struct Capture {
    inner: Arc<Mutex<Option<(String, String)>>>,
}

impl Capture {
    fn callback(self) -> super::VaultSecretSubmitted {
        Box::new(move |label, secret| {
            *self.inner.lock().unwrap() = Some((label, secret));
        })
    }

    fn taken(&self) -> Option<(String, String)> {
        self.inner.lock().unwrap().take()
    }
}

#[test]
fn advances_from_label_to_secret_then_submits() {
    let capture = Capture::default();
    let mut view = VaultSecretEntryView::new(capture.clone().callback());

    // Type the label.
    for c in "ambient/prod".chars() {
        view.handle_key_event(char_event(c));
    }
    // Submitting the label must NOT fire the callback yet.
    view.handle_key_event(enter_event());
    assert!(capture.taken().is_none(), "label submit must not finalize");
    assert_eq!(view.field, Field::Secret);

    // Type the (masked) secret.
    for c in "sk-secret-123".chars() {
        view.handle_key_event(char_event(c));
    }
    view.handle_key_event(enter_event());

    assert_eq!(
        capture.taken(),
        Some(("ambient/prod".to_string(), "sk-secret-123".to_string())),
    );
    assert!(view.is_complete());
}

#[test]
fn empty_label_does_not_advance() {
    let capture = Capture::default();
    let mut view = VaultSecretEntryView::new(capture.clone().callback());
    view.handle_key_event(enter_event());
    assert_eq!(view.field, Field::Label);
    assert!(capture.taken().is_none());
    assert!(!view.is_complete());
}

#[test]
fn empty_secret_does_not_finalize() {
    let capture = Capture::default();
    let mut view = VaultSecretEntryView::new(capture.clone().callback());
    for c in "label".chars() {
        view.handle_key_event(char_event(c));
    }
    view.handle_key_event(enter_event());
    assert_eq!(view.field, Field::Secret);
    // Enter with empty secret stays on the secret field.
    view.handle_key_event(enter_event());
    assert_eq!(view.field, Field::Secret);
    assert!(capture.taken().is_none());
    assert!(!view.is_complete());
}

#[test]
fn escape_cancels_without_submitting() {
    let capture = Capture::default();
    let mut view = VaultSecretEntryView::new(capture.clone().callback());
    view.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(view.is_complete());
    assert!(
        capture.taken().is_none(),
        "cancel must not submit the secret"
    );
}

#[test]
fn label_is_trimmed_before_submission() {
    let capture = Capture::default();
    let mut view = VaultSecretEntryView::new(capture.clone().callback());
    for c in "  spaced-label  ".chars() {
        view.handle_key_event(char_event(c));
    }
    view.handle_key_event(enter_event());
    for c in "secret".chars() {
        view.handle_key_event(char_event(c));
    }
    view.handle_key_event(enter_event());
    assert_eq!(capture.taken().unwrap().0, "spaced-label");
}

#[test]
fn secret_is_preserved_byte_for_byte_without_trimming() {
    // Leading/trailing whitespace can be meaningful for PEM blobs, seed phrases, pasted keys.
    let capture = Capture::default();
    let mut view = VaultSecretEntryView::new(capture.clone().callback());
    for c in "label".chars() {
        view.handle_key_event(char_event(c));
    }
    view.handle_key_event(enter_event());
    for c in "  sk-with-padding  ".chars() {
        view.handle_key_event(char_event(c));
    }
    view.handle_key_event(enter_event());

    let (_label, secret) = capture.taken().expect("secret submitted");
    assert_eq!(secret, "  sk-with-padding  ", "secret must not be trimmed");
}

#[test]
fn all_whitespace_secret_is_rejected() {
    let capture = Capture::default();
    let mut view = VaultSecretEntryView::new(capture.clone().callback());
    for c in "label".chars() {
        view.handle_key_event(char_event(c));
    }
    view.handle_key_event(enter_event());
    // Whitespace-only secret must not finalize.
    view.handle_key_event(char_event(' '));
    view.handle_key_event(char_event(' '));
    view.handle_key_event(enter_event());
    assert_eq!(view.field, Field::Secret);
    assert!(capture.taken().is_none());
    assert!(!view.is_complete());
}

#[test]
fn fixed_secret_mode_submits_known_label_without_label_step() {
    let capture = Capture::default();
    let mut view = VaultSecretEntryView::new_fixed_secret(
        "provider/zai_api_key".to_string(),
        "Add Provider: Z.AI API Key".to_string(),
        "ZAI_API_KEY (masked)".to_string(),
        capture.clone().callback(),
    );
    assert_eq!(view.field, Field::Secret);

    for c in "zai-secret".chars() {
        view.handle_key_event(char_event(c));
    }
    view.handle_key_event(enter_event());

    assert_eq!(
        capture.taken(),
        Some(("provider/zai_api_key".to_string(), "zai-secret".to_string()))
    );
    assert!(view.is_complete());
}
