//! SSE event parsing for streaming responses

#[derive(Debug, Clone)]
pub enum SseEvent {
    TextDelta(String),
    ToolUse { id: String, name: String, input: String },
    MessageStop,
    Usage { input_tokens: u64, output_tokens: u64 },
}

/// Context for parsing SSE events - holds accumulated state across multiple lines
#[derive(Default)]
pub struct SseParserContext {
    pub current_event_type: Option<String>,
    pub tool_id: Option<String>,
    pub tool_name: Option<String>,
    pub input_json_accumulator: String,
    pub in_tool_use_block: bool,
}


/// Parse a single SSE line and return any events produced
/// The context is updated to maintain state across multiple lines
pub fn parse_sse_line(line: &str, ctx: &mut SseParserContext) -> Vec<SseEvent> {
    let line = line.trim();
    let mut events = Vec::new();

    // Empty line or comment - may indicate end of event
    if line.is_empty() || line.starts_with(':') {
        return events;
    }

    // Handle "event: type" lines - update current event type
    if let Some(event_type) = line.strip_prefix("event:") {
        ctx.current_event_type = Some(event_type.trim().to_string());
        return events;
    }

    // Handle "data: ..." lines
    let Some(data) = line.strip_prefix("data: ") else {
        return events;
    };

    if data == "[DONE]" {
        events.push(SseEvent::MessageStop);
        return events;
    }

    let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
        return events;
    };

    // Use the event type from the preceding event line, or fall back to JSON type
    let event_type = ctx.current_event_type.clone()
        .or_else(|| json.get("type").and_then(|t| t.as_str()).map(|s| s.to_string()));

    let event_type = match event_type {
        Some(t) => t,
        None => return events,
    };

    log::debug!("[SSE] Event type: {}, data: {}", event_type, &data[..data.len().min(200)]);

    match event_type.as_str() {
        "content_block_delta" => {
            let delta = match json.get("delta") {
                Some(d) => d,
                None => return events,
            };
            let delta_type = delta.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
            log::info!("[SSE] content_block_delta, delta_type={}", delta_type);

            match delta_type {
                "text_delta" => {
                    if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                        events.push(SseEvent::TextDelta(text.to_string()));
                    }
                }
                "input_json_delta" => {
                    if let Some(partial) = delta.get("partial_json").and_then(|p| p.as_str()) {
                        log::info!("[SSE] input_json_delta partial: {}", partial);
                        ctx.input_json_accumulator.push_str(partial);
                    }
                }
                "thinking_delta" => {
                    // Skip thinking content
                }
                _ => {
                    log::debug!("[SSE] Unknown delta type: {}", delta_type);
                }
            }
        }
        "message_delta" => {
            if let Some(stop_reason) = json.get("delta").and_then(|d| d.get("stop_reason")) {
                if stop_reason.as_str() == Some("tool_use") || stop_reason.as_str() == Some("end_turn") {
                    events.push(SseEvent::MessageStop);
                }
            }
        }
        "message_stop" => {
            events.push(SseEvent::MessageStop);
        }
        "content_block_start" => {
            if let Some(content_block) = json.get("content_block") {
                let block_type = content_block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                log::info!("[SSE] content_block_start, block_type={}", block_type);

                if block_type == "tool_use" {
                    ctx.tool_id = content_block.get("id").and_then(|i| i.as_str()).map(|s| s.to_string());
                    ctx.tool_name = content_block.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());
                    ctx.input_json_accumulator.clear();
                    ctx.in_tool_use_block = true;
                    log::info!("[SSE] ToolUse block started: id={:?}, name={:?}", ctx.tool_id, ctx.tool_name);
                }
            }
        }
        "content_block_stop" => {
            if ctx.in_tool_use_block {
                // Try to parse accumulated JSON
                let accumulated = &ctx.input_json_accumulator;
                log::info!("[SSE] content_block_stop, accumulated JSON: {}", accumulated);

                if !accumulated.is_empty() {
                    if let Ok(input_val) = serde_json::from_str::<serde_json::Value>(accumulated) {
                        let id = ctx.tool_id.clone().unwrap_or_else(|| "tool-1".to_string());
                        let name = ctx.tool_name.clone().unwrap_or_else(|| "unknown".to_string());
                        let input_str = serde_json::to_string(&input_val).unwrap_or_else(|_| "{}".to_string());
                        log::info!("[SSE] Emitting ToolUse: id={}, name={}, input={}", id, name, input_str);
                        events.push(SseEvent::ToolUse { id, name, input: input_str });
                    } else {
                        log::warn!("[SSE] Failed to parse accumulated input_json: {}", accumulated);
                    }
                }

                ctx.in_tool_use_block = false;
                ctx.tool_id = None;
                ctx.tool_name = None;
                ctx.input_json_accumulator.clear();
            }
        }
        "message_start" | "ping" => {
            // These are informational, skip
        }
        "message_metadata" => {
            // Extract usage information from metadata
            if let Some(metadata) = json.get("metadata") {
                let input_tokens = metadata.get("usage")
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0);
                let output_tokens = metadata.get("usage")
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0);
                if input_tokens > 0 || output_tokens > 0 {
                    events.push(SseEvent::Usage { input_tokens, output_tokens });
                }
            }
        }
        _ => {
            log::warn!("[SSE] Unhandled event type: {}", event_type);
        }
    }

    // Clear the current event type after processing this data line
    ctx.current_event_type = None;

    events
}

/// Parse a complete SSE payload (multiple lines) into events
#[allow(dead_code)]
pub fn parse_sse_payload(payload: &str) -> Vec<SseEvent> {
    let mut events = Vec::new();
    let mut ctx = SseParserContext::default();

    for line in payload.lines() {
        let line_events = parse_sse_line(line, &mut ctx);
        events.extend(line_events);
    }

    events
}
