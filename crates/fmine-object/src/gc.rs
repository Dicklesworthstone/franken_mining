//! Reachability garbage collection from live ledger roots (`BD-091`, plan §26.5).
//!
//! Fail-closed mark-and-sweep:
//! - Every root in `live_roots` must resolve to a valid manifest whose chunks all exist and verify
//!   on disk before any sweep is planned (a broken live root refuses the GC plan rather than risking
//!   data loss).
//! - Content-addressed chunks shared between a reachable object and an unreachable object remain
//!   marked live and are never swept.
//! - Chunks named by a pending `OB-SEAL` staging marker stay live, so an interrupted publication can
//!   resume (`PO-BED-3`). The marker is an open obligation; GC never resolves it.
//! - [`apply_gc`] plans again and refuses (`FMINE-REF-502`) unless the store still yields the plan
//!   it was given, so a write between a dry run and its application cannot lose a chunk it reused.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use fmine_core::hash::Digest32;

use crate::cas::CasError;
use crate::store::CasStore;

/// Deterministic dry-run plan computed by [`plan_gc`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GcPlan {
    /// Live published manifest roots retained.
    pub retained_roots: Vec<Digest32>,
    /// Roots of pending `OB-SEAL` staging markers whose chunks are retained.
    pub retained_staged_roots: Vec<Digest32>,
    /// Live chunk digests retained (union across `retained_roots` and `retained_staged_roots`).
    pub retained_chunks: usize,
    /// Unreachable published manifest roots eligible for removal.
    pub sweep_roots: Vec<Digest32>,
    /// Unreachable chunk digests eligible for removal (never referenced by any live root).
    pub sweep_chunks: Vec<Digest32>,
    /// Exact byte count of sweepable chunk files.
    pub reclaimable_chunk_bytes: u64,
}

/// Receipt returned after [`apply_gc`] removes unreachable objects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GcReceipt {
    pub removed_manifests: usize,
    pub removed_chunks: usize,
    pub reclaimed_chunk_bytes: u64,
}

/// Compute a fail-closed reachability GC plan over `store` given the set of `live_roots` (`BD-091`).
pub fn plan_gc(store: &CasStore, live_roots: &BTreeSet<Digest32>) -> Result<GcPlan, CasError> {
    let mut live_chunks = BTreeSet::new();
    let mut retained_roots = Vec::with_capacity(live_roots.len());

    // Mark phase: verify every live root and all of its chunks on disk first!
    for root in live_roots {
        let manifest = store.get_manifest(root)?;
        for cref in &manifest.chunks {
            store.verify_chunk_on_disk(cref)?;
            live_chunks.insert(cref.sha256);
        }
        retained_roots.push(*root);
    }

    // A staged object may hold only some of its chunks yet, so they are marked without verifying.
    let mut retained_staged_roots = Vec::new();
    for manifest in store.list_staged_manifests()? {
        live_chunks.extend(manifest.chunks.iter().map(|cref| cref.sha256));
        retained_staged_roots.push(manifest.root_digest);
    }

    let published = store.list_published_roots()?;
    let mut sweep_roots = Vec::new();
    for root in published {
        if !live_roots.contains(&root) {
            sweep_roots.push(root);
        }
    }

    let all_chunks = store.list_chunks()?;
    let mut sweep_chunks = Vec::new();
    let mut reclaimable_chunk_bytes = 0u64;
    for (digest, _path, len) in all_chunks {
        if !live_chunks.contains(&digest) {
            sweep_chunks.push(digest);
            reclaimable_chunk_bytes = reclaimable_chunk_bytes.saturating_add(len);
        }
    }

    Ok(GcPlan {
        retained_roots,
        retained_staged_roots,
        retained_chunks: live_chunks.len(),
        sweep_roots,
        sweep_chunks,
        reclaimable_chunk_bytes,
    })
}

