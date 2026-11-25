//! Lightweight local HTTP bridge translating Anthropic `/v1/messages`
//! requests from Claude Code into OpenAI `/chat/completions` calls.
//!
//! Enables any `@opencode/ai/providers/openai-compatible` custom model (such as
//! a local or Colab llama.cpp / vLLM / Ollama endpoint) to power Claude Code
//! directly without external proxies.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use serde_json::{json, Value};

#[derive(Debug, Clone, Default)]
pub struct BridgeTarget {
    pub base_url: String,
    pub api_key: String,
    pub default_model: Option<String>,
    pub allowed_models: Vec<String>,
    pub local_api_key: String,
}

#[derive(Default)]
pub struct OpenAiBridgeState {
    inner: Mutex<Option<RunningBridge>>,
}

struct RunningBridge {
    port: u16,
    target: Arc<RwLock<BridgeTarget>>,
}

impl OpenAiBridgeState {
    /// Ensure the loopback bridge server is listening and update its live target.
    /// Returns `(endpoint_url, local_api_key)`.
    pub fn configure(&self, mut next: BridgeTarget) -> Result<(String, String), String> {
        if next.local_api_key.trim().is_empty() {
            next.local_api_key = format!("hb-{}", uuid::Uuid::new_v4().simple());
        }
        let mut guard = self.inner.lock().unwrap();
        if let Some(running) = guard.as_ref() {
            let local_key = {
                let mut t = running.target.write().unwrap();
                if !t.local_api_key.is_empty() {
                    next.local_api_key = t.local_api_key.clone();
                }
                let k = next.local_api_key.clone();
                *t = next;
                k
            };
            return Ok((format!("http://127.0.0.1:{}", running.port), local_key));
        }

        let local_key = next.local_api_key.clone();
        let target = Arc::new(RwLock::new(next));
        let port = spawn_bridge_listener(target.clone())?;
        *guard = Some(RunningBridge { port, target });
        Ok((format!("http://127.0.0.1:{port}"), local_key))
    }
}

pub fn spawn_bridge_listener(target: Arc<RwLock<BridgeTarget>>) -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("OpenAI bridge portu bağlanamadı: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("OpenAI bridge adresi okunamadı: {e}"))?
        .port();

    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let t = target.clone();
            std::thread::spawn(move || {
                let _ = handle_connection(stream, t);
            });
        }
    });

    Ok(port)
}

fn handle_connection(mut stream: TcpStream, target: Arc<RwLock<BridgeTarget>>) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(120)));

    let (method, path, body) = match read_http_request(&mut stream) {
        Ok(req) => req,
        Err(_) => return Ok(()),
    };

    let clean_path = path.split('?').next().unwrap_or(path.as_str());
    if method == "GET" && (clean_path == "/health" || clean_path == "/v1/models") {
        let t = target.read().unwrap().clone();
        let payload = if clean_path == "/health" {
            json!({ "ok": true }).to_string()
        } else {
            let data: Vec<Value> = t
                .allowed_models
                .iter()
                .map(|m| json!({ "id": m, "object": "model" }))
                .collect();
            json!({ "object": "list", "data": data }).to_string()
        };
        return write_json_response(&mut stream, 200, &payload);
    }

    if method == "POST" && (clean_path == "/v1/messages" || clean_path == "/messages") {
        let req_json: Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => {
                let err = json!({ "type": "error", "error": { "type": "invalid_request_error", "message": format!("Invalid JSON: {e}") } });
                return write_json_response(&mut stream, 400, &err.to_string());
            }
        };

        let t = target.read().unwrap().clone();
        let is_stream = req_json.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
        let openai_req = anthropic_to_openai_request(&req_json, &t);
        let url = chat_completions_url(&t.base_url);

        let mut agent_req = ureq::post(&url)
            .timeout(Duration::from_secs(180))
            .set("Content-Type", "application/json");
        if !t.api_key.trim().is_empty() {
            agent_req = agent_req.set("Authorization", &format!("Bearer {}", t.api_key.trim()));
        }

        let resp = match agent_req.send_json(openai_req) {
            Ok(r) => r,
            Err(ureq::Error::Status(code, r)) => {
                let msg = r.into_string().unwrap_or_else(|_| format!("Upstream HTTP {code}"));
                let err = json!({ "type": "error", "error": { "type": "api_error", "message": msg } });
                return write_json_response(&mut stream, code, &err.to_string());
            }
            Err(e) => {
                let err = json!({ "type": "error", "error": { "type": "api_error", "message": format!("Upstream error: {e}") } });
                return write_json_response(&mut stream, 502, &err.to_string());
            }
        };

        let requested_model = req_json
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("custom")
            .to_string();

        if is_stream {
            return stream_openai_to_anthropic(resp.into_reader(), &mut stream, &requested_model);
        }

        let body_str = resp.into_string().unwrap_or_default();
        let openai_json: Value = serde_json::from_str(&body_str).unwrap_or_else(|_| json!({}));
        let anthropic_resp = openai_to_anthropic_response(&openai_json, &requested_model);
        return write_json_response(&mut stream, 200, &anthropic_resp.to_string());
    }

    let not_found = json!({ "type": "error", "error": { "type": "not_found_error", "message": "Not found" } });
    write_json_response(&mut stream, 404, &not_found.to_string())
}

