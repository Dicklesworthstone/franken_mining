//! Bounded local Unix domain socket NDJSON IPC server and client for `fmined`
//! (`BD-119`, plan §30.3: serves `<node>/fmined.sock`).

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::node::FminedNode;
use crate::{ServiceError, escape_json};

/// Maximum accepted single IPC request line (64 KiB) preventing unbounded socket allocation.
pub const MAX_IPC_LINE_BYTES: usize = 64 * 1024;

const CLIENT_IO_TIMEOUT: Duration = Duration::from_secs(5);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Live Unix domain socket server wrapping a shared [`FminedNode`].
///
/// Cleans up its bound socket file automatically on [`Drop`].
pub struct FminedServer {
    socket_path: PathBuf,
    listener: UnixListener,
    node: Arc<Mutex<FminedNode>>,
    stop_flag: Arc<AtomicBool>,
}

impl FminedServer {
    /// Binds `node`'s configured `<node>/fmined.sock`.
    ///
    /// Refuses with [`ServiceError::SocketInUse`] if another live daemon is already accepting
    /// connections at `socket_path`. Removes a dead/stale socket file only after confirming a
    /// connect probe fails.
    pub fn bind(node: Arc<Mutex<FminedNode>>) -> Result<Self, ServiceError> {
        let socket_path = {
            let guard = node.lock().map_err(|_| ServiceError::Io {
                detail: "node mutex poisoned".to_string(),
            })?;
            guard.config().socket_path.clone()
        };
        if let Some(parent) = socket_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|e| ServiceError::Io {
                detail: format!("failed to create socket parent dir: {e}"),
            })?;
        }
        if let Ok(meta) = fs::symlink_metadata(&socket_path) {
            if !meta.file_type().is_socket() {
                return Err(ServiceError::Io {
                    detail: format!(
                        "refusing to overwrite non-socket path '{}'",
                        socket_path.display()
                    ),
                });
            }
            match UnixStream::connect(&socket_path) {
                Ok(_) => {
                    return Err(ServiceError::SocketInUse {
                        path: socket_path.display().to_string(),
                    });
                }
                Err(_) => {
                    fs::remove_file(&socket_path).map_err(|e| ServiceError::Io {
                        detail: format!(
                            "failed to clear stale socket '{}': {e}",
                            socket_path.display()
                        ),
                    })?;
                }
            }
        }
        let listener = UnixListener::bind(&socket_path).map_err(|e| ServiceError::Io {
            detail: format!("failed to bind socket '{}': {e}", socket_path.display()),
        })?;
        listener
            .set_nonblocking(true)
            .map_err(|e| ServiceError::Io {
                detail: format!("failed to configure nonblocking listener: {e}"),
            })?;
        Ok(Self {
            socket_path,
            listener,
            node,
            stop_flag: Arc::new(AtomicBool::new(false)),
        })
    }

    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    #[must_use]
    pub fn node(&self) -> Arc<Mutex<FminedNode>> {
        Arc::clone(&self.node)
    }

    /// Signals the accept loop to stop promptly.
    pub fn request_stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Runs the local IPC accept loop until a `{"op":"shutdown"}` request arrives or
    /// [`Self::request_stop`] is called.
    pub fn serve_until_shutdown(&self) -> Result<(), ServiceError> {
        while !self.stop_flag.load(Ordering::SeqCst) {
            match self.listener.accept() {
                Ok((stream, _addr)) => {
                    let should_stop = self.handle_connection(stream);
                    if should_stop {
                        self.stop_flag.store(true, Ordering::SeqCst);
                        break;
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(ACCEPT_POLL_INTERVAL);
                }
                Err(err) => {
                    return Err(ServiceError::Io {
                        detail: format!("socket accept error: {err}"),
                    });
                }
            }
        }
        if let Ok(mut guard) = self.node.lock() {
            guard.shutdown();
        }
        Ok(())
    }

    fn handle_connection(&self, mut stream: UnixStream) -> bool {
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(CLIENT_IO_TIMEOUT));
        let _ = stream.set_write_timeout(Some(CLIENT_IO_TIMEOUT));

        let mut raw = Vec::new();
        let read_res = {
            let mut limited = (&stream).take((MAX_IPC_LINE_BYTES + 1) as u64);
            let mut reader = BufReader::new(&mut limited);
            reader.read_until(b'\n', &mut raw)
        };
        let (response, stop) = match read_res {
            Err(err) => (
                error_json(5, "FMINE-REF-506", &format!("socket read failed: {err}")),
                false,
            ),
            Ok(0) => return false,
            Ok(_) if raw.len() > MAX_IPC_LINE_BYTES => (
                error_json(
                    3,
                    "FMINE-REF-208",
                    &format!("IPC request exceeds {MAX_IPC_LINE_BYTES} bytes"),
                ),
                false,
            ),
            Ok(_) => match std::str::from_utf8(&raw) {
                Ok(line) => self.dispatch_line(line.trim()),
                Err(_) => (
                    error_json(3, "FMINE-REF-208", "IPC request is not valid UTF-8"),
                    false,
                ),
            },
        };
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();
        stop
    }

    fn dispatch_line(&self, line: &str) -> (String, bool) {
        let Some(op) = extract_json_string(line, "op") else {
            return (
                error_json(
                    3,
                    "FMINE-REF-208",
                    "IPC request must be a JSON object with a string 'op' field",
                ),
                false,
            );
        };

        let Ok(mut node) = self.node.lock() else {
            return (
                error_json(5, "FMINE-REF-506", "fmined node state lock poisoned"),
                false,
            );
        };

        match op.as_str() {
            "ping" => (
                format!(
                    "{{\"ok\":true,\"exit_code\":0,\"node_id\":\"{}\",\"role\":\"{}\",\"mode\":\"{}\"}}",
                    escape_json(node.node_id()),
                    crate::config::role_name(node.role()),
                    node.mode().as_str(),
                ),
                false,
            ),
            "status" => (
                format!(
                    "{{\"ok\":true,\"exit_code\":0,\"status\":{}}}",
                    node.status_json()
                ),
                false,
            ),
            "leases" => (
                format!(
                    "{{\"ok\":true,\"exit_code\":0,\"leases\":{}}}",
                    node.leases_json()
                ),
                false,
            ),
            "commit" => {
                let Some(cell_id) = extract_json_string(line, "cell_id") else {
                    return (
                        error_json(3, "FMINE-REF-208", "commit request missing 'cell_id'"),
                        false,
                    );
                };
                let Some(key) = extract_json_string(line, "key") else {
                    return (
                        error_json(3, "FMINE-REF-208", "commit request missing 'key'"),
                        false,
                    );
                };
                let Some(value) = extract_json_string(line, "value") else {
                    return (
                        error_json(3, "FMINE-REF-208", "commit request missing 'value'"),
                        false,
                    );
                };
                match node.commit_put(&cell_id, &key, value.into_bytes(), current_unix_ns()) {
                    Ok(receipt) => (
                        format!(
                            "{{\"ok\":true,\"exit_code\":0,\"cell_id\":\"{}\",\"epoch\":{},\"seq\":{},\"batch_digest\":\"{}\",\"mmr_root\":\"{}\"}}",
                            escape_json(&receipt.cell_id),
                            receipt.epoch,
                            receipt.seq,
                            receipt.batch_digest,
                            receipt.mmr_root,
                        ),
                        false,
                    ),
                    Err(err) => {
                        let refusal = err.to_refusal();
                        (
                            error_json(err.exit_code(), refusal.code, &err.to_string()),
                            false,
                        )
                    }
                }
            }
            "shutdown" => {
                node.shutdown();
                (
                    "{\"ok\":true,\"exit_code\":0,\"status\":\"shutting_down\"}".to_string(),
                    true,
                )
            }
            other => (
                error_json(
                    3,
                    "FMINE-REF-208",
                    &format!("unknown fmined IPC op '{other}'"),
                ),
                false,
            ),
        }
    }
}

