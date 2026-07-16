//! Framed Unix-socket request/response protocol.
//!
//! Wire format: 4-byte big-endian length prefix + one JSON document.
//! Requests are single frames; responses are one or more frames — the
//! final frame always has `"final": true`. Oversized or malformed
//! frames are rejected without crashing the daemon.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// Maximum accepted request frame (spec: reject oversized requests).
pub const MAX_REQUEST: u32 = 4 * 1024 * 1024;
/// Maximum response frame we will emit (streamed data is chunked).
pub const MAX_FRAME: u32 = 8 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// Execute one TCL operation in the persistent session.
    Exec { tcl: String },
    /// Graceful daemon shutdown; refused while a command is in flight.
    Stop,
    /// Run a managed tool operation (xsct/xsdb/bootgen/dtc).
    RunTool { tool: String, argv: Vec<String> },
    /// Protocol ping (used by readiness wait).
    Ping,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    /// Completed TCL result (mirrors result.json content).
    Result {
        #[serde(rename = "final")]
        final_: bool,
        result: serde_json::Value,
    },
    /// Immediate busy rejection.
    Busy {
        #[serde(rename = "final")]
        final_: bool,
        current_command: String,
        dispatched_at: String,
        last_pty_read_at: Option<String>,
    },
    /// Streamed tool output chunk (lossy UTF-8).
    Stream {
        #[serde(rename = "final")]
        final_: bool,
        fd: u8, // 1 = stdout, 2 = stderr
        data: String,
    },
    /// Tool operation completion.
    Exit {
        #[serde(rename = "final")]
        final_: bool,
        exit_status: i32,
    },
    /// Generic acknowledgement (stop accepted, pong).
    Ok {
        #[serde(rename = "final")]
        final_: bool,
        message: String,
    },
    /// Structured error.
    Error {
        #[serde(rename = "final")]
        final_: bool,
        code: String, // "busy" | "usage" | "crashed" | "protocol" | "internal"
        message: String,
    },
}

impl Response {
    #[allow(dead_code)] // used by protocol tests and future streaming clients
    pub fn is_final(&self) -> bool {
        match self {
            Response::Result { final_, .. }
            | Response::Busy { final_, .. }
            | Response::Stream { final_, .. }
            | Response::Exit { final_, .. }
            | Response::Ok { final_, .. }
            | Response::Error { final_, .. } => *final_,
        }
    }
}

pub fn write_frame<W: Write, T: Serialize>(w: &mut W, value: &T) -> std::io::Result<()> {
    let body = serde_json::to_vec(value)?;
    if body.len() as u32 > MAX_FRAME {
        return Err(std::io::Error::other("frame too large"));
    }
    w.write_all(&(body.len() as u32).to_be_bytes())?;
    w.write_all(&body)?;
    w.flush()
}

/// Read one length-prefixed JSON frame, enforcing `max` bytes.
pub fn read_frame<R: Read, T: for<'de> Deserialize<'de>>(
    r: &mut R,
    max: u32,
) -> std::io::Result<T> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf);
    if len == 0 || len > max {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("rejected frame of {len} bytes (max {max})"),
        ));
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("malformed frame: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn roundtrip(req: &Request) -> Request {
        let mut buf = Vec::new();
        write_frame(&mut buf, req).unwrap();
        read_frame(&mut Cursor::new(buf), MAX_REQUEST).unwrap()
    }

    #[test]
    fn exec_roundtrip_preserves_metacharacters() {
        let tcl = r#"puts "a b;c $x \" [pwd]""#.to_string();
        match roundtrip(&Request::Exec { tcl: tcl.clone() }) {
            Request::Exec { tcl: got } => assert_eq!(got, tcl),
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn oversized_frame_rejected() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(MAX_REQUEST + 1).to_be_bytes());
        buf.extend_from_slice(b"xxxx");
        let err = read_frame::<_, Request>(&mut Cursor::new(buf), MAX_REQUEST).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn malformed_json_rejected() {
        let body = b"{not json";
        let mut buf = Vec::new();
        buf.extend_from_slice(&(body.len() as u32).to_be_bytes());
        buf.extend_from_slice(body);
        let err = read_frame::<_, Request>(&mut Cursor::new(buf), MAX_REQUEST).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("malformed"));
    }

    #[test]
    fn zero_length_rejected() {
        let buf = 0u32.to_be_bytes().to_vec();
        assert!(read_frame::<_, Request>(&mut Cursor::new(buf), MAX_REQUEST).is_err());
    }

    #[test]
    fn response_final_flag() {
        let r = Response::Exit { final_: true, exit_status: 3 };
        assert!(r.is_final());
        let s = Response::Stream { final_: false, fd: 1, data: "x".into() };
        assert!(!s.is_final());
    }
}
