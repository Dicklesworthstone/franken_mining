//! Root-last content-addressed object store (`BD-090`, `PO-BED-3`, `OB-SEAL`).
//!
//! No CAS root is visible (`manifests/<aa>/<root>.casm`) until EVERY chunk referenced by its
//! manifest is persisted to `chunks/<aa>/<sha256>.chunk`, fsynced, and re-verified against its
//! expected SHA-256 and fanout-16 Merkle root. A crash at any stage boundary leaves only staged
//! artifacts and keeps the root invisible; re-staging resumes idempotently over already-verified
//! chunks (`OB-SEAL`).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use fmine_core::hash::{Digest32, sha256};

use crate::cas::{CasError, CasManifest, ChunkRef, chunk_bytes};

/// Receipt of a staged (not yet root-published) object (`OB-SEAL`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedObject {
    /// Complete validated manifest awaiting root-last publication.
    pub manifest: CasManifest,
    /// Number of chunks written in this staging pass (excluding already-present verified chunks).
    pub chunks_written: usize,
    /// Number of chunks reused from existing verified CAS storage (deduplicated/resumed).
    pub chunks_reused: usize,
    /// Path to the staging obligation marker file (`staging/<root>.stage`).
    pub staging_marker: PathBuf,
}

/// Local filesystem `FMT-CAS-1` store enforcing `PO-BED-3` root-last publication.
#[derive(Clone, Debug)]
pub struct CasStore {
    root_dir: PathBuf,
}

impl CasStore {
    /// Open or initialize a CAS directory tree (`chunks/`, `manifests/`, `staging/`).
    pub fn open(root_dir: impl Into<PathBuf>) -> Result<Self, CasError> {
        let root_dir = root_dir.into();
        for sub in ["chunks", "manifests", "staging"] {
            fs::create_dir_all(root_dir.join(sub)).map_err(|e| CasError::Io {
                operation: "create_cas_dirs",
                detail: format!("{}: {e}", root_dir.join(sub).display()),
            })?;
        }
        Ok(Self { root_dir })
    }

    /// Root directory of this CAS store.
    #[must_use]
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Canonical on-disk path for a chunk digest (`chunks/<aa>/<sha256>.chunk`).
    #[must_use]
    pub fn chunk_path(&self, digest: &Digest32) -> PathBuf {
        let hex = digest.to_hex();
        self.root_dir
            .join("chunks")
            .join(&hex[..2])
            .join(format!("{hex}.chunk"))
    }

    /// Canonical on-disk path for a published manifest root (`manifests/<aa>/<root>.casm`).
    #[must_use]
    pub fn manifest_path(&self, root: &Digest32) -> PathBuf {
        let hex = root.to_hex();
        self.root_dir
            .join("manifests")
            .join(&hex[..2])
            .join(format!("{hex}.casm"))
    }

    /// Path for a pending `OB-SEAL` staging marker (`staging/<root>.stage`).
    #[must_use]
    pub fn staging_path(&self, root: &Digest32) -> PathBuf {
        self.root_dir
            .join("staging")
            .join(format!("{}.stage", root.to_hex()))
    }

    /// Stage chunks for `bytes` up to `max_chunks_to_write` (pass `None` to stage all chunks).
    ///
    /// Does **not** publish the manifest root (`PO-BED-3`): `get_manifest(root)` stays invisible
    /// until [`Self::publish_staged`] verifies all chunks and atomically publishes the manifest.
    pub fn stage_object(
        &self,
        media_type: &str,
        bytes: &[u8],
        chunk_size: usize,
    ) -> Result<StagedObject, CasError> {
        self.stage_partial(media_type, bytes, chunk_size, None)
    }