impl Drop for FminedServer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket_path);
    }
}

/// Thin local Unix socket client for talking to `<node>/fmined.sock`.
pub struct FminedClient;

impl FminedClient {
    /// Sends one NDJSON request line to `socket_path` and returns the trimmed response line.
    pub fn send_request(socket_path: &Path, request_json: &str) -> Result<String, ServiceError> {
        if request_json.len() > MAX_IPC_LINE_BYTES || request_json.contains('\n') {
            return Err(ServiceError::Protocol {
                detail: "request must be a single line within MAX_IPC_LINE_BYTES".to_string(),
            });
        }
        let mut stream = UnixStream::connect(socket_path).map_err(|e| ServiceError::Io {
            detail: format!("failed to connect to '{}': {e}", socket_path.display()),
        })?;
        let _ = stream.set_read_timeout(Some(CLIENT_IO_TIMEOUT));
        let _ = stream.set_write_timeout(Some(CLIENT_IO_TIMEOUT));
        stream
            .write_all(request_json.as_bytes())
            .and_then(|()| stream.write_all(b"\n"))
            .and_then(|()| stream.flush())
            .map_err(|e| ServiceError::Io {
                detail: format!("failed to write IPC request: {e}"),
            })?;

        let mut buf = Vec::new();
        let mut limited = (&stream).take((MAX_IPC_LINE_BYTES + 1) as u64);
        let mut reader = BufReader::new(&mut limited);
        reader
            .read_until(b'\n', &mut buf)
            .map_err(|e| ServiceError::Io {
                detail: format!("failed to read IPC response: {e}"),
            })?;
        if buf.len() > MAX_IPC_LINE_BYTES {
            return Err(ServiceError::Protocol {
                detail: "IPC response exceeded MAX_IPC_LINE_BYTES".to_string(),
            });
        }
        let text = String::from_utf8(buf).map_err(|_| ServiceError::Protocol {
            detail: "IPC response is not valid UTF-8".to_string(),
        })?;
        Ok(text.trim_end_matches(&['\r', '\n'][..]).to_string())
    }
}

