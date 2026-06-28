//! Text summarization and shell command parsing for Claude pane progress previews.

use std::path::Path;

use serde_json::Value;

use super::progress::truncate_for_display;

pub(crate) const TOOL_PREVIEW_MAX_CHARS: usize = 120;
pub(crate) const REASONING_PREVIEW_MAX_CHARS: usize = 240;
pub(crate) const ASSISTANT_UPDATE_MAX_CHARS: usize = 300;
pub(crate) const ASSISTANT_UPDATE_VISIBLE_COUNT: usize = 6;
pub(crate) const REASONING_VISIBLE_COUNT: usize = 4;
pub(crate) const TOOL_VISIBLE_COUNT: usize = 5;
pub(crate) const CLAUDE_TOOL_CALL_PREFIX: &str = "Claude tool call: ";
pub(crate) const CLAUDE_REASONING_PREFIX: &str = "Claude reasoning: ";
pub(crate) const SEND_TASK_FENCE_OPEN_MARKER: &str = "```pfterminal-send-task";
pub(crate) const SEND_TASK_FENCE_CLOSE_MARKER: &str = "```";
pub(crate) const SEND_TASK_XML_OPEN_MARKER: &str = "<pfterminal_send_task";
pub(crate) const SEND_TASK_XML_CLOSE_MARKER: &str = "</pfterminal_send_task>";

pub(crate) fn summarize_reasoning_text(text: &str) -> String {
    truncate_for_display(
        &collapse_whitespace(text.trim()),
        REASONING_PREVIEW_MAX_CHARS,
    )
}

pub(crate) fn assistant_update_blurbs_from_buffer(buffer: &str) -> Vec<String> {
    let visible = visible_assistant_text_from_buffer(buffer);
    let mut blurbs = Vec::new();
    let mut paragraph = String::new();

    for raw_line in visible.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            push_assistant_update_blurb(&mut blurbs, &mut paragraph);
            continue;
        }
        if line.starts_with("```") {
            continue;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(line);
    }
    push_assistant_update_blurb(&mut blurbs, &mut paragraph);
    blurbs
}

pub(crate) fn visible_assistant_text_from_buffer(buffer: &str) -> String {
    let stable_text = strip_incomplete_spawn_dispatch_tail(buffer);
    let (visible, _) = crate::spawn_orchestration::extract_spawn_task_dispatches(stable_text);
    visible
}

pub(crate) fn push_assistant_update_blurb(blurbs: &mut Vec<String>, paragraph: &mut String) {
    let compact = collapse_whitespace(paragraph.trim());
    paragraph.clear();
    if compact.is_empty() {
        return;
    }
    for blurb in assistant_blurbs_from_paragraph(&compact) {
        if blurbs.last() != Some(&blurb) {
            blurbs.push(blurb);
        }
    }
}

pub(crate) fn assistant_blurbs_from_paragraph(paragraph: &str) -> Vec<String> {
    let sentences = assistant_sentences_from_paragraph(paragraph);
    let mut blurbs = Vec::new();
    let mut current = String::new();

    for sentence in sentences {
        if sentence.chars().count() > ASSISTANT_UPDATE_MAX_CHARS {
            if !current.is_empty() {
                blurbs.push(std::mem::take(&mut current));
            }
            blurbs.push(truncate_for_display(sentence, ASSISTANT_UPDATE_MAX_CHARS));
            continue;
        }

        let candidate = if current.is_empty() {
            sentence.to_string()
        } else {
            format!("{current} {sentence}")
        };
        if candidate.chars().count() <= ASSISTANT_UPDATE_MAX_CHARS {
            current = candidate;
        } else {
            blurbs.push(std::mem::replace(&mut current, sentence.to_string()));
        }
    }

    if !current.is_empty() {
        blurbs.push(current);
    }
    blurbs
}

pub(crate) fn assistant_sentences_from_paragraph(paragraph: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0;
    for (index, ch) in paragraph.char_indices() {
        if !is_assistant_sentence_boundary(paragraph, index, ch) {
            continue;
        }
        let end = index + ch.len_utf8();
        let sentence = paragraph[start..end].trim();
        if !sentence.is_empty() {
            sentences.push(sentence);
        }
        start = next_non_whitespace_byte(paragraph, end);
    }

    let remainder = paragraph[start..].trim();
    if !remainder.is_empty() {
        sentences.push(remainder);
    }
    sentences
}

