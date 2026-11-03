//! The OMF 2 files published with omf-rust (github.com/gmggroup/omf-rust, MIT), read by `fmine_io::omf` and checked
//! against the values their generators wrote (`BD-194`). omf-rust writes Parquet with parquet-rs, so these files come
//! from an encoder independent of this reader and its test writer.
//!
//! The files are not in this repository. Set `FMINE_OMF_DIR` to an omf-rust checkout and run with `--ignored`:
//! `FMINE_OMF_DIR=/path/to/omf-rust cargo test -p fmine-io --test omf_samples -- --ignored`. The constants below are
//! the vendor's own, from its `tests/integration_tests.rs`, `examples/*/main.rs` and
//! `omf-python/scripts/generate_test_data.rs`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use fmine_io::omf::{
    self, Attribute, AttributeData, Boundaries, Colormap, Element, FreeformSubblock, Geometry,
    Grid2, Grid3, Json, Location, Numbers, Orient2, Project, RegularSubblock, SubblockMode,
    Subblocks, Vectors,
};

const PYRAMID_VERTICES: [[f32; 3]; 5] = [
    [-1.0, -1.0, 0.0],
    [1.0, -1.0, 0.0],
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
];
const PYRAMID_TRIANGLES: [[u32; 3]; 6] = [
    [0, 1, 4],
    [1, 2, 4],
    [2, 3, 4],
    [3, 0, 4],
    [0, 2, 1],
    [0, 3, 2],
];
const PYRAMID_SEGMENTS: [[u32; 2]; 8] = [
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 0],
    [0, 4],
    [1, 4],
    [2, 4],
    [3, 4],
];
const CUBE_VERTICES: [[f64; 3]; 8] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [1.0, 0.0, 1.0],
    [1.0, 1.0, 1.0],
    [0.0, 1.0, 1.0],
];
const CUBE_TRIANGLES: [[u32; 3]; 12] = [
    [0, 2, 1],
    [0, 3, 2],
    [0, 1, 5],
    [0, 5, 4],
    [1, 2, 6],
    [1, 6, 5],
    [2, 3, 7],
    [2, 7, 6],
    [3, 0, 4],
    [3, 4, 7],
    [4, 5, 6],
    [4, 6, 7],
];
const CUBE_SEGMENTS: [[u32; 2]; 12] = [
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 0],
    [0, 4],
    [1, 5],
    [2, 6],
    [3, 7],
    [4, 5],
    [5, 6],
    [6, 7],
    [7, 4],
];
const OCTREE_SUBBLOCKS: [([u32; 3], [u32; 6]); 11] = [
    ([0, 0, 0], [0, 0, 0, 4, 4, 4]),
    ([1, 0, 0], [0, 0, 0, 4, 4, 4]),
    ([0, 1, 0], [0, 0, 0, 4, 4, 4]),
    ([1, 1, 0], [0, 0, 0, 4, 4, 4]),
    ([0, 0, 1], [0, 0, 0, 4, 4, 4]),
    ([1, 0, 1], [0, 0, 0, 4, 4, 4]),
    ([0, 1, 1], [0, 0, 0, 4, 4, 4]),
    ([1, 1, 1], [0, 0, 0, 2, 2, 2]),
    ([1, 1, 1], [0, 2, 0, 1, 3, 1]),
    ([1, 1, 1], [2, 0, 0, 3, 1, 1]),
    ([1, 1, 1], [0, 0, 2, 1, 1, 3]),
];
const FREEFORM_SUBBLOCKS: [([u32; 3], [f32; 6]); 10] = [
    ([0, 0, 0], [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]),
    ([1, 0, 0], [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]),
    ([0, 1, 0], [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]),
    ([1, 1, 0], [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]),
    ([0, 0, 1], [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]),
    ([1, 0, 1], [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]),
    ([0, 1, 1], [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]),
    ([1, 1, 1], [0.0, 0.0, 0.0, 1.0, 1.0, 0.3333]),
    ([1, 1, 1], [0.0, 0.0, 0.3333, 0.75, 0.75, 0.6666]),
    ([1, 1, 1], [0.0, 0.0, 0.6666, 0.5, 0.5, 1.0]),
];
const I: [f64; 3] = [1.0, 0.0, 0.0];
const J: [f64; 3] = [0.0, 1.0, 0.0];
const K: [f64; 3] = [0.0, 0.0, 1.0];
/// 2000-01-01T00:00:00Z and one hour, in microseconds.
const Y2K: i64 = 946_684_800_000_000;
const HOUR: i64 = 3_600_000_000;
/// 1995-05-01, 1996-06-01, 1997-07-01 and 1998-08-01, in days; then each at 05:01, 06:01, 07:01 and 08:01 UTC, in
/// microseconds (computed with CPython's `datetime`).
const DAYS: [i32; 4] = [9251, 9648, 10_043, 10_439];
const MICROS: [i64; 4] = [
    799_304_460_000_000,
    833_608_860_000_000,
    867_740_460_000_000,
    901_958_460_000_000,
];

