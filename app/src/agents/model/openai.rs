//! OpenAI's Responses API with the user's key, stored nowhere on OpenAI's side: each request
//! carries the conversation, and the reasoning goes back encrypted, as OpenAI hands it over. Its
//! listing names models alone, so what each can do is this file's table.

use serde_json::{Value, json};

use super::{
    Caps, Delta, Failure, Item, Listed, Part, Reader, Request, Signed, Stop, Thinking, Wire,
    args_of, efforts_by_prefix, text_of,
};

const MAKER: &str = "openai";
/// The models that chat, by id prefix; the rest of the listing (pictures, speech, embeddings)
/// doesn't.
const CHATS: &[&str] = &["gpt-4o", "gpt-4.1", "gpt-5", "o1", "o3", "o4"];
const NOT_CHATS: &[&str] = &["audio", "realtime", "tts", "transcribe", "image", "search"];
const EFFORTS: &[(&str, &[&str])] = &[
    ("o1", &["low", "medium", "high"]),
    ("o3", &["low", "medium", "high"]),
    ("o4", &["low", "medium", "high"]),
    ("gpt-5", &["minimal", "low", "medium", "high"]),
];
const CONTEXT: &[(&str, u32)] = &[
    ("gpt-4o", 128_000),
    ("gpt-4.1", 1_047_576),
    ("gpt-5", 400_000),
    ("o1", 200_000),
    ("o3", 200_000),
    ("o4", 200_000),
];

pub struct OpenAi;

impl Wire for OpenAi {
    fn maker(&self) -> &'static str {
        MAKER
    }

    fn home(&self) -> Option<&'static str> {
        Some("https://api.openai.com")
    }

    fn listing(&self) -> &'static str {
        "/v1/models"
    }

    fn auth(&self, key: &str) -> Vec<(&'static str, String)> {
        vec![("authorization", format!("Bearer {key}"))]
    }

    fn models(&self, listing: &Value) -> Vec<Listed> {
        let models = listing.get("data").and_then(Value::as_array);
        models
            .into_iter()
            .flatten()
            .filter_map(|model| model.get("id")?.as_str())
            .filter(|id| CHATS.iter().any(|chat| id.starts_with(chat)))
            .filter(|id| !NOT_CHATS.iter().any(|not| id.contains(not)))
            .map(|id| {
                let efforts = efforts_by_prefix(EFFORTS, id);
                let context = CONTEXT
                    .iter()
                    .filter(|(prefix, _)| id.starts_with(prefix))
                    .max_by_key(|(prefix, _)| prefix.len())
                    .map_or(128_000, |(_, context)| *context);
                Listed {
                    id: id.to_string(),
                    name: id.to_string(),
                    caps: Caps {
                        images_in: true,
                        structured: true,
                        thinking: match efforts.is_empty() {
                            true => Thinking::None,
                            false => Thinking::Summaries,
                        },
                        efforts,
                        ..Caps::plain(context)
                    },
                }
            })
            .collect()
    }

    fn call(&self, model: &str, request: &Request) -> (String, Value) {
        let mut body = json!({
            "model": model,
            "stream": true,
            "store": false,
            "instructions": request.instructions,
            "input": input(&request.items),
            "max_output_tokens": request.max_out,
        });
        if !request.tools.is_empty() {
            body["tools"] = request
                .tools
                .iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.schema,
                        "strict": false,
                    })
                })
                .collect();
        }
        if request.thinking {
            body["reasoning"] = json!({ "summary": "auto" });
            body["include"] = json!(["reasoning.encrypted_content"]);
            if let Some(effort) = &request.effort {
                body["reasoning"]["effort"] = json!(effort);
            }
        }
        if let Some(schema) = &request.schema {
            body["text"] = json!({ "format": {
                "type": "json_schema", "name": "answer", "schema": schema, "strict": true,
            } });
        }
        ("/v1/responses".into(), body)
    }

    fn reader(&self) -> Box<dyn Reader> {
        Box::new(Events::default())
    }
}

