//! The OMF 2 reader (`fmine_io::omf`: the stored-only zip reader with its CRC-32 check, the gzip index, the JSON
//! parser, the Parquet subset and the OMF checks).
//!
//! Input: the members of a would-be OMF file, not the file, because each member carries a CRC-32 that almost any
//! mutated byte would break. First the index as JSON text (a `u32` LE length, then the text), then any number of
//! members (a `u8` name length, the name, a `u32` LE length, the bytes); a length past the end takes what is left.
//! The target stores the index as a gzip member named `index.json.gz`, zips every member stored with true CRC-32s
//! under the comment "Open Mining Format 2.0" and reads the file. A project that is read must hold only the
//! indices the reader promises to check: vertex indices below the vertex count, category indices below the names,
//! and sub-blocks inside their parents.
#![no_main]

use fmine_io::omf::{Attribute, AttributeData, Element, Geometry, Subblocks, read};
use libfuzzer_sys::fuzz_target;

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

/// A gzip member whose DEFLATE stream is stored blocks.
fn gzip_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 0xff];
    let mut blocks: Vec<&[u8]> = data.chunks(65_535).collect();
    if blocks.is_empty() {
        blocks.push(&[]);
    }
    for (i, b) in blocks.iter().enumerate() {
        out.push(u8::from(i + 1 == blocks.len()));
        out.extend_from_slice(&(b.len() as u16).to_le_bytes());
        out.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        out.extend_from_slice(b);
    }
    out.extend_from_slice(&crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

/// A zip of stored members with true CRC-32s and `comment`.
fn stored_zip(members: &[(Vec<u8>, Vec<u8>)], comment: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in members {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        let name_len = name.len() as u16;
        out.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(data);
        central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let central_offset = out.len() as u32;
    let count = members.len() as u16;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&(comment.len() as u16).to_le_bytes());
    out.extend_from_slice(comment);
    out
}

/// The next `n` bytes, or what is left.
fn take<'a>(data: &mut &'a [u8], n: usize) -> &'a [u8] {
    let (head, tail) = data.split_at(n.min(data.len()));
    *data = tail;
    head
}

fn take_u32(data: &mut &[u8]) -> usize {
    let mut b = [0_u8; 4];
    let head = take(data, 4);
    b[..head.len()].copy_from_slice(head);
    u32::from_le_bytes(b) as usize
}

fn check_attributes(attributes: &[Attribute]) {
    for a in attributes {
        if let AttributeData::Category {
            values,
            names,
            attributes,
            ..
        } = &a.data
        {
            for v in values.iter().flatten() {
                assert!(
                    (*v as usize) < names.len(),
                    "category {v} of {} names",
                    names.len()
                );
            }
            check_attributes(attributes);
        }
        let _ = a.data.item_count();
    }
}

fn check(elements: &[Element]) {
    for e in elements {
        check_attributes(&e.attributes);
        match &e.geometry {
            Geometry::LineSet {
                vertices, segments, ..
            } => {
                for s in segments {
                    assert!(
                        s.iter().all(|&i| (i as usize) < vertices.len()),
                        "segment {s:?}"
                    );
                }
            }
            Geometry::Surface {
                vertices,
                triangles,
                ..
            } => {
                for t in triangles {
                    assert!(
                        t.iter().all(|&i| (i as usize) < vertices.len()),
                        "triangle {t:?}"
                    );
                }
            }
            Geometry::BlockModel(m) => {
                let parents = m.grid.count();
                match &m.subblocks {
                    Some(Subblocks::Regular { count, blocks, .. }) => {
                        for b in blocks {
                            for i in 0..3 {
                                assert!(b.parent[i] < parents[i], "parent {:?}", b.parent);
                                assert!(
                                    b.min[i] < b.max[i] && b.max[i] <= count[i],
                                    "sub-block {b:?}"
                                );
                            }
                        }
                    }
                    Some(Subblocks::Freeform { blocks }) => {
                        for b in blocks {
                            for i in 0..3 {
                                assert!(b.parent[i] < parents[i], "parent {:?}", b.parent);
                                assert!(
                                    0.0 <= b.min[i] && b.min[i] < b.max[i] && b.max[i] <= 1.0,
                                    "sub-block {b:?}"
                                );
                            }
                        }
                    }
                    None => {}
                }
            }
            Geometry::Composite { elements } => check(elements),
            Geometry::PointSet { .. } | Geometry::GridSurface { .. } => {}
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let mut rest = data;
    let n = take_u32(&mut rest);
    let index = take(&mut rest, n);
    let mut members = vec![(b"index.json.gz".to_vec(), gzip_stored(index))];
    while !rest.is_empty() {
        let name_len = take(&mut rest, 1).first().copied().unwrap_or(0) as usize;
        let name = take(&mut rest, name_len).to_vec();
        let n = take_u32(&mut rest);
        members.push((name, take(&mut rest, n).to_vec()));
    }
    if let Ok(project) = read(&stored_zip(&members, b"Open Mining Format 2.0")) {
        check(&project.elements);
    }
});