fn path(rel: &str) -> PathBuf {
    let dir = std::env::var_os("FMINE_OMF_DIR").expect("set FMINE_OMF_DIR to an omf-rust checkout");
    PathBuf::from(dir).join(rel)
}

fn load(rel: &str) -> Project {
    omf::read_path(&path(rel)).unwrap_or_else(|e| panic!("{rel}: {e:?}"))
}

fn widen<const N: usize>(rows: &[[f32; N]]) -> Vec<[f64; N]> {
    rows.iter().map(|r| r.map(f64::from)).collect()
}

fn attr<'e>(e: &'e Element, name: &str) -> &'e Attribute {
    e.attributes
        .iter()
        .find(|a| a.name == name)
        .unwrap_or_else(|| panic!("{}: no attribute {name}", e.name))
}

fn meta<'m>(m: &'m [(String, Json)], key: &str) -> &'m Json {
    m.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .unwrap_or_else(|| panic!("no metadata {key}"))
}

fn num(s: &str) -> Json {
    Json::Number(s.to_owned())
}

#[test]
#[ignore = "needs omf-rust's sample files: set FMINE_OMF_DIR and run with --ignored"]
fn one_of_everything_reads_as_the_vendor_built_it() {
    let p = load("tests/one_of_everything.omf");
    assert!(
        p.format.starts_with("Open Mining Format 2."),
        "{}",
        p.format
    );
    assert_eq!(
        (
            p.name.as_str(),
            p.author.as_str(),
            p.date.as_str(),
            p.origin
        ),
        (
            "One of everything",
            "Tim Evans",
            "1970-01-01T00:00:00Z",
            [0.0; 3]
        )
    );
    assert_eq!(
        p.description,
        "An OMF 2.0 project containing roughly one of every different type."
    );
    assert_eq!(meta(&p.metadata, "null"), &Json::Null);
    assert_eq!(meta(&p.metadata, "bool"), &Json::Bool(true));
    assert_eq!(
        meta(&p.metadata, "string"),
        &Json::String("value".to_owned())
    );
    assert_eq!(meta(&p.metadata, "number"), &num("42"));
    assert_eq!(
        meta(&p.metadata, "array"),
        &Json::Array(vec![num("1"), num("2"), num("3")])
    );
    assert_eq!(
        meta(&p.metadata, "object"),
        &Json::Object(vec![("a".to_owned(), num("1")), ("b".to_owned(), num("2"))])
    );
    let element_names: Vec<&str> = p.elements.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        element_names,
        [
            "Pyramid surface",
            "Pyramid points",
            "Pyramid lines",
            "Pyramid grid surface",
            "Regular block model",
            "Tensor block model",
            "Sub-blocked block model, regular",
            "Sub-blocked block model, free-form",
            "Composite",
            "Textured"
        ]
    );
    let e = &p.elements;

    // The surface, its numbers, colors and date-times with a discrete colormap.
    assert_eq!(
        e[0].geometry,
        Geometry::Surface {
            origin: [0.0; 3],
            vertices: widen(&PYRAMID_VERTICES),
            triangles: PYRAMID_TRIANGLES.to_vec()
        }
    );
    assert_eq!(
        (e[0].description.as_str(), e[0].color),
        ("A surface forming a pyramid", Some([255, 128, 0, 255]))
    );
    let a = attr(&e[0], "Numbers");
    assert_eq!(a.location, Location::Vertices);
    assert_eq!(
        a.data,
        AttributeData::Number {
            values: Numbers::Float(vec![Some(1.0), Some(2.0), Some(3.0), Some(4.0), None]),
            colormap: None
        }
    );
    let a = attr(&e[0], "Colors");
    assert_eq!(a.location, Location::Primitives);
    assert_eq!(
        a.data,
        AttributeData::Color {
            values: [
                [255, 0, 0, 255],
                [255, 255, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [255, 255, 255, 255],
                [255, 255, 255, 255]
            ]
            .map(Some)
            .to_vec()
        }
    );
    assert_eq!(
        attr(&e[0], "Date-times").data,
        AttributeData::Number {
            values: Numbers::DateTime((0..5).map(|h| Some(Y2K + h * HOUR)).collect()),
            colormap: Some(Colormap::Discrete {
                // `Boundary::Less` is exclusive and `Boundary::LessEqual` inclusive.
                boundaries: Boundaries::DateTime(vec![
                    (Y2K + HOUR, false),
                    (Y2K + 2 * HOUR, false),
                    (Y2K + 3 * HOUR, true)
                ]),
                gradient: vec![
                    [0, 0, 255, 255],
                    [0, 255, 0, 255],
                    [255, 0, 0, 255],
                    [255, 255, 255, 255]
                ]
            })
        }
    );

    // The point set, with categories that have their own attribute, and nullable 2-D and 3-D vectors.
    assert_eq!(
        e[1].geometry,
        Geometry::PointSet {
            origin: [0.0; 3],
            vertices: widen(&PYRAMID_VERTICES)
        }
    );
    let a = attr(&e[1], "Categories");
    assert_eq!(
        (a.description.as_str(), a.units.as_str(), a.location),
        (
            "Divides the points into top and base.",
            "whatever",
            Location::Vertices
        )
    );
    assert_eq!(
        a.metadata,
        vec![("key".to_owned(), Json::String("value".to_owned()))]
    );
    let AttributeData::Category {
        values,
        names,
        gradient,
        attributes,
    } = &a.data
    else {
        panic!("Categories is {:?}", a.data);
    };
    assert_eq!(values, &[0, 0, 0, 0, 1].map(Some).to_vec());
    assert_eq!(names, &["Base", "Top"]);
    assert_eq!(
        gradient,
        &Some(vec![[255, 128, 0, 255], [0, 128, 255, 255]])
    );
    assert_eq!(attributes.len(), 1);
    assert_eq!(
        (attributes[0].name.as_str(), attributes[0].location),
        ("Layer", Location::Categories)
    );
    assert_eq!(
        attributes[0].data,
        AttributeData::Number {
            values: Numbers::Integer(vec![Some(1), Some(2)]),
            colormap: None
        }
    );
    assert_eq!(
        attr(&e[1], "2D Vectors").data,
        AttributeData::Vector {
            values: Vectors::Two(vec![
                Some([1.0, 0.0]),
                Some([1.0, 1.0]),
                Some([0.0, 1.0]),
                Some([0.0, 0.0]),
                None
            ])
        }
    );
    assert_eq!(
        attr(&e[1], "3D Vectors").data,
        AttributeData::Vector {
            values: Vectors::Three(vec![None, None, None, None, Some([0.0, 0.0, 1.0])])
        }
    );

    // The line set, with text on four of its segments.
    assert_eq!(
        e[2].geometry,
        Geometry::LineSet {
            origin: [0.0; 3],
            vertices: widen(&PYRAMID_VERTICES),
            segments: PYRAMID_SEGMENTS.to_vec()
        }
    );
    let a = attr(&e[2], "Strings");
    assert_eq!(a.location, Location::Primitives);
    let text = |s: &str| Some(s.to_owned());
    assert_eq!(
        a.data,
        AttributeData::Text {
            values: vec![
                None,
                None,
                None,
                None,
                text("sw"),
                text("se"),
                text("ne"),
                text("nw")
            ]
        }
    );

    // The tensor grid surface with 32-bit heights.
    assert_eq!(
        e[3].geometry,
        Geometry::GridSurface {
            orient: Orient2 {
                origin: [-1.5, -1.5, 0.0],
                u: I,
                v: J
            },
            grid: Grid2::Tensor {
                u: vec![1.0, 2.0],
                v: vec![1.5, 1.5]
            },
            heights: Some(vec![0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0])
        }
    );

    // The four block models.
    let bm = |i: usize| match &e[i].geometry {
        Geometry::BlockModel(b) => b,
        other => panic!("{}: {other:?}", e[i].name),
    };
    for i in 4..8 {
        assert_eq!(
            (
                bm(i).orient.origin,
                bm(i).orient.u,
                bm(i).orient.v,
                bm(i).orient.w
            ),
            ([-1.0; 3], I, J, K)
        );
    }
    let regular = Grid3::Regular {
        size: [1.0; 3],
        count: [2; 3],
    };
    assert_eq!((&bm(4).grid, &bm(4).subblocks), (&regular, &None));
    assert_eq!(
        attr(&e[4], "Filter").data,
        AttributeData::Boolean {
            values: [false, false, false, false, false, false, false, true]
                .map(Some)
                .to_vec()
        }
    );
    assert_eq!(
        bm(5).grid,
        Grid3::Tensor {
            u: vec![0.6666, 1.333],
            v: vec![0.6666, 1.333],
            w: vec![1.0, 1.0]
        }
    );
    assert_eq!(bm(6).grid, regular);
    assert_eq!(
        bm(6).subblocks,
        Some(Subblocks::Regular {
            count: [4, 4, 4],
            mode: Some(SubblockMode::Octree),
            blocks: OCTREE_SUBBLOCKS
                .iter()
                .map(|(p, c)| RegularSubblock {
                    parent: *p,
                    min: [c[0], c[1], c[2]],
                    max: [c[3], c[4], c[5]]
                })
                .collect()
        })
    );
    assert_eq!(
        bm(7).subblocks,
        Some(Subblocks::Freeform {
            blocks: FREEFORM_SUBBLOCKS
                .iter()
                .map(|(p, c)| {
                    let c = c.map(f64::from);
                    FreeformSubblock {
                        parent: *p,
                        min: [c[0], c[1], c[2]],
                        max: [c[3], c[4], c[5]],
                    }
                })
                .collect()
        })
    );

    // The composite of a cube's faces and edges.
    let Geometry::Composite { elements } = &e[8].geometry else {
        panic!("Composite is {:?}", e[8].geometry);
    };
    assert_eq!(
        elements
            .iter()
            .map(|c| (c.name.as_str(), &c.geometry))
            .collect::<Vec<_>>(),
        [
            (
                "Cube faces",
                &Geometry::Surface {
                    origin: [0.0; 3],
                    vertices: CUBE_VERTICES.to_vec(),
                    triangles: CUBE_TRIANGLES.to_vec()
                }
            ),
            (
                "Cube edges",
                &Geometry::LineSet {
                    origin: [0.0; 3],
                    vertices: CUBE_VERTICES.to_vec(),
                    segments: CUBE_SEGMENTS.to_vec()
                }
            )
        ]
    );

    // The rectangle with a projected and a mapped texture of one image.
    let a = attr(&e[9], "Projected");
    let AttributeData::ProjectedTexture {
        image,
        orient,
        width,
        height,
    } = &a.data
    else {
        panic!("Projected is {:?}", a.data);
    };
    assert_eq!(a.location, Location::Projected);
    assert!(image.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(
        (orient, *width, *height),
        (
            &Orient2 {
                origin: [0.0; 3],
                u: I,
                v: J
            },
            1.0,
            1.0
        )
    );
    assert_eq!(
        attr(&e[9], "Mapped").data,
        AttributeData::MappedTexture {
            image: image.clone(),
            texcoords: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
        }
    );

    // omf-python keeps its own copy, written by a later build; the same project, element for element.
    let q = load("omf-python/tests/data/one_of_everything.omf");
    assert_eq!(
        q.elements
            .iter()
            .map(|e| e.name.as_str())
            .collect::<Vec<_>>(),
        element_names
    );
    assert_eq!(q.elements, p.elements);
}

#[test]
#[ignore = "needs omf-rust's sample files: set FMINE_OMF_DIR and run with --ignored"]
fn the_examples_read_as_their_sources_wrote_them() {
    let p = load("examples/pyramid/pyramid.omf");
    assert_eq!(
        (
            p.name.as_str(),
            p.description.as_str(),
            p.author.as_str(),
            p.application.as_str()
        ),
        (
            "Pyramid",
            "Contains a square pyramid.",
            "Somebody",
            "OMF Rust example"
        )
    );
    assert_eq!(meta(&p.metadata, "revision"), &num("1.2"));
    assert_eq!(
        meta(&p.metadata, "tags"),
        &Json::Array(vec![
            Json::String("foo".to_owned()),
            Json::String("bar".to_owned())
        ])
    );
    assert_eq!(p.elements.len(), 2);
    assert_eq!(
        (p.elements[0].name.as_str(), p.elements[0].color),
        ("Pyramid surface", Some([255, 255, 0, 255]))
    );
    assert_eq!(
        p.elements[0].geometry,
        Geometry::Surface {
            origin: [0.0; 3],
            vertices: widen(&PYRAMID_VERTICES),
            triangles: PYRAMID_TRIANGLES.to_vec()
        }
    );
    assert_eq!(
        (p.elements[1].name.as_str(), p.elements[1].color),
        ("Pyramid outline", Some([0, 0, 0, 255]))
    );
    assert_eq!(
        p.elements[1].geometry,
        Geometry::LineSet {
            origin: [0.0; 3],
            vertices: widen(&PYRAMID_VERTICES),
            segments: PYRAMID_SEGMENTS.to_vec()
        }
    );

    // The bunny: its source mesh is not published, so only its shape is checked; every triangle's indices were
    // checked against the vertex count as it was read.
    let p = load("examples/bunny/bunny.omf");
    assert_eq!(
        p.description,
        "The Stanford bunny, with the holes patched, and rotated to make Z up."
    );
    assert_eq!(
        meta(&p.metadata, "source"),
        &Json::String("Stanford University Computer Graphics Laboratory".to_owned())
    );
    let Geometry::Surface {
        vertices,
        triangles,
        ..
    } = &p.elements[0].geometry
    else {
        panic!("bunny is {:?}", p.elements[0].geometry);
    };
    assert!(!vertices.is_empty() && !triangles.is_empty());
    println!(
        "bunny: {} vertices, {} triangles",
        vertices.len(),
        triangles.len()
    );
}

/// The octree bunny against the CSV it was made from: each sub-block's centroid, size and category, and, for each
/// body block, the centroid the vendor's own reader wrote out.
#[test]
#[ignore = "needs omf-rust's sample files: set FMINE_OMF_DIR and run with --ignored"]
fn the_octree_bunny_matches_its_source_csv() {
    let p = load("examples/bunny_blocks/bunny_blocks.omf");
    assert_eq!(
        p.description,
        "The Stanford bunny, as an octree block model."
    );
    let e = &p.elements[0];
    assert_eq!(e.name, "Octree Block Model");
    let Geometry::BlockModel(b) = &e.geometry else {
        panic!("{:?}", e.geometry);
    };
    assert_eq!(b.orient.origin, [-1662.0, -1340.0, -423.0]);
    // A 10° turn about Z, as glam's `from_axis_angle` computed it.
    assert_eq!(
        b.orient.u,
        [0.984_807_753_012_208, 0.173_648_177_666_930_33, 0.0]
    );
    assert_eq!(
        b.orient.v,
        [-0.173_648_177_666_930_33, 0.984_807_753_012_208, 0.0]
    );
    assert_eq!(b.orient.w, K);
    assert_eq!(
        b.grid,
        Grid3::Regular {
            size: [400.0; 3],
            count: [8, 6, 8]
        }
    );
    let Some(Subblocks::Regular {
        count: [4, 4, 4],
        mode: Some(SubblockMode::Octree),
        blocks,
    }) = &b.subblocks
    else {
        panic!("{:?}", b.subblocks);
    };
    let a = attr(e, "Bunny");
    let AttributeData::Category {
        values,
        names,
        gradient,
        attributes,
    } = &a.data
    else {
        panic!("{:?}", a.data);
    };
    assert_eq!(a.location, Location::Subblocks);
    assert_eq!(names, &["Air", "Body"]);
    assert_eq!(
        gradient,
        &Some(vec![[173, 216, 230, 255], [232, 193, 138, 255]])
    );
    assert!(attributes.is_empty());

    // The vendor's transform, in its order of operations: corners on the parent grid, scaled by the sub-block size,
    // then u·x + v·y + w·z (glam's matrix product), then the origin.
    let place = |s: &RegularSubblock| {
        let corner = |c: [u32; 3]| -> [f64; 3] {
            std::array::from_fn(|i| f64::from(s.parent[i] * 4 + c[i]) * 100.0)
        };
        let (lo, hi) = (corner(s.min), corner(s.max));
        let size: [f64; 3] = std::array::from_fn(|i| hi[i] - lo[i]);
        let c: [f64; 3] = std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0);
        let (u, v, w, o) = (b.orient.u, b.orient.v, b.orient.w, b.orient.origin);
        let centroid: [f64; 3] =
            std::array::from_fn(|i| u[i] * c[0] + v[i] * c[1] + w[i] * c[2] + o[i]);
        (centroid, size)
    };
    let csv = std::fs::read_to_string(path("examples/bunny_blocks/bunny_blocks.csv")).unwrap();
    let rows: Vec<Vec<&str>> = csv
        .lines()
        .skip(1)
        .map(|l| l.split(',').collect())
        .collect();
    assert_eq!((rows.len(), blocks.len(), values.len()), (5123, 5123, 5123));
    let mut body = Vec::new();
    let mut worst = 0.0_f64;
    for (r, (row, (s, v))) in rows.iter().zip(blocks.iter().zip(values)).enumerate() {
        let f = |i: usize| row[i].parse::<f64>().unwrap();
        let (centroid, size) = place(s);
        for i in 0..3 {
            worst = worst.max((centroid[i] - f(1 + i)).abs());
            assert_eq!(size[i], f(4 + i), "row {r}");
        }
        assert_eq!(*v, Some(u32::from(row[7] == "Body")), "row {r}");
        if row[7] == "Body" {
            body.push(centroid);
        }
    }
    // The CSV's centroids were rounded onto the sub-block grid when the file was written.
    assert!(
        worst < 1e-6,
        "centroids differ from the CSV by up to {worst}"
    );
    println!(
        "bunny_blocks: {} sub-blocks, {} body; worst centroid difference {worst:e}",
        blocks.len(),
        body.len()
    );

    let out =
        std::fs::read_to_string(path("examples/bunny_blocks/bunny_centroids_out.csv")).unwrap();
    let vendor: Vec<[f64; 3]> = out
        .lines()
        .skip(1)
        .map(|l| {
            let f: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
            [f[0], f[1], f[2]]
        })
        .collect();
    assert_eq!(vendor.len(), body.len());
    // Rust prints the shortest text that parses back to the same f64, so this is bit-for-bit.
    for (k, (mine, theirs)) in body.iter().zip(&vendor).enumerate() {
        assert_eq!(mine, theirs, "body block {k}");
    }
}

