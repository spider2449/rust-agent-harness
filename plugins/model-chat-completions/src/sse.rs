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
                if let Some(frame) = self.end_line()? {
                    frames.push(frame);
                }
            } else {
                self.line.push(byte);
            }
        }
        Ok(frames)
    }

    fn end_line(&mut self) -> Result<Option<Frame>, E> {
        let bytes = std::mem::take(&mut self.line);
        let line = std::str::from_utf8(&bytes).map_err(|_| E::Sse)?;
        if line.is_empty() {
            self.size = 0;
            if self.data.is_empty() {
                return Ok(None);
            }
            let data = std::mem::take(&mut self.data);
            let data = data.trim_end_matches('\n');
            if data.trim() == "[DONE]" {
                return Ok(Some(Frame::Done));
            }
            let chunk = serde_json::from_str(data).map_err(|_| E::ChunkJson)?;
            return Ok(Some(Frame::Chunk(chunk)));
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
