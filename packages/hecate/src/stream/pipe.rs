/*
   Copyright (C) 2026 Kamila Szewczyk <k@iczelia.net> on the behalf of the Saarland University.

   This program is free software; you can redistribute it and/or modify
   it under the terms of the GNU General Public License as published by
   the Free Software Foundation; either version 3 of the License, or
   (at your option) any later version.

   This program is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
   GNU General Public License for more details.

   You should have received a copy of the GNU General Public License
   along with this program. If not, see <http://www.gnu.org/licenses/>.
*/

use super::traits::{StreamRead, StreamWrite};
use std::fs::File;
use std::io::{self, Read, Write};
use std::process::{Child, Command, Stdio};

/// Input stream from a child process's stdout.
pub struct PipeInput {
    stdout: std::process::ChildStdout,
    child: Option<Child>,
    finished: bool,
}

impl PipeInput {
    /// Spawn a process from a shell command string. The child's stdout is captured.
    pub fn from_command(cmd: &str) -> io::Result<Self> {
        let args =
            shell_words::split(cmd).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        if args.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty pipe command",
            ));
        }
        let mut command = Command::new(&args[0]);
        command.args(&args[1..]);
        command.stdout(Stdio::piped());
        command.stdin(Stdio::null());

        let mut child = command.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("failed to capture child stdout"))?;

        Ok(Self {
            stdout,
            child: Some(child),
            finished: false,
        })
    }

    /// Spawn a filter process: stdin reads from the given file, stdout is
    /// piped (for reading). Used for --filter in encode mode.
    pub fn filter_file(cmd: &str, input_file: File) -> io::Result<Self> {
        let args =
            shell_words::split(cmd).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        if args.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty filter command",
            ));
        }
        let mut command = Command::new(&args[0]);
        command.args(&args[1..]);
        command.stdout(Stdio::piped());
        command.stdin(Stdio::from(input_file));

        let mut child = command.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("failed to capture child stdout"))?;

        Ok(Self {
            stdout,
            child: Some(child),
            finished: false,
        })
    }

    fn wait_child(&mut self) -> io::Result<()> {
        if self.finished {
            return Ok(());
        }
        if let Some(mut child) = self.child.take() {
            let status = child.wait()?;
            if !status.success() {
                return Err(io::Error::other(format!(
                    "pipe command exited with status {}",
                    status
                )));
            }
        }
        self.finished = true;
        Ok(())
    }
}

impl Drop for PipeInput {
    fn drop(&mut self) {
        let _ = self.wait_child();
    }
}

impl StreamRead for PipeInput {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.stdout.read(buf)?;
        if n == 0 {
            self.wait_child()?;
        }
        Ok(n)
    }
}

/// Output stream to a child process's stdin.
pub struct PipeOutput {
    stdin: Option<std::process::ChildStdin>,
    child: Option<Child>,
    finished: bool,
}

impl PipeOutput {
    /// Spawn a process from a shell command string. The child's stdin is captured.
    pub fn from_command(cmd: &str) -> io::Result<Self> {
        let args =
            shell_words::split(cmd).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        if args.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty pipe command",
            ));
        }
        let mut command = Command::new(&args[0]);
        command.args(&args[1..]);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::null());

        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("failed to capture child stdin"))?;

        Ok(Self {
            stdin: Some(stdin),
            child: Some(child),
            finished: false,
        })
    }

    /// Spawn a filter process: stdin is piped (for writing), stdout goes to
    /// the given file. Used for --filter in decode/slice mode with file output.
    pub fn filter_to_file(cmd: &str, output_file: File) -> io::Result<Self> {
        let args =
            shell_words::split(cmd).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        if args.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty filter command",
            ));
        }
        let mut command = Command::new(&args[0]);
        command.args(&args[1..]);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::from(output_file));

        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("failed to capture child stdin"))?;

        Ok(Self {
            stdin: Some(stdin),
            child: Some(child),
            finished: false,
        })
    }

    /// Spawn a filter process: stdin is piped (for writing), stdout is
    /// inherited (terminal). Used for --filter in decode/slice mode with
    /// stdout output.
    pub fn filter_to_stdout(cmd: &str) -> io::Result<Self> {
        let args =
            shell_words::split(cmd).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        if args.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty filter command",
            ));
        }
        let mut command = Command::new(&args[0]);
        command.args(&args[1..]);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::inherit());

        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("failed to capture child stdin"))?;

        Ok(Self {
            stdin: Some(stdin),
            child: Some(child),
            finished: false,
        })
    }

    fn finish(&mut self) -> io::Result<()> {
        if self.finished {
            return Ok(());
        }
        // Drop stdin to signal EOF to child
        if let Some(mut stdin) = self.stdin.take() {
            stdin.flush()?;
        }
        if let Some(mut child) = self.child.take() {
            let status = child.wait()?;
            if !status.success() {
                return Err(io::Error::other(format!(
                    "pipe command exited with status {}",
                    status
                )));
            }
        }
        self.finished = true;
        Ok(())
    }
}

impl Drop for PipeOutput {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

impl StreamWrite for PipeOutput {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "pipe stdin closed"))?;
        stdin.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(stdin) = self.stdin.as_mut() {
            stdin.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipe_input_echo() {
        let mut pi = PipeInput::from_command("echo hello").unwrap();
        let mut buf = Vec::new();
        loop {
            let mut tmp = [0u8; 64];
            let n = pi.read(&mut tmp).unwrap();
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
        }
        assert_eq!(String::from_utf8_lossy(&buf).trim(), "hello");
    }

    #[test]
    fn test_pipe_output_cat() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();

        // Use tee to write to file
        let cmd = format!("tee {}", shell_words::quote(&path));
        let mut po = PipeOutput::from_command(&cmd).unwrap();
        po.write_all(b"test data").unwrap();
        po.finish().unwrap(); // finish() closes stdin and waits for child

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "test data");
    }
}