pub fn chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    }
}

pub fn pick_target_model(requested: Option<&str>, target: &BridgeTarget) -> String {
    if let Some(req) = requested {
        if target.allowed_models.iter().any(|m| m == req) {
            return req.to_string();
        }
    }
    target
        .default_model
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| target.allowed_models.first().cloned())
        .or_else(|| requested.map(String::from))
        .unwrap_or_else(|| "default".to_string())
}

pub fn anthropic_to_openai_request(req: &Value, target: &BridgeTarget) -> Value {
    let requested = req.get("model").and_then(|v| v.as_str());
    let model = pick_target_model(requested, target);
    let mut messages: Vec<Value> = Vec::new();

    if let Some(sys) = req.get("system") {
        let text = match sys {
            Value::String(s) => s.clone(),
            Value::Array(arr) => arr
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        };
        if !text.is_empty() {
            messages.push(json!({ "role": "system", "content": text }));
        }
    }

    if let Some(arr) = req.get("messages").and_then(|v| v.as_array()) {
        for msg in arr {
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let content = msg.get("content").unwrap_or(&Value::Null);
            match content {
                Value::String(s) => {
                    messages.push(json!({ "role": role, "content": s }));
                }
                Value::Array(blocks) => {
                    let mut text_parts = Vec::new();
                    let mut tool_calls = Vec::new();
                    let mut tool_results = Vec::new();

                    for b in blocks {
                        let btype = b.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        match btype {
                            "text" => {
                                if let Some(t) = b.get("text").and_then(|v| v.as_str()) {
                                    text_parts.push(t.to_string());
                                }
                            }
                            "tool_use" => {
                                let id = b.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                let name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                let input = b.get("input").cloned().unwrap_or_else(|| json!({}));
                                tool_calls.push(json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": serde_json::to_string(&input).unwrap_or_else(|_| "{}".into()),
                                    }
                                }));
                            }
                            "tool_result" => {
                                let call_id = b
                                    .get("tool_use_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let text = match b.get("content") {
                                    Some(Value::String(s)) => s.clone(),
                                    Some(Value::Array(parts)) => parts
                                        .iter()
                                        .filter_map(|p| p.get("text").and_then(|v| v.as_str()))
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                    Some(other) => other.to_string(),
                                    None => String::new(),
                                };
                                tool_results.push(json!({
                                    "role": "tool",
                                    "tool_call_id": call_id,
                                    "content": text,
                                }));
                            }
                            _ => {}
                        }
                    }

                    if !tool_results.is_empty() {
                        messages.extend(tool_results);
                    }
                    if !text_parts.is_empty() || !tool_calls.is_empty() {
                        let mut out = json!({ "role": role });
                        let joined = text_parts.join("\n");
                        if !joined.is_empty() || tool_calls.is_empty() {
                            out["content"] = Value::String(joined);
                        } else {
                            out["content"] = Value::Null;
                        }
                        if !tool_calls.is_empty() {
                            out["tool_calls"] = Value::Array(tool_calls);
                        }
                        messages.push(out);
                    }
                }
                _ => {}
            }
        }
    }

    let mut payload = json!({
        "model": model,
        "messages": messages,
        "stream": req.get("stream").and_then(|v| v.as_bool()).unwrap_or(false),
    });
    if let Some(max_t) = req.get("max_tokens").and_then(|v| v.as_u64()) {
        payload["max_tokens"] = json!(max_t);
    }
    if let Some(temp) = req.get("temperature") {
        payload["temperature"] = temp.clone();
    }
    if let Some(tools) = req.get("tools").and_then(|v| v.as_array()) {
        let converted: Vec<Value> = tools
            .iter()
            .filter_map(|tool| {
                let name = tool.get("name")?.as_str()?;
                let desc = tool.get("description").and_then(|v| v.as_str()).unwrap_or("");
                let params = tool
                    .get("input_schema")
                    .cloned()
                    .unwrap_or_else(|| json!({ "type": "object", "properties": {} }));
                Some(json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": desc,
                        "parameters": params,
                    }
                }))
            })
            .collect();
        if !converted.is_empty() {
            payload["tools"] = Value::Array(converted);
        }
    }
    payload
}

