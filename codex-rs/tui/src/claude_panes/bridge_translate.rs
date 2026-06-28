//! Request/response translation between Claude Code's Anthropic wire format
//! and Ambient Chat Completions, plus SSE event writers for streaming.

use anyhow::Result;
use anyhow::anyhow;
use serde_json::Value;
use tokio::io::AsyncWriteExt;

use super::turn_types::BridgeToolCall;
use uuid::Uuid;
pub(crate) fn ambient_chat_messages_from_claude_request(request: &Value) -> Result<Vec<Value>> {
    let mut messages = Vec::new();
    if let Some(system) = request.get("system") {
        let system_text = claude_content_to_text(system);
        if !system_text.trim().is_empty() {
            messages.push(serde_json::json!({ "role": "system", "content": system_text }));
        }
    }
    for message in request
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Claude Messages request missing messages array"))?
    {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user");
        let content = message.get("content").unwrap_or(&Value::Null);
        if role == "assistant" {
            let text = claude_text_blocks_to_text(content);
            let tool_calls = ambient_assistant_tool_calls_from_claude_content(content);
            if text.trim().is_empty() && tool_calls.is_empty() {
                continue;
            }
            let mut assistant = serde_json::Map::new();
            assistant.insert("role".to_string(), Value::String("assistant".to_string()));
            assistant.insert(
                "content".to_string(),
                if text.trim().is_empty() {
                    Value::Null
                } else {
                    Value::String(text)
                },
            );
            if !tool_calls.is_empty() {
                assistant.insert("tool_calls".to_string(), Value::Array(tool_calls));
            }
            messages.push(Value::Object(assistant));
            continue;
        }

        let text = claude_text_blocks_to_text(content);
        if !text.trim().is_empty() {
            messages.push(serde_json::json!({ "role": role, "content": text }));
        }
        for tool_result in ambient_tool_result_messages_from_claude_content(content) {
            messages.push(tool_result);
        }
    }
    if messages.is_empty() {
        messages.push(serde_json::json!({ "role": "user", "content": "Continue." }));
    }
    Ok(messages)
}

pub(crate) fn ambient_chat_tools_from_claude_request(request: &Value) -> Vec<Value> {
    request
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| {
            let name = tool.get("name").and_then(Value::as_str)?;
            let description = tool
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let parameters = tool
                .get("input_schema")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({ "type": "object" }));
            Some(serde_json::json!({
                "type": "function",
                "function": {
                    "name": name,
                    "description": description,
                    "parameters": parameters
                }
            }))
        })
        .collect()
}

pub(crate) fn ambient_assistant_tool_calls_from_claude_content(content: &Value) -> Vec<Value> {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter_map(|item| {
            let id = item.get("id").and_then(Value::as_str)?;
            let name = item.get("name").and_then(Value::as_str)?;
            let input = item
                .get("input")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            Some(serde_json::json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": input.to_string()
                }
            }))
        })
        .collect()
}

pub(crate) fn ambient_tool_result_messages_from_claude_content(content: &Value) -> Vec<Value> {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("tool_result"))
        .filter_map(|item| {
            let tool_call_id = item.get("tool_use_id").and_then(Value::as_str)?;
            Some(serde_json::json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": claude_content_to_text(item.get("content").unwrap_or(&Value::Null))
            }))
        })
        .collect()
}

pub(crate) fn claude_text_blocks_to_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

pub(crate) fn claude_content_to_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    return Some(text.to_string());
                }
                if let Some(text) = item.get("content").and_then(Value::as_str) {
                    return Some(text.to_string());
                }
                None
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

pub(crate) fn anthropic_message_response(model: &str, text: &str, usage: &Value) -> Value {
    serde_json::json!({
        "id": format!("msg_pfterminal_{}", Uuid::new_v4().simple()),
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": [{ "type": "text", "text": text }],
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": anthropic_response_usage(usage)
    })
}

pub(crate) fn bridge_tool_calls_from_ambient_response(upstream: &Value) -> Vec<BridgeToolCall> {
    upstream
        .pointer("/choices/0/message/tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool_call| {
            let id = tool_call.get("id").and_then(Value::as_str)?;
            let function = tool_call.get("function")?;
            let name = function.get("name").and_then(Value::as_str)?;
            let arguments = function
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let input = serde_json::from_str(arguments).unwrap_or_else(|_| {
                serde_json::json!({
                    "_raw_arguments": arguments
                })
            });
            Some(BridgeToolCall {
                id: id.to_string(),
                name: name.to_string(),
                input,
            })
        })
        .collect()
}