fn input(items: &[Item]) -> Vec<Value> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::User(parts) => Some(json!({
                "role": "user",
                "content": parts.iter().map(content).collect::<Vec<_>>(),
            })),
            Item::Said(text) => Some(json!({ "role": "assistant", "content": text })),
            Item::Thought(signed) if signed.maker == MAKER => {
                let kept: Value = serde_json::from_str(&signed.signature).ok()?;
                let summary: Vec<Value> = match signed.text.is_empty() {
                    true => Vec::new(),
                    false => vec![json!({ "type": "summary_text", "text": signed.text })],
                };
                Some(json!({
                    "type": "reasoning",
                    "id": kept["id"],
                    "summary": summary,
                    "encrypted_content": kept["encrypted_content"],
                }))
            }
            Item::Thought(_) => None,
            Item::Call { id, name, args } => Some(json!({
                "type": "function_call",
                "call_id": id,
                "name": name,
                "arguments": args.to_string(),
            })),
            Item::Result { id, parts, .. } => Some(json!({
                "type": "function_call_output",
                "call_id": id,
                "output": match parts.iter().all(|part| matches!(part, Part::Text(_))) {
                    true => json!(text_of(parts)),
                    false => json!(parts.iter().map(content).collect::<Vec<_>>()),
                },
            })),
        })
        .collect()
}

fn content(part: &Part) -> Value {
    match part {
        Part::Text(text) => json!({ "type": "input_text", "text": text }),
        Part::Image { media, base64 } => json!({
            "type": "input_image",
            "image_url": format!("data:{media};base64,{base64}"),
        }),
    }
}

#[derive(Default)]
struct Events {
    calls: bool,
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
        let usage = || Delta::Usage {
            input: event
                .pointer("/response/usage/input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            output: event
                .pointer("/response/usage/output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
        };
        match event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "response.output_text.delta" => vec![Delta::Text(text("/delta"))],
            "response.reasoning_summary_text.delta" => vec![Delta::Thinking(text("/delta"))],
            "response.output_item.done" => match text("/item/type").as_str() {
                "function_call" => {
                    self.calls = true;
                    vec![Delta::Call {
                        id: text("/item/call_id"),
                        name: text("/item/name"),
                        args: args_of(&text("/item/arguments")),
                    }]
                }
                "reasoning" if event.pointer("/item/encrypted_content").is_some() => {
                    let summary = event.pointer("/item/summary").and_then(Value::as_array);
                    let text: Vec<&str> = summary
                        .into_iter()
                        .flatten()
                        .filter_map(|part| part.get("text").and_then(Value::as_str))
                        .collect();
                    vec![Delta::Signed(Signed {
                        maker: MAKER.into(),
                        text: text.join("\n\n"),
                        signature: json!({
                            "id": event.pointer("/item/id"),
                            "encrypted_content": event.pointer("/item/encrypted_content"),
                        })
                        .to_string(),
                    })]
                }
                _ => Vec::new(),
            },
            "response.completed" => vec![
                usage(),
                Delta::Ended(match self.calls {
                    true => Stop::Calls,
                    false => Stop::Done,
                }),
            ],
            "response.incomplete" => vec![usage(), Delta::Ended(Stop::Length)],
            "response.failed" => vec![Delta::Failed(failure(
                &text("/response/error/code"),
                text("/response/error/message"),
            ))],
            "error" => vec![Delta::Failed(failure(&text("/code"), text("/message")))],
            _ => Vec::new(),
        }
    }
}

fn failure(code: &str, message: String) -> Failure {
    match code {
        "rate_limit_exceeded" => Failure::Limit(None),
        "server_error" | "server_is_overloaded" => Failure::Busy,
        "insufficient_quota" => Failure::Credit,
        "invalid_api_key" => Failure::Refused,
        "model_not_found" => Failure::Withdrawn,
        _ => Failure::Other(message),
    }
}
