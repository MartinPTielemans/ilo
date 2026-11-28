//! OpenAI's chat completions, as servers on this computer (Ollama, LM Studio, llama.cpp) and most
//! gateways speak it, at the address the user gives. Its listing names models and, where it's
//! OpenRouter's, says what each can do; elsewhere a model is taken to call tools and read text.

use serde_json::{Value, json};

use super::{
    Caps, Delta, Item, Listed, Part, Reader, Request, Stop, Thinking, Wire, args_of, fresh_id,
    text_of,
};

/// The context a model is taken to have where its listing doesn't say: a local server's default.
const CONTEXT: u32 = 8192;

pub struct Compat;

impl Wire for Compat {
    fn maker(&self) -> &'static str {
        "compat"
    }

    fn listing(&self) -> &'static str {
        "/models"
    }

    fn auth(&self, key: &str) -> Vec<(&'static str, String)> {
        vec![("authorization", format!("Bearer {key}"))]
    }

    fn models(&self, listing: &Value) -> Vec<Listed> {
        let models = listing.get("data").and_then(Value::as_array);
        models
            .into_iter()
            .flatten()
            .filter_map(|model| {
                let id = model.get("id")?.as_str()?.to_string();
                let name = model
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(&id)
                    .to_string();
                let mut caps = Caps::plain(
                    model
                        .get("context_length")
                        .and_then(Value::as_u64)
                        .map_or(CONTEXT, |context| context as u32),
                );
                let takes = |what: &str| {
                    model
                        .get("supported_parameters")
                        .and_then(Value::as_array)
                        .map(|all| all.iter().any(|it| it.as_str() == Some(what)))
                };
                if let Some(tools) = takes("tools") {
                    caps.tools = tools;
                    caps.parallel_tools = tools;
                }
                if takes("reasoning") == Some(true) {
                    caps.thinking = Thinking::Summaries;
                }
                caps.structured = takes("structured_outputs") == Some(true);
                caps.images_in = model
                    .pointer("/architecture/input_modalities")
                    .and_then(Value::as_array)
                    .is_some_and(|all| all.iter().any(|it| it.as_str() == Some("image")));
                Some(Listed { id, name, caps })
            })
            .collect()
    }

    fn call(&self, model: &str, request: &Request) -> (String, Value) {
        let mut body = json!({
            "model": model,
            "stream": true,
            "stream_options": { "include_usage": true },
            "max_tokens": request.max_out,
            "messages": messages(request),
        });
        if !request.tools.is_empty() {
            body["tools"] = request
                .tools
                .iter()
                .map(|tool| {
                    json!({ "type": "function", "function": {
                        "name": tool.name, "description": tool.description, "parameters": tool.schema,
                    } })
                })
                .collect();
        }
        if let Some(effort) = &request.effort {
            body["reasoning_effort"] = json!(effort);
        }
        if let Some(schema) = &request.schema {
            body["response_format"] = json!({ "type": "json_schema", "json_schema": {
                "name": "answer", "schema": schema, "strict": true,
            } });
        }
        ("/chat/completions".into(), body)
    }

    fn reader(&self) -> Box<dyn Reader> {
        Box::new(Chunks::default())
    }
}

/// The conversation as chat messages: what the model said and the calls after it in one
/// assistant message, each result a tool message, and a result's pictures in a user message
/// after the results, as a tool message takes text alone.
fn messages(request: &Request) -> Vec<Value> {
    let mut messages = vec![json!({ "role": "system", "content": request.instructions })];
    let mut pictures = Vec::new();
    for item in &request.items {
        if !matches!(item, Item::Result { .. }) && !pictures.is_empty() {
            messages.push(json!({ "role": "user", "content": std::mem::take(&mut pictures) }));
        }
        match item {
            Item::User(parts) => {
                messages.push(json!({ "role": "user", "content": content(parts) }))
            }
            Item::Said(text) => messages.push(json!({ "role": "assistant", "content": text })),
            Item::Thought(_) => {}
            Item::Call { id, name, args } => {
                let call = json!({ "id": id, "type": "function", "function": {
                    "name": name, "arguments": args.to_string(),
                } });
                match messages.last_mut() {
                    Some(last) if last["role"] == "assistant" => {
                        match last["tool_calls"].as_array_mut() {
                            Some(calls) => calls.push(call),
                            None => last["tool_calls"] = json!([call]),
                        }
                    }
                    _ => messages.push(
                        json!({ "role": "assistant", "content": null, "tool_calls": [call] }),
                    ),
                }
            }
            Item::Result { id, parts, .. } => {
                messages
                    .push(json!({ "role": "tool", "tool_call_id": id, "content": text_of(parts) }));
                pictures.extend(parts.iter().filter_map(|part| match part {
                    Part::Image { .. } => Some(part_of(part)),
                    Part::Text(_) => None,
                }));
            }
        }
    }
    if !pictures.is_empty() {
        messages.push(json!({ "role": "user", "content": pictures }));
    }
    messages
}

