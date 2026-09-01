//! Agent protocol and integration layer.
//!
//! Provides the structured protocol for AI agents to connect to,
//! inspect, and control Dewey applications.

pub mod driver;
pub mod mcp;
pub mod protocol;
pub mod rpc;
pub mod session;
#[cfg(feature = "ws-transport")]
pub mod ws_transport;

pub use driver::HeadlessDriver;
pub use mcp::McpServer;
pub use protocol::{AgentEvent, AgentRequest, AgentResponse};
pub use rpc::RpcTransport;
pub use session::AgentSession;
#[cfg(feature = "ws-transport")]
pub use ws_transport::WsTransport;

use std::io::{self, BufRead};

/// Read a single newline-terminated record from `reader`, capping the amount of
/// memory buffered at `max_bytes`.
///
/// This prevents a malicious or malformed client from exhausting memory by
/// sending a very long line with no newline: a naive `BufRead::lines()` would
/// allocate the entire line before any size guard could reject it. Here the
/// buffer never grows past `max_bytes`; any excess (up to the next newline) is
/// drained from the stream so parsing can resume cleanly on the next record.
///
/// Returns `Ok(None)` at end of input. Otherwise returns the line bytes (without
/// the trailing newline) and a flag indicating whether the line exceeded
/// `max_bytes` and was therefore truncated.
pub(crate) fn read_capped_line<R: BufRead>(
    reader: &mut R,
    max_bytes: usize,
) -> io::Result<Option<(Vec<u8>, bool)>> {
    let mut buf = Vec::new();
    let mut oversized = false;
    let mut saw_any = false;
    loop {
        let available = match reader.fill_buf() {
            Ok(b) => b,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            if !saw_any {
                return Ok(None);
            }
            return Ok(Some((buf, oversized)));
        }
        saw_any = true;
        let newline = available.iter().position(|&b| b == b'\n');
        let chunk_len = newline.unwrap_or(available.len());
        let room = max_bytes.saturating_sub(buf.len());
        let take = room.min(chunk_len);
        buf.extend_from_slice(&available[..take]);
        if chunk_len > room {
            oversized = true;
        }
        match newline {
            Some(idx) => {
                reader.consume(idx + 1);
                return Ok(Some((buf, oversized)));
            }
            None => {
                let consumed = available.len();
                reader.consume(consumed);
            }
        }
    }
}

#[cfg(test)]
mod capped_line_tests {
    use super::read_capped_line;
    use std::io::Cursor;

    #[test]
    fn reads_normal_lines() {
        let mut r = Cursor::new(b"hello\nworld\n".to_vec());
        let (a, over_a) = read_capped_line(&mut r, 1024).unwrap().unwrap();
        assert_eq!(a, b"hello");
        assert!(!over_a);
        let (b, over_b) = read_capped_line(&mut r, 1024).unwrap().unwrap();
        assert_eq!(b, b"world");
        assert!(!over_b);
        assert!(read_capped_line(&mut r, 1024).unwrap().is_none());
    }

    #[test]
    fn final_line_without_newline() {
        let mut r = Cursor::new(b"tail".to_vec());
        let (line, oversized) = read_capped_line(&mut r, 1024).unwrap().unwrap();
        assert_eq!(line, b"tail");
        assert!(!oversized);
        assert!(read_capped_line(&mut r, 1024).unwrap().is_none());
    }

    #[test]
    fn oversized_line_is_capped_and_drained() {
        // 100-byte line, cap at 10 bytes: buffer must not exceed the cap, and
        // the next record must parse cleanly after the oversized one is drained.
        let mut data = vec![b'x'; 100];
        data.push(b'\n');
        data.extend_from_slice(b"next\n");
        let mut r = Cursor::new(data);

        let (line, oversized) = read_capped_line(&mut r, 10).unwrap().unwrap();
        assert!(oversized);
        assert!(line.len() <= 10, "buffer exceeded cap: {}", line.len());

        let (line2, oversized2) = read_capped_line(&mut r, 10).unwrap().unwrap();
        assert_eq!(line2, b"next");
        assert!(!oversized2);
    }

    #[test]
    fn empty_input_returns_none() {
        let mut r = Cursor::new(Vec::new());
        assert!(read_capped_line(&mut r, 1024).unwrap().is_none());
    }
}
