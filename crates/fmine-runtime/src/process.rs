//! Owned synchronous subprocesses with exact stdin bytes and bounded output capture.
//! Every pipe worker is scoped and joined. Startup/read failures and output limits kill/reap the child;
//! this adapter supplies no timeout, ambient clock, detached task or command policy.

use std::fmt;
use std::io::{self, Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, Scope, ScopedJoinHandle};

#[derive(Debug)]
pub enum ProcessError {
    Spawn(io::Error),
    Worker(io::Error),
    Read {
        stream: &'static str,
        source: io::Error,
    },
    OutputLimit {
        stream: &'static str,
        limit: usize,
    },
    Wait(io::Error),
    MissingPipe,
    WorkerPanicked,
    InvalidLimit,
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(e) => write!(f, "cannot start child: {e}"),
            Self::Worker(e) => write!(f, "cannot start pipe worker: {e}"),
            Self::Read { stream, source } => write!(f, "cannot read {stream}: {source}"),
            Self::OutputLimit { stream, limit } => write!(f, "{stream} exceeds {limit} bytes"),
            Self::Wait(e) => write!(f, "cannot reap child: {e}"),
            Self::MissingPipe => f.write_str("child pipe is unavailable"),
            Self::WorkerPanicked => f.write_str("child pipe worker panicked"),
            Self::InvalidLimit => f.write_str("output bound is outside the supported range"),
        }
    }
}

impl std::error::Error for ProcessError {}

/// A nonzero exit remains observable even if the child also closed stdin early. Callers inspect the
/// status first, then require `input_error` to be absent before accepting successful processing.
#[derive(Debug)]
pub struct PipedOutput {
    pub output: Output,
    pub input_error: Option<io::Error>,
}