    /// Stage at most `limit` chunks (used directly by `PO-BED-3` crash-boundary tests to simulate
    /// a process crash after writing `k` chunks before root publication).
    pub fn stage_partial(
        &self,
        media_type: &str,
        bytes: &[u8],
        chunk_size: usize,
        limit: Option<usize>,
    ) -> Result<StagedObject, CasError> {
        let sliced = chunk_bytes(bytes, chunk_size)?;
        let refs: Vec<ChunkRef> = sliced.iter().map(|(r, _)| *r).collect();
        let manifest = CasManifest::from_chunks(media_type, chunk_size as u32, refs)?;
        let staging_marker = self.staging_path(&manifest.root_digest);
        let manifest_bytes = manifest.to_bytes()?;
        write_atomic(&staging_marker, &manifest_bytes)?;

        let mut chunks_written = 0usize;
        let mut chunks_reused = 0usize;
        let cap = limit.unwrap_or(sliced.len());

        for (cref, data) in sliced.into_iter().take(cap) {
            let path = self.chunk_path(&cref.sha256);
            if path.is_file() {
                let existing = fs::read(&path).map_err(|e| CasError::Io {
                    operation: "read_existing_chunk",
                    detail: format!("{}: {e}", path.display()),
                })?;
                let actual = sha256(&existing);
                if actual != cref.sha256 {
                    return Err(CasError::ChunkDigestMismatch {
                        expected: cref.sha256,
                        actual,
                    });
                }
                chunks_reused += 1;
            } else {
                write_atomic(&path, data)?;
                chunks_written += 1;
            }
        }

        Ok(StagedObject {
            manifest,
            chunks_written,
            chunks_reused,
            staging_marker,
        })
    }

    /// Verify EVERY chunk referenced by `staged.manifest` on disk and, ONLY if 100% pass,
    /// atomically publish the manifest root (`PO-BED-3` root-last) and clear the `OB-SEAL` marker.
    pub fn publish_staged(&self, staged: &StagedObject) -> Result<CasManifest, CasError> {
        let published = self.publish_manifest_if_complete(&staged.manifest)?;
        let _ = fs::remove_file(&staged.staging_marker);
        Ok(published)
    }

    /// Persist one chunk into the content-addressed chunk pool after verifying `sha256(data) == expected`
    /// (`BD-106` streaming drain). Never publishes any manifest root (`PO-BED-3`).
    pub fn put_verified_chunk(&self, expected: Digest32, data: &[u8]) -> Result<bool, CasError> {
        let actual = sha256(data);
        if actual != expected {
            return Err(CasError::ChunkDigestMismatch { expected, actual });
        }
        let path = self.chunk_path(&expected);
        if path.is_file() {
            let existing = fs::read(&path).map_err(|e| CasError::Io {
                operation: "read_existing_chunk",
                detail: format!("{}: {e}", path.display()),
            })?;
            let disk_actual = sha256(&existing);
            if disk_actual != expected {
                return Err(CasError::ChunkDigestMismatch {
                    expected,
                    actual: disk_actual,
                });
            }
            return Ok(false);
        }
        write_atomic(&path, data)?;
        Ok(true)
    }

    /// Verify EVERY chunk referenced by `manifest` on disk and, ONLY if 100% pass, atomically
    /// publish the manifest root (`PO-BED-3` root-last).
    pub fn publish_manifest_if_complete(
        &self,
        manifest: &CasManifest,
    ) -> Result<CasManifest, CasError> {
        manifest.validate()?;
        for cref in &manifest.chunks {
            self.verify_chunk_on_disk(cref)?;
        }
        let manifest_bytes = manifest.to_bytes()?;
        let final_path = self.manifest_path(&manifest.root_digest);
        write_atomic(&final_path, &manifest_bytes)?;
        Ok(manifest.clone())
    }

    /// Returns `true` iff `root` has been published root-last into `manifests/`.
    #[must_use]
    pub fn has_manifest(&self, root: &Digest32) -> bool {
        self.manifest_path(root).is_file()
    }

    /// Stage all chunks and publish root-last in one call (`PO-BED-3`).
    pub fn put_bytes(
        &self,
        media_type: &str,
        bytes: &[u8],
        chunk_size: usize,
    ) -> Result<CasManifest, CasError> {
        let staged = self.stage_object(media_type, bytes, chunk_size)?;
        self.publish_staged(&staged)
    }

    /// Read a published manifest by its fanout-16 `root` digest.
    /// Returns [`CasError::ManifestNotFound`] if the root has not been published.
    pub fn get_manifest(&self, root: &Digest32) -> Result<CasManifest, CasError> {
        let path = self.manifest_path(root);
        if !path.is_file() {
            return Err(CasError::ManifestNotFound { root: *root });
        }
        let bytes = fs::read(&path).map_err(|e| CasError::Io {
            operation: "read_manifest",
            detail: format!("{}: {e}", path.display()),
        })?;
        let manifest = CasManifest::from_bytes(&bytes)?;
        if manifest.root_digest != *root {
            return Err(CasError::RootMismatch {
                expected: *root,
                actual: manifest.root_digest,
            });
        }
        Ok(manifest)
    }

