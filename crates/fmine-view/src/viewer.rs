//! Determinism: BitwisePortable
//! Strictly read-only static HTML viewer shell for exported glTF 2.0 / GLB 2.0 scenes (`BD-256`).
//! Holds zero write/authority endpoints and escapes all untrusted text against HTML injection.

use crate::VIEW_NO_CLAIM;
use crate::gltf::{GltfExport, crs_label};

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn json_for_script_tag(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    for ch in json.chars() {
        match ch {
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            c => out.push(c),
        }
    }
    out
}

/// Render a self-contained, strictly read-only static HTML page inspecting `export` (`BD-256`).
#[must_use]
pub fn render_static_viewer_html(export: &GltfExport) -> String {
    let title = html_escape(&export.scene_name);
    let no_claim = html_escape(VIEW_NO_CLAIM);
    let crs_str = html_escape(&crs_label(export.crs_id));
    let digest_hex = export.scene_sha256.to_hex();
    let gltf_script_json = json_for_script_tag(&export.gltf_json);

    format!(
        "<!doctype html>\n\
<html lang=\"en\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<title>FrankenMining Read-Only Scene: {title}</title>\n\
<style>body{{font-family:monospace;background:#111;color:#eee;margin:2rem;}} \
.banner{{border:1px solid #d97706;padding:0.75rem;margin-bottom:1rem;background:#1f1608;}}</style>\n\
</head>\n\
<body>\n\
<h1>{title} [READ-ONLY — NO AUTHORITY PATH]</h1>\n\
<div class=\"banner\"><strong>NO CLAIM:</strong> {no_claim}</div>\n\
<ul>\n\
<li>CRS: {crs_str}</li>\n\
<li>Vertices: {verts}</li>\n\
<li>Triangles: {tris}</li>\n\
<li>GLB SHA-256: <code>{digest_hex}</code></li>\n\
</ul>\n\
<script id=\"fmine-gltf-scene\" type=\"application/json\">{gltf_script_json}</script>\n\
</body>\n\
</html>\n",
        verts = export.vertex_count,
        tris = export.triangle_count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gltf::{GltfSceneInput, export_gltf_scene};
    use crate::voxel_mesh::{VoxelGridFrame, extract_voxel_surface_mesh};
    use fmine_crs::CrsId;
    use fmine_geom::vdb::VdbGrid;

    #[test]
    fn test_static_viewer_html_readonly_and_escaped() {
        let mut grid = VdbGrid::new(0u8);
        grid.set([0, 0, 0], 1);
        let frame = VoxelGridFrame {
            origin_m: [0.0, 0.0, 0.0],
            voxel_size_m: [5.0, 5.0, 5.0],
            crs_id: CrsId::REKO_DIQ_UTM41N,
        };
        let mesh =
            extract_voxel_surface_mesh("block", &grid, &frame, [0.2, 0.6, 0.9, 1.0], None).unwrap();
        let export = export_gltf_scene(&GltfSceneInput {
            scene_name: "pit <script>alert(1)</script>".to_string(),
            meshes: vec![mesh],
        })
        .unwrap();

        let html = render_static_viewer_html(&export);
        assert!(html.contains("READ-ONLY — NO AUTHORITY PATH"));
        assert!(html.contains("NO CLAIM:"));
        assert!(html.contains("CRS: EPSG:32641"));
        assert!(!html.contains("Wgs84Utm"));
        assert!(html.contains(&export.scene_sha256.to_hex()));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }
}