pub fn openai_to_anthropic_response(resp: &Value, fallback_model: &str) -> Value {
    let choice = resp
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or_else(|| json!({}));
    let msg = choice.get("message").cloned().unwrap_or_else(|| json!({}));

    let mut blocks: Vec<Value> = Vec::new();
    if let Some(text) = msg.get("content").and_then(|v| v.as_str()) {
        if !text.is_empty() {
            blocks.push(json!({ "type": "text", "text": text }));
        }
    }
    let mut has_tool_use = false;
    if let Some(calls) = msg.get("tool_calls").and_then(|v| v.as_array()) {
        for (i, c) in calls.iter().enumerate() {
            has_tool_use = true;
            let id = c
                .get("id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| format!("toolu_{i}"));
            let name = c
                .pointer("/function/name")
                .and_then(|v| v.as_str())
                .unwrap_or("tool");
            let args_raw = c
                .pointer("/function/arguments")
                .and_then(|v| v.as_str())
                .unwrap_or("{}");
            let input: Value = serde_json::from_str(args_raw).unwrap_or_else(|_| json!({}));
            blocks.push(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input,
            }));
        }
    }
    if blocks.is_empty() {
        blocks.push(json!({ "type": "text", "text": "" }));
    }

    let finish = choice
        .get("finish_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("stop");
    let stop_reason = if has_tool_use || finish == "tool_calls" {
        "tool_use"
    } else if finish == "length" {
        "max_tokens"
    } else {
        "end_turn"
    };

    let input_tokens = resp
        .pointer("/usage/prompt_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output_tokens = resp
        .pointer("/usage/completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    json!({
        "id": resp.get("id").and_then(|v| v.as_str()).unwrap_or("msg_bridge"),
        "type": "message",
        "role": "assistant",
        "model": resp.get("model").and_then(|v| v.as_str()).unwrap_or(fallback_model),
        "content": blocks,
        "stop_reason": stop_reason,
        "stop_sequence": Value::Null,
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
        }
    })
}