pub(crate) fn anthropic_tool_use_response(
    model: &str,
    tool_calls: &[BridgeToolCall],
    usage: &Value,
) -> Value {
    let content = tool_calls
        .iter()
        .map(|tool_call| {
            serde_json::json!({
                "type": "tool_use",
                "id": tool_call.id,
                "name": tool_call.name,
                "input": tool_call.input
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "id": format!("msg_pfterminal_{}", Uuid::new_v4().simple()),
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content,
        "stop_reason": "tool_use",
        "stop_sequence": null,
        "usage": anthropic_response_usage(usage)
    })
}

pub(crate) fn anthropic_response_usage(usage: &Value) -> Value {
    let mut usage_map = serde_json::Map::new();
    usage_map.insert(
        "input_tokens".to_string(),
        Value::from(
            usage
                .get("prompt_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        ),
    );
    usage_map.insert(
        "output_tokens".to_string(),
        Value::from(
            usage
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        ),
    );
    for source in ["cached_tokens", "cache_read_input_tokens"] {
        if let Some(value) = usage.get(source).and_then(Value::as_u64) {
            usage_map.insert("cache_read_input_tokens".to_string(), Value::from(value));
        }
    }
    Value::Object(usage_map)
}

pub(crate) async fn write_json_response(
    stream: &mut tokio::net::TcpStream,
    body: Value,
) -> Result<()> {
    write_json_status_response(stream, 200, body).await
}

pub(crate) async fn write_json_status_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    body: Value,
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        429 => "Too Many Requests",
        _ => "Error",
    };
    let body = body.to_string();
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

pub(crate) async fn write_raw_http_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body).await?;
    Ok(())
}

pub(crate) async fn write_anthropic_stream_headers(
    stream: &mut tokio::net::TcpStream,
) -> Result<()> {
    let response = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n";
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

pub(crate) async fn write_anthropic_stream_start(
    stream: &mut tokio::net::TcpStream,
    model: &str,
    usage: &Value,
) -> Result<()> {
    write_sse_event(
        stream,
        "message_start",
        &anthropic_stream_start_event(model, usage),
    )
    .await
}

pub(crate) async fn write_anthropic_stream_ping(stream: &mut tokio::net::TcpStream) -> Result<()> {
    write_sse_event(stream, "ping", &serde_json::json!({ "type": "ping" })).await
}

pub(crate) async fn write_anthropic_stream_error(
    stream: &mut tokio::net::TcpStream,
    error_type: &str,
    message: &str,
) -> Result<()> {
    write_sse_event(
        stream,
        "error",
        &anthropic_stream_error_event(error_type, message),
    )
    .await
}

pub(crate) fn anthropic_stream_error_event(error_type: &str, message: &str) -> Value {
    serde_json::json!({
        "type": "error",
        "error": {
            "type": error_type,
            "message": message
        }
    })
}

pub(crate) fn anthropic_stream_start_event(model: &str, usage: &Value) -> Value {
    let mut usage_map = serde_json::Map::new();
    usage_map.insert(
        "input_tokens".to_string(),
        Value::from(
            usage
                .get("prompt_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        ),
    );
    usage_map.insert("output_tokens".to_string(), Value::from(0_u64));
    for source in ["cached_tokens", "cache_read_input_tokens"] {
        if let Some(value) = usage.get(source).and_then(Value::as_u64) {
            usage_map.insert("cache_read_input_tokens".to_string(), Value::from(value));
        }
    }
    serde_json::json!({
        "type": "message_start",
        "message": {
            "id": format!("msg_pfterminal_{}", Uuid::new_v4().simple()),
            "type": "message",
            "role": "assistant",
            "model": model,
            "content": [],
            "stop_reason": null,
            "stop_sequence": null,
            "usage": Value::Object(usage_map)
        }
    })
}

pub(crate) async fn write_anthropic_stream_text_completion(
    stream: &mut tokio::net::TcpStream,
    model: &str,
    text: &str,
    usage: &Value,
) -> Result<()> {
    write_anthropic_stream_start(stream, model, usage).await?;
    write_sse_event(
        stream,
        "content_block_start",
        &serde_json::json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": { "type": "text", "text": "" }
        }),
    )
    .await?;
    write_sse_event(
        stream,
        "content_block_delta",
        &serde_json::json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": { "type": "text_delta", "text": text }
        }),
    )
    .await?;
    write_sse_event(
        stream,
        "content_block_stop",
        &serde_json::json!({ "type": "content_block_stop", "index": 0 }),
    )
    .await?;
    write_anthropic_stream_stop(stream, "end_turn", usage).await
}

pub(crate) async fn write_anthropic_stream_tool_use_completion(
    stream: &mut tokio::net::TcpStream,
    model: &str,
    tool_calls: &[BridgeToolCall],
    usage: &Value,
) -> Result<()> {
    write_anthropic_stream_start(stream, model, usage).await?;
    for (index, tool_call) in tool_calls.iter().enumerate() {
        let partial_json = tool_call.input.to_string();
        write_sse_event(
            stream,
            "content_block_start",
            &serde_json::json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {
                    "type": "tool_use",
                    "id": tool_call.id,
                    "name": tool_call.name,
                    "input": {}
                }
            }),
        )
        .await?;
        write_sse_event(
            stream,
            "content_block_delta",
            &serde_json::json!({
                "type": "content_block_delta",
                "index": index,
                "delta": { "type": "input_json_delta", "partial_json": partial_json }
            }),
        )
        .await?;
        write_sse_event(
            stream,
            "content_block_stop",
            &serde_json::json!({ "type": "content_block_stop", "index": index }),
        )
        .await?;
    }
    write_anthropic_stream_stop(stream, "tool_use", usage).await
}

pub(crate) async fn write_anthropic_stream_stop(
    stream: &mut tokio::net::TcpStream,
    stop_reason: &str,
    usage: &Value,
) -> Result<()> {
    write_sse_event(
        stream,
        "message_delta",
        &anthropic_stream_stop_event(stop_reason, usage),
    )
    .await?;
    write_sse_event(
        stream,
        "message_stop",
        &serde_json::json!({ "type": "message_stop" }),
    )
    .await
}

pub(crate) fn anthropic_stream_stop_event(stop_reason: &str, usage: &Value) -> Value {
    serde_json::json!({
        "type": "message_delta",
        "delta": { "stop_reason": stop_reason, "stop_sequence": null },
        "usage": {
            "output_tokens": usage.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0)
        }
    })
}

pub(crate) async fn write_sse_event(
    stream: &mut tokio::net::TcpStream,
    event: &str,
    data: &Value,
) -> Result<()> {
    let body = format!("event: {event}\ndata: {data}\n\n");
    stream.write_all(body.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}