/// Apply `plan`, removing ONLY unreachable manifests and unreachable chunks. The plan is computed
/// again from `live_roots` first, and a difference refuses with nothing removed.
pub fn apply_gc(
    store: &CasStore,
    live_roots: &BTreeSet<Digest32>,
    plan: &GcPlan,
) -> Result<GcReceipt, CasError> {
    let current = plan_gc(store, live_roots)?;
    if current != *plan {
        return Err(CasError::StaleGcPlan {
            detail: format!(
                "the store now gives {} sweepable manifests and {} sweepable chunks, the plan {} and {}",
                current.sweep_roots.len(),
                current.sweep_chunks.len(),
                plan.sweep_roots.len(),
                plan.sweep_chunks.len()
            ),
        });
    }

    let mut removed_manifests = 0usize;
    for root in &plan.sweep_roots {
        let path: PathBuf = store.manifest_path(root);
        if path.is_file() {
            fs::remove_file(&path).map_err(|e| CasError::Io {
                operation: "gc_remove_manifest",
                detail: format!("{}: {e}", path.display()),
            })?;
            removed_manifests += 1;
        }
    }

    let mut removed_chunks = 0usize;
    for digest in &plan.sweep_chunks {
        let path = store.chunk_path(digest);
        if path.is_file() {
            fs::remove_file(&path).map_err(|e| CasError::Io {
                operation: "gc_remove_chunk",
                detail: format!("{}: {e}", path.display()),
            })?;
            removed_chunks += 1;
        }
    }

    Ok(GcReceipt {
        removed_manifests,
        removed_chunks,
        reclaimed_chunk_bytes: plan.reclaimable_chunk_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reachability_gc_preserves_shared_chunks_and_sweeps_unreachable_objects() {
        let dir = std::env::temp_dir().join(format!("fmine-object-gc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = CasStore::open(&dir).expect("open");

        // Object A and Object B share chunk 0 (1024 identical bytes) but differ in chunk 1.
        let shared = vec![0xAAu8; 1024];
        let mut obj_a = shared.clone();
        obj_a.extend_from_slice(&[0x11u8; 1024]);
        let mut obj_b = shared;
        obj_b.extend_from_slice(&[0x22u8; 1024]);

        let ma = store
            .put_bytes("application/octet-stream", &obj_a, 1024)
            .expect("put A");
        let mb = store
            .put_bytes("application/octet-stream", &obj_b, 1024)
            .expect("put B");

        // Keep only A live; B is unreachable, yet chunk 0 is shared and MUST survive GC!
        let mut live = BTreeSet::new();
        live.insert(ma.root_digest);

        let plan = plan_gc(&store, &live).expect("plan gc");
        assert_eq!(plan.retained_roots, vec![ma.root_digest]);
        assert_eq!(plan.retained_chunks, 2);
        assert_eq!(plan.sweep_roots, vec![mb.root_digest]);
        assert_eq!(plan.sweep_chunks.len(), 1); // ONLY B's unique chunk 1!
        assert_eq!(plan.reclaimable_chunk_bytes, 1024);

        let receipt = apply_gc(&store, &live, &plan).expect("apply gc");
        assert_eq!(receipt.removed_manifests, 1);
        assert_eq!(receipt.removed_chunks, 1);
        assert_eq!(receipt.reclaimed_chunk_bytes, 1024);

        // A still reads back completely verified; B is gone.
        assert_eq!(store.read_object(&ma.root_digest).expect("read A"), obj_a);
        assert!(matches!(
            store.get_manifest(&mb.root_digest),
            Err(CasError::ManifestNotFound { .. })
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    fn fresh_store(tag: &str) -> (PathBuf, CasStore) {
        let dir =
            std::env::temp_dir().join(format!("fmine-object-gc-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = CasStore::open(&dir).expect("open");
        (dir, store)
    }

    fn two_chunks(first: u8, second: u8) -> Vec<u8> {
        let mut bytes = vec![first; 1024];
        bytes.extend_from_slice(&[second; 1024]);
        bytes
    }

    #[test]
    fn test_stale_plan_refuses_instead_of_deleting_a_chunk_a_later_object_reuses() {
        let (dir, store) = fresh_store("stale");
        let mb = store
            .put_bytes("application/octet-stream", &two_chunks(0xAA, 0x22), 1024)
            .expect("put B");
        let plan = plan_gc(&store, &BTreeSet::new()).expect("plan gc");
        assert_eq!(plan.sweep_chunks.len(), 2);

        // C is written after the dry run and reuses B's second chunk instead of writing it again.
        let obj_c = two_chunks(0x22, 0x33);
        let mc = store
            .put_bytes("application/octet-stream", &obj_c, 1024)
            .expect("put C");
        let both_ways = [BTreeSet::new(), BTreeSet::from([mc.root_digest])];
        for live in &both_ways {
            let err = apply_gc(&store, live, &plan).expect_err("stale plan must refuse");
            assert!(matches!(err, CasError::StaleGcPlan { .. }), "{err}");
            assert_eq!(err.to_refusal().code, "FMINE-REF-502");
        }
        assert!(store.has_manifest(&mb.root_digest));
        assert_eq!(store.read_object(&mc.root_digest).expect("read C"), obj_c);

        // A fresh plan keeps the shared chunk and removes only what B alone held.
        let live = BTreeSet::from([mc.root_digest]);
        let fresh = plan_gc(&store, &live).expect("fresh plan");
        assert_eq!(fresh.sweep_roots, vec![mb.root_digest]);
        assert_eq!(fresh.sweep_chunks.len(), 1);
        let receipt = apply_gc(&store, &live, &fresh).expect("apply fresh plan");
        assert_eq!((receipt.removed_manifests, receipt.removed_chunks), (1, 1));
        assert_eq!(store.read_object(&mc.root_digest).expect("read C"), obj_c);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_staged_object_keeps_its_chunks_until_published_and_released() {
        let (dir, store) = fresh_store("staged");
        let obj_d = two_chunks(0x44, 0x55);
        // A crash after the first chunk leaves the OB-SEAL marker and one chunk on disk.
        let partial = store
            .stage_partial("application/octet-stream", &obj_d, 1024, Some(1))
            .expect("stage D partially");
        let root = partial.manifest.root_digest;

        let none = BTreeSet::new();
        let plan = plan_gc(&store, &none).expect("plan gc");
        assert_eq!(plan.retained_staged_roots, vec![root]);
        assert_eq!(plan.retained_chunks, 2);
        assert!(plan.sweep_chunks.is_empty());
        apply_gc(&store, &none, &plan).expect("apply gc");

        let resumed = store
            .stage_object("application/octet-stream", &obj_d, 1024)
            .expect("resume D");
        assert_eq!((resumed.chunks_reused, resumed.chunks_written), (1, 1));
        store.publish_staged(&resumed).expect("publish D");
        assert_eq!(store.read_object(&root).expect("read D"), obj_d);

        // Published and not live, with its marker cleared, D is now unreachable.
        let plan = plan_gc(&store, &none).expect("plan gc after publication");
        assert!(plan.retained_staged_roots.is_empty());
        assert_eq!(
            (plan.sweep_roots.clone(), plan.sweep_chunks.len()),
            (vec![root], 2)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_undecodable_staging_marker_refuses_the_plan() {
        let (dir, store) = fresh_store("badmarker");
        store
            .put_bytes("application/octet-stream", &two_chunks(0x66, 0x77), 1024)
            .expect("put E");
        let marker = store.staging_path(&Digest32([0u8; 32]));
        fs::create_dir_all(marker.parent().expect("staging dir")).expect("mkdir staging");
        fs::write(&marker, b"not a manifest").expect("write marker");
        assert!(plan_gc(&store, &BTreeSet::new()).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