fn stream_openai_to_anthropic<R: Read>(
    reader: R,
    out: &mut TcpStream,
    model: &str,
) -> std::io::Result<()> {
    out.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
    )?;

    let start_evt = json!({
        "type": "message_start",
        "message": {
            "id": format!("msg_{}", uuid::Uuid::new_v4().simple()),
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": model,
            "stop_reason": Value::Null,
            "stop_sequence": Value::Null,
            "usage": { "input_tokens": 0, "output_tokens": 0 }
        }
    });
    write_sse(out, "message_start", &start_evt)?;

    let mut next_block_idx: usize = 0;
    let mut active_text_idx: Option<usize> = None;
    let mut tool_idx_map: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut stop_reason = "end_turn".to_string();
    let mut output_tokens: u64 = 0;

    let buf = BufReader::new(reader);
    for line in buf.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        let Some(data) = trimmed.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        let Ok(chunk) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if let Some(usage) = chunk.get("usage") {
            if let Some(ot) = usage.get("completion_tokens").and_then(|v| v.as_u64()) {
                output_tokens = ot;
            }
        }
        let Some(choice) = chunk
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
        else {
            continue;
        };
        if let Some(fin) = choice.get("finish_reason").and_then(|v| v.as_str()) {
            stop_reason = match fin {
                "tool_calls" => "tool_use".into(),
                "length" => "max_tokens".into(),
                _ => "end_turn".into(),
            };
        }
        let Some(delta) = choice.get("delta") else {
            continue;
        };
        if let Some(txt) = delta.get("content").and_then(|v| v.as_str()) {
            if !txt.is_empty() {
                let idx = match active_text_idx {
                    Some(i) => i,
                    None => {
                        let i = next_block_idx;
                        next_block_idx += 1;
                        active_text_idx = Some(i);
                        write_sse(
                            out,
                            "content_block_start",
                            &json!({
                                "type": "content_block_start",
                                "index": i,
                                "content_block": { "type": "text", "text": "" }
                            }),
                        )?;
                        i
                    }
                };
                write_sse(
                    out,
                    "content_block_delta",
                    &json!({
                        "type": "content_block_delta",
                        "index": idx,
                        "delta": { "type": "text_delta", "text": txt }
                    }),
                )?;
            }
        }
        if let Some(tcalls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
            if let Some(ti) = active_text_idx.take() {
                write_sse(
                    out,
                    "content_block_stop",
                    &json!({ "type": "content_block_stop", "index": ti }),
                )?;
            }
            for tc in tcalls {
                let tc_idx = tc.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let block_idx = match tool_idx_map.get(&tc_idx).copied() {
                    Some(bi) => bi,
                    None => {
                        let bi = next_block_idx;
                        next_block_idx += 1;
                        tool_idx_map.insert(tc_idx, bi);
                        let id = tc
                            .get("id")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .unwrap_or_else(|| format!("toolu_{bi}"));
                        let name = tc
                            .pointer("/function/name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("tool");
                        write_sse(
                            out,
                            "content_block_start",
                            &json!({
                                "type": "content_block_start",
                                "index": bi,
                                "content_block": {
                                    "type": "tool_use",
                                    "id": id,
                                    "name": name,
                                    "input": {}
                                }
                            }),
                        )?;
                        bi
                    }
                };
                if let Some(args_part) = tc.pointer("/function/arguments").and_then(|v| v.as_str()) {
                    if !args_part.is_empty() {
                        write_sse(
                            out,
                            "content_block_delta",
                            &json!({
                                "type": "content_block_delta",
                                "index": block_idx,
                                "delta": { "type": "input_json_delta", "partial_json": args_part }
                            }),
                        )?;
                    }
                }
            }
        }
    }

    if let Some(ti) = active_text_idx.take() {
        write_sse(
            out,
            "content_block_stop",
            &json!({ "type": "content_block_stop", "index": ti }),
        )?;
    }
    let mut closed_tools: Vec<usize> = tool_idx_map.into_values().collect();
    closed_tools.sort_unstable();
    for bi in closed_tools {
        write_sse(
            out,
            "content_block_stop",
            &json!({ "type": "content_block_stop", "index": bi }),
        )?;
    }

    write_sse(
        out,
        "message_delta",
        &json!({
            "type": "message_delta",
            "delta": { "stop_reason": stop_reason, "stop_sequence": Value::Null },
            "usage": { "output_tokens": output_tokens }
        }),
    )?;
    write_sse(out, "message_stop", &json!({ "type": "message_stop" }))?;
    out.flush()
}

fn write_sse(out: &mut TcpStream, event: &str, data: &Value) -> std::io::Result<()> {
    let msg = format!("event: {event}\ndata: {}\n\n", data);
    out.write_all(msg.as_bytes())
}