fn error_json(exit_code: u8, code: &str, detail: &str) -> String {
    format!(
        "{{\"ok\":false,\"exit_code\":{exit_code},\"code\":\"{}\",\"error\":\"{}\"}}",
        escape_json(code),
        escape_json(detail)
    )
}

fn current_unix_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

/// Extracts a top-level JSON string value for `"key"` from a flat JSON object, honoring standard
/// JSON string escapes (`\"`, `\\`, `\n`, `\r`, `\t`).
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let trimmed = json.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let mut i = 1usize;
    let end = bytes.len() - 1;
    while i < end {
        while i < end && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= end {
            break;
        }
        let (field_name, next_i) = parse_json_string_at(trimmed, i)?;
        i = next_i;
        while i < end && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= end || bytes[i] != b':' {
            return None;
        }
        i += 1;
        while i < end && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= end {
            return None;
        }
        if bytes[i] == b'"' {
            let (val, after_val) = parse_json_string_at(trimmed, i)?;
            if field_name == key {
                return Some(val);
            }
            i = after_val;
        } else {
            // Skip primitive / nested value cleanly to next comma at depth 0.
            let mut depth = 0usize;
            while i < end {
                match bytes[i] {
                    b'"' => {
                        let (_, after) = parse_json_string_at(trimmed, i)?;
                        i = after;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        if depth == 0 {
                            break;
                        }
                        depth -= 1;
                    }
                    b',' if depth == 0 => break,
                    _ => {}
                }
                i += 1;
            }
        }
    }
    None
}

fn parse_json_string_at(s: &str, start: usize) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = s[start + 1..].char_indices();
    while let Some((rel, ch)) = chars.next() {
        match ch {
            '"' => return Some((out, start + 1 + rel + 1)),
            '\\' => {
                let (_, esc) = chars.next()?;
                match esc {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    _ => return None,
                }
            }
            c if (c as u32) < 0x20 => return None,
            c => out.push(c),
        }
    }
    None
}
