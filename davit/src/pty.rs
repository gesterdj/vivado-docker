//! PTY ownership of the Vivado process.
//!
//! The daemon owns Vivado through a PTY: the child is spawned with the
//! PTY slave as its controlling terminal, and a reader thread drains the
//! master continuously, appending every raw byte to the session raw log
//! and matching the exact Vivado prompt.

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};

/// The exact Vivado TCL prompt.
pub const PROMPT: &str = "Vivado% ";

pub struct Pty {
    pub master: File,
    pub child: Child,
}

/// Spawn `argv` on a new PTY. Echo is disabled on the slave so command
/// input is not duplicated into the output stream.
pub fn spawn_on_pty(argv: &[String], envs: &[(String, String)]) -> std::io::Result<Pty> {
    let mut master: RawFd = -1;
    let mut slave: RawFd = -1;
    let rc = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }

    // Disable echo on the slave.
    unsafe {
        let mut tio: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(slave, &mut tio) == 0 {
            tio.c_lflag &= !(libc::ECHO | libc::ECHONL);
            libc::tcsetattr(slave, libc::TCSANOW, &tio);
        }
    }

    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let slave_fd = slave;
    unsafe {
        cmd.pre_exec(move || {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::ioctl(slave_fd, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            for fd in 0..3 {
                if libc::dup2(slave_fd, fd) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            libc::close(slave_fd);
            Ok(())
        });
    }
    let child = cmd.spawn()?;
    unsafe { libc::close(slave) };
    let master = unsafe { File::from_raw_fd(master) };
    Ok(Pty { master, child })
}

impl Pty {
    /// Write a line (command + newline) to the Vivado stdin.
    #[allow(dead_code)] // daemon writes via a cloned master; used in tests
    pub fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.master.write_all(line.as_bytes())?;
        self.master.write_all(b"\n")?;
        self.master.flush()
    }

    pub fn try_clone_master(&self) -> std::io::Result<File> {
        self.master.try_clone()
    }
}

/// Incremental prompt matcher over a byte stream. Tracks whether the
/// accumulated output currently ends with the exact prompt.
pub struct PromptMatcher {
    tail: Vec<u8>,
}

impl PromptMatcher {
    pub fn new() -> Self {
        Self { tail: Vec::new() }
    }

    /// Feed bytes; returns true if the stream now ends with the prompt.
    pub fn feed(&mut self, bytes: &[u8]) -> bool {
        self.tail.extend_from_slice(bytes);
        let keep = PROMPT.len().max(64);
        if self.tail.len() > keep {
            let cut = self.tail.len() - keep;
            self.tail.drain(..cut);
        }
        self.at_prompt()
    }

    pub fn at_prompt(&self) -> bool {
        self.tail.ends_with(PROMPT.as_bytes())
    }

    pub fn reset(&mut self) {
        self.tail.clear();
    }
}

/// Strip trailing prompt and surrounding whitespace from captured output.
pub fn strip_prompt(mut s: &str) -> &str {
    s = s.trim_end();
    if let Some(pre) = s.strip_suffix(PROMPT.trim_end()) {
        s = pre.trim_end();
    }
    s
}

/// Read available bytes from a PTY master File (blocking read).
pub fn read_chunk(master: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    master.read(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_matcher_across_chunks() {
        let mut m = PromptMatcher::new();
        assert!(!m.feed(b"some output\nViva"));
        assert!(m.feed(b"do% "));
        assert!(!m.feed(b"more"));
    }

    #[test]
    fn prompt_in_middle_does_not_match() {
        let mut m = PromptMatcher::new();
        assert!(!m.feed(b"echo Vivado% something\n"));
    }

    #[test]
    fn strip_prompt_works() {
        assert_eq!(strip_prompt("result\nVivado% "), "result");
        assert_eq!(strip_prompt("bare"), "bare");
    }

    #[test]
    fn pty_spawn_echo_roundtrip() {
        // Use a tiny shell as a stand-in interpreter to prove PTY wiring.
        let mut pty = spawn_on_pty(
            &["/bin/sh".into(), "-c".into(), "read x; printf '%s\\n' \"got:$x\"".into()],
            &[],
        )
        .unwrap();
        pty.write_line("hello world").unwrap();
        let mut out = String::new();
        let mut buf = [0u8; 256];
        loop {
            match read_chunk(&mut pty.master, &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => out.push_str(&String::from_utf8_lossy(&buf[..n])),
            }
        }
        let _ = pty.child.wait();
        assert!(out.contains("got:hello world"), "out={out:?}");
    }
}