pub(crate) fn is_assistant_sentence_boundary(paragraph: &str, index: usize, ch: char) -> bool {
    if !matches!(ch, '.' | '!' | '?') {
        return false;
    }
    paragraph[index + ch.len_utf8()..]
        .chars()
        .next()
        .is_none_or(char::is_whitespace)
}

pub(crate) fn next_non_whitespace_byte(value: &str, start: usize) -> usize {
    value[start..]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(offset, _)| start + offset)
        .unwrap_or(value.len())
}

pub(crate) fn strip_incomplete_spawn_dispatch_tail(text: &str) -> &str {
    let mut end = text.len();
    if let Some(index) = text.rfind(SEND_TASK_FENCE_OPEN_MARKER) {
        let tail = &text[index..];
        if !tail
            .get(SEND_TASK_FENCE_OPEN_MARKER.len()..)
            .is_some_and(|rest| rest.contains(SEND_TASK_FENCE_CLOSE_MARKER))
        {
            end = end.min(index);
        }
    }
    if let Some(index) = text.rfind(SEND_TASK_XML_OPEN_MARKER) {
        let tail = &text[index..];
        if !tail.contains(SEND_TASK_XML_CLOSE_MARKER) {
            end = end.min(index);
        }
    }
    &text[..end]
}

pub(crate) fn summarize_tool_call_input(name: &str, input: &Value) -> String {
    if let Some(description) = string_field(input, &["description"]) {
        let description = collapse_whitespace(description);
        if !description.is_empty() {
            return truncate_for_display(&description, TOOL_PREVIEW_MAX_CHARS);
        }
    }

    let lower_name = name.to_ascii_lowercase();
    let summary = match lower_name.as_str() {
        "bash" | "shell" => summarize_bash_input(input),
        "read" => summarize_path_tool("reading", input),
        "write" => summarize_path_tool("writing", input),
        "edit" | "multiedit" => summarize_path_tool("editing", input),
        "ls" | "list" => summarize_path_tool("listing", input),
        "grep" => summarize_grep_input(input),
        "glob" => string_field(input, &["pattern"]).map(|pattern| {
            format!(
                "matching {}",
                truncate_for_display(&collapse_whitespace(pattern), 90)
            )
        }),
        "webfetch" => summarize_path_tool("fetching", input),
        "websearch" => string_field(input, &["query"]).map(|query| {
            format!(
                "searching {}",
                truncate_for_display(&collapse_whitespace(query), 90)
            )
        }),
        "todowrite" => Some("updating todo list".to_string()),
        _ => summarize_generic_tool_input(input),
    };

    summary
        .map(|value| truncate_for_display(&value, TOOL_PREVIEW_MAX_CHARS))
        .unwrap_or_else(|| "running tool".to_string())
}

pub(crate) fn summarize_bash_input(input: &Value) -> Option<String> {
    let command = string_field(input, &["command", "cmd", "script"])?;
    summarize_bash_command(command)
}

pub(crate) fn summarize_bash_command(command: &str) -> Option<String> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }

    if let Some(target) = shell_write_target(command) {
        return Some(format!("writing {}", compact_shell_target(&target)));
    }

    if let Some(target) = shell_mkdir_target(command) {
        return Some(format!("creating directory {}", compact_tool_path(&target)));
    }

    first_meaningful_shell_fragment(command).map(|fragment| {
        truncate_for_display(&collapse_whitespace(&fragment), TOOL_PREVIEW_MAX_CHARS)
    })
}

pub(crate) fn summarize_path_tool(verb: &str, input: &Value) -> Option<String> {
    let path = string_field(
        input,
        &["file_path", "path", "notebook_path", "url", "directory"],
    )?;
    Some(format!("{verb} {}", compact_tool_path(path)))
}

pub(crate) fn summarize_grep_input(input: &Value) -> Option<String> {
    let pattern = string_field(input, &["pattern", "query"])?;
    if let Some(path) = string_field(input, &["path", "directory"]) {
        return Some(format!(
            "searching {} in {}",
            truncate_for_display(&collapse_whitespace(pattern), 60),
            compact_tool_path(path)
        ));
    }
    Some(format!(
        "searching {}",
        truncate_for_display(&collapse_whitespace(pattern), 90)
    ))
}

