//! Anthropic's Messages API with the user's key. Its listing says what each model can do; its
//! reasoning comes as summaries, signed, and goes back to it unchanged. The instructions, the
//! tools and the conversation so far are marked for its cache, so each step of a turn pays for
//! what's new alone.

use serde_json::{Value, json};

use super::{
    Caps, Delta, Failure, Item, Listed, Part, Reader, Request, Signed, Stop, Thinking, Wire,
    args_of, efforts_by_prefix,
};

const MAKER: &str = "anthropic";
const LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
/// The levels of models listed before the listing said what each can do.
const EFFORTS: &[(&str, &[&str])] = &[
    ("claude-opus-4-5", &["low", "medium", "high"]),
    ("claude-opus-4-6", &["low", "medium", "high", "max"]),
    ("claude-sonnet-4-6", &["low", "medium", "high", "max"]),
    ("claude-opus-4-7", &LEVELS),
    ("claude-opus-4-8", &LEVELS),
];

pub struct Anthropic;

impl Wire for Anthropic {
    fn maker(&self) -> &'static str {
        MAKER
    }

    fn home(&self) -> Option<&'static str> {
        Some("https://api.anthropic.com")
    }

    fn listing(&self) -> &'static str {
        "/v1/models?limit=1000"
    }

    fn auth(&self, key: &str) -> Vec<(&'static str, String)> {
        vec![
            ("x-api-key", key.to_string()),
            ("anthropic-version", "2023-06-01".into()),
        ]
    }

    fn models(&self, listing: &Value) -> Vec<Listed> {
        let models = listing.get("data").and_then(Value::as_array);
        models
            .into_iter()
            .flatten()
            .filter_map(|model| {
                let id = model.get("id")?.as_str()?.to_string();
                let name = model
                    .get("display_name")
                    .and_then(Value::as_str)
                    .unwrap_or(&id)
                    .to_string();
                let context = model
                    .get("max_input_tokens")
                    .and_then(Value::as_u64)
                    .map_or(200_000, |context| context as u32);
                let caps = match model.get("capabilities") {
                    Some(listed) => {
                        let has = |pointer: &str| {
                            listed.pointer(pointer).and_then(Value::as_bool) == Some(true)
                        };
                        Caps {
                            images_in: has("/image_input/supported"),
                            structured: has("/structured_outputs/supported"),
                            thinking: match has("/thinking/types/adaptive/supported") {
                                true => Thinking::Summaries,
                                false => Thinking::None,
                            },
                            efforts: LEVELS
                                .iter()
                                .filter(|level| has(&format!("/effort/{level}/supported")))
                                .map(|level| level.to_string())
                                .collect(),
                            ..Caps::plain(context)
                        }
                    }
                    None => {
                        let efforts = efforts_by_prefix(EFFORTS, &id);
                        Caps {
                            images_in: true,
                            thinking: match efforts.is_empty() {
                                true => Thinking::None,
                                false => Thinking::Summaries,
                            },
                            efforts,
                            ..Caps::plain(context)
                        }
                    }
                };
                Some(Listed { id, name, caps })
            })
            .collect()
    }

    fn call(&self, model: &str, request: &Request) -> (String, Value) {
        let mut body = json!({
            "model": model,
            "max_tokens": request.max_out,
            "stream": true,
            "system": [{
                "type": "text",
                "text": request.instructions,
                "cache_control": { "type": "ephemeral" },
            }],
            "messages": messages(&request.items),
        });
        if !request.tools.is_empty() {
            let mut tools: Vec<Value> = request
                .tools
                .iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "description": tool.description,
                        "input_schema": tool.schema,
                    })
                })
                .collect();
            if let Some(last) = tools.last_mut() {
                last["cache_control"] = json!({ "type": "ephemeral" });
            }
            body["tools"] = json!(tools);
        }
        if request.thinking {
            body["thinking"] = json!({ "type": "adaptive", "display": "summarized" });
        }
        if let Some(effort) = &request.effort {
            body["output_config"]["effort"] = json!(effort);
        }
        if let Some(schema) = &request.schema {
            body["output_config"]["format"] = json!({ "type": "json_schema", "schema": schema });
        }
        ("/v1/messages".into(), body)
    }

    fn reader(&self) -> Box<dyn Reader> {
        Box::new(Events::default())
    }
}

