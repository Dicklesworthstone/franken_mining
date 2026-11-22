# CONTRACT.md — `fmine-view`

## 1. Purpose
Adit U-layer read-only 3-D scene exporter (glTF 2.0 JSON + binary GLB 2.0 subset) and self-contained static HTML viewer (`BD-256`, plan §28.3, §39 WP-20):
- `voxel_mesh`: extracts outward-oriented exposed boundary-face triangle meshes directly from `fmine_geom::vdb::VdbGrid<u8>` sparse voxel grids (culling internal faces between adjacent active voxels), enforcing explicit `CrsId` provenance (`G-CRS-1`, `FMINE-REF-106`) and finite geometry bounds;
- `gltf`: deterministic glTF 2.0 JSON (with embedded base64 data URI) and aligned binary GLB 2.0 serializer (`magic = "glTF"`, version 2, JSON + BIN chunks) embedding `no_claim`, `crs_id`, `read_only = true`, and `INV-002` `false` authority flags, verified by SHA-256 golden digests;
- `viewer`: generates a strictly read-only static HTML viewer page embedding the glTF scene metadata, CRS banner, `no_claim` watermark, and HTML-escaped titles with zero authority or write path.

- **Crate:** `fmine-view`
- **Layer:** U
- **Subsystem:** Adit
- **LOC budget:** 15,000 planned / 17,000 ceiling
- **Allowed Module Families:** `["voxel_mesh", "gltf", "viewer"]`

## 2. Inputs
- Sparse `fmine_geom::vdb::VdbGrid<u8>` voxel grids, `VoxelGridFrame` (`origin_m`, `voxel_size_m`, `crs_id`), and `SceneLayerSpec` lists.
- Dependencies: `fmine-core`, `fmine-crs`, `fmine-evidence`, `fmine-geom` (`U → K0/S0`, zero external crates, zero ledger/authority imports). Performs zero filesystem I/O, zero network I/O, zero clocks, and zero threads.

## 3. Outputs
- `SceneMesh`, deterministic `GltfExport { gltf_json, glb_bytes, bin_bytes, scene_sha256, triangle_count, vertex_count }`, and strictly read-only HTML static viewer strings.
- Emits no `Claim`s and holds zero authority or control write paths (`INV-002`, `INV-011`).

## 4. Invariants
- **Read-only boundary (`INV-002`, §28.3):** `fmine-view` imports no ledger, authz, or effect crate; every exported glTF `asset.extras` sets `read_only: true`, `execution_authorized: false`, `reserve_disclosure_claim: false`, `geotechnical_signoff: false`, and `regulatory_compliance_claim: false`.
- **CRS uniformity (`G-CRS-1`):** combining layers with distinct `CrsId`s refuses with `FMINE-REF-106`.
- **Exposed-surface culling & determinism:** internal shared faces between adjacent active `VdbGrid` voxels are omitted; traversal follows `VdbGrid::iter_active()` total canonical order so repeated exports yield bit-identical GLB/JSON bytes and `scene_sha256`.
- **GLB 2.0 framing:** every emitted GLB obeys 4-byte chunk alignment, exact `JSON` (`0x4E4F534A`) and `BIN` (`0x004E4942`) chunk headers, and exact total length.

## 5. Refusals
- `ViewError::CrsMismatch` → `FMINE-REF-106` (exit 3): scene layers declare incompatible `CrsId`s.
- `ViewError::EmptyScene` → `FMINE-REF-201` (exit 3): grid or scene contains zero active geometry.
- `ViewError::InvalidGeometry` / `ViewError::Overflow` → `FMINE-REF-109` (exit 3): non-finite/non-positive voxel dimensions, invalid RGBA values, or index overflow.

## 6. Determinism class
All files (`lib.rs`, `voxel_mesh.rs`, `gltf.rs`, `viewer.rs`) declare `//! Determinism: BitwisePortable`. Arithmetic uses IEEE `+ - * /` and integer indexing over deterministic `VdbGrid::iter_active()` traversal with zero transcendentals and zero hash-map iteration.

## 7. Evidence class of outputs
Not applicable: read-only visual export of geometry passed by callers; carries explicit `no_claim` watermark.

## 8. Validity domain
- Finite 3-D scenes exported in single-precision IEEE-754 `f32` local/shifted coordinates (`<= u32::MAX` indices) matching the glTF 2.0 core specification (`POSITION`, `NORMAL`, indexed `TRIANGLES`, `baseColorFactor`).

## 9. Oracles
- Exposed-face culling and CRS mismatch tests in `voxel_mesh.rs`, pinned golden-scene SHA-256 and GLB binary structure tests in `gltf.rs`, and read-only / HTML-escaping tests in `viewer.rs`.

## 10. Known gaps
- Interactive WebGL2 shader controls and optional glTF PBR textures are deferred to later UI polish passes; the current static viewer renders deterministic read-only scene inspection and JSON/GLB download hooks.

## 11. Public API
- `ViewError`, `VIEW_NO_CLAIM`
- `voxel_mesh::{VoxelGridFrame, SceneMesh, extract_voxel_surface_mesh}`
- `gltf::{GltfSceneInput, GltfExport, GlbHeaderInfo, export_gltf_scene, inspect_glb_header}`
- `viewer::{render_static_viewer_html}`

## 12. Proof obligations
- `BD-256` (golden scenes, deterministic GLB/glTF 2.0 bytes, read-only boundary, CRS safety): `voxel_mesh::tests::test_vdb_voxel_surface_extraction_culls_internal_faces_and_enforces_crs`, `gltf::tests::test_gltf_and_glb_golden_scene_determinism_and_structure`, `viewer::tests::test_static_viewer_html_readonly_and_escaped`.
