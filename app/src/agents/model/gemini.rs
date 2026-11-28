//! Google's Gemini API with the user's key. Its listing gives each model's context and whether it
//! thinks; the levels it takes are this file's table. Its calls come without ids, so each is given
//! one, and the signature its reasoning leaves on a call goes back on that call.

use serde_json::{Value, json};

use super::{
    Caps, Delta, Failure, Item, Listed, Part, Reader, Request, Signed, Stop, Thinking, Wire,
    call_name, efforts_by_prefix, fresh_id, text_of,
};

const MAKER: &str = "gemini";
const EFFORTS: &[(&str, &[&str])] = &[("gemini-3", &["low", "high"])];

pub struct Gemini;

impl Wire for Gemini {
    fn maker(&self) -> &'static str {
        MAKER
    }

    fn home(&self) -> Option<&'static str> {
        Some("https://generativelanguage.googleapis.com")
    }

    fn listing(&self) -> &'static str {
        "/v1beta/models?pageSize=1000"
    }

    fn auth(&self, key: &str) -> Vec<(&'static str, String)> {
        vec![("x-goog-api-key", key.to_string())]
    }

    fn models(&self, listing: &Value) -> Vec<Listed> {
        let models = listing.get("models").and_then(Value::as_array);
        models
            .into_iter()
            .flatten()
            .filter(|model| {
                let methods = model
                    .get("supportedGenerationMethods")
                    .and_then(Value::as_array);
                methods
                    .into_iter()
                    .flatten()
                    .any(|method| method.as_str() == Some("generateContent"))
            })
            .filter_map(|model| {
                let id = model.get("name")?.as_str()?;
                let id = id.strip_prefix("models/").unwrap_or(id).to_string();
                if id.contains("embedding") {
                    return None;
                }
                let name = model
                    .get("displayName")
                    .and_then(Value::as_str)
                    .unwrap_or(&id)
                    .to_string();
                let context = model
                    .get("inputTokenLimit")
                    .and_then(Value::as_u64)
                    .map_or(32_768, |context| context as u32);
                let thinks = model.get("thinking").and_then(Value::as_bool) == Some(true);
                let caps = Caps {
                    images_in: true,
                    structured: true,
                    thinking: match thinks {
                        true => Thinking::Summaries,
                        false => Thinking::None,
                    },
                    efforts: match thinks {
                        true => efforts_by_prefix(EFFORTS, &id),
                        false => Vec::new(),
                    },
                    ..Caps::plain(context)
                };
                Some(Listed { id, name, caps })
            })
            .collect()
    }

    fn call(&self, model: &str, request: &Request) -> (String, Value) {
        let mut config = json!({ "maxOutputTokens": request.max_out });
        if request.thinking {
            config["thinkingConfig"] = json!({ "includeThoughts": true });
            if let Some(effort) = &request.effort {
                config["thinkingConfig"]["thinkingLevel"] = json!(effort);
            }
        }
        if let Some(schema) = &request.schema {
            config["responseMimeType"] = json!("application/json");
            config["responseJsonSchema"] = schema.clone();
        }
        let mut body = json!({
            "systemInstruction": { "parts": [{ "text": request.instructions }] },
            "contents": contents(&request.items),
            "generationConfig": config,
        });
        if !request.tools.is_empty() {
            let declarations: Vec<Value> = request
                .tools
                .iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "description": tool.description,
                        "parametersJsonSchema": tool.schema,
                    })
                })
                .collect();
            body["tools"] = json!([{ "functionDeclarations": declarations }]);
        }
        (
            format!("/v1beta/models/{model}:streamGenerateContent?alt=sse"),
            body,
        )
    }

    fn reader(&self) -> Box<dyn Reader> {
        Box::new(Chunks::default())
    }
}