    /// Read and reassemble a published object, verifying EVERY chunk's SHA-256 and total length.
    pub fn read_object(&self, root: &Digest32) -> Result<Vec<u8>, CasError> {
        let manifest = self.get_manifest(root)?;
        let mut out = Vec::with_capacity(manifest.total_bytes as usize);
        for cref in &manifest.chunks {
            let chunk_bytes = self.read_verified_chunk(cref)?;
            out.extend_from_slice(&chunk_bytes);
        }
        if (out.len() as u64) != manifest.total_bytes {
            return Err(CasError::InvalidField {
                field: "total_bytes",
                detail: format!(
                    "reassembled {} bytes != manifest {}",
                    out.len(),
                    manifest.total_bytes
                ),
            });
        }
        Ok(out)
    }

    /// List all published manifest roots currently visible in `manifests/`.
    pub fn list_published_roots(&self) -> Result<Vec<Digest32>, CasError> {
        let mut roots = Vec::new();
        for path in collect_files(&self.root_dir.join("manifests"), "casm")? {
            let bytes = fs::read(&path).map_err(|e| CasError::Io {
                operation: "read_manifest_list",
                detail: format!("{}: {e}", path.display()),
            })?;
            let manifest = CasManifest::from_bytes(&bytes)?;
            roots.push(manifest.root_digest);
        }
        roots.sort();
        Ok(roots)
    }

    /// Manifests of every pending `OB-SEAL` staging marker in `staging/`, sorted by root. A marker
    /// that does not decode refuses: GC could not tell which chunks it holds.
    pub fn list_staged_manifests(&self) -> Result<Vec<CasManifest>, CasError> {
        let mut staged = Vec::new();
        for path in collect_files(&self.root_dir.join("staging"), "stage")? {
            let bytes = fs::read(&path).map_err(|e| CasError::Io {
                operation: "read_staging_marker",
                detail: format!("{}: {e}", path.display()),
            })?;
            staged.push(CasManifest::from_bytes(&bytes)?);
        }
        staged.sort_by_key(|m| m.root_digest);
        Ok(staged)
    }

    /// List all chunk digests currently stored in `chunks/`.
    pub fn list_chunks(&self) -> Result<Vec<(Digest32, PathBuf, u64)>, CasError> {
        let mut out = Vec::new();
        for path in collect_files(&self.root_dir.join("chunks"), "chunk")? {
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let digest = parse_digest32(stem)?;
            let len = fs::metadata(&path)
                .map_err(|e| CasError::Io {
                    operation: "stat_chunk",
                    detail: format!("{}: {e}", path.display()),
                })?
                .len();
            out.push((digest, path, len));
        }
        out.sort_by_key(|(d, _, _)| *d);
        Ok(out)
    }

    pub(crate) fn verify_chunk_on_disk(&self, cref: &ChunkRef) -> Result<(), CasError> {
        let _ = self.read_verified_chunk(cref)?;
        Ok(())
    }

    fn read_verified_chunk(&self, cref: &ChunkRef) -> Result<Vec<u8>, CasError> {
        let path = self.chunk_path(&cref.sha256);
        if !path.is_file() {
            return Err(CasError::MissingChunk {
                sha256: cref.sha256,
            });
        }
        let bytes = fs::read(&path).map_err(|e| CasError::Io {
            operation: "read_chunk",
            detail: format!("{}: {e}", path.display()),
        })?;
        let actual = sha256(&bytes);
        if actual != cref.sha256 {
            return Err(CasError::ChunkDigestMismatch {
                expected: cref.sha256,
                actual,
            });
        }
        if bytes.len() != cref.len as usize {
            return Err(CasError::InvalidField {
                field: "chunk.len",
                detail: format!(
                    "chunk {} len {} != declared {}",
                    cref.sha256,
                    bytes.len(),
                    cref.len
                ),
            });
        }
        Ok(bytes)
    }
}

fn write_atomic(dest: &Path, bytes: &[u8]) -> Result<(), CasError> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| CasError::Io {
        operation: "create_parent_dir",
        detail: format!("{}: {e}", parent.display()),
    })?;
    let mut tmp_os = dest.as_os_str().to_os_string();
    tmp_os.push(format!(".tmp.{}", std::process::id()));
    let tmp = PathBuf::from(tmp_os);

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)
        .map_err(|e| CasError::Io {
            operation: "open_tmp",
            detail: format!("{}: {e}", tmp.display()),
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| CasError::Io {
            operation: "write_sync_tmp",
            detail: format!("{}: {e}", tmp.display()),
        })?;
    drop(file);

    fs::rename(&tmp, dest).map_err(|e| CasError::Io {
        operation: "atomic_rename",
        detail: format!("{} -> {}: {e}", tmp.display(), dest.display()),
    })?;
    sync_dir(parent)?;
    Ok(())
}

