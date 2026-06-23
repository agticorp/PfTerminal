//! Secure secret-entry overlay for `/vault credential add`.
//!
//! This view captures a label and a raw secret through a dedicated masked-input overlay rather
//! than the chat composer. The submitted text is delivered to a callback and **never** enters
//! the composer, prompt history, agent context, or the transcript. Only a non-secret confirmation
//! is surfaced to chat history by the callback.
//!
//! Field flow: Enter submits the current field. Field 1 (label) echoes normally; field 2
//! (secret) is masked with `•`. The secret field is never rendered in cleartext.

use std::cell::RefCell;
use std::time::Instant;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;

use crate::key_hint::has_ctrl_or_alt;
use crate::render::renderable::Renderable;

use super::CancellationEvent;
use super::bottom_pane_view::ViewCompletion;
use super::paste_burst::PasteBurst;
use super::popup_consts::standard_popup_hint_line;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

/// Callback invoked with the validated label and raw secret on final submit.
pub(crate) type VaultSecretSubmitted = Box<dyn FnOnce(String, String) + Send + Sync>;

const MASK_CHAR: char = '•';
const LABEL_PROMPT: &str = "Label (for example: ambient/prod)";
const SECRET_PROMPT: &str = "Secret value (masked — not shown, not stored in chat)";

/// Which field is currently active in the two-step entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Label,
    Secret,
}

/// Two-field masked secret-entry overlay (label, then secret).
pub(crate) struct VaultSecretEntryView {
    field: Field,
    label: String,
    on_submit: VaultSecretSubmitted,
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    paste_burst: PasteBurst,
    completion: Option<ViewCompletion>,
}

impl VaultSecretEntryView {
    /// Build a new entry view. `on_submit` receives `(label, secret)` once the secret field is
    /// submitted non-empty after a non-empty label.
    pub(crate) fn new(on_submit: VaultSecretSubmitted) -> Self {
        Self {
            field: Field::Label,
            label: String::new(),
            on_submit,
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            paste_burst: PasteBurst::default(),
            completion: None,
        }
    }

    fn handle_key_event_at(&mut self, key_event: KeyEvent, now: Instant) {
        match key_event {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.completion = Some(ViewCompletion::Cancelled);
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers,
                ..
            } if modifiers == KeyModifiers::NONE => self.advance(),
            KeyEvent {
                code: KeyCode::Char(_),
                modifiers,
                ..
            } if !has_ctrl_or_alt(modifiers) && self.textarea.allows_paste_burst() => {
                let paste_like_burst = self.paste_burst.on_plain_char_no_hold(now).is_some();
                self.textarea.input(key_event);
                if paste_like_burst {
                    self.paste_burst.extend_window(now);
                }
            }
            other => {
                self.textarea.input(other);
                self.paste_burst.clear_after_explicit_paste();
            }
        }
    }

    fn advance(&mut self) {
        let raw = self.textarea.text().to_string();
        match self.field {
            Field::Label => {
                let value = raw.trim().to_string();
                if value.is_empty() {
                    return;
                }
                self.label = value;
                self.field = Field::Secret;
                self.reset_textarea();
            }
            Field::Secret => {
                // Preserve the secret byte-for-byte (leading/trailing whitespace can be
                // meaningful for PEM blobs, seed phrases, and pasted keys); only reject
                // all-whitespace as empty.
                if raw.trim().is_empty() {
                    return;
                }
                let label = std::mem::take(&mut self.label);
                let on_submit = std::mem::replace(&mut self.on_submit, Box::new(|_, _| {}));
                on_submit(label, raw);
                self.completion = Some(ViewCompletion::Accepted);
            }
        }
    }

    fn reset_textarea(&mut self) {
        self.textarea = TextArea::new();
        self.textarea_state = RefCell::new(TextAreaState::default());
    }

    fn active_prompt(&self) -> &'static str {
        match self.field {
            Field::Label => LABEL_PROMPT,
            Field::Secret => SECRET_PROMPT,
        }
    }

    fn input_height(&self, width: u16) -> u16 {
        let usable_width = width.saturating_sub(2);
        let text_height = self.textarea.desired_height(usable_width).clamp(1, 5);
        text_height.saturating_add(1).min(6)
    }
}