/// The conversation as contents, each run of one side's items one content. A signature of
/// Gemini's goes on the call after it.
fn contents(items: &[Item]) -> Vec<Value> {
    let mut contents: Vec<Value> = Vec::new();
    let mut signature = None;
    for item in items {
        let (role, parts) = match item {
            Item::User(parts) => ("user", parts.iter().map(part).collect()),
            Item::Result { id, parts, .. } => {
                let mut out = vec![json!({ "functionResponse": {
                    "name": call_name(items, id),
                    "response": { "content": text_of(parts) },
                } })];
                out.extend(
                    parts
                        .iter()
                        .filter(|it| matches!(it, Part::Image { .. }))
                        .map(part),
                );
                ("user", out)
            }
            Item::Said(text) => ("model", vec![json!({ "text": text })]),
            Item::Thought(signed) => {
                if signed.maker == MAKER {
                    signature = Some(signed.signature.clone());
                }
                continue;
            }
            Item::Call { name, args, .. } => {
                let mut call = json!({ "functionCall": { "name": name, "args": args } });
                if let Some(signature) = signature.take() {
                    call["thoughtSignature"] = json!(signature);
                }
                ("model", vec![call])
            }
        };
        match contents.last_mut() {
            Some(last) if last["role"] == role => {
                if let Some(all) = last["parts"].as_array_mut() {
                    all.extend(parts);
                }
            }
            _ => contents.push(json!({ "role": role, "parts": parts })),
        }
    }
    contents
}

fn part(part: &Part) -> Value {
    match part {
        Part::Text(text) => json!({ "text": text }),
        Part::Image { media, base64 } => {
            json!({ "inlineData": { "mimeType": media, "data": base64 } })
        }
    }
}

/// A turn's chunks: its end is the chunk with a finish reason, said once the stream closes, as
/// the last usage comes with it.
#[derive(Default)]
struct Chunks {
    thought: String,
    calls: bool,
    stop: Option<Stop>,
    usage: Option<Delta>,
}

impl Reader for Chunks {
    fn event(&mut self, _: Option<&str>, data: &str) -> Vec<Delta> {
        let Ok(chunk) = serde_json::from_str::<Value>(data) else {
            return Vec::new();
        };
        if let Some(error) = chunk.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default();
            return vec![Delta::Failed(
                match error.get("code").and_then(Value::as_u64) {
                    Some(429) => Failure::Limit(None),
                    Some(500 | 503) => Failure::Busy,
                    _ => Failure::Other(message.into()),
                },
            )];
        }
        let mut deltas = Vec::new();
        let parts = chunk
            .pointer("/candidates/0/content/parts")
            .and_then(Value::as_array);
        for part in parts.into_iter().flatten() {
            let text = part.get("text").and_then(Value::as_str);
            let thought = part.get("thought").and_then(Value::as_bool) == Some(true);
            match (text, thought) {
                (Some(text), true) => {
                    self.thought.push_str(text);
                    deltas.push(Delta::Thinking(text.into()));
                }
                (Some(text), false) if !text.is_empty() => deltas.push(Delta::Text(text.into())),
                _ => {}
            }
            if let Some(call) = part.get("functionCall") {
                if let Some(signature) = part.get("thoughtSignature").and_then(Value::as_str) {
                    deltas.push(Delta::Signed(Signed {
                        maker: MAKER.into(),
                        text: std::mem::take(&mut self.thought),
                        signature: signature.into(),
                    }));
                }
                self.calls = true;
                deltas.push(Delta::Call {
                    id: call
                        .get("id")
                        .and_then(Value::as_str)
                        .map_or_else(fresh_id, str::to_string),
                    name: call
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .into(),
                    args: call.get("args").cloned().unwrap_or_else(|| json!({})),
                });
            }
        }
        if let Some(usage) = chunk.get("usageMetadata") {
            let count = |key| usage.get(key).and_then(Value::as_u64).unwrap_or(0) as u32;
            self.usage = Some(Delta::Usage {
                input: count("promptTokenCount"),
                output: count("candidatesTokenCount") + count("thoughtsTokenCount"),
            });
        }
        if let Some(reason) = chunk
            .pointer("/candidates/0/finishReason")
            .and_then(Value::as_str)
        {
            self.stop = Some(match reason {
                "MAX_TOKENS" => Stop::Length,
                _ => Stop::Done,
            });
        }
        deltas
    }

    fn end(&mut self) -> Vec<Delta> {
        let Some(stop) = self.stop.take() else {
            return Vec::new();
        };
        let stop = match (self.calls, stop) {
            (true, Stop::Done) => Stop::Calls,
            (_, stop) => stop,
        };
        self.usage
            .take()
            .into_iter()
            .chain([Delta::Ended(stop)])
            .collect()
    }
}