fn sync_dir(dir: &Path) -> Result<(), CasError> {
    #[cfg(unix)]
    {
        File::open(dir)
            .and_then(|d| d.sync_all())
            .map_err(|e| CasError::Io {
                operation: "sync_dir",
                detail: format!("{}: {e}", dir.display()),
            })?;
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
    }
    Ok(())
}

fn collect_files(dir: &Path, ext: &str) -> Result<Vec<PathBuf>, CasError> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let rd = fs::read_dir(&cur).map_err(|e| CasError::Io {
            operation: "read_dir",
            detail: format!("{}: {e}", cur.display()),
        })?;
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == ext) {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn parse_digest32(hex: &str) -> Result<Digest32, CasError> {
    let bytes = hex.as_bytes();
    if bytes.len() != 64 {
        return Err(CasError::InvalidField {
            field: "digest_hex",
            detail: format!("expected 64 hex chars, got {}", bytes.len()),
        });
    }
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = nibble(bytes[i * 2])?;
        let lo = nibble(bytes[i * 2 + 1])?;
        *slot = (hi << 4) | lo;
    }
    Ok(Digest32(out))
}

fn nibble(b: u8) -> Result<u8, CasError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err(CasError::InvalidField {
            field: "digest_hex",
            detail: "non-hex character".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_store(tag: &str) -> (CasStore, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("fmine-object-store-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = CasStore::open(&dir).expect("open store");
        (store, dir)
    }

    #[test]
    fn test_po_bed_3_crash_boundaries_keep_partial_roots_invisible_and_resume_cleanly() {
        let (store, dir) = scratch_store("po_bed_3");
        let payload: Vec<u8> = (0..18_000usize)
            .map(|i| {
                let x = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                ((x >> 32) ^ (x >> 16) ^ x) as u8
            })
            .collect();
        let chunk_size = 1024;

        // Crash boundary 1: process dies after writing only 5 of 18 chunks.
        let partial = store
            .stage_partial("application/octet-stream", &payload, chunk_size, Some(5))
            .expect("stage partial");
        let root = partial.manifest.root_digest;
        assert_eq!(partial.chunks_written, 5);
        assert!(partial.staging_marker.is_file());

        // PO-BED-3: Root MUST be completely invisible before publication!
        assert!(matches!(
            store.get_manifest(&root),
            Err(CasError::ManifestNotFound { .. })
        ));
        // Attempting to publish with missing chunks MUST fail closed!
        assert!(matches!(
            store.publish_staged(&partial),
            Err(CasError::MissingChunk { .. })
        ));
        assert!(matches!(
            store.get_manifest(&root),
            Err(CasError::ManifestNotFound { .. })
        ));

        // Resume (OB-SEAL): re-staging reuses the 5 valid chunks and writes only the remaining 13!
        let resumed = store
            .stage_object("application/octet-stream", &payload, chunk_size)
            .expect("resume stage");
        assert_eq!(resumed.chunks_reused, 5);
        assert_eq!(resumed.chunks_written, 13);
        // Still invisible until publish_staged completes!
        assert!(matches!(
            store.get_manifest(&root),
            Err(CasError::ManifestNotFound { .. })
        ));

        // Publish root-last -> now visible, staging marker cleared, bytes reassemble identically!
        let published = store.publish_staged(&resumed).expect("publish");
        assert_eq!(published.root_digest, root);
        assert!(!resumed.staging_marker.exists());
        assert_eq!(store.read_object(&root).expect("read back"), payload);

        // Tampering with one chunk on disk MUST cause read_object to refuse!
        let victim = store.chunk_path(&published.chunks[7].sha256);
        let mut mutated = fs::read(&victim).expect("read chunk");
        mutated[0] ^= 0xFF;
        fs::write(&victim, &mutated).expect("tamper");
        assert!(matches!(
            store.read_object(&root),
            Err(CasError::ChunkDigestMismatch { .. })
        ));

        let _ = fs::remove_dir_all(&dir);
    }
}
