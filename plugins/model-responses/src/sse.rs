use serde_json::Value;

use crate::error::E;

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
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Value>, E> {
        let mut events = Vec::new();
        for &byte in bytes {
            if self.cr && byte == b'\n' {
                self.cr = false;
                continue;
            }
            self.cr = false;
            self.size += 1;
            if self.size > MAX_FRAME {
                return Err(E::Sse);
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

    fn end_line(&mut self) -> Result<Option<Value>, E> {
        let bytes = std::mem::take(&mut self.line);
        let line = std::str::from_utf8(&bytes).map_err(|_| E::Sse)?;
        if line.is_empty() {
            self.size = 0;
            if self.data.is_empty() {
                self.event = None;
                return Ok(None);
            }
            let event: Value =
                serde_json::from_str(self.data.trim_end_matches('\n')).map_err(|_| E::EventJson)?;
            self.data.clear();
            if let Some(name) = self.event.take()
                && event.get("type").and_then(Value::as_str) != Some(name.as_str())
            {
                return Err(E::Protocol);
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
            // Neither reconnection nor provider ID projection; extensions are inert.
            _ => {}
        }
        Ok(None)
    }

    pub fn finish(&self) -> Result<(), E> {
        if !self.line.is_empty() || !self.data.is_empty() || self.event.is_some() {
            Err(E::Sse)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn all_chunk_boundaries_and_line_endings_frame_identically() {
        let body = "event: a\r\ndata: {\"type\":\"a\",\"t\":\"\u{00e9}\"}\r\n\r\n: comment\ndata: {\"type\":\"b\"}\n\n";
        for split in 0..body.len() {
            let mut sse = Sse::default();
            let mut events = sse.push(&body.as_bytes()[..split]).unwrap();
            events.extend(sse.push(&body.as_bytes()[split..]).unwrap());
            sse.finish().unwrap();
            assert_eq!(
                events,
                [json!({"type":"a","t":"\u{00e9}"}), json!({"type":"b"})]
            );
        }
    }

    #[test]
    fn mismatched_event_names_bad_json_and_oversized_frames_fail() {
        assert_eq!(
            Sse::default().push(b"event: x\ndata: {\"type\":\"y\"}\n\n"),
            Err(E::Protocol)
        );
        assert_eq!(Sse::default().push(b"data: {nope\n\n"), Err(E::EventJson));
        assert_eq!(Sse::default().push(&vec![b'a'; MAX_FRAME + 1]), Err(E::Sse));
        let mut torn = Sse::default();
        torn.push(b"data: {}").unwrap();
        assert_eq!(torn.finish(), Err(E::Sse));
    }
}