pub(crate) fn summarize_generic_tool_input(input: &Value) -> Option<String> {
    if let Some(path_summary) = summarize_path_tool("using", input) {
        return Some(path_summary);
    }
    if let Some(value) = input.as_str() {
        let value = collapse_whitespace(value);
        if !value.is_empty() {
            return Some(value);
        }
    }
    let object = input.as_object()?;
    let fields = object
        .keys()
        .take(3)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    if fields.is_empty() {
        None
    } else {
        Some(format!("input fields: {fields}"))
    }
}

pub(crate) fn string_field<'a>(input: &'a Value, keys: &[&str]) -> Option<&'a str> {
    for key in keys {
        if let Some(value) = input.get(*key).and_then(Value::as_str) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

pub(crate) fn shell_write_target(command: &str) -> Option<String> {
    for line in command
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(target) = extract_cat_write_target(line) {
            return Some(target);
        }
        if let Some(target) = extract_tee_write_target(line) {
            return Some(target);
        }
        if let Some(target) = extract_redirection_target(line) {
            return Some(target);
        }
        if line.contains("<<") {
            break;
        }
    }
    None
}

pub(crate) fn extract_cat_write_target(line: &str) -> Option<String> {
    if let Some(index) = line.find("cat >") {
        return first_shell_token(&line[index + "cat >".len()..])
            .filter(|target| is_useful_shell_target(target));
    }
    if line.starts_with("cat <<")
        && let Some(index) = line.rfind('>')
    {
        return first_shell_token(&line[index + 1..])
            .filter(|target| is_useful_shell_target(target));
    }
    None
}

pub(crate) fn extract_tee_write_target(line: &str) -> Option<String> {
    let index = line.find("tee ")?;
    let after = &line[index + "tee ".len()..];
    for token in after.split_whitespace() {
        if token.starts_with('-') {
            continue;
        }
        let token = clean_shell_token(token);
        if is_useful_shell_target(&token) {
            return Some(token);
        }
    }
    None
}

pub(crate) fn extract_redirection_target(line: &str) -> Option<String> {
    let mut target = None;
    for (index, _) in line.match_indices('>') {
        if let Some(token) = first_shell_token(&line[index + 1..])
            && is_useful_shell_target(&token)
        {
            target = Some(token);
        }
    }
    target
}

pub(crate) fn shell_mkdir_target(command: &str) -> Option<String> {
    for line in command
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(index) = line.find("mkdir ") {
            let after = &line[index + "mkdir ".len()..];
            for token in after.split_whitespace() {
                if token.starts_with('-') {
                    continue;
                }
                let token = clean_shell_token(token);
                if is_useful_shell_target(&token) {
                    return Some(token);
                }
            }
        }
    }
    None
}

pub(crate) fn first_meaningful_shell_fragment(command: &str) -> Option<String> {
    let line = command
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))?;
    let fragment = line
        .split("&&")
        .next()
        .unwrap_or(line)
        .split(';')
        .next()
        .unwrap_or(line)
        .trim();
    if fragment.is_empty() {
        None
    } else {
        Some(fragment.to_string())
    }
}

pub(crate) fn first_shell_token(value: &str) -> Option<String> {
    let value = value.trim_start();
    let mut chars = value.chars();
    let quote = match chars.next()? {
        '"' => Some('"'),
        '\'' => Some('\''),
        _ => None,
    };
    let mut token = String::new();
    if let Some(quote) = quote {
        for ch in chars {
            if ch == quote {
                break;
            }
            token.push(ch);
        }
    } else {
        for ch in value.chars() {
            if ch.is_whitespace() || matches!(ch, ';' | '|' | '<' | '>') {
                break;
            }
            token.push(ch);
        }
    }
    let token = clean_shell_token(&token);
    if token.is_empty() { None } else { Some(token) }
}

pub(crate) fn clean_shell_token(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_end_matches(';')
        .to_string()
}

pub(crate) fn is_useful_shell_target(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value != "/dev/null"
        && value != "&1"
        && value != "&2"
        && value != "1"
        && value != "2"
        && !value.starts_with('$')
}

pub(crate) fn compact_tool_path(path: &str) -> String {
    let path = collapse_whitespace(path);
    if path.chars().count() <= 90 {
        return path;
    }
    Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| truncate_for_display(&path, 90))
}

pub(crate) fn compact_claude_pane_metadata(text: &str, max_chars: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = compact.chars().take(max_chars).collect::<String>();
    if compact.chars().count() > max_chars {
        out.push('…');
    }
    out
}

pub(crate) fn compact_shell_target(path: &str) -> String {
    let path = collapse_whitespace(path);
    Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| compact_tool_path(&path))
}

pub(crate) fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