fn write_json_response(out: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        502 => "Bad Gateway",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    out.write_all(head.as_bytes())?;
    out.write_all(body.as_bytes())?;
    out.flush()
}

fn read_http_request(stream: &mut TcpStream) -> std::io::Result<(String, String, Vec<u8>)> {
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 2048];
    let header_end;
    loop {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "eof"));
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_double_crlf(&buf) {
            header_end = pos;
            break;
        }
        if buf.len() > 256 * 1024 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "header too large"));
        }
    }

    let head_str = String::from_utf8_lossy(&buf[..header_end]);
    let mut lines = head_str.lines();
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let path = parts.next().unwrap_or("/").to_string();

    let mut content_len: usize = 0;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                content_len = v.trim().parse().unwrap_or(0);
            }
        }
    }

    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < content_len {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_len);
    Ok((method, path, body))
}

fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_anthropic_to_openai_and_back() {
        let target = BridgeTarget {
            base_url: "https://colab.example/v1".into(),
            api_key: "secret".into(),
            default_model: Some("Qwen3.5-9B.gguf".into()),
            allowed_models: vec!["Qwen3.5-9B.gguf".into()],
            local_api_key: "hb-1".into(),
        };
        let req = json!({
            "model": "claude-sonnet-4-5",
            "system": "Be concise",
            "max_tokens": 512,
            "messages": [
                { "role": "user", "content": [{ "type": "text", "text": "Hi" }] }
            ],
            "tools": [{
                "name": "read",
                "description": "Read file",
                "input_schema": { "type": "object" }
            }]
        });
        let openai = anthropic_to_openai_request(&req, &target);
        assert_eq!(openai["model"], json!("Qwen3.5-9B.gguf"));
        assert_eq!(openai["messages"][0]["role"], json!("system"));
        assert_eq!(openai["tools"][0]["function"]["name"], json!("read"));

        let upstream = json!({
            "id": "chatcmpl-1",
            "model": "Qwen3.5-9B.gguf",
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "content": "Checking",
                    "tool_calls": [{
                        "id": "call_1",
                        "function": { "name": "read", "arguments": "{\"path\":\"a.txt\"}" }
                    }]
                }
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5 }
        });
        let anth = openai_to_anthropic_response(&upstream, "fallback");
        assert_eq!(anth["stop_reason"], json!("tool_use"));
        assert_eq!(anth["content"][0]["text"], json!("Checking"));
        assert_eq!(anth["content"][1]["name"], json!("read"));
        assert_eq!(anth["content"][1]["input"]["path"], json!("a.txt"));
    }

    #[test]
    fn bridge_forwards_non_stream_over_loopback() {
        let upstream = TcpListener::bind("127.0.0.1:0").unwrap();
        let up_port = upstream.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = upstream.accept() {
                let _ = read_http_request(&mut s);
                let reply = json!({
                    "id": "cmpl-test",
                    "model": "qwen-9b",
                    "choices": [{
                        "finish_reason": "stop",
                        "message": { "content": "Merhaba dunya" }
                    }],
                    "usage": { "prompt_tokens": 4, "completion_tokens": 2 }
                })
                .to_string();
                let _ = write_json_response(&mut s, 200, &reply);
            }
        });

        let bridge = OpenAiBridgeState::default();
        let (endpoint, local_key) = bridge
            .configure(BridgeTarget {
                base_url: format!("http://127.0.0.1:{up_port}/v1"),
                api_key: "upstream-key".into(),
                default_model: Some("qwen-9b".into()),
                allowed_models: vec!["qwen-9b".into()],
                local_api_key: String::new(),
            })
            .unwrap();
        assert!(!local_key.is_empty());

        let resp = ureq::post(&format!("{endpoint}/v1/messages"))
            .send_json(json!({
                "model": "claude-sonnet-4-5",
                "max_tokens": 64,
                "messages": [{ "role": "user", "content": "Selam" }]
            }))
            .unwrap();
        let val: Value = resp.into_json().unwrap();
        assert_eq!(val["content"][0]["text"], json!("Merhaba dunya"));
        assert_eq!(val["model"], json!("qwen-9b"));
    }
}
