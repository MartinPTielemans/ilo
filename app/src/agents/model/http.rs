//! The network, for every maker alike: a listing, and a turn's stream on a thread of its own. ureq
//! is blocking, so a turn is a thread that sleeps in the kernel between the stream's reads, and
//! the browser has no async runtime to start for it. Only the endpoint the account names is
//! called.

use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::channel::mpsc::UnboundedSender;
use serde_json::Value;

use super::sse::Sse;
use super::{Caps, Delta, Failure, Listed, Model, Request, Wire};

/// An account's key. Neither Debug nor any log, crash report or sync prints it.
#[derive(Clone)]
pub struct Key(String);

impl Key {
    pub fn new(key: String) -> Self {
        Self(key)
    }

    /// Its last four characters, which Settings names it by (AP2).
    pub fn last4(&self) -> String {
        let chars: Vec<char> = self.0.chars().collect();
        chars[chars.len().saturating_sub(4)..].iter().collect()
    }
}

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Key(..)")
    }
}

/// Where an account's models are.
#[derive(Clone, Debug)]
pub struct Endpoint {
    /// The maker's address, or the user's server's, with no slash at its end.
    pub base: String,
    /// None for a server on this computer that takes none.
    pub key: Option<Key>,
    /// How long a connection and a response's start may take.
    pub patience: Duration,
}

/// The pauses before each quiet retry of a turn its maker was busy for or that found no answer,
/// before any of it streamed. A test's stand-in answers at once, so it waits none.
const PAUSES: [Duration; 2] = if cfg!(test) {
    [Duration::ZERO, Duration::ZERO]
} else {
    [Duration::from_secs(1), Duration::from_secs(4)]
};

/// The longest a stream may say nothing, past any model's pause to think, before its turn is
/// taken as unanswered and its thread ends.
const SILENCE: Duration = Duration::from_secs(300);

fn agent(patience: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(patience))
        .timeout_recv_response(Some(patience))
        .timeout_per_read(Some(SILENCE.max(patience)))
        // A redirect would take the key's header to an address the account didn't name.
        .max_redirects(0)
        .max_redirects_will_error(false)
        .build()
        .into()
}

fn auth(wire: &dyn Wire, endpoint: &Endpoint) -> Vec<(&'static str, String)> {
    endpoint
        .key
        .as_ref()
        .map(|key| wire.auth(&key.0))
        .unwrap_or_default()
}

/// The models the endpoint lists.
pub fn list(wire: &dyn Wire, endpoint: &Endpoint) -> Result<Vec<Listed>, Failure> {
    let mut request = agent(endpoint.patience).get(format!("{}{}", endpoint.base, wire.listing()));
    for (name, value) in auth(wire, endpoint) {
        request = request.header(name, value);
    }
    let response = request.call().map_err(|_| Failure::Unreachable)?;
    let status = response.status().as_u16();
    let after = retry_after(&response);
    let body = response
        .into_body()
        .read_to_string()
        .map_err(|_| Failure::Unreachable)?;
    if !(200..300).contains(&status) {
        return Err(failure(status, after, &body, endpoint));
    }
    let listing: Value =
        serde_json::from_str(&body).map_err(|_| Failure::Other("not a listing".into()))?;
    Ok(wire.models(&listing))
}

/// Streams one turn of `model` to `out`, until it ends, fails or `cancel` is set. What fails
/// before any of it streamed, for being busy or unanswered, is tried twice more, quietly.
pub fn stream(
    wire: &dyn Wire,
    endpoint: &Endpoint,
    model: &str,
    request: &Request,
    cancel: &AtomicBool,
    out: &mut dyn FnMut(Delta),
) {
    let (path, body) = wire.call(model, request);
    let body = body.to_string();
    let mut pauses = PAUSES.iter();
    loop {
        let failure = match attempt(wire, endpoint, &path, &body, cancel, out) {
            Ok(()) => return,
            Err(failure) => failure,
        };
        let quiet = matches!(failure, Failure::Busy | Failure::Unreachable);
        match pauses.next() {
            Some(pause) if quiet => std::thread::sleep(*pause),
            _ => return out(Delta::Failed(failure)),
        }
        if cancel.load(Ordering::Relaxed) {
            return;
        }
    }
}

