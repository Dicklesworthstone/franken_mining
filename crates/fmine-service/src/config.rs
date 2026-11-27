//! Determinism: BitwisePortable
//!
//! Configuration and CLI argument parsing for `fmined` (`BD-119`, plan §26.6, §30.3).

use std::collections::BTreeSet;
use std::path::PathBuf;

use fmine_runtime::region::Role;

use crate::ServiceError;

/// CLI usage summary for `fmined`.
pub const USAGE: &str = "fmined --role <site|regional|enterprise|relay> --node-id <id> --data-dir <dir> [--socket <path>] [--cell <cell_id>]... [--peer <peer_id>]... [--once-status]";

/// Validated node daemon configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeConfig {
    pub node_id: String,
    pub role: Role,
    pub data_dir: PathBuf,
    pub socket_path: PathBuf,
    /// Initial local authority cells leased by this node.
    pub cells: Vec<String>,
    /// Initial sync peers managed under `SyncRegion`.
    pub peers: Vec<String>,
    /// When `true`, persist committed batches to `FMT-SEG-1` segments and stage CAS under `data_dir`.
    pub persist_segments: bool,
}

impl NodeConfig {
    /// Builds a validated [`NodeConfig`] with default socket path `<data_dir>/fmined.sock`.
    pub fn new(
        node_id: impl Into<String>,
        role: Role,
        data_dir: impl Into<PathBuf>,
    ) -> Result<Self, ServiceError> {
        let data_dir = data_dir.into();
        let socket_path = data_dir.join("fmined.sock");
        let cfg = Self {
            node_id: node_id.into(),
            role,
            data_dir,
            socket_path,
            cells: Vec::new(),
            peers: Vec::new(),
            persist_segments: false,
        };
        cfg.validate()?;
        Ok(cfg)
    }

    /// Validates node identifier, socket path, and deduplicated cell/peer names.
    pub fn validate(&self) -> Result<(), ServiceError> {
        let id = self.node_id.trim();
        if id.is_empty() || id != self.node_id || id.len() > 128 {
            return Err(ServiceError::InvalidConfig {
                detail: "node_id must be 1..=128 non-padded characters".to_string(),
            });
        }
        if self.data_dir.as_os_str().is_empty() {
            return Err(ServiceError::InvalidConfig {
                detail: "data_dir must not be empty".to_string(),
            });
        }
        if self.socket_path.as_os_str().is_empty() {
            return Err(ServiceError::InvalidConfig {
                detail: "socket_path must not be empty".to_string(),
            });
        }
        let mut seen_cells = BTreeSet::new();
        for cell in &self.cells {
            if cell.trim().is_empty() || cell.trim() != cell {
                return Err(ServiceError::InvalidConfig {
                    detail: format!("invalid cell_id '{cell}'"),
                });
            }
            if !seen_cells.insert(cell.as_str()) {
                return Err(ServiceError::InvalidConfig {
                    detail: format!("duplicate cell_id '{cell}'"),
                });
            }
        }
        let mut seen_peers = BTreeSet::new();
        for peer in &self.peers {
            if peer.trim().is_empty() || peer.trim() != peer {
                return Err(ServiceError::InvalidConfig {
                    detail: format!("invalid peer_id '{peer}'"),
                });
            }
            if !seen_peers.insert(peer.as_str()) {
                return Err(ServiceError::InvalidConfig {
                    detail: format!("duplicate peer_id '{peer}'"),
                });
            }
        }
        Ok(())
    }
}

/// Parses one of the four plan §26.6 node roles.
pub fn parse_role(raw: &str) -> Result<Role, ServiceError> {
    match raw.trim() {
        "site" => Ok(Role::Site),
        "regional" => Ok(Role::Regional),
        "enterprise" => Ok(Role::Enterprise),
        "relay" => Ok(Role::Relay),
        other => Err(ServiceError::InvalidConfig {
            detail: format!("unknown role '{other}' (expected site|regional|enterprise|relay)"),
        }),
    }
}