enum Completion {
    Input(io::Result<()>),
    Stdout(Result<Vec<u8>, ProcessError>),
    Stderr(Result<Vec<u8>, ProcessError>),
    Panicked,
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn worker<'scope, 'env, F>(
    scope: &'scope Scope<'scope, 'env>,
    child: &mut Child,
    sender: Sender<Completion>,
    work: F,
) -> Result<ScopedJoinHandle<'scope, ()>, ProcessError>
where
    F: FnOnce() -> Completion + Send + 'scope,
{
    thread::Builder::new()
        .spawn_scoped(scope, move || {
            let done = catch_unwind(AssertUnwindSafe(work)).unwrap_or(Completion::Panicked);
            let _ = sender.send(done);
        })
        .map_err(|e| {
            stop(child);
            ProcessError::Worker(e)
        })
}

fn capture(reader: impl Read, stream: &'static str, limit: usize) -> Result<Vec<u8>, ProcessError> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| ProcessError::Read { stream, source })?;
    if bytes.len() > limit {
        return Err(ProcessError::OutputLimit { stream, limit });
    }
    Ok(bytes)
}

/// Run the supplied command with piped stdin/stdout/stderr, preserving the input snapshot exactly.
/// Each output stream retains at most `limit + 1` bytes; the extra byte detects overflow. Readers and
/// writer run concurrently, so full pipes cannot deadlock each other. A reader/worker failure or output
/// overflow kills/reaps the child and joins every worker. An input-write error is retained while the
/// child finishes its bounded diagnostics and exit status. There is deliberately no wall-clock timeout.
pub fn output_with_input(
    command: &mut Command,
    input: &[u8],
    limit: usize,
) -> Result<PipedOutput, ProcessError> {
    if u64::try_from(limit)
        .ok()
        .and_then(|n| n.checked_add(1))
        .is_none()
    {
        return Err(ProcessError::InvalidLimit);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(ProcessError::Spawn)?;
    let (Some(mut stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        stop(&mut child);
        return Err(ProcessError::MissingPipe);
    };
    thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel();
        let writer = worker(scope, &mut child, sender.clone(), move || {
            Completion::Input(stdin.write_all(input))
        })?;
        let out_reader = worker(scope, &mut child, sender.clone(), move || {
            Completion::Stdout(capture(stdout, "stdout", limit))
        })?;
        let err_reader = worker(scope, &mut child, sender.clone(), move || {
            Completion::Stderr(capture(stderr, "stderr", limit))
        })?;
        drop(sender);
        let (mut stdout, mut stderr, mut input_error) = (Vec::new(), Vec::new(), None);
        let mut failure = None;
        for _ in 0..3 {
            let error = match receiver.recv() {
                Ok(Completion::Input(result)) => {
                    // Closing stdin can precede a legitimate rejection and its diagnostic output.
                    // Retain that write failure without replacing the child's exit with our kill.
                    input_error = result.err();
                    None
                }
                Ok(Completion::Stdout(Ok(bytes))) => {
                    stdout = bytes;
                    None
                }
                Ok(Completion::Stderr(Ok(bytes))) => {
                    stderr = bytes;
                    None
                }
                Ok(Completion::Stdout(Err(e)) | Completion::Stderr(Err(e))) => Some(e),
                Ok(Completion::Panicked) | Err(_) => Some(ProcessError::WorkerPanicked),
            };
            if let Some(error) = error {
                failure.get_or_insert(error);
                stop(&mut child);
            }
        }
        for handle in [writer, out_reader, err_reader] {
            if handle.join().is_err() {
                stop(&mut child);
                failure.get_or_insert(ProcessError::WorkerPanicked);
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        let status = child.wait().map_err(|e| {
            stop(&mut child);
            ProcessError::Wait(e)
        })?;
        Ok(PipedOutput {
            output: Output {
                status,
                stdout,
                stderr,
            },
            input_error,
        })
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn full_pipes_preserve_exact_input_and_both_outputs() {
        let input: Vec<_> = (0..262_144).map(|n| (n % 256) as u8).collect();
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "head -c 131072 /dev/zero; head -c 131072 /dev/zero >&2; cat",
        ]);
        let result = output_with_input(&mut command, &input, 524_288).expect("owned full pipes");
        assert!(result.output.status.success());
        assert!(result.input_error.is_none());
        let mut expected = vec![0; 131_072];
        expected.extend_from_slice(&input);
        assert_eq!(result.output.stdout, expected);
        assert_eq!(result.output.stderr, vec![0; 131_072]);
    }

    #[test]
    fn either_output_limit_stops_and_joins_a_blocked_input_writer() {
        for (script, expected) in [
            ("exec cat /dev/zero", "stdout"),
            ("exec cat /dev/zero >&2", "stderr"),
        ] {
            let mut command = Command::new("sh");
            command.args(["-c", script]);
            let error =
                output_with_input(&mut command, &vec![7; 262_144], 32).expect_err("bounded output");
            assert!(
                matches!(error, ProcessError::OutputLimit { stream, limit: 32 } if stream == expected)
            );
        }
    }

    #[test]
    fn nonzero_status_and_stderr_survive_early_stdin_closure() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf rejected >&2; exit 9"]);
        let result = output_with_input(&mut command, &vec![7; 262_144], 8).expect("exit retained");
        assert_eq!(result.output.status.code(), Some(9));
        assert_eq!(result.output.stderr, b"rejected");
        assert!(result.input_error.is_some());
    }

    #[test]
    fn closed_stdin_does_not_cancel_later_diagnostics_or_exit_status() {
        for exit_code in [0, 9] {
            let mut command = Command::new("sh");
            command.args([
                "-c",
                &format!(
                    "exec 0<&-; sleep 0.1; printf stdout-note; printf rejected >&2; exit {exit_code}"
                ),
            ]);
            let result = output_with_input(&mut command, &vec![7; 262_144], 16)
                .expect("closed stdin is separate from process completion");
            assert_eq!(result.output.status.code(), Some(exit_code));
            assert_eq!(result.output.stdout, b"stdout-note");
            assert_eq!(result.output.stderr, b"rejected");
            assert_eq!(
                result.input_error.as_ref().map(io::Error::kind),
                Some(io::ErrorKind::BrokenPipe)
            );
        }
    }
}