/// A user message's content: its text, or its parts where it has a picture.
fn content(parts: &[Part]) -> Value {
    match parts.iter().all(|part| matches!(part, Part::Text(_))) {
        true => json!(text_of(parts)),
        false => parts.iter().map(part_of).collect(),
    }
}

fn part_of(part: &Part) -> Value {
    match part {
        Part::Text(text) => json!({ "type": "text", "text": text }),
        Part::Image { media, base64 } => {
            json!({ "type": "image_url", "image_url": { "url": format!("data:{media};base64,{base64}") } })
        }
    }
}

/// A turn's chunks. Calls come in pieces by their index and are whole at the turn's end; the end
/// is `[DONE]`, after the usage, or the stream's close where a server sends no `[DONE]`.
#[derive(Default)]
struct Chunks {
    calls: Vec<(String, String, String)>,
    stop: Option<Stop>,
}

impl Chunks {
    fn finish(&mut self) -> Vec<Delta> {
        let Some(stop) = self.stop.take() else {
            return Vec::new();
        };
        let mut deltas: Vec<Delta> = self
            .calls
            .drain(..)
            .map(|(id, name, args)| Delta::Call {
                // Some servers send a call no id; a result names it by this one.
                id: match id.is_empty() {
                    true => fresh_id(),
                    false => id,
                },
                name,
                args: args_of(&args),
            })
            .collect();
        let stop = match deltas.is_empty() {
            true => stop,
            false => Stop::Calls,
        };
        deltas.push(Delta::Ended(stop));
        deltas
    }
}

impl Reader for Chunks {
    fn event(&mut self, _: Option<&str>, data: &str) -> Vec<Delta> {
        if data.trim() == "[DONE]" {
            return self.finish();
        }
        let Ok(chunk) = serde_json::from_str::<Value>(data) else {
            return Vec::new();
        };
        let mut deltas = Vec::new();
        if let Some(message) = chunk.pointer("/error/message").and_then(Value::as_str) {
            return vec![Delta::Failed(super::Failure::Other(message.into()))];
        }
        let choice = chunk.pointer("/choices/0");
        let delta = choice.and_then(|choice| choice.get("delta"));
        for key in ["reasoning_content", "reasoning"] {
            if let Some(text) = delta
                .and_then(|delta| delta.get(key))
                .and_then(Value::as_str)
                && !text.is_empty()
            {
                deltas.push(Delta::Thinking(text.into()));
            }
        }
        if let Some(text) = delta
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
            && !text.is_empty()
        {
            deltas.push(Delta::Text(text.into()));
        }
        let calls = delta
            .and_then(|delta| delta.get("tool_calls"))
            .and_then(Value::as_array);
        for call in calls.into_iter().flatten() {
            let index = call.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            if self.calls.len() <= index {
                self.calls.resize(index + 1, Default::default());
            }
            let (id, name, args) = &mut self.calls[index];
            if let Some(piece) = call.get("id").and_then(Value::as_str) {
                id.push_str(piece);
            }
            if let Some(piece) = call.pointer("/function/name").and_then(Value::as_str) {
                name.push_str(piece);
            }
            if let Some(piece) = call.pointer("/function/arguments").and_then(Value::as_str) {
                args.push_str(piece);
            }
        }
        if let Some(reason) = choice
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
        {
            self.stop = Some(match reason {
                "length" => Stop::Length,
                "tool_calls" => Stop::Calls,
                _ => Stop::Done,
            });
        }
        if let Some(usage) = chunk.get("usage").filter(|usage| usage.is_object()) {
            let count = |key| usage.get(key).and_then(Value::as_u64).unwrap_or(0) as u32;
            deltas.push(Delta::Usage {
                input: count("prompt_tokens"),
                output: count("completion_tokens"),
            });
        }
        deltas
    }

    fn end(&mut self) -> Vec<Delta> {
        self.finish()
    }
}