/// One try of a turn: Err only while none of it has streamed.
fn attempt(
    wire: &dyn Wire,
    endpoint: &Endpoint,
    path: &str,
    body: &str,
    cancel: &AtomicBool,
    out: &mut dyn FnMut(Delta),
) -> Result<(), Failure> {
    let mut request = agent(endpoint.patience)
        .post(format!("{}{path}", endpoint.base))
        .header("content-type", "application/json")
        .header("accept", "text/event-stream");
    for (name, value) in auth(wire, endpoint) {
        request = request.header(name, value);
    }
    let response = request.send(body).map_err(|_| Failure::Unreachable)?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let after = retry_after(&response);
        let body = response.into_body().read_to_string().unwrap_or_default();
        return Err(failure(status, after, &body, endpoint));
    }
    let mut lines = BufReader::new(response.into_body().into_reader());
    let mut reader = wire.reader();
    let mut sse = Sse::default();
    let (mut streamed, mut ended) = (false, false);
    let mut line = String::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        line.clear();
        let event = match lines.read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => match sse.line(&line) {
                Some(event) => Some(event),
                None => continue,
            },
            Err(_) if streamed => return Ok(out(Delta::Failed(Failure::Unreachable))),
            Err(_) => return Err(Failure::Unreachable),
        };
        let closed = event.is_none();
        let deltas = match event.or_else(|| sse.finish()) {
            Some((name, data)) => {
                let mut deltas = reader.event(name.as_deref(), &data);
                if closed {
                    deltas.extend(reader.end());
                }
                deltas
            }
            None => reader.end(),
        };
        for delta in deltas {
            if let Delta::Failed(failure @ (Failure::Busy | Failure::Unreachable)) = &delta
                && !streamed
            {
                return Err(failure.clone());
            }
            ended |= matches!(delta, Delta::Ended(_) | Delta::Failed(_));
            streamed = true;
            out(delta);
            if ended {
                return Ok(());
            }
        }
        if closed {
            // The stream broke off before its end.
            return match streamed {
                true => Ok(out(Delta::Failed(Failure::Unreachable))),
                false => Err(Failure::Unreachable),
            };
        }
    }
}

fn retry_after(response: &ureq::http::Response<ureq::Body>) -> Option<u64> {
    response
        .headers()
        .get("retry-after")?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// A status's failure, with the maker's message from its body, never the key it may repeat.
fn failure(status: u16, after: Option<u64>, body: &str, endpoint: &Endpoint) -> Failure {
    let body: Value = serde_json::from_str(body).unwrap_or_default();
    let message = ["/error/message", "/0/error/message", "/message"]
        .iter()
        .find_map(|pointer| body.pointer(pointer).and_then(Value::as_str))
        .unwrap_or_default();
    let message = match &endpoint.key {
        Some(key) if !key.0.is_empty() => message.replace(&key.0, "…"),
        _ => message.to_string(),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    Failure::of_status(status, after, now, message)
}

/// A listed model at an endpoint, each turn streamed on a thread of its own.
pub struct Remote {
    wire: &'static dyn Wire,
    endpoint: Arc<Endpoint>,
    model: String,
    caps: Caps,
    /// The turn under way and what stops it.
    going: RefCell<Option<(u32, Arc<AtomicBool>)>>,
}

impl Remote {
    pub fn new(wire: &'static dyn Wire, endpoint: Arc<Endpoint>, listed: Listed) -> Self {
        Self {
            wire,
            endpoint,
            model: listed.id,
            caps: listed.caps,
            going: RefCell::new(None),
        }
    }
}

impl Model for Remote {
    fn caps(&self) -> &Caps {
        &self.caps
    }

    fn turn(&self, turn: u32, request: Request, out: UnboundedSender<(u32, Delta)>) {
        let cancel = Arc::new(AtomicBool::new(false));
        // A conversation has one turn under way, so one before it is over.
        if let Some((_, before)) = self.going.replace(Some((turn, cancel.clone()))) {
            before.store(true, Ordering::Relaxed);
        }
        let (wire, endpoint, model) = (self.wire, self.endpoint.clone(), self.model.clone());
        let failed = out.clone();
        let started = std::thread::Builder::new()
            .name("model turn".into())
            .spawn(move || {
                stream(wire, &endpoint, &model, &request, &cancel, &mut |delta| {
                    let _ = out.unbounded_send((turn, delta));
                });
            });
        if started.is_err() {
            let _ = failed.unbounded_send((turn, Delta::Failed(Failure::Other(String::new()))));
        }
    }

    fn cancel(&self, turn: u32) {
        if let Some((going, cancel)) = &*self.going.borrow()
            && *going == turn
        {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}
