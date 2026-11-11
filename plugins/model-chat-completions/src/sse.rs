use serde_json::Value;

use crate::error::E;

pub(crate) const MAX_FRAME: usize = 256 * 1024;

/// One SSE frame of a Chat Completions stream.
#[derive(Debug, PartialEq)]
pub(crate) enum Frame {
    Chunk(Value),
    /// `data: [DONE]`, the end of the stream.
    Done,
}

/// Byte-level incremental SSE framing: decoding only complete lines preserves
/// split UTF-8 codepoints. CR, LF and CRLF are accepted, including split CRLF.
/// Chat Completions frames carry only `data`; event names are ignored.
#[derive(Default)]
pub(crate) struct Sse {
    line: Vec<u8>,
    data: String,
    size: usize,
    cr: bool,
}

impl Sse {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Frame>, E> {
        let mut frames = Vec::new();
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
                let frame = self.end_line();
                self.line.clear();
                if let Some(frame) = frame? {
                    frames.push(frame);
                }
            } else {
                self.line.push(byte);
            }
        }
        Ok(frames)
    }

    fn end_line(&mut self) -> Result<Option<Frame>, E> {
        let line = std::str::from_utf8(&self.line).map_err(|_| E::Sse)?;
        if line.is_empty() {
            self.size = 0;
            if self.data.is_empty() {
                return Ok(None);
            }
            let data = self.data.trim_end_matches('\n');
            let frame = if data.trim() == "[DONE]" {
                Ok(Frame::Done)
            } else {
                serde_json::from_str(data)
                    .map(Frame::Chunk)
                    .map_err(|_| E::ChunkJson)
            };
            self.data.clear();
            return frame.map(Some);
        }
        if line.starts_with(':') {
            return Ok(None);
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        if field == "data" {
            self.data.push_str(value.strip_prefix(' ').unwrap_or(value));
            self.data.push('\n');
        }
        Ok(None)
    }

    pub fn finish(&self) -> Result<(), E> {
        if !self.line.is_empty() || !self.data.is_empty() {
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
    fn repeated_events_and_done_reuse_buffers() {
        let mut sse = Sse::default();
        let body = b"data: {\"text\":\"hello\"}\n\n";
        let expected = vec![Frame::Chunk(json!({"text":"hello"}))];
        assert_eq!(sse.push(body).unwrap(), expected);
        let capacities = (sse.line.capacity(), sse.data.capacity());
        assert!(capacities.0 >= body.len() - 2);
        assert!(capacities.1 > 0);
        for _ in 0..100 {
            assert_eq!(sse.push(body).unwrap(), expected);
            assert!(sse.line.is_empty());
            assert!(sse.data.is_empty());
            assert_eq!((sse.line.capacity(), sse.data.capacity()), capacities);
        }
        assert_eq!(sse.push(b"data: [DONE]\n\n").unwrap(), [Frame::Done]);
        assert_eq!((sse.line.capacity(), sse.data.capacity()), capacities);
        sse.finish().unwrap();
    }

    #[test]
    fn all_chunk_boundaries_and_line_endings_frame_identically() {
        let body = "data: {\"t\":\"\u{00e9}\"}\r\n\r\n: keep-alive\nevent: x\ndata: {\"n\":1}\n\ndata: [DONE]\n\n";
        for split in 0..body.len() {
            let mut sse = Sse::default();
            let mut frames = sse.push(&body.as_bytes()[..split]).unwrap();
            frames.extend(sse.push(&body.as_bytes()[split..]).unwrap());
            sse.finish().unwrap();
            assert_eq!(
                frames,
                [
                    Frame::Chunk(json!({"t":"\u{00e9}"})),
                    Frame::Chunk(json!({"n":1})),
                    Frame::Done
                ]
            );
        }
    }

    #[test]
    fn bad_json_and_oversized_or_torn_frames_fail() {
        assert_eq!(Sse::default().push(b"data: {nope\n\n"), Err(E::ChunkJson));
        assert_eq!(Sse::default().push(&vec![b'a'; MAX_FRAME + 1]), Err(E::Sse));
        let mut torn = Sse::default();
        torn.push(b"data: {}").unwrap();
        assert_eq!(torn.finish(), Err(E::Sse));
    }
}
