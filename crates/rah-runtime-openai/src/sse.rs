use crate::OpenAiAdapterError;
use serde_json::Value;

pub(crate) const MAX_FRAME: usize = 256 * 1024;
/// Byte-level incremental SSE framing: decoding only complete lines preserves
/// split UTF-8 codepoints. CR, LF and CRLF are accepted, including split CRLF.
#[derive(Default)]
pub(crate) struct Sse {
    line: Vec<u8>,
    data: String,
    event: Option<String>,
    size: usize,
    cr: bool,
}
impl Sse {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Value>, OpenAiAdapterError> {
        let mut events = Vec::new();
        for &byte in bytes {
            if self.cr && byte == b'\n' {
                self.cr = false;
                continue;
            }
            self.cr = false;
            self.size += 1;
            if self.size > MAX_FRAME {
                return Err(OpenAiAdapterError::Sse);
            }
            if byte == b'\r' || byte == b'\n' {
                self.cr = byte == b'\r';
                if let Some(event) = self.end_line()? {
                    events.push(event);
                }
            } else {
                self.line.push(byte);
            }
        }
        Ok(events)
    }
    fn end_line(&mut self) -> Result<Option<Value>, OpenAiAdapterError> {
        let bytes = std::mem::take(&mut self.line);
        let line = std::str::from_utf8(&bytes).map_err(|_| OpenAiAdapterError::Sse)?;
        if line.is_empty() {
            self.size = 0;
            if self.data.is_empty() {
                self.event = None;
                return Ok(None);
            }
            let event: Value = serde_json::from_str(self.data.trim_end_matches('\n'))
                .map_err(|_| OpenAiAdapterError::EventJson)?;
            self.data.clear();
            if let Some(name) = self.event.take()
                && event.get("type").and_then(Value::as_str) != Some(name.as_str())
            {
                return Err(OpenAiAdapterError::Protocol);
            }
            return Ok(Some(event));
        }
        if line.starts_with(':') {
            return Ok(None);
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "data" => {
                self.data.push_str(value);
                self.data.push('\n');
            }
            "event" => self.event = Some(value.to_owned()),
            "id" | "retry" => {} // Neither reconnection nor provider ID projection.
            _ => {}              // SSE extension fields are inert.
        }
        Ok(None)
    }
    pub fn finish(&self) -> Result<(), OpenAiAdapterError> {
        if !self.line.is_empty() || !self.data.is_empty() || self.event.is_some() {
            Err(OpenAiAdapterError::Sse)
        } else {
            Ok(())
        }
    }
}