#[test]
#[ignore = "needs omf-rust's sample files: set FMINE_OMF_DIR and run with --ignored"]
fn omf_python_test_files_read_or_are_refused_as_the_vendor_reads_them() {
    let data = |name: &str| format!("omf-python/tests/data/{name}.omf");

    let p = load(&data("continuous_colormap"));
    let s = &p.elements[0];
    assert_eq!(s.name, "Test Surface");
    let continuous = |a: &Attribute| match &a.data {
        AttributeData::Number {
            values,
            colormap: Some(Colormap::Continuous { min, max, gradient }),
        } => {
            assert_eq!(
                gradient,
                &vec![[0, 0, 255, 255], [0, 255, 0, 255], [255, 0, 0, 255]]
            );
            (values.clone(), min.clone(), max.clone())
        }
        other => panic!("{}: {other:?}", a.name),
    };
    let floats = |v: [f64; 4]| Numbers::Float(v.map(Some).to_vec());
    let (v, min, max) = continuous(attr(s, "Elevation"));
    assert_eq!(
        (v, min.f64(), max.f64()),
        (floats([0.0, 1.0, 2.0, 1.5]), Some(0.0), Some(2.0))
    );
    let (v, min, max) = continuous(attr(s, "DateRange"));
    assert_eq!(v, Numbers::Date(DAYS.map(Some).to_vec()));
    assert_eq!(
        (min.as_str(), max.as_str()),
        (Some("1995-05-01"), Some("1998-08-01"))
    );
    let (v, min, max) = continuous(attr(s, "DateTimeRange"));
    assert_eq!(v, Numbers::DateTime(MICROS.map(Some).to_vec()));
    assert_eq!(
        (min.as_str(), max.as_str()),
        (Some("1995-05-01T05:01:00Z"), Some("1998-08-01T08:01:00Z"))
    );
    let rest: Vec<_> = s.attributes[3..].iter().map(continuous).collect();
    assert_eq!(rest.len(), 2);
    assert_eq!(
        (&rest[0].0, rest[0].1.f64(), rest[0].2.f64()),
        (&floats([0.0, 1.0, 2.0, 1.5]), Some(0.0), Some(2.0))
    );
    assert_eq!(
        (&rest[1].0, rest[1].1.i64(), rest[1].2.i64()),
        (
            &Numbers::Integer([0, 100, 200, 150].map(Some).to_vec()),
            Some(0),
            Some(200)
        )
    );

    let p = load(&data("null_attribute_values"));
    let got: Vec<&AttributeData> = p.elements[0].attributes.iter().map(|a| &a.data).collect();
    let number = |values: Numbers| AttributeData::Number {
        values,
        colormap: None,
    };
    let with_null = |v: [f64; 3]| Numbers::Float(vec![Some(v[0]), Some(v[1]), None, Some(v[2])]);
    assert_eq!(
        got,
        [
            &number(with_null([0.0, 1.0, 1.5])),
            &number(with_null([0.0, 1.0, 1.5])),
            &number(Numbers::Integer(vec![Some(0), Some(100), None, Some(150)])),
            &number(Numbers::Date(vec![
                Some(DAYS[0]),
                Some(DAYS[1]),
                None,
                Some(DAYS[3])
            ])),
            &number(Numbers::DateTime(vec![
                Some(MICROS[0]),
                Some(MICROS[1]),
                None,
                Some(MICROS[3])
            ])),
            &AttributeData::Category {
                values: vec![Some(0), Some(1), None, Some(2)],
                names: vec!["Zero".to_owned(), "One".to_owned(), "Two".to_owned()],
                gradient: Some(vec![[0, 0, 255, 255], [0, 255, 0, 255], [255, 0, 0, 255]]),
                attributes: vec![]
            },
            &AttributeData::Boolean {
                values: vec![Some(false), Some(true), None, Some(false)]
            },
            &AttributeData::Color {
                values: vec![
                    Some([0, 0, 255, 255]),
                    Some([0, 255, 0, 255]),
                    None,
                    Some([255, 0, 0, 255])
                ]
            },
        ]
    );

    // A duplicate name is a vendor warning, not an error.
    let p = load(&data("duplicate_element_name"));
    assert_eq!(
        p.elements
            .iter()
            .map(|e| e.name.as_str())
            .collect::<Vec<_>>(),
        ["Duplicate", "Duplicate"]
    );

    let p = load(&data("element_metadata"));
    let m = &p.elements[0].metadata;
    assert_eq!(meta(m, "sub-type"), &Json::String("point".to_owned()));
    assert_eq!(
        meta(m, "date_created"),
        &Json::String("2024-10-14T00:00:00Z".to_owned())
    );
    assert_eq!(
        meta(m, "date_modified"),
        &Json::String("2024-10-15T00:00:00Z".to_owned())
    );

    // The two the vendor reader refuses.
    let refused = |name: &str, why: &str| {
        let e = omf::read_path(&path(&data(name))).unwrap_err();
        assert_eq!(e.code, "FMINE-REF-506");
        assert!(e.what.contains(why), "{name}: {}", e.what);
    };
    refused(
        "missing_parquet",
        "refers to non-existent archive member '1.parquet'",
    );
    refused(
        "array_length_mismatch",
        "does not match the declared length 999",
    );
}