/// Stable lowercase spelling of [`Role`].
#[must_use]
pub const fn role_name(role: Role) -> &'static str {
    match role {
        Role::Site => "site",
        Role::Regional => "regional",
        Role::Enterprise => "enterprise",
        Role::Relay => "relay",
    }
}

/// Parsed CLI invocation for `fmined`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    pub config: NodeConfig,
    /// If `true`, boot, print deterministic status JSON, and exit `0` without binding the socket.
    pub once_status: bool,
    /// If `true`, print usage and exit `0`.
    pub help: bool,
}

/// Parses `fmined` command-line arguments (excluding argv\[0\]).
pub fn parse_cli_args(args: &[String]) -> Result<CliArgs, String> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        let placeholder = NodeConfig {
            node_id: "help".to_string(),
            role: Role::Site,
            data_dir: PathBuf::from("."),
            socket_path: PathBuf::from("./fmined.sock"),
            cells: Vec::new(),
            peers: Vec::new(),
            persist_segments: false,
        };
        return Ok(CliArgs {
            config: placeholder,
            once_status: true,
            help: true,
        });
    }

    let mut role: Option<Role> = None;
    let mut node_id: Option<String> = None;
    let mut data_dir: Option<PathBuf> = None;
    let mut socket_path: Option<PathBuf> = None;
    let mut cells: Vec<String> = Vec::new();
    let mut peers: Vec<String> = Vec::new();
    let mut once_status = false;
    let mut persist_segments = true;

    let mut i = 0;
    while let Some(arg) = args.get(i) {
        match arg.as_str() {
            "--role" => {
                i += 1;
                let v = args
                    .get(i)
                    .filter(|s| !s.is_empty() && !s.starts_with("--"))
                    .ok_or_else(|| "missing value for --role".to_string())?;
                role = Some(parse_role(v).map_err(|e| e.to_string())?);
            }
            "--node-id" => {
                i += 1;
                let v = args
                    .get(i)
                    .filter(|s| !s.is_empty() && !s.starts_with("--"))
                    .ok_or_else(|| "missing value for --node-id".to_string())?;
                node_id = Some(v.clone());
            }
            "--data-dir" => {
                i += 1;
                let v = args
                    .get(i)
                    .filter(|s| !s.is_empty() && !s.starts_with("--"))
                    .ok_or_else(|| "missing value for --data-dir".to_string())?;
                data_dir = Some(PathBuf::from(v));
            }
            "--socket" => {
                i += 1;
                let v = args
                    .get(i)
                    .filter(|s| !s.is_empty() && !s.starts_with("--"))
                    .ok_or_else(|| "missing value for --socket".to_string())?;
                socket_path = Some(PathBuf::from(v));
            }
            "--cell" => {
                i += 1;
                let v = args
                    .get(i)
                    .filter(|s| !s.is_empty() && !s.starts_with("--"))
                    .ok_or_else(|| "missing value for --cell".to_string())?;
                cells.push(v.clone());
            }
            "--peer" => {
                i += 1;
                let v = args
                    .get(i)
                    .filter(|s| !s.is_empty() && !s.starts_with("--"))
                    .ok_or_else(|| "missing value for --peer".to_string())?;
                peers.push(v.clone());
            }
            "--once-status" => {
                once_status = true;
            }
            "--in-memory" => {
                persist_segments = false;
            }
            other => {
                return Err(format!("unknown argument '{other}'; usage: {USAGE}"));
            }
        }
        i += 1;
    }

    let role = role.ok_or_else(|| format!("missing required --role; usage: {USAGE}"))?;
    let node_id = node_id.ok_or_else(|| format!("missing required --node-id; usage: {USAGE}"))?;
    let data_dir =
        data_dir.ok_or_else(|| format!("missing required --data-dir; usage: {USAGE}"))?;
    let socket_path = socket_path.unwrap_or_else(|| data_dir.join("fmined.sock"));

    let config = NodeConfig {
        node_id,
        role,
        data_dir,
        socket_path,
        cells,
        peers,
        persist_segments,
    };
    config.validate().map_err(|e| e.to_string())?;

    Ok(CliArgs {
        config,
        once_status,
        help: false,
    })
}