/// The conversation as messages, each run of one side's items one message, the last marked for
/// the cache. Another maker's reasoning is left out.
fn messages(items: &[Item]) -> Vec<Value> {
    let mut messages: Vec<Value> = Vec::new();
    for item in items {
        let (role, blocks) = match item {
            Item::User(parts) => ("user", parts.iter().map(block).collect()),
            Item::Result { id, parts, error } => (
                "user",
                vec![json!({
                    "type": "tool_result",
                    "tool_use_id": id,
                    "content": parts.iter().map(block).collect::<Vec<_>>(),
                    "is_error": error,
                })],
            ),
            Item::Said(text) => ("assistant", vec![json!({ "type": "text", "text": text })]),
            Item::Thought(signed) if signed.maker == MAKER => (
                "assistant",
                vec![json!({
                    "type": "thinking",
                    "thinking": signed.text,
                    "signature": signed.signature,
                })],
            ),
            Item::Thought(_) => continue,
            Item::Call { id, name, args } => (
                "assistant",
                vec![json!({ "type": "tool_use", "id": id, "name": name, "input": args })],
            ),
        };
        match messages.last_mut() {
            Some(last) if last["role"] == role => {
                if let Some(content) = last["content"].as_array_mut() {
                    content.extend(blocks);
                }
            }
            _ => messages.push(json!({ "role": role, "content": blocks })),
        }
    }
    if let Some(block) = messages
        .last_mut()
        .and_then(|last| last["content"].as_array_mut())
        .and_then(|content| content.last_mut())
    {
        block["cache_control"] = json!({ "type": "ephemeral" });
    }
    messages
}

fn block(part: &Part) -> Value {
    match part {
        Part::Text(text) => json!({ "type": "text", "text": text }),
        Part::Image { media, base64 } => json!({
            "type": "image",
            "source": { "type": "base64", "media_type": media, "data": base64 },
        }),
    }
}

/// The block being streamed.
enum Block {
    Other,
    Thinking {
        text: String,
        signature: String,
    },
    Call {
        id: String,
        name: String,
        json: String,
    },
}

#[derive(Default)]
struct Events {
    block: Option<Block>,
    input: u32,
    stop: Option<Stop>,
}

impl Reader for Events {
    fn event(&mut self, _: Option<&str>, data: &str) -> Vec<Delta> {
        let Ok(event) = serde_json::from_str::<Value>(data) else {
            return Vec::new();
        };
        let text = |pointer: &str| {
            event
                .pointer(pointer)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let count =
            |pointer: &str| event.pointer(pointer).and_then(Value::as_u64).unwrap_or(0) as u32;
        match event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "message_start" => {
                self.input = count("/message/usage/input_tokens")
                    + count("/message/usage/cache_read_input_tokens")
                    + count("/message/usage/cache_creation_input_tokens");
                Vec::new()
            }
            "content_block_start" => {
                self.block = Some(match text("/content_block/type").as_str() {
                    "thinking" => Block::Thinking {
                        text: String::new(),
                        signature: String::new(),
                    },
                    "tool_use" => Block::Call {
                        id: text("/content_block/id"),
                        name: text("/content_block/name"),
                        json: String::new(),
                    },
                    _ => Block::Other,
                });
                Vec::new()
            }
            "content_block_delta" => match (text("/delta/type").as_str(), &mut self.block) {
                ("text_delta", _) => vec![Delta::Text(text("/delta/text"))],
                ("thinking_delta", Some(Block::Thinking { text: thought, .. })) => {
                    let piece = text("/delta/thinking");
                    thought.push_str(&piece);
                    vec![Delta::Thinking(piece)]
                }
                ("signature_delta", Some(Block::Thinking { signature, .. })) => {
                    signature.push_str(&text("/delta/signature"));
                    Vec::new()
                }
                ("input_json_delta", Some(Block::Call { json, .. })) => {
                    json.push_str(&text("/delta/partial_json"));
                    Vec::new()
                }
                _ => Vec::new(),
            },
            "content_block_stop" => match self.block.take() {
                Some(Block::Thinking { text, signature }) if !signature.is_empty() => {
                    vec![Delta::Signed(Signed {
                        maker: MAKER.into(),
                        text,
                        signature,
                    })]
                }
                Some(Block::Call { id, name, json }) => vec![Delta::Call {
                    id,
                    name,
                    args: args_of(&json),
                }],
                _ => Vec::new(),
            },
            "message_delta" => {
                if let Some(reason) = event.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    self.stop = Some(match reason {
                        "tool_use" => Stop::Calls,
                        "max_tokens" => Stop::Length,
                        _ => Stop::Done,
                    });
                }
                vec![Delta::Usage {
                    input: self.input,
                    output: count("/usage/output_tokens"),
                }]
            }
            "message_stop" => vec![Delta::Ended(self.stop.take().unwrap_or(Stop::Done))],
            "error" => vec![Delta::Failed(match text("/error/type").as_str() {
                "overloaded_error" | "api_error" => Failure::Busy,
                "rate_limit_error" => Failure::Limit(None),
                "authentication_error" | "permission_error" => Failure::Refused,
                "billing_error" => Failure::Credit,
                "not_found_error" => Failure::Withdrawn,
                _ => Failure::Other(text("/error/message")),
            })],
            _ => Vec::new(),
        }
    }
}
