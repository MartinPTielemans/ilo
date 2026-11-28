//! Server-sent events, a line at a time, as every maker streams a turn.

/// The event being read.
#[derive(Default)]
pub struct Sse {
    event: Option<String>,
    data: Option<String>,
}

impl Sse {
    /// One line, with or without its end. A blank line ends an event, which comes back as its
    /// name, if it has one, and its data.
    pub fn line(&mut self, line: &str) -> Option<(Option<String>, String)> {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            return self.finish();
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.event = Some(value.to_string()),
            "data" => match &mut self.data {
                Some(data) => {
                    data.push('\n');
                    data.push_str(value);
                }
                None => self.data = Some(value.to_string()),
            },
            _ => {}
        }
        None
    }

    /// The event the stream closed on, which some servers don't end with a blank line.
    pub fn finish(&mut self) -> Option<(Option<String>, String)> {
        let event = self.event.take();
        self.data.take().map(|data| (event, data))
    }
}