impl super::bottom_pane_view::BottomPaneView for VaultSecretEntryView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        self.handle_key_event_at(key_event, Instant::now());
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.completion = Some(ViewCompletion::Cancelled);
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.completion.is_some()
    }

    fn completion(&self) -> Option<ViewCompletion> {
        self.completion
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if pasted.is_empty() {
            return false;
        }
        self.textarea.insert_str(&pasted);
        self.paste_burst.clear_after_explicit_paste();
        true
    }
}

impl Renderable for VaultSecretEntryView {
    fn desired_height(&self, width: u16) -> u16 {
        // title + status line + input + spacer + hint
        1u16 + 1u16 + self.input_height(width) + 1u16 + 1u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Title line
        Paragraph::new(Line::from(vec![gutter(), "Add vault credential".bold()])).render(
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
            buf,
        );

        // Status line (which field, masked indicator)
        let status_y = area.y.saturating_add(1);
        let status = match self.field {
            Field::Label => "1/2 — label",
            Field::Secret => "2/2 — secret (masked)",
        };
        Paragraph::new(Line::from(vec![gutter(), Span::from(status).cyan()])).render(
            Rect {
                x: area.x,
                y: status_y,
                width: area.width,
                height: 1,
            },
            buf,
        );

        // Input area
        let input_y = status_y.saturating_add(1);
        let input_height = self.input_height(area.width);
        let input_area = Rect {
            x: area.x,
            y: input_y,
            width: area.width,
            height: input_height,
        };
        if input_area.width >= 2 {
            for row in 0..input_area.height {
                Paragraph::new(Line::from(vec![gutter()])).render(
                    Rect {
                        x: input_area.x,
                        y: input_area.y.saturating_add(row),
                        width: 2,
                        height: 1,
                    },
                    buf,
                );
            }
            let text_area_height = input_area.height.saturating_sub(1);
            if text_area_height > 0 && input_area.width > 2 {
                let textarea_rect = Rect {
                    x: input_area.x.saturating_add(2),
                    y: input_area.y.saturating_add(1),
                    width: input_area.width.saturating_sub(2),
                    height: text_area_height,
                };
                Clear.render(
                    Rect {
                        x: textarea_rect.x,
                        y: textarea_rect.y.saturating_sub(1),
                        width: textarea_rect.width,
                        height: 1,
                    },
                    buf,
                );
                let mut state = self.textarea_state.borrow_mut();
                match self.field {
                    Field::Secret => {
                        StatefulWidgetRef::render_ref(
                            &(&self.textarea),
                            textarea_rect,
                            buf,
                            &mut state,
                        );
                        // Re-mask the textarea region on top of the normal render.
                        mask_region(textarea_rect, buf, MASK_CHAR);
                    }
                    Field::Label => {
                        StatefulWidgetRef::render_ref(
                            &(&self.textarea),
                            textarea_rect,
                            buf,
                            &mut state,
                        );
                    }
                }
                if self.textarea.text().is_empty() {
                    Paragraph::new(Line::from(self.active_prompt().dim()))
                        .render(textarea_rect, buf);
                }
            }
        }

        // Hint line
        let hint_y = input_y.saturating_add(input_height).saturating_add(1);
        if hint_y < area.y.saturating_add(area.height) {
            Paragraph::new(standard_popup_hint_line()).render(
                Rect {
                    x: area.x,
                    y: hint_y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if area.height < 4 || area.width <= 2 {
            return None;
        }
        let text_area_height = self.input_height(area.width).saturating_sub(1);
        if text_area_height == 0 {
            return None;
        }
        // title + status + spacer => top offset of textarea
        let top_offset = 1u16 + 1u16 + 1u16;
        let textarea_rect = Rect {
            x: area.x.saturating_add(2),
            y: area.y.saturating_add(top_offset),
            width: area.width.saturating_sub(2),
            height: text_area_height,
        };
        let state = *self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, state)
    }
}

fn mask_region(area: Rect, buf: &mut Buffer, mask_char: char) {
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let Some(cell) = buf.cell_mut((x, y))
                && cell.symbol() != " "
            {
                cell.set_char(mask_char);
            }
        }
    }
}

fn gutter() -> Span<'static> {
    "▌ ".cyan()
}

#[cfg(test)]
#[path = "vault_secret_entry_tests.rs"]
mod tests;
