//! OMF files built here, member by member, read back whole; then one defect at a time, each refused.

use std::cell::Cell;

use super::gzip::gunzip;
use super::parquet::testkit::{Col, Opts, gzip_stored, nullable as ncol, required as rcol, write};
use super::parquet::{Logical, Physical, Values};
use super::*;
use crate::zip_inflate::{read_stored_entries, write_stored_zip, write_stored_zip_with_crcs};

const U32: Logical = Logical::Integer {
    bits: 32,
    signed: false,
};
const U8: Logical = Logical::Integer {
    bits: 8,
    signed: false,
};
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nnot really an image";
const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3];
const MICROS_2000: i64 = 946_684_800_000_000;

fn dbl(name: &'static str, v: &[f64]) -> Col {
    rcol(
        name,
        Physical::Double,
        Logical::None,
        Values::Double(v.to_vec()),
    )
}

fn flt(name: &'static str, v: &[f32]) -> Col {
    rcol(
        name,
        Physical::Float,
        Logical::None,
        Values::Float(v.to_vec()),
    )
}

fn u32c(name: &'static str, v: &[u32]) -> Col {
    // Parquet keeps an unsigned 32-bit integer's bits in INT32.
    rcol(
        name,
        Physical::Int32,
        U32,
        Values::Int32(v.iter().map(|&x| x as i32).collect()),
    )
}

fn u8c(name: &'static str, v: &[i32]) -> Col {
    rcol(name, Physical::Int32, U8, Values::Int32(v.to_vec()))
}

fn bools(name: &'static str, v: &[bool]) -> Col {
    rcol(
        name,
        Physical::Boolean,
        Logical::None,
        Values::Boolean(v.to_vec()),
    )
}

/// `c` moved into an optional group, with `present` rows (its values are the present ones only).
fn grouped(mut c: Col, group: &'static str, present: &[bool]) -> Col {
    c.group = Some((group, true));
    c.present = Some(present.to_vec());
    c
}

thread_local! {
    /// The page form `pq` writes; the mutation test varies it.
    static PAGES: Cell<Opts> = Cell::new(Opts::default());
}

fn pq(cols: &[Col], rows: usize) -> Vec<u8> {
    write(cols, rows, PAGES.with(Cell::get))
}

fn xyz(v: &[[f64; 3]]) -> Vec<u8> {
    let axis = |i: usize| v.iter().map(|p| p[i]).collect::<Vec<_>>();
    pq(
        &[dbl("x", &axis(0)), dbl("y", &axis(1)), dbl("z", &axis(2))],
        v.len(),
    )
}

fn gradient(rows: &[[i32; 4]]) -> Vec<u8> {
    let ch = |i: usize| rows.iter().map(|r| r[i]).collect::<Vec<_>>();
    pq(
        &[
            u8c("r", &ch(0)),
            u8c("g", &ch(1)),
            u8c("b", &ch(2)),
            u8c("a", &ch(3)),
        ],
        rows.len(),
    )
}

fn subblock_table(parents: &[[u32; 3]], corners: &[[u32; 6]]) -> Vec<u8> {
    let p = |i: usize| parents.iter().map(|x| x[i]).collect::<Vec<_>>();
    let c = |i: usize| corners.iter().map(|x| x[i]).collect::<Vec<_>>();
    pq(
        &[
            u32c("parent_u", &p(0)),
            u32c("parent_v", &p(1)),
            u32c("parent_w", &p(2)),
            u32c("corner_min_u", &c(0)),
            u32c("corner_min_v", &c(1)),
            u32c("corner_min_w", &c(2)),
            u32c("corner_max_u", &c(3)),
            u32c("corner_max_v", &c(4)),
            u32c("corner_max_w", &c(5)),
        ],
        parents.len(),
    )
}

/// The members of an OMF file under construction.
#[derive(Default)]
struct Build {
    members: Vec<(String, Vec<u8>)>,
}

impl Build {
    /// Adds a Parquet member of `rows` rows; returns its array reference.
    fn arr(&mut self, bytes: Vec<u8>, rows: usize) -> String {
        let name = format!("{}.parquet", self.members.len() + 1);
        self.members.push((name.clone(), bytes));
        format!(r#"{{"filename":"{name}","item_count":{rows}}}"#)
    }

    fn image(&mut self, bytes: &[u8], ext: &str) -> String {
        let name = format!("{}.{ext}", self.members.len() + 1);
        self.members.push((name.clone(), bytes.to_vec()));
        format!(r#"{{"filename":"{name}","item_count":0}}"#)
    }

    /// The archive, with `index` as its index and `comment` as its comment.
    fn zip_with(&self, index: &str, comment: &str) -> Vec<u8> {
        let gz = gzip_stored(index.as_bytes());
        let mut entries: Vec<(&str, &[u8])> = vec![(INDEX, &gz)];
        entries.extend(self.members.iter().map(|(n, b)| (n.as_str(), b.as_slice())));
        with_comment(write_stored_zip_with_crcs(&entries), comment)
    }

    fn file(&self, elements: &str) -> Vec<u8> {
        self.zip_with(&project(elements), "Open Mining Format 2.0")
    }
}

fn with_comment(mut zip: Vec<u8>, comment: &str) -> Vec<u8> {
    let n = zip.len();
    zip[n - 2..].copy_from_slice(&(comment.len() as u16).to_le_bytes());
    zip.extend_from_slice(comment.as_bytes());
    zip
}

fn project(elements: &str) -> String {
    format!(
        r#"{{"name":"pit","description":"d","coordinate_reference_system":"EPSG:32633","units":"m",
        "origin":[100,200,0],"author":"a","application":"t","date":"2000-01-01T00:00:00Z",
        "metadata":{{"k":1}},"unknown_key":true,"elements":[{elements}]}}"#
    )
}

fn element(name: &str, geometry: &str, attributes: &str) -> String {
    format!(r#"{{"name":"{name}","geometry":{geometry},"attributes":[{attributes}]}}"#)
}

fn attribute(name: &str, location: &str, data: &str) -> String {
    format!(r#"{{"name":"{name}","units":"u","location":"{location}","data":{data}}}"#)
}

fn point_set(b: &mut Build, vertices: &[[f64; 3]]) -> String {
    let v = b.arr(xyz(vertices), vertices.len());
    format!(r#"{{"type":"PointSet","vertices":{v}}}"#)
}

fn refused(bytes: &[u8], why: &str) {
    let e = read(bytes).unwrap_err();
    assert_eq!(e.code, "FMINE-REF-506", "{e:?}");
    assert!(e.what.contains("OMF: "), "{}", e.what);
    assert!(e.what.contains(why), "wanted {why:?} in {}", e.what);
}

const TRI: [[f64; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

const fn form(
    v2: bool,
    gzip: bool,
    rows_per_page: usize,
    rows_per_group: usize,
    split: bool,
) -> Opts {
    Opts {
        v2,
        gzip,
        rows_per_page,
        rows_per_group,
        bitpacked_levels: split,
        crc: split,
    }
}

/// The page forms besides the default (v2 pages, GZIP): v1 and v2 without a codec, v1 with GZIP, and pages and row
/// groups split small, with bit-packed levels and page CRCs.
const FORMS: [Opts; 5] = [
    form(false, false, usize::MAX, usize::MAX, false),
    form(true, false, usize::MAX, usize::MAX, false),
    form(false, true, usize::MAX, usize::MAX, false),
    form(false, true, 2, 3, true),
    form(true, false, 1, 2, true),
];

/// A file holding every geometry and attribute type, its Parquet written in the current `PAGES` form.
fn every_kind() -> Vec<u8> {
    let mut b = Build::default();

    // A point set with nullable floats and a continuous colormap.
    let pv = b.arr(xyz(&TRI), 3);
    let grade = b.arr(
        pq(
            &[ncol(
                "number",
                Physical::Double,
                Logical::None,
                vec![true, false, true],
                Values::Double(vec![1.5, 2.5]),
            )],
            3,
        ),
        3,
    );
    let ramp = b.arr(gradient(&[[0, 0, 0, 255], [255, 255, 255, 255]]), 2);
    let points = format!(
        r#"{{"name":"points","color":[1,2,3,255],"metadata":{{"m":"x"}},
        "geometry":{{"type":"PointSet","origin":[1,2,3],"vertices":{pv}}},
        "attributes":[{}]}}"#,
        attribute(
            "grade",
            "Vertices",
            &format!(
                r#"{{"type":"Number","values":{grade},"colormap":{{"type":"Continuous","range":{{"min":0,"max":10}},"gradient":{ramp}}}}}"#
            )
        )
    );

    // A line set with 32-bit vertices and text on its segments.
    let lv = b.arr(
        pq(
            &[
                flt("x", &[0.0, 0.1, 2.0]),
                flt("y", &[0.0, 1.0, 2.0]),
                flt("z", &[0.0, 1.0, -2.5]),
            ],
            3,
        ),
        3,
    );
    let seg = b.arr(pq(&[u32c("a", &[0, 1]), u32c("b", &[1, 2])], 2), 2);
    let label = b.arr(
        pq(
            &[ncol(
                "text",
                Physical::ByteArray,
                Logical::String,
                vec![true, false],
                Values::Bytes(vec![b"a".to_vec()]),
            )],
            2,
        ),
        2,
    );
    let lines = element(
        "lines",
        &format!(r#"{{"type":"LineSet","vertices":{lv},"segments":{seg}}}"#),
        &attribute(
            "label",
            "Primitives",
            &format!(r#"{{"type":"Text","values":{label}}}"#),
        ),
    );

    // A surface with colors, a mapped texture and a projected texture.
    let sv = b.arr(xyz(&TRI), 3);
    let tris = b.arr(
        pq(&[u32c("a", &[0]), u32c("b", &[1]), u32c("c", &[2])], 1),
        1,
    );
    let present = [true, false, true];
    let paint = b.arr(
        pq(
            &[
                grouped(u8c("r", &[1, 5]), "color", &present),
                grouped(u8c("g", &[2, 6]), "color", &present),
                grouped(u8c("b", &[3, 7]), "color", &present),
                grouped(u8c("a", &[4, 8]), "color", &present),
            ],
            3,
        ),
        3,
    );
    let png = b.image(PNG, "png");
    let uv = b.arr(
        pq(&[dbl("u", &[0.0, 1.0, 0.0]), dbl("v", &[0.0, 0.0, 1.0])], 3),
        3,
    );
    let jpg = b.image(JPEG, "jpg");
    let surface = element(
        "tri",
        &format!(r#"{{"type":"Surface","vertices":{sv},"triangles":{tris}}}"#),
        &[
            attribute("paint", "Vertices", &format!(r#"{{"type":"Color","values":{paint}}}"#)),
            attribute(
                "tex",
                "Vertices",
                &format!(r#"{{"type":"MappedTexture","image":{png},"texcoords":{uv}}}"#),
            ),
            attribute(
                "proj",
                "Projected",
                &format!(
                    r#"{{"type":"ProjectedTexture","image":{jpg},"orient":{{"origin":[0,0,0],"u":[1,0,0],"v":[0,1,0]}},"width":2,"height":3}}"#
                ),
            ),
        ]
        .join(","),
    );

    // A grid surface with heights and booleans on its cells.
    let heights = b.arr(pq(&[dbl("scalar", &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0])], 6), 6);
    let wet = b.arr(
        pq(
            &[ncol(
                "bool",
                Physical::Boolean,
                Logical::None,
                vec![true, false],
                Values::Boolean(vec![true]),
            )],
            2,
        ),
        2,
    );
    let grid = element(
        "grid",
        &format!(
            r#"{{"type":"GridSurface","orient":{{"origin":[0,0,0],"u":[0,1,0],"v":[-1,0,0]}},
            "grid":{{"type":"Regular","size":[1,2],"count":[2,1]}},"heights":{heights}}}"#
        ),
        &attribute(
            "wet",
            "Primitives",
            &format!(r#"{{"type":"Boolean","values":{wet}}}"#),
        ),
    );

    // A rotated block model with octree sub-blocks, categories with their own attribute, and date-times on the
    // parent blocks with a discrete colormap.
    let sub = b.arr(
        subblock_table(
            &[[0, 0, 0], [1, 0, 0], [1, 0, 0]],
            &[[0, 0, 0, 2, 2, 2], [0, 0, 0, 1, 1, 1], [1, 1, 1, 2, 2, 2]],
        ),
        3,
    );
    let rock = b.arr(
        pq(
            &[ncol(
                "index",
                Physical::Int32,
                U32,
                vec![true, true, false],
                Values::Int32(vec![0, 1]),
            )],
            3,
        ),
        3,
    );
    let names = b.arr(
        pq(
            &[rcol(
                "name",
                Physical::ByteArray,
                Logical::String,
                Values::Bytes(vec![b"ore".to_vec(), b"waste".to_vec()]),
            )],
            2,
        ),
        2,
    );
    let colors = b.arr(gradient(&[[255, 0, 0, 255], [0, 0, 255, 255]]), 2);
    let density = b.arr(
        pq(
            &[ncol(
                "number",
                Physical::Int64,
                Logical::None,
                vec![true, false],
                Values::Int64(vec![3]),
            )],
            2,
        ),
        2,
    );
    let mined = b.arr(
        pq(
            &[ncol(
                "number",
                Physical::Int64,
                Logical::TimestampMicrosUtc,
                vec![true, false],
                Values::Int64(vec![MICROS_2000]),
            )],
            2,
        ),
        2,
    );
    let bounds = b.arr(
        pq(
            &[
                rcol(
                    "value",
                    Physical::Int64,
                    Logical::TimestampMicrosUtc,
                    Values::Int64(vec![0, MICROS_2000]),
                ),
                bools("inclusive", &[false, true]),
            ],
            2,
        ),
        2,
    );
    let ramp3 = b.arr(gradient(&[[1, 1, 1, 1], [2, 2, 2, 2], [3, 3, 3, 3]]), 3);
    let blocks = element(
        "blocks",
        &format!(
            r#"{{"type":"BlockModel","orient":{{"origin":[10,20,30],"u":[0.6,0.8,0],"v":[-0.8,0.6,0],"w":[0,0,1]}},
            "grid":{{"type":"Regular","size":[10,10,5],"count":[2,1,1]}},
            "subblocks":{{"type":"Regular","count":[2,2,2],"subblocks":{sub},"mode":"Octree"}}}}"#
        ),
        &[
            format!(
                r#"{{"name":"rock","units":"u","location":"Subblocks","data":{{"type":"Category","values":{rock},"names":{names},"gradient":{colors},
                "attributes":[{}]}}}}"#,
                attribute("density", "Categories", &format!(r#"{{"type":"Number","values":{density}}}"#))
            ),
            attribute(
                "mined",
                "Primitives",
                &format!(
                    r#"{{"type":"Number","values":{mined},"colormap":{{"type":"Discrete","boundaries":{bounds},"gradient":{ramp3}}}}}"#
                ),
            ),
        ]
        .join(","),
    );

    // A tensor block model with free-form sub-blocks, dates and 2-D vectors on them.
    let tu = b.arr(pq(&[dbl("scalar", &[1.0, 2.0])], 2), 2);
    let tv = b.arr(pq(&[dbl("scalar", &[3.0])], 1), 1);
    let tw = b.arr(pq(&[flt("scalar", &[4.0, 5.0, 6.5])], 3), 3);
    let free = b.arr(
        pq(
            &[
                u32c("parent_u", &[1]),
                u32c("parent_v", &[0]),
                u32c("parent_w", &[2]),
                flt("corner_min_u", &[0.0]),
                flt("corner_min_v", &[0.0]),
                flt("corner_min_w", &[0.0]),
                flt("corner_max_u", &[0.5]),
                flt("corner_max_v", &[1.0]),
                flt("corner_max_w", &[0.25]),
            ],
            1,
        ),
        1,
    );
    let when = b.arr(
        pq(
            &[ncol(
                "number",
                Physical::Int32,
                Logical::Date,
                vec![true],
                Values::Int32(vec![10_957]),
            )],
            1,
        ),
        1,
    );
    let dir = b.arr(
        pq(
            &[
                grouped(dbl("x", &[1.0]), "vector", &[true]),
                grouped(dbl("y", &[2.0]), "vector", &[true]),
            ],
            1,
        ),
        1,
    );
    let tensor = element(
        "tensor",
        &format!(
            r#"{{"type":"BlockModel","orient":{{"origin":[0,0,0]}},
            "grid":{{"type":"Tensor","u":{tu},"v":{tv},"w":{tw}}},
            "subblocks":{{"type":"Freeform","subblocks":{free}}}}}"#
        ),
        &[
            attribute(
                "when",
                "Subblocks",
                &format!(r#"{{"type":"Number","values":{when}}}"#),
            ),
            attribute(
                "dir",
                "Subblocks",
                &format!(r#"{{"type":"Vector","values":{dir}}}"#),
            ),
        ]
        .join(","),
    );

    // A composite of two point sets, with 3-D vectors on its elements.
    let c1 = element("child", &point_set(&mut b, &[[5.0, 5.0, 5.0]]), "");
    let c2 = element("child", &point_set(&mut b, &[[6.0, 6.0, 6.0]]), "");
    let arrow = b.arr(
        pq(
            &[
                grouped(dbl("x", &[0.0]), "vector", &[true, false]),
                grouped(dbl("y", &[0.0]), "vector", &[true, false]),
                grouped(dbl("z", &[1.0]), "vector", &[true, false]),
            ],
            2,
        ),
        2,
    );
    let composite = element(
        "group",
        &format!(r#"{{"type":"Composite","elements":[{c1},{c2}]}}"#),
        &attribute(
            "arrow",
            "Elements",
            &format!(r#"{{"type":"Vector","values":{arrow}}}"#),
        ),
    );

    b.file(&[points, lines, surface, grid, blocks, tensor, composite].join(","))
}

#[test]
fn a_file_of_every_geometry_and_attribute_reads_back() {
    let p = read(&every_kind()).unwrap();

    let attr = |name: &str, location: Location, data: AttributeData| Attribute {
        name: name.to_owned(),
        description: String::new(),
        units: "u".to_owned(),
        metadata: vec![],
        location,
        data,
    };
    let el = |name: &str, attributes: Vec<Attribute>, geometry: Geometry| Element {
        name: name.to_owned(),
        description: String::new(),
        color: None,
        metadata: vec![],
        attributes,
        geometry,
    };
    let child = |v: f64| {
        el(
            "child",
            vec![],
            Geometry::PointSet {
                origin: [0.0; 3],
                vertices: vec![[v; 3]],
            },
        )
    };
    let tri_axes = Orient2 {
        origin: [0.0; 3],
        u: [1.0, 0.0, 0.0],
        v: [0.0, 1.0, 0.0],
    };
    let expected = Project {
        format: "Open Mining Format 2.0".to_owned(),
        name: "pit".to_owned(),
        description: "d".to_owned(),
        coordinate_reference_system: "EPSG:32633".to_owned(),
        units: "m".to_owned(),
        origin: [100.0, 200.0, 0.0],
        author: "a".to_owned(),
        application: "t".to_owned(),
        date: "2000-01-01T00:00:00Z".to_owned(),
        metadata: vec![("k".to_owned(), Json::Number("1".to_owned()))],
        elements: vec![
            Element {
                name: "points".to_owned(),
                description: String::new(),
                color: Some([1, 2, 3, 255]),
                metadata: vec![("m".to_owned(), Json::String("x".to_owned()))],
                attributes: vec![attr(
                    "grade",
                    Location::Vertices,
                    AttributeData::Number {
                        values: Numbers::Float(vec![Some(1.5), None, Some(2.5)]),
                        colormap: Some(Colormap::Continuous {
                            min: Json::Number("0".to_owned()),
                            max: Json::Number("10".to_owned()),
                            gradient: vec![[0, 0, 0, 255], [255, 255, 255, 255]],
                        }),
                    },
                )],
                geometry: Geometry::PointSet {
                    origin: [1.0, 2.0, 3.0],
                    vertices: TRI.to_vec(),
                },
            },
            el(
                "lines",
                vec![attr(
                    "label",
                    Location::Primitives,
                    AttributeData::Text {
                        values: vec![Some("a".to_owned()), None],
                    },
                )],
                Geometry::LineSet {
                    origin: [0.0; 3],
                    // 32-bit values widen exactly: 0.1f32 is not 0.1.
                    vertices: vec![
                        [0.0, 0.0, 0.0],
                        [f64::from(0.1_f32), 1.0, 1.0],
                        [2.0, 2.0, -2.5],
                    ],
                    segments: vec![[0, 1], [1, 2]],
                },
            ),
            el(
                "tri",
                vec![
                    attr(
                        "paint",
                        Location::Vertices,
                        AttributeData::Color {
                            values: vec![Some([1, 2, 3, 4]), None, Some([5, 6, 7, 8])],
                        },
                    ),
                    attr(
                        "tex",
                        Location::Vertices,
                        AttributeData::MappedTexture {
                            image: PNG.to_vec(),
                            texcoords: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                        },
                    ),
                    attr(
                        "proj",
                        Location::Projected,
                        AttributeData::ProjectedTexture {
                            image: JPEG.to_vec(),
                            orient: tri_axes,
                            width: 2.0,
                            height: 3.0,
                        },
                    ),
                ],
                Geometry::Surface {
                    origin: [0.0; 3],
                    vertices: TRI.to_vec(),
                    triangles: vec![[0, 1, 2]],
                },
            ),
            el(
                "grid",
                vec![attr(
                    "wet",
                    Location::Primitives,
                    AttributeData::Boolean {
                        values: vec![Some(true), None],
                    },
                )],
                Geometry::GridSurface {
                    orient: Orient2 {
                        origin: [0.0; 3],
                        u: [0.0, 1.0, 0.0],
                        v: [-1.0, 0.0, 0.0],
                    },
                    grid: Grid2::Regular {
                        size: [1.0, 2.0],
                        count: [2, 1],
                    },
                    heights: Some(vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]),
                },
            ),
            el(
                "blocks",
                vec![
                    attr(
                        "rock",
                        Location::Subblocks,
                        AttributeData::Category {
                            values: vec![Some(0), Some(1), None],
                            names: vec!["ore".to_owned(), "waste".to_owned()],
                            gradient: Some(vec![[255, 0, 0, 255], [0, 0, 255, 255]]),
                            attributes: vec![attr(
                                "density",
                                Location::Categories,
                                AttributeData::Number {
                                    values: Numbers::Integer(vec![Some(3), None]),
                                    colormap: None,
                                },
                            )],
                        },
                    ),
                    attr(
                        "mined",
                        Location::Primitives,
                        AttributeData::Number {
                            values: Numbers::DateTime(vec![Some(MICROS_2000), None]),
                            colormap: Some(Colormap::Discrete {
                                boundaries: Boundaries::DateTime(vec![
                                    (0, false),
                                    (MICROS_2000, true),
                                ]),
                                gradient: vec![[1, 1, 1, 1], [2, 2, 2, 2], [3, 3, 3, 3]],
                            }),
                        },
                    ),
                ],
                Geometry::BlockModel(BlockModel {
                    orient: Orient3 {
                        origin: [10.0, 20.0, 30.0],
                        u: [0.6, 0.8, 0.0],
                        v: [-0.8, 0.6, 0.0],
                        w: [0.0, 0.0, 1.0],
                    },
                    grid: Grid3::Regular {
                        size: [10.0, 10.0, 5.0],
                        count: [2, 1, 1],
                    },
                    subblocks: Some(Subblocks::Regular {
                        count: [2, 2, 2],
                        mode: Some(SubblockMode::Octree),
                        blocks: vec![
                            RegularSubblock {
                                parent: [0, 0, 0],
                                min: [0, 0, 0],
                                max: [2, 2, 2],
                            },
                            RegularSubblock {
                                parent: [1, 0, 0],
                                min: [0, 0, 0],
                                max: [1, 1, 1],
                            },
                            RegularSubblock {
                                parent: [1, 0, 0],
                                min: [1, 1, 1],
                                max: [2, 2, 2],
                            },
                        ],
                    }),
                }),
            ),
            el(
                "tensor",
                vec![
                    attr(
                        "when",
                        Location::Subblocks,
                        AttributeData::Number {
                            values: Numbers::Date(vec![Some(10_957)]),
                            colormap: None,
                        },
                    ),
                    attr(
                        "dir",
                        Location::Subblocks,
                        AttributeData::Vector {
                            values: Vectors::Two(vec![Some([1.0, 2.0])]),
                        },
                    ),
                ],
                Geometry::BlockModel(BlockModel {
                    orient: Orient3 {
                        origin: [0.0; 3],
                        u: I,
                        v: J,
                        w: K,
                    },
                    grid: Grid3::Tensor {
                        u: vec![1.0, 2.0],
                        v: vec![3.0],
                        w: vec![4.0, 5.0, 6.5],
                    },
                    subblocks: Some(Subblocks::Freeform {
                        blocks: vec![FreeformSubblock {
                            parent: [1, 0, 2],
                            min: [0.0; 3],
                            max: [0.5, 1.0, 0.25],
                        }],
                    }),
                }),
            ),
            el(
                "group",
                vec![attr(
                    "arrow",
                    Location::Elements,
                    AttributeData::Vector {
                        values: Vectors::Three(vec![Some([0.0, 0.0, 1.0]), None]),
                    },
                )],
                Geometry::Composite {
                    elements: vec![child(5.0), child(6.0)],
                },
            ),
        ],
    };
    assert_eq!(p, expected);
    for (k, pages) in FORMS.into_iter().enumerate() {
        PAGES.with(|c| c.set(pages));
        assert!(read(&every_kind()).unwrap() == expected, "page form {k}");
    }
    PAGES.with(|c| c.set(Opts::default()));
    // Duplicate element names, a vendor warning, are read; and so are keys OMF does not define.
    let Geometry::Composite { elements } = &p.elements[6].geometry else {
        panic!("not a composite");
    };
    assert_eq!(elements[0].name, elements[1].name);
    // The counts that sizes follow from.
    let Geometry::BlockModel(bm) = &p.elements[5].geometry else {
        panic!("not a block model");
    };
    assert_eq!((bm.grid.count(), bm.grid.blocks()), ([2, 1, 3], 6));
    assert_eq!(
        location_len(&p.elements[5].geometry, Location::Vertices),
        Some(3 * 2 * 4)
    );
    assert_eq!(
        location_len(&p.elements[3].geometry, Location::Vertices),
        Some(6)
    );
    assert_eq!(
        location_len(&p.elements[0].geometry, Location::Primitives),
        None
    );
}

#[test]
fn the_archive_is_refused_unless_it_is_omf_2() {
    let mut b = Build::default();
    let ps = element("p", &point_set(&mut b, &TRI), "");
    let index = project(&ps);
    assert!(read(&b.zip_with(&index, "Open Mining Format 2.0-beta.1")).is_ok());
    refused(
        &b.zip_with(&index, "Open Mining Format 1.0"),
        "OMF version 1.0; this reader reads 2.x",
    );
    refused(
        &b.zip_with(&index, "a zip"),
        "not an OMF file: the archive comment is \"a zip\"",
    );
    refused(
        &write_stored_zip_with_crcs(&[("x", b"y")]),
        "the archive comment is \"\"",
    );
    let no_index = with_comment(
        write_stored_zip_with_crcs(&[("1.parquet", &b.members[0].1)]),
        "Open Mining Format 2.0",
    );
    refused(&no_index, "the archive has no index.json.gz");
    refused(b"PK", "invalid ZIP archive");

    // A compressed member: the central directory's method for the first member set to DEFLATE.
    let good = b.file(&ps);
    let cd = good
        .windows(4)
        .position(|w| w == [0x50, 0x4b, 0x01, 0x02])
        .unwrap();
    let mut z = good.clone();
    z[cd + 10] = 8;
    refused(&z, "unsupported ZIP compression method 8");
    // A flipped byte inside a member's payload.
    let par = good.windows(4).position(|w| w == b"PAR1").unwrap();
    let mut z = good.clone();
    z[par + 5] ^= 0x40;
    refused(&z, "member CRC-32 does not match");
    // The `.xlsx` writer's zero CRC-32s are refused too.
    let gz = gzip_stored(index.as_bytes());
    let zero = write_stored_zip(&[(INDEX, &gz), ("1.parquet", &b.members[0].1)]);
    refused(
        &with_comment(zero, "Open Mining Format 2.0"),
        "member CRC-32 does not match",
    );

    // An index that is not gzip, or not JSON.
    let gz_raw = |payload: &[u8]| {
        with_comment(
            write_stored_zip_with_crcs(&[(INDEX, payload), ("1.parquet", &b.members[0].1)]),
            "Open Mining Format 2.0",
        )
    };
    refused(
        &gz_raw(index.as_bytes()),
        "index.json.gz: not a gzip member",
    );
    refused(&gz_raw(&gzip_stored(b"{\"date\":")), "index.json.gz: ");
    refused(&gz_raw(&gzip_stored(b"[]")), "project is not an object");
    refused(&gz_raw(&gzip_stored(b"{}")), "project: \"date\" is missing");
}

#[test]
fn arrays_are_refused_unless_they_are_the_declared_omf_arrays() {
    let ok_points = |b: &mut Build| element("p", &point_set(b, &TRI), "");

    let mut b = Build::default();
    let v = b.arr(xyz(&TRI), 3);
    refused(
        &b.file(&element(
            "p",
            r#"{"type":"PointSet","vertices":{"filename":"9.parquet","item_count":3}}"#,
            "",
        )),
        "elements[0] \"p\".geometry.vertices refers to non-existent archive member '9.parquet'",
    );
    refused(
        &b.file(&element(
            "p",
            &format!(
                r#"{{"type":"PointSet","vertices":{}}}"#,
                v.replace(":3}", ":4}")
            ),
            "",
        )),
        "array length 3 does not match the declared length 4",
    );
    let png = b.image(PNG, "png");
    refused(
        &b.file(&element(
            "p",
            &format!(r#"{{"type":"PointSet","vertices":{png}}}"#),
            "",
        )),
        "is not a .parquet file",
    );

    // Columns other than the schema's, mixed widths, and nullability the schema does not give.
    let mut b = Build::default();
    let xy = b.arr(pq(&[dbl("x", &[0.0]), dbl("y", &[0.0])], 1), 1);
    let mixed = b.arr(
        pq(&[flt("x", &[0.0]), dbl("y", &[0.0]), dbl("z", &[0.0])], 1),
        1,
    );
    let nx = b.arr(
        pq(
            &[
                ncol(
                    "x",
                    Physical::Double,
                    Logical::None,
                    vec![true],
                    Values::Double(vec![0.0]),
                ),
                dbl("y", &[0.0]),
                dbl("z", &[0.0]),
            ],
            1,
        ),
        1,
    );
    let ints = b.arr(
        pq(&[u32c("x", &[0]), u32c("y", &[0]), u32c("z", &[0])], 1),
        1,
    );
    let ps = |v: &str| element("p", &format!(r#"{{"type":"PointSet","vertices":{v}}}"#), "");
    refused(
        &b.file(&ps(&xy)),
        "columns [\"x\", \"y\"], where OMF has [\"x\", \"y\", \"z\"]",
    );
    refused(&b.file(&ps(&mixed)), "float columns of mixed widths");
    refused(
        &b.file(&ps(&nx)),
        "column x is optional where OMF requires it",
    );
    refused(&b.file(&ps(&ints)), "column x is not float or double");

    let mut b = Build::default();
    let p = ok_points(&mut b);
    let req = b.arr(pq(&[dbl("number", &[1.0, 2.0, 3.0])], 3), 3);
    let wrong = b.arr(
        pq(
            &[ncol(
                "number",
                Physical::Int32,
                U32,
                vec![true; 3],
                Values::Int32(vec![1, 2, 3]),
            )],
            3,
        ),
        3,
    );
    let with = |a: &str| p.replace(r#""attributes":[]"#, &format!(r#""attributes":[{a}]"#));
    let num = |v: &str| {
        attribute(
            "n",
            "Vertices",
            &format!(r#"{{"type":"Number","values":{v}}}"#),
        )
    };
    refused(
        &b.file(&with(&num(&req))),
        "column number is required where OMF has it optional",
    );
    refused(
        &b.file(&with(&num(&wrong))),
        "column number is not a float, int64, date or date-time",
    );

    // A channel past 8 bits.
    let mut b = Build::default();
    let p = ok_points(&mut b);
    let hot = b.arr(gradient(&[[256, 0, 0, 0]]), 1);
    let vals = b.arr(
        pq(
            &[ncol(
                "number",
                Physical::Double,
                Logical::None,
                vec![true; 3],
                Values::Double(vec![0.0; 3]),
            )],
            3,
        ),
        3,
    );
    // The range runs backward, which the vendor reader allows; the channel is what is refused.
    let m = format!(
        r#"{{"type":"Number","values":{vals},"colormap":{{"type":"Continuous","range":{{"min":1,"max":0}},"gradient":{hot}}}}}"#
    );
    let with = |a: &str| p.replace(r#""attributes":[]"#, &format!(r#""attributes":[{a}]"#));
    refused(
        &b.file(&with(&attribute("n", "Vertices", &m))),
        "column r holds 256, outside 8 bits",
    );
}

#[test]
fn indices_past_their_targets_are_refused() {
    let mut b = Build::default();
    let v = b.arr(xyz(&TRI), 3);
    let seg = b.arr(pq(&[u32c("a", &[0]), u32c("b", &[3])], 1), 1);
    let tri = b.arr(
        pq(
            &[u32c("a", &[0]), u32c("b", &[1]), u32c("c", &[u32::MAX])],
            1,
        ),
        1,
    );
    refused(
        &b.file(&element(
            "l",
            &format!(r#"{{"type":"LineSet","vertices":{v},"segments":{seg}}}"#),
            "",
        )),
        "geometry.segments[0]: vertex index 3 is not below the 3 vertices",
    );
    refused(
        &b.file(&element(
            "s",
            &format!(r#"{{"type":"Surface","vertices":{v},"triangles":{tri}}}"#),
            "",
        )),
        "vertex index 4294967295 is not below the 3 vertices",
    );

    let idx = b.arr(
        pq(
            &[ncol(
                "index",
                Physical::Int32,
                U32,
                vec![true, false, true],
                Values::Int32(vec![0, 2]),
            )],
            3,
        ),
        3,
    );
    let names = b.arr(
        pq(
            &[rcol(
                "name",
                Physical::ByteArray,
                Logical::String,
                Values::Bytes(vec![b"a".to_vec(), b"b".to_vec()]),
            )],
            2,
        ),
        2,
    );
    let cat = attribute(
        "c",
        "Vertices",
        &format!(r#"{{"type":"Category","values":{idx},"names":{names}}}"#),
    );
    refused(
        &b.file(&element(
            "p",
            &format!(r#"{{"type":"PointSet","vertices":{v}}}"#),
            &cat,
        )),
        "category index 2 is not below the 2 names",
    );
}

#[test]
fn frames_and_grids_are_refused_unless_valid() {
    let bm = |orient: &str, grid: &str| {
        let b = Build::default();
        b.file(&element(
            "b",
            &format!(r#"{{"type":"BlockModel","orient":{orient},"grid":{grid}}}"#),
            "",
        ))
    };
    let unit = r#"{"type":"Regular","size":[1,1,1],"count":[1,1,1]}"#;
    assert!(read(&bm(r#"{"origin":[0,0,0]}"#, unit)).is_ok());
    refused(
        &bm(r#"{"origin":[0,0,0],"u":[1,1,0]}"#, unit),
        "orient.u [1.0, 1.0, 0.0] is not a unit vector",
    );
    // The vendor validator compares u with v three times; u with w is checked here.
    refused(
        &bm(
            r#"{"origin":[0,0,0],"u":[1,0,0],"v":[0,1,0],"w":[0.6,0,0.8]}"#,
            unit,
        ),
        "axes [1.0, 0.0, 0.0] and [0.6, 0.0, 0.8] are not orthogonal",
    );
    refused(
        &bm(
            r#"{"origin":[0,0,0],"u":[1,0,0],"v":[0,1,0],"w":[0,0.6,0.8]}"#,
            unit,
        ),
        "axes [0.0, 1.0, 0.0] and [0.0, 0.6, 0.8] are not orthogonal",
    );
    refused(
        &bm(r#"{"origin":[0,1e400,0]}"#, unit),
        "orient.origin is not 3 finite numbers",
    );
    refused(&bm(r#"{"u":[1,0,0]}"#, unit), "\"origin\" is missing");
    refused(
        &bm(
            r#"{"origin":[0,0,0]}"#,
            r#"{"type":"Regular","size":[1,0,1],"count":[1,1,1]}"#,
        ),
        "grid.size[1] = 0 is not a finite size above zero",
    );
    refused(
        &bm(
            r#"{"origin":[0,0,0]}"#,
            r#"{"type":"Regular","size":[1,-2,1],"count":[1,1,1]}"#,
        ),
        "grid.size[1] = -2 is not a finite size above zero",
    );
    refused(
        &bm(
            r#"{"origin":[0,0,0]}"#,
            r#"{"type":"Regular","size":[1,1,1],"count":[1,0,1]}"#,
        ),
        "grid.count [1, 0, 1] has a zero count",
    );
    refused(
        &bm(
            r#"{"origin":[0,0,0]}"#,
            r#"{"type":"Regular","size":[1,1,1],"count":[1,1,4294967296]}"#,
        ),
        "is not 3 whole numbers below 2^32",
    );
    refused(
        &bm(r#"{"origin":[0,0,0]}"#, r#"{"type":"Hex"}"#),
        "grid type \"Hex\" is unknown",
    );

    let mut b = Build::default();
    let neg = b.arr(pq(&[dbl("scalar", &[1.0, -1.0])], 2), 2);
    let one = b.arr(pq(&[dbl("scalar", &[1.0])], 1), 1);
    let empty = b.arr(pq(&[dbl("scalar", &[])], 0), 0);
    let tensor = |u: &str, v: &str| {
        b.file(&element(
            "b",
            &format!(r#"{{"type":"BlockModel","orient":{{"origin":[0,0,0]}},"grid":{{"type":"Tensor","u":{u},"v":{v},"w":{one}}}}}"#),
            "",
        ))
    };
    refused(
        &tensor(&neg, &one),
        "grid.u[1] = -1 is not a finite size above zero",
    );
    refused(&tensor(&one, &empty), "grid.v is empty");

    // A grid surface's heights must cover its corners.
    let h = b.arr(pq(&[dbl("scalar", &[0.0; 5])], 5), 5);
    refused(
        &b.file(&element(
            "g",
            &format!(
                r#"{{"type":"GridSurface","orient":{{"origin":[0,0,0]}},"grid":{{"type":"Regular","size":[1,1],"count":[2,1]}},"heights":{h}}}"#
            ),
            "",
        )),
        "geometry.heights: 5 heights for 6 grid corners",
    );
}

#[test]
fn sub_blocks_are_refused_outside_their_parents_and_modes() {
    let regular = |count: &str, mode: &str, parents: &[[u32; 3]], corners: &[[u32; 6]]| {
        let mut b = Build::default();
        let s = b.arr(subblock_table(parents, corners), parents.len());
        b.file(&element(
            "b",
            &format!(
                r#"{{"type":"BlockModel","orient":{{"origin":[0,0,0]}},"grid":{{"type":"Regular","size":[1,1,1],"count":[2,1,1]}},
                "subblocks":{{"type":"Regular","count":{count},"subblocks":{s}{mode}}}}}"#
            ),
            "",
        ))
    };
    let octree = r#","mode":"Octree""#;
    let full = r#","mode":"Full""#;
    assert!(
        read(&regular(
            "[4,4,4]",
            octree,
            &[[1, 0, 0]],
            &[[2, 0, 0, 4, 2, 2]]
        ))
        .is_ok()
    );
    assert!(read(&regular("[3,4,4]", "", &[[1, 0, 0]], &[[1, 0, 0, 3, 2, 2]])).is_ok());
    assert!(
        read(&regular(
            "[3,4,4]",
            full,
            &[[1, 0, 0]],
            &[[0, 0, 0, 3, 4, 4]]
        ))
        .is_ok()
    );
    refused(
        &regular("[4,4,4]", "", &[[2, 0, 0]], &[[0, 0, 0, 1, 1, 1]]),
        "geometry.subblocks.subblocks[0]: parent [2, 0, 0] outside the [2, 1, 1] parent blocks",
    );
    refused(
        &regular("[4,4,4]", "", &[[0, 0, 0]], &[[1, 0, 0, 1, 1, 1]]),
        "corners [1, 0, 0] to [1, 1, 1] have no size",
    );
    refused(
        &regular("[4,4,4]", "", &[[0, 0, 0]], &[[0, 0, 0, 5, 1, 1]]),
        "corner [5, 1, 1] outside the [4, 4, 4] sub-blocks",
    );
    // Octree: the right size in the wrong place, then a size that is no halving.
    refused(
        &regular("[4,4,4]", octree, &[[0, 0, 0]], &[[1, 0, 0, 3, 2, 2]]),
        "corners [1, 0, 0] to [3, 2, 2] are not a sub-block of mode Octree",
    );
    refused(
        &regular("[4,4,4]", octree, &[[0, 0, 0]], &[[0, 0, 0, 3, 3, 3]]),
        "of mode Octree",
    );
    refused(
        &regular("[4,4,4]", full, &[[0, 0, 0]], &[[0, 0, 0, 2, 2, 2]]),
        "are not a sub-block of mode Full",
    );
    refused(
        &regular("[3,4,4]", octree, &[[0, 0, 0]], &[[0, 0, 0, 1, 1, 1]]),
        "octree sub-block count [3, 4, 4] is not a power of two on every axis",
    );
    refused(
        &regular("[4,4,4]", r#","mode":"Quad""#, &[], &[]),
        "sub-block mode \"Quad\" is unknown",
    );
    refused(
        &regular("[4,4,4]", r#","mode":2"#, &[], &[]),
        "sub-block mode is not a string",
    );

    let freeform = |min: [f32; 3], max: [f32; 3]| {
        let mut b = Build::default();
        let s = b.arr(
            pq(
                &[
                    u32c("parent_u", &[0]),
                    u32c("parent_v", &[0]),
                    u32c("parent_w", &[0]),
                    flt("corner_min_u", &[min[0]]),
                    flt("corner_min_v", &[min[1]]),
                    flt("corner_min_w", &[min[2]]),
                    flt("corner_max_u", &[max[0]]),
                    flt("corner_max_v", &[max[1]]),
                    flt("corner_max_w", &[max[2]]),
                ],
                1,
            ),
            1,
        );
        b.file(&element(
            "b",
            &format!(
                r#"{{"type":"BlockModel","orient":{{"origin":[0,0,0]}},"grid":{{"type":"Regular","size":[1,1,1],"count":[1,1,1]}},
                "subblocks":{{"type":"Freeform","subblocks":{s}}}}}"#
            ),
            "",
        ))
    };
    assert!(read(&freeform([0.0; 3], [1.0; 3])).is_ok());
    refused(&freeform([0.0; 3], [1.5, 1.0, 1.0]), "outside the parent");
    refused(&freeform([-0.25, 0.0, 0.0], [1.0; 3]), "outside the parent");
    refused(
        &freeform([0.0, f32::NAN, 0.0], [1.0; 3]),
        "outside the parent",
    );
    refused(&freeform([0.5, 0.0, 0.0], [0.5, 1.0, 1.0]), "have no size");
}

#[test]
fn attributes_are_refused_off_their_locations() {
    let mut b = Build::default();
    let v = b.arr(xyz(&TRI), 3);
    let two = b.arr(
        pq(
            &[ncol(
                "number",
                Physical::Double,
                Logical::None,
                vec![true, true],
                Values::Double(vec![0.0, 1.0]),
            )],
            2,
        ),
        2,
    );
    let three = b.arr(
        pq(
            &[ncol(
                "number",
                Physical::Double,
                Logical::None,
                vec![true; 3],
                Values::Double(vec![0.0; 3]),
            )],
            3,
        ),
        3,
    );
    let jpg = b.image(JPEG, "jpg");
    let gif = b.image(b"GIF89a", "gif");
    let idx = b.arr(
        pq(
            &[ncol(
                "index",
                Physical::Int32,
                U32,
                vec![true; 3],
                Values::Int32(vec![0; 3]),
            )],
            3,
        ),
        3,
    );
    let names = b.arr(
        pq(
            &[rcol(
                "name",
                Physical::ByteArray,
                Logical::String,
                Values::Bytes(vec![b"a".to_vec(), b"b".to_vec()]),
            )],
            2,
        ),
        2,
    );
    let points = |a: &str| {
        b.file(&element(
            "p",
            &format!(r#"{{"type":"PointSet","vertices":{v}}}"#),
            a,
        ))
    };
    let number = |loc: &str, vals: &str| {
        attribute("n", loc, &format!(r#"{{"type":"Number","values":{vals}}}"#))
    };
    assert!(read(&points(&number("Vertices", &three))).is_ok());
    refused(
        &points(&number("Vertices", &two)),
        "attributes[0] \"n\": 2 values where location Vertices has 3",
    );
    refused(
        &points(&number("Subblocks", &three)),
        "location Subblocks does not apply here",
    );
    refused(
        &points(&number("Primitives", &three)),
        "location Primitives does not apply here",
    );
    refused(
        &points(&number("Faces", &three)),
        "location \"Faces\" is unknown",
    );
    refused(
        &points(&number("Projected", &three)),
        "a projected texture goes with location Projected, and only it",
    );
    let projected = |loc: &str, img: &str| {
        attribute(
            "t",
            loc,
            &format!(
                r#"{{"type":"ProjectedTexture","image":{img},"orient":{{"origin":[0,0,0]}},"width":1,"height":1}}"#
            ),
        )
    };
    assert!(read(&points(&projected("Projected", &jpg))).is_ok());
    refused(
        &points(&projected("Vertices", &jpg)),
        "a projected texture goes with location Projected",
    );
    refused(
        &points(&projected("Projected", &gif)),
        "member '5.gif' is neither PNG nor JPEG",
    );
    refused(
        &points(&projected("Projected", &jpg).replace(r#""width":1"#, r#""width":0"#)),
        "data.width[0] = 0 is not a finite size above zero",
    );
    refused(
        &points(&attribute("x", "Vertices", r#"{"type":"Tensor"}"#)),
        "attribute type \"Tensor\" is unknown",
    );
    refused(
        &b.file(&element("p", r#"{"type":"Mesh"}"#, "")),
        "elements[0] \"p\".geometry: geometry type \"Mesh\" is unknown",
    );

    // A category's own attributes go on its categories.
    let category = |sub: &str| {
        attribute(
            "c",
            "Vertices",
            &format!(
                r#"{{"type":"Category","values":{idx},"names":{names},"attributes":[{sub}]}}"#
            ),
        )
    };
    assert!(read(&points(&category(&number("Categories", &two)))).is_ok());
    refused(
        &points(&category(&number("Vertices", &three))),
        "location Vertices does not apply here",
    );
    refused(
        &points(&category(&number("Categories", &three))),
        "3 values where location Categories has 2",
    );
}

#[test]
fn colormap_boundaries_must_not_decrease() {
    let mut b = Build::default();
    let v = b.arr(xyz(&TRI), 3);
    let vals = b.arr(
        pq(
            &[ncol(
                "number",
                Physical::Double,
                Logical::None,
                vec![true; 3],
                Values::Double(vec![0.0; 3]),
            )],
            3,
        ),
        3,
    );
    let ramp = b.arr(gradient(&[[0, 0, 0, 0], [1, 1, 1, 1], [2, 2, 2, 2]]), 3);
    let bounds = |b: &mut Build, v: &[f64]| {
        b.arr(
            pq(
                &[dbl("value", v), bools("inclusive", &vec![true; v.len()])],
                v.len(),
            ),
            v.len(),
        )
    };
    let ok = bounds(&mut b, &[1.0, 1.0]);
    let down = bounds(&mut b, &[2.0, 1.0]);
    let nan = bounds(&mut b, &[f64::NAN]);
    let file = |bd: &str| {
        b.file(&element(
            "p",
            &format!(r#"{{"type":"PointSet","vertices":{v}}}"#),
            &attribute(
                "n",
                "Vertices",
                &format!(
                    r#"{{"type":"Number","values":{vals},"colormap":{{"type":"Discrete","boundaries":{bd},"gradient":{ramp}}}}}"#
                ),
            ),
        ))
    };
    assert!(read(&file(&ok)).is_ok());
    refused(&file(&down), "colormap.boundaries: boundary 1 decreases");
    refused(
        &file(&nan),
        "colormap.boundaries: boundary 0 is not a number",
    );
}

#[test]
fn read_path_reads_a_file_and_names_a_missing_one() {
    let mut b = Build::default();
    let p = element("p", &point_set(&mut b, &TRI), "");
    let bytes = b.file(&p);
    let dir = std::env::temp_dir().join(format!("fmine-io-omf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("one.omf");
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(read_path(&path).unwrap(), read(&bytes).unwrap());
    let e = read_path(&dir.join("absent.omf")).unwrap_err();
    assert!(e.what.contains("absent.omf"), "{}", e.what);
}

/// splitmix64, from a fixed seed, so every run makes the same mutants.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn below(s: &mut u64, n: usize) -> usize {
    (splitmix(s) % n.max(1) as u64) as usize
}

/// One to four edits: a bit flipped, a byte replaced, inserted or removed, or the bytes cut short.
fn mutate(bytes: &mut Vec<u8>, s: &mut u64) {
    for _ in 0..=below(s, 4) {
        let at = below(s, bytes.len());
        match below(s, 5) {
            0 if !bytes.is_empty() => bytes[at] ^= 1_u8 << below(s, 8),
            1 if !bytes.is_empty() => bytes[at] = splitmix(s) as u8,
            2 => bytes.truncate(at),
            3 => bytes.insert(at, splitmix(s) as u8),
            _ if !bytes.is_empty() => {
                bytes.remove(at);
            }
            _ => {}
        }
    }
}

/// Mutants of the every-kind file, 1,000 in each of three page forms: GZIP v2 (omf-rust's), and v1 and v2 without a
/// codec, where an edit to a page reaches the value and level decoders instead of the gzip CRC-32. Each mutant changes
/// one member, the index as JSON text and any other member as bytes, and is zipped again with true CRC-32s, so the
/// change reaches the parsers rather than the archive's own check. No mutant panics, every refusal is
/// `FMINE-REF-506`, and in each form at least one mutant in fifty is read and one in fifty refused, so the search is
/// not vacuous. The floor was one in twenty until a first run of the GZIP form read 50 of 1,500 (3.3%): there the
/// gzip CRC-32 refuses most edits to a page, as it should. A bounded random search, not a proof.
#[test]
fn mutated_files_are_read_or_refused_and_never_panic() {
    const N: usize = 1000;
    let mut s = 0x0194_2026_1103_0F4F_u64;
    for (k, pages) in [Opts::default(), FORMS[0], FORMS[1]]
        .into_iter()
        .enumerate()
    {
        PAGES.with(|c| c.set(pages));
        let file = every_kind();
        let members = read_stored_entries(&file, MAX_MEMBER_BYTES, MAX_ARCHIVE_BYTES).unwrap();
        let names: Vec<&String> = members.keys().collect();
        let (mut accepted, mut refusals) = (0, 0);
        for _ in 0..N {
            let name = names[below(&mut s, names.len())];
            let mut m = members.clone();
            let Some(bytes) = m.get_mut(name) else {
                panic!("{name} is not a member");
            };
            if name == INDEX {
                let mut text = gunzip(bytes, MAX_INDEX_BYTES).unwrap();
                mutate(&mut text, &mut s);
                *bytes = gzip_stored(&text);
            } else {
                mutate(bytes, &mut s);
            }
            let entries: Vec<(&str, &[u8])> =
                m.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
            match read(&with_comment(
                write_stored_zip_with_crcs(&entries),
                "Open Mining Format 2.0",
            )) {
                Ok(_) => accepted += 1,
                Err(e) => {
                    assert_eq!(e.code, "FMINE-REF-506", "{e:?}");
                    refusals += 1;
                }
            }
        }
        println!("page form {k}: {accepted} read, {refusals} refused");
        assert!(
            accepted * 50 >= N && refusals * 50 >= N,
            "page form {k}: {accepted} read, {refusals} refused"
        );
    }
    PAGES.with(|c| c.set(Opts::default()));
}

/// A closed, outward box Surface from `origin + lo` to `origin + hi`: corner `i` takes `hi` on axis `a` when bit
/// `a` of `i` is set, two triangles a face.
fn box_surface(b: &mut Build, origin: [f64; 3], lo: [f64; 3], hi: [f64; 3]) -> String {
    let corners: Vec<[f64; 3]> = (0..8usize)
        .map(|i| std::array::from_fn(|a| if (i >> a) & 1 == 1 { hi[a] } else { lo[a] }))
        .collect();
    corner_surface(b, origin, &corners, false)
}

/// A closed box Surface on eight corners in [`box_surface`]'s order, two triangles a face. It faces outward when the
/// corners keep the axes' handedness; `reversed` turns each triangle, for corners that mirror them.
fn corner_surface(b: &mut Build, origin: [f64; 3], corners: &[[f64; 3]], reversed: bool) -> String {
    const FACES: [[u32; 3]; 12] = [
        [0, 2, 1],
        [1, 2, 3],
        [4, 5, 6],
        [5, 7, 6],
        [0, 1, 4],
        [1, 5, 4],
        [2, 6, 3],
        [3, 6, 7],
        [0, 4, 2],
        [2, 4, 6],
        [1, 3, 5],
        [3, 7, 5],
    ];
    let faces = FACES.map(|[x, y, z]| if reversed { [x, z, y] } else { [x, y, z] });
    let col = |k: usize| faces.iter().map(|t| t[k]).collect::<Vec<_>>();
    let v = b.arr(xyz(corners), 8);
    let t = b.arr(
        pq(
            &[u32c("a", &col(0)), u32c("b", &col(1)), u32c("c", &col(2))],
            12,
        ),
        12,
    );
    format!(r#"{{"type":"Surface","origin":{origin:?},"vertices":{v},"triangles":{t}}}"#)
}

fn refused_as<T>(r: Result<T, Refusal>, code: &str, why: &str) {
    let e = r.map(drop).unwrap_err();
    assert_eq!(e.code, code, "{e:?}");
    assert!(e.what.contains(why), "wanted {why:?} in {}", e.what);
}

#[test]
fn a_wireframe_against_a_block_model_gives_each_parent_block_its_proportion() {
    // The box [2.5, 4.25] x [4, 4.75] x [5, 6.5], stored as an element origin plus vertices from zero, inside a
    // composite; and 3 x 1 x 2 unit blocks from (2, 4, 5). The box covers 0.5, 1 and 0.25 of the blocks along x,
    // 0.75 along y, and 1 and 0.5 along z, all at quarters, so from depth 2 every block's proportion is exact.
    let mut b = Build::default();
    let shell = element(
        "pit shell",
        &box_surface(&mut b, [2.5, 4.0, 5.0], [0.0; 3], [1.75, 0.75, 1.5]),
        "",
    );
    let design = element(
        "design",
        &format!(r#"{{"type":"Composite","elements":[{shell}]}}"#),
        "",
    );
    let model = element(
        "model",
        r#"{"type":"BlockModel","orient":{"origin":[2,4,5]},"grid":{"type":"Regular","size":[1,1,1],"count":[3,1,2]}}"#,
        "",
    );
    let p = read(&b.file(&[design, model].join(","))).unwrap();
    let volume = surface_mesh(&p, "pit shell").unwrap().volume();
    let exact = 1.75 * 0.75 * 1.5;
    assert!(volume.lo() <= exact && exact <= volume.hi(), "{volume:?}");

    let fx = [0.5, 1.0, 0.25];
    let fz = [1.0, 0.5];
    for depth in 0..=3u8 {
        let props = proportions_in_surface(&p, "pit shell", "model", depth).unwrap();
        let mut inside = 0.0;
        for (i, x) in fx.iter().enumerate() {
            for (k, z) in fz.iter().enumerate() {
                let exact = x * 0.75 * z;
                let ijk = [i as u32, 0, k as u32];
                let [lo, hi] = props.bounds(ijk);
                assert!(
                    lo <= exact && exact <= hi,
                    "depth {depth} block {ijk:?}: {exact} not in [{lo}, {hi}]"
                );
                if depth >= 2 {
                    assert_eq!((lo, hi), (exact, exact), "depth {depth} block {ijk:?}");
                }
                inside += lo;
            }
        }
        println!(
            r#"{{"test":"omf_proportions","depth":{depth},"blocks":{},"inside_volume":{inside},"stats":"{:?}"}}"#,
            props.iter().count(),
            props.stats()
        );
        if depth >= 2 {
            assert_eq!(inside, 1.75 * 0.75 * 1.5);
            assert_eq!(props.iter().count(), 6);
        }
    }
    refused_as(
        proportions_in_surface(&p, "pit shell", "model", 99),
        "FMINE-REF-109",
        "depth 99",
    );
}

#[test]
fn proportions_refuse_missing_ambiguous_and_unsuitable_elements() {
    let p = read(&every_kind()).unwrap();
    refused_as(
        surface_mesh(&p, "nowhere"),
        "FMINE-REF-506",
        r#"no element is called "nowhere""#,
    );
    refused_as(
        surface_mesh(&p, "child"),
        "FMINE-REF-506",
        r#"2 elements are called "child""#,
    );
    refused_as(
        surface_mesh(&p, "points"),
        "FMINE-REF-506",
        "is a PointSet, not a Surface",
    );
    refused_as(surface_mesh(&p, "tri"), "FMINE-REF-207", "closed mesh");
    refused_as(
        parent_grid(&p, "tri"),
        "FMINE-REF-506",
        "is a Surface, not a BlockModel",
    );
    refused_as(parent_grid(&p, "tensor"), "FMINE-REF-109", "tensor grid");
    refused_as(
        proportions_in_surface(&p, "tri", "blocks", 1),
        "FMINE-REF-207",
        "closed mesh",
    );
}

#[test]
fn a_turned_or_mirrored_block_model_gives_the_box_its_proportions_in_its_own_frame() {
    // bd-aqfl's box, [0.5, 2.25] x [0, 0.75] x [0, 1.5] in the model's frame, placed in the world through the
    // model's axes: x, y and z; a turn that also tilts (exact in decimals, not in binary); and a mirror.
    let (lo, hi, origin) = ([0.5, 0.0, 0.0], [2.25, 0.75, 1.5], [2.0, 4.0, 5.0]);
    let frames: [(&str, [[f64; 3]; 3]); 3] = [
        ("axes", [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
        (
            "tilted",
            [[0.6, 0.8, 0.0], [-0.48, 0.36, 0.8], [0.64, -0.48, 0.6]],
        ),
        (
            "mirrored",
            [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        ),
    ];
    let exact =
        |ijk: [u32; 3]| [0.5, 1.0, 0.25][ijk[0] as usize] * 0.75 * [1.0, 0.5][ijk[2] as usize];
    for (name, [u, v, w]) in frames {
        let mut b = Build::default();
        let corners: Vec<[f64; 3]> = (0..8usize)
            .map(|i| {
                let c: [f64; 3] =
                    std::array::from_fn(|a| if (i >> a) & 1 == 1 { hi[a] } else { lo[a] });
                std::array::from_fn(|a| origin[a] + u[a] * c[0] + v[a] * c[1] + w[a] * c[2])
            })
            .collect();
        let shell = element(
            "pit shell",
            &corner_surface(&mut b, [0.0; 3], &corners, name == "mirrored"),
            "",
        );
        let model = element(
            "model",
            &format!(
                r#"{{"type":"BlockModel","orient":{{"origin":{origin:?},"u":{u:?},"v":{v:?},"w":{w:?}}},"grid":{{"type":"Regular","size":[1,1,1],"count":[3,1,2]}}}}"#
            ),
            "",
        );
        let mut p = read(&b.file(&[shell, model].join(","))).unwrap();
        // The surface is a valid outward box in the world before it is placed in the model's frame.
        let world = surface_mesh(&p, "pit shell").unwrap().volume();
        // In the model's frame the box is 1.75 x 0.75 x 1.5 = 1.96875; the tilted frame rounds every coordinate.
        let local = surface_mesh_in(&p, "pit shell", "model").unwrap().volume();
        assert!(
            (local.lo() - 1.96875).abs() < 1e-12 && (local.hi() - 1.96875).abs() < 1e-12,
            "{name}: {local:?}"
        );
        if name != "tilted" {
            assert!(
                local.lo() <= 1.96875 && 1.96875 <= local.hi(),
                "{name}: {local:?}"
            );
        }
        let mut widths = Vec::new();
        for depth in 0..=3u8 {
            let props = proportions_in_surface(&p, "pit shell", "model", depth).unwrap();
            assert_eq!(props.grid().origin(), [0.0; 3]);
            let mut width = 0.0;
            for ijk in [
                [0, 0, 0],
                [1, 0, 0],
                [2, 0, 0],
                [0, 0, 1],
                [1, 0, 1],
                [2, 0, 1],
            ] {
                let [l, h] = props.bounds(ijk);
                assert!(
                    l <= exact(ijk) && exact(ijk) <= h,
                    "{name} depth {depth} block {ijk:?}: {} not in [{l}, {h}]",
                    exact(ijk)
                );
                if name != "tilted" && depth >= 2 {
                    assert_eq!((l, h), (exact(ijk), exact(ijk)), "{name} depth {depth}");
                }
                width += h - l;
            }
            widths.push(width);
        }
        println!(
            r#"{{"test":"omf_turned_proportions","frame":"{name}","world_volume":[{},{}],"band_by_depth":{widths:?}}}"#,
            world.lo(),
            world.hi()
        );
        // The tilted box's faces sit within rounding of sub-cell planes, so at depth 3 at most the two layers of
        // eighth-cells beside each face are in the band: the faces cross 10.125 block faces' worth of area.
        assert!(
            widths[3] < widths[1] && widths[3] <= 2.0 * 10.125 / 8.0,
            "{name}: {widths:?}"
        );

        if let Geometry::BlockModel(m) = &mut p.elements[1].geometry {
            m.orient.v = m.orient.u;
            m.orient.w = m.orient.u;
        }
        refused_as(
            proportions_in_surface(&p, "pit shell", "model", 1),
            "FMINE-REF-109",
            "span no volume",
        );
    }
}

/// A Number attribute at `location` from one nullable `number` column of `n` rows.
fn numbers_at(b: &mut Build, name: &str, location: &str, col: Col, n: usize) -> String {
    let arr = b.arr(pq(&[col], n), n);
    attribute(
        name,
        location,
        &format!(r#"{{"type":"Number","values":{arr}}}"#),
    )
}

fn floats(v: &[Option<f64>]) -> Col {
    ncol(
        "number",
        Physical::Double,
        Logical::None,
        v.iter().map(Option::is_some).collect(),
        Values::Double(v.iter().flatten().copied().collect()),
    )
}

fn integers(v: &[i64]) -> Col {
    ncol(
        "number",
        Physical::Int64,
        Logical::None,
        vec![true; v.len()],
        Values::Int64(v.to_vec()),
    )
}

fn block_model_at(
    name: &str,
    orient: &str,
    size: &str,
    count: &str,
    attributes: &[String],
) -> String {
    element(
        name,
        &format!(
            r#"{{"type":"BlockModel","orient":{orient},"grid":{{"type":"Regular","size":{size},"count":{count}}}}}"#
        ),
        &attributes.join(","),
    )
}

/// A 5 x 1 x 3 section of 12.5 m blocks, OMF order `w·5 + u` with `w = 0` at the bottom: waste at -1 everywhere but
/// the ore at the bottom centre, block 2, worth 9.25 ("value") or 7.75 ("low value"). At 45 degrees its cone is the
/// three blocks above it and the whole top layer: blocks 2, 6-8 and 10-14.
fn section() -> Project {
    let mut b = Build::default();
    let blocks = |b: &mut Build, name: &str, col: Col| numbers_at(b, name, "Primitives", col, 15);
    let with_ore = |ore: f64| -> Vec<Option<f64>> {
        (0..15)
            .map(|i| Some(if i == 2 { ore } else { -1.0 }))
            .collect()
    };
    let value = blocks(&mut b, "value", floats(&with_ore(9.25)));
    let low = blocks(&mut b, "low value", floats(&with_ore(7.75)));
    let mut gappy = with_ore(9.25);
    gappy[7] = None;
    let gaps = blocks(&mut b, "gaps", floats(&gappy));
    let mut huge = with_ore(9.25);
    huge[0] = Some(1e19);
    let huge = blocks(&mut b, "huge", floats(&huge));
    let tonnes = blocks(&mut b, "tonnes", floats(&[Some(1.2); 15]));
    // Revenue at the ore (10) and at the top-left corner, block 10 (3); cost 2 at the ore and 1 elsewhere.
    let revenue: Vec<i64> = (0..15)
        .map(|i| match i {
            2 => 10,
            10 => 3,
            _ => 0,
        })
        .collect();
    let revenue = blocks(&mut b, "revenue", integers(&revenue));
    let cost: Vec<i64> = (0..15).map(|i| if i == 2 { 2 } else { 1 }).collect();
    let cost = blocks(&mut b, "cost", integers(&cost));
    // A client's schedule in years from 1, as a planning tool might export it: the top layer in year 1, the three
    // middle blocks and the ore in year 2, the rest left out.
    let plan: Vec<Option<f64>> = (0..15)
        .map(|i| match i {
            10..=14 => Some(1.0),
            2 | 6..=8 => Some(2.0),
            _ => None,
        })
        .collect();
    let plan = blocks(&mut b, "plan", floats(&plan));
    // Slope domains: the top layer soft (0), the two below hard (1).
    let rock_idx = b.arr(
        pq(
            &[ncol(
                "index",
                Physical::Int32,
                U32,
                vec![true; 15],
                Values::Int32((0..15).map(|i| i32::from(i < 10)).collect()),
            )],
            15,
        ),
        15,
    );
    let rock_names = b.arr(
        pq(
            &[rcol(
                "name",
                Physical::ByteArray,
                Logical::String,
                Values::Bytes(vec![b"soft".to_vec(), b"hard".to_vec()]),
            )],
            2,
        ),
        2,
    );
    let rock = attribute(
        "rock",
        "Primitives",
        &format!(r#"{{"type":"Category","values":{rock_idx},"names":{rock_names}}}"#),
    );
    let corners = numbers_at(
        &mut b,
        "corner value",
        "Vertices",
        floats(&[Some(0.0); 48]),
        48,
    );
    let level = r#"{"origin":[1000,2000,300]}"#;
    // Designed pits as boxes through the section's full width (V): the middle three columns and half of each corner
    // column, all three layers ("design") or the lower two ("deep design"); the same one with its sides 0.25 m and
    // 0.5 m off the sub-cell faces ("rough design"); and one reaching 1 m past the model's left side ("too wide").
    let mut design = |name: &str, from: [f64; 2], to: [f64; 2]| {
        let geometry = box_surface(
            &mut b,
            [1000.0 + from[0], 2000.0, 300.0 + from[1]],
            [0.0; 3],
            [to[0] - from[0], 12.5, to[1] - from[1]],
        );
        element(name, &geometry, "")
    };
    let designs = [
        design("design", [6.25, 0.0], [56.25, 37.5]),
        design("deep design", [6.25, 0.0], [56.25, 25.0]),
        design("rough design", [6.0, 0.0], [56.5, 37.5]),
        design("too wide", [-1.0, 0.0], [56.25, 37.5]),
    ];
    let elements = [
        designs.join(","),
        block_model_at(
            "section",
            level,
            "[12.5,12.5,12.5]",
            "[5,1,3]",
            &[
                value, low, gaps, huge, tonnes, revenue, cost, plan, rock, corners,
            ],
        ),
        block_model_at(
            "tilted",
            r#"{"origin":[0,0,0],"u":[1,0,0],"v":[0,0.6,0.8],"w":[0,-0.8,0.6]}"#,
            "[1,1,1]",
            "[1,1,1]",
            &[],
        ),
        block_model_at(
            "mirrored",
            r#"{"origin":[0,0,0],"u":[0,1,0],"v":[1,0,0],"w":[0,0,1]}"#,
            "[1,1,1]",
            "[1,1,1]",
            &[],
        ),
        block_model_at("decimetre", level, "[0.1,1,1]", "[1,1,1]", &[]),
    ];
    read(&b.file(&elements.join(","))).unwrap()
}

fn omf_blocks(flags: &[bool]) -> Vec<usize> {
    (0..flags.len()).filter(|&b| flags[b]).collect()
}

#[test]
fn an_omf_block_model_gives_its_certified_ultimate_pit_and_shells() {
    use fmine_optimize::pitflow::shells::RevenueFactor;
    use fmine_optimize::pitflow::slope::{Sector, SlopeTable, SlopeTangent};

    let p = section();
    let steep = SlopeTangent::new(1, 1).unwrap();
    let uniform = SlopeTable::uniform(steep);
    let m = pit::PitModel::new(&p, "section", &uniform, None, 2).unwrap();
    assert_eq!(m.size_scale(), 1, "12.5 m blocks are 25 half-metres");
    let cone = vec![2, 6, 7, 8, 10, 11, 12, 13, 14];

    // The ore pays for its cone: 9.25 x 4 = 37 quarter-units against 8 x 4 of waste.
    let pit = m.ultimate_pit("value", 4).unwrap();
    assert_eq!(
        (pit.value(), omf_blocks(&m.omf_order(pit.in_pit()))),
        (5, cone.clone())
    );
    // 7.75 does not: 31 against 32, so the best pit is empty.
    let pit = m.ultimate_pit("low value", 4).unwrap();
    assert_eq!((pit.value(), pit.pit_size()), (0, 0));
    // At whole units 9.25 rounds to 9, still one more than the waste.
    assert_eq!(m.ultimate_pit("value", 1).unwrap().value(), 1);

    // A hard domain at 2:1 below the soft top layer narrows the cone to the block above and three on top, which
    // 7.75 pays for: 31 - 4 x 4 = 15.
    let hard = SlopeTangent::new(2, 1).unwrap();
    let two = SlopeTable::new(vec![
        vec![Sector::new(1, 0, steep).unwrap()],
        vec![Sector::new(1, 0, hard).unwrap()],
    ])
    .unwrap();
    let m2 = pit::PitModel::new(&p, "section", &two, Some("rock"), 2).unwrap();
    let pit = m2.ultimate_pit("low value", 4).unwrap();
    assert_eq!(
        (pit.value(), omf_blocks(&m2.omf_order(pit.in_pit()))),
        (15, vec![2, 7, 11, 12, 13])
    );

    // Shells at revenue factors 1/2, 3/2 and 2: the corner alone (3/2 - 1, in halves 1), then the cone (in halves
    // 3 x 13 - 2 x 10 = 19), then the cone again (2 x 13 - 10 = 16).
    let factors = [
        RevenueFactor::new(1, 2).unwrap(),
        RevenueFactor::new(3, 2).unwrap(),
        RevenueFactor::new(2, 1).unwrap(),
    ];
    let shells = m.shells("revenue", "cost", 1, &factors).unwrap();
    let summary: Vec<(usize, i64)> = shells
        .shells()
        .iter()
        .map(|s| (s.blocks, s.value))
        .collect();
    assert_eq!(summary, vec![(1, 1), (9, 19), (9, 16)]);
    let first = m.omf_order(shells.first_shell());
    for (b, &f) in first.iter().enumerate() {
        let want = if b == 10 {
            0
        } else if cone.contains(&b) {
            1
        } else {
            u32::MAX
        };
        assert_eq!(f, want, "block {b}");
    }
    println!(
        r#"{{"test":"omf_pit","arcs":{},"cone":{cone:?},"shells":{summary:?}}}"#,
        m.arc_count()
    );

    // every_kind's block model is turned about W, which a pit accepts.
    let every = read(&every_kind()).unwrap();
    let turned = pit::PitModel::new(&every, "blocks", &uniform, None, 1).unwrap();
    assert_eq!((turned.grid().block_count(), turned.arc_count()), (2, 0));
}

#[test]
fn omf_pits_refuse_unsuitable_models_and_attributes() {
    use fmine_optimize::pitflow::slope::{SlopeTable, SlopeTangent};

    let p = section();
    let uniform = SlopeTable::uniform(SlopeTangent::new(1, 1).unwrap());
    let model =
        |name: &str, domain: Option<&str>| pit::PitModel::new(&p, name, &uniform, domain, 2);
    refused_as(model("tilted", None), "FMINE-REF-109", "W straight up");
    refused_as(model("mirrored", None), "FMINE-REF-109", "is mirrored");
    refused_as(
        model("decimetre", None),
        "FMINE-REF-109",
        "not integers below 2^32",
    );
    refused_as(
        model("nowhere", None),
        "FMINE-REF-506",
        r#"no element is called "nowhere""#,
    );
    refused_as(
        model("section", Some("value")),
        "FMINE-REF-506",
        "is not a Category",
    );
    // The rock categories name domain 1, which a one-domain table lacks.
    refused_as(model("section", Some("rock")), "FMINE-REF-207", "domain");
    let every = read(&every_kind()).unwrap();
    refused_as(
        pit::PitModel::new(&every, "tensor", &uniform, None, 1),
        "FMINE-REF-109",
        "tensor grid",
    );
    refused_as(
        pit::PitModel::new(&every, "tri", &uniform, None, 1),
        "FMINE-REF-506",
        "is a Surface, not a BlockModel",
    );

    let m = model("section", None).unwrap();
    refused_as(
        m.weights("nothing", 1),
        "FMINE-REF-506",
        r#"0 attributes called "nothing""#,
    );
    refused_as(
        m.weights("corner value", 1),
        "FMINE-REF-506",
        "is on Vertices",
    );
    refused_as(m.weights("rock", 1), "FMINE-REF-506", "is not a Number");
    refused_as(
        m.weights("gaps", 1),
        "FMINE-REF-506",
        "no value for block [2, 0, 1]",
    );
    refused_as(
        m.weights("value", 0),
        "FMINE-REF-109",
        "per_unit of at least 1",
    );
    refused_as(m.weights("huge", 1), "FMINE-REF-109", "is not a finite i64");
    // Revenue must not be negative for shells to nest.
    let factors = [fmine_optimize::pitflow::shells::RevenueFactor::new(1, 1).unwrap()];
    refused_as(
        m.shells("value", "cost", 1, &factors),
        "FMINE-REF-207",
        "negative revenue",
    );
}

#[test]
fn an_omf_block_model_gives_a_feasible_discounted_schedule() {
    use fmine_optimize::pitflow::slope::{SlopeTable, SlopeTangent};
    use fmine_optimize::toposort::check;
    use fmine_optimize::toposort::search::DISCOUNT_SCALE;

    let p = section();
    let uniform = SlopeTable::uniform(SlopeTangent::new(1, 1).unwrap());
    let m = pit::PitModel::new(&p, "section", &uniform, None, 2).unwrap();
    // Rank the ultimate pit's blocks 0 and leave the rest out.
    let pit = m.ultimate_pit("value", 4).unwrap();
    let rank: Vec<Option<i64>> = m
        .omf_order(pit.in_pit())
        .iter()
        .map(|&p| p.then_some(0))
        .collect();
    // Blocks of 1.2 t use 2 units each at 1 per unit, rounded up, so 10 a period holds five.
    let limits = [10, 10, 10];
    let tonnes = [pit::Capacity {
        attribute: "tonnes",
        per_unit: 1,
        limits: &limits,
    }];
    let spec = pit::ScheduleSpec {
        value: "value",
        per_unit: 4,
        capacities: &tonnes,
        periods: 3,
        discount: [100_000_000, 1_000_000_000],
        max_rounds: 100,
    };
    let s = m.schedule(&spec, &rank).unwrap();
    let i = s.improvement();
    // The ore needs all eight waste blocks above it by its own period, and a period holds five. At best, four waste
    // blocks go in period 0 (-16 quarter-units) and the fifth, the three middle ones and the ore (37) in period 1,
    // where the factor is 2^40 x 10/11 rounded down.
    let f1 = i128::from(DISCOUNT_SCALE) * 10 / 11;
    assert_eq!(i.after, -16 * i128::from(DISCOUNT_SCALE) + 21 * f1);
    assert_eq!(s.factors()[1] as i128, f1);
    let c = check(s.problem(), &i.schedule).unwrap();
    assert!(c.feasible(), "{c:?}");
    assert_eq!(
        (c.mined, c.period_values.clone(), c.used[0].clone()),
        (9, vec![-16, 21, 0], vec![8, 10, 0])
    );
    let periods = m.omf_order(&i.schedule);
    for b in [2, 6, 7, 8] {
        assert_eq!(periods[b], Some(1), "block {b}");
    }
    for b in [0, 1, 3, 4, 5, 9] {
        assert_eq!(periods[b], None, "block {b}");
    }
    let top = |t| (10..15).filter(|&b| periods[b] == Some(t)).count();
    assert_eq!((top(0), top(1)), (4, 1));
    println!(
        r#"{{"test":"omf_schedule","before":{},"after":{},"rounds":{},"moves":"{:?}","periods":{periods:?}}}"#,
        i.before, i.after, i.rounds, i.moves
    );

    refused_as(
        m.schedule(&pit::ScheduleSpec { periods: 2, ..spec }, &rank),
        "FMINE-REF-207",
        "limits",
    );
    let none = [pit::Capacity {
        attribute: "nothing",
        ..tonnes[0]
    }];
    refused_as(
        m.schedule(
            &pit::ScheduleSpec {
                capacities: &none,
                ..spec
            },
            &rank,
        ),
        "FMINE-REF-506",
        r#"0 attributes called "nothing""#,
    );
}

#[test]
fn an_lp_guided_omf_schedule_lies_under_its_certified_bound() {
    use fmine_lp::pdlp::PdlpOptions;
    use fmine_optimize::pitflow::slope::{SlopeTable, SlopeTangent};
    use fmine_optimize::toposort::check;
    use fmine_optimize::toposort::search::DISCOUNT_SCALE;

    let p = section();
    let uniform = SlopeTable::uniform(SlopeTangent::new(1, 1).unwrap());
    let m = pit::PitModel::new(&p, "section", &uniform, None, 2).unwrap();
    let limits = [10, 10, 10];
    let tonnes = [pit::Capacity {
        attribute: "tonnes",
        per_unit: 1,
        limits: &limits,
    }];
    let spec = pit::ScheduleSpec {
        value: "value",
        per_unit: 4,
        capacities: &tonnes,
        periods: 3,
        discount: [100_000_000, 1_000_000_000],
        max_rounds: 100,
    };
    let pdlp = PdlpOptions {
        tolerance: 1e-6,
        ..PdlpOptions::default()
    };
    let s = m.lp_schedule(&spec, &pdlp).unwrap();
    let found = s.incumbent();

    // The same hand optimum as the ranked schedule, reached from the LP's starts.
    let f1 = i128::from(DISCOUNT_SCALE) * 10 / 11;
    assert_eq!(
        found.searched[found.best].after,
        -16 * i128::from(DISCOUNT_SCALE) + 21 * f1
    );
    let c = check(s.problem(), found.schedule()).unwrap();
    assert!(c.feasible(), "{c:?}");
    assert_eq!(c.period_values, vec![-16, 21, 0]);

    // The relaxation splits the cone: 5/9 of it in period 0 (all a period holds) and 4/9 in period 1, each ninth worth
    // 5 quarter-units, so its optimum is 25/9 + 20/9.9. The certified bound lies at or above it, and above the
    // schedule's own value, -16 + 21/1.1.
    let lp_best = 25.0 / 9.0 + 20.0 / 9.9;
    let npv = -16.0 + 21.0 / 1.1;
    let bound = s.upper_bound().unwrap();
    println!(
        r#"{{"test":"omf_lp_schedule","status":"{:?}","bound":{bound},"lp_best":{lp_best},"npv":{npv},"gap":{},"starts":{}}}"#,
        s.lp().status(),
        (bound - npv) / bound,
        found.searched.len()
    );
    assert!(
        bound >= lp_best - 1e-9 && bound <= lp_best * (1.0 + 1e-3),
        "{bound} against {lp_best}"
    );
    assert!(npv < bound);

    refused_as(
        m.lp_schedule(&pit::ScheduleSpec { periods: 2, ..spec }, &pdlp),
        "FMINE-REF-207",
        "limits",
    );
}

/// [`an_lp_guided_omf_schedule_lies_under_its_certified_bound`]'s model and schedule: the section, 10 tonnes a period
/// over 3 periods, values at 4 to the unit, 10% a period.
fn with_section_schedule<R>(f: impl FnOnce(&pit::PitModel, &pit::ScheduleSpec<'_>) -> R) -> R {
    use fmine_optimize::pitflow::slope::{SlopeTable, SlopeTangent};

    let p = section();
    let uniform = SlopeTable::uniform(SlopeTangent::new(1, 1).unwrap());
    let m = pit::PitModel::new(&p, "section", &uniform, None, 2).unwrap();
    let limits = [10, 10, 10];
    let tonnes = [pit::Capacity {
        attribute: "tonnes",
        per_unit: 1,
        limits: &limits,
    }];
    let spec = pit::ScheduleSpec {
        value: "value",
        per_unit: 4,
        capacities: &tonnes,
        periods: 3,
        discount: [100_000_000, 1_000_000_000],
        max_rounds: 100,
    };
    f(&m, &spec)
}

/// `values` as a solution file under `fmine_lp::mps::write`'s names, `C` or `R`, in Rust's shortest round-trip form.
fn solution_file(prefix: char, values: &[f64]) -> String {
    use std::fmt::Write as _;
    let mut out = String::from("# Objective value = not read\n");
    for (i, v) in values.iter().enumerate() {
        let _ = writeln!(out, "{prefix}{i} {v:?}");
    }
    out
}

#[test]
fn the_schedule_lp_goes_out_as_mps_and_its_solution_comes_back_as_a_checked_schedule_and_bound() {
    use fmine_lp::mps::{self, Names};
    use fmine_lp::pdlp::PdlpOptions;
    use fmine_optimize::toposort::check;

    with_section_schedule(|m, spec| {
        // Out: the relaxation as MPS, which reads back to the same problem.
        let lp = m.schedule_lp(spec).unwrap();
        let text = mps::write("section", &lp).unwrap();
        let back = mps::read(&text).unwrap();
        assert_eq!(mps::write("section", &back.problem).unwrap(), text);
        assert_eq!(
            (back.problem.rows(), back.problem.cols()),
            (lp.rows(), lp.cols())
        );

        // Back: PDLP's own solution and multipliers, as files under the MPS names, give lp_schedule's schedule and
        // bound bit for bit.
        let pdlp = PdlpOptions {
            tolerance: 1e-6,
            ..PdlpOptions::default()
        };
        let s = m.lp_schedule(spec, &pdlp).unwrap();
        let x = mps::read_values(
            &solution_file('C', s.lp().x()),
            Names::Columns,
            &back.problem,
        )
        .unwrap();
        let y = mps::read_values(
            &solution_file('R', s.lp().bound_multipliers()),
            Names::Rows,
            &back.problem,
        )
        .unwrap();
        let g = m.guided_schedule(spec, x).unwrap();
        assert_eq!(g.incumbent(), s.incumbent());
        assert_eq!((g.problem(), g.factors()), (s.problem(), s.factors()));
        let bound = m.schedule_bound(spec, &y).unwrap().unwrap();
        assert_eq!(Some(bound.to_bits()), s.upper_bound().map(f64::to_bits));

        // A poor solution is only worse: nothing mined gives a checked schedule, and zero multipliers a weaker bound.
        let nothing = m.guided_schedule(spec, vec![0.0; lp.cols()]).unwrap();
        let c = check(nothing.problem(), nothing.incumbent().schedule()).unwrap();
        assert!(c.feasible(), "{c:?}");
        let best = &s.incumbent().searched[s.incumbent().best];
        assert!(nothing.incumbent().searched[nothing.incumbent().best].after <= best.after);
        let weak = m
            .schedule_bound(spec, &vec![0.0; lp.rows()])
            .unwrap()
            .unwrap();
        assert!(weak >= bound, "{weak} below {bound}");
        println!(
            r#"{{"test":"omf_schedule_lp_out_and_back","rows":{},"cols":{},"mps_bytes":{},"bound":{bound},"zero_y_bound":{weak}}}"#,
            lp.rows(),
            lp.cols(),
            text.len()
        );

        // Refused, not clamped: a share just above 1, and counts that do not match the problem.
        let mut over = s.lp().x().to_vec();
        over[0] = f64::from_bits(1.0_f64.to_bits() + 1);
        refused_as(
            m.guided_schedule(spec, over),
            "FMINE-REF-207",
            "not in [0, 1]",
        );
        refused_as(
            m.guided_schedule(spec, vec![0.0; 3]),
            "FMINE-REF-207",
            "3 shares",
        );
        refused_as(
            m.schedule_bound(spec, &[0.0; 3]),
            "FMINE-REF-207",
            "3 multipliers",
        );
    });
}

/// The section's schedule LP as `fmine_lp::mps::write` gives it: the input HiGHS solved for the two fixtures below.
const SECTION_MPS: &str = include_str!("../../testdata/omf_section_schedule.mps");

/// HiGHS's column values for [`SECTION_MPS`] (oracle fixture, §33.2).
const HIGHS_COLUMNS: &str =
    include_str!("../../../../fixtures/fmine-io/omf_section_schedule_highs_columns.txt");

/// HiGHS's row multipliers for [`SECTION_MPS`] (oracle fixture, §33.2).
const HIGHS_ROWS: &str =
    include_str!("../../../../fixtures/fmine-io/omf_section_schedule_highs_rows.txt");

/// The §15.4 loop with an outside solver (bd-06kg). HiGHS's solution of the exported LP, captured once from
/// [`SECTION_MPS`] by `scripts/oracle/highs_lp.py` (its capture records are beside the fixtures, and nothing here
/// regenerates them), comes back through `read_values` as a checked schedule and a certified bound.
#[test]
fn highs_solution_of_the_exported_schedule_lp_comes_back_as_a_checked_schedule_and_bound() {
    use fmine_lp::mps::{self, Names};
    use fmine_optimize::toposort::check;
    use fmine_optimize::toposort::search::DISCOUNT_SCALE;

    with_section_schedule(|m, spec| {
        let lp = m.schedule_lp(spec).unwrap();
        let text = mps::write("section", &lp).unwrap();
        assert!(
            text == SECTION_MPS,
            "the exported LP is no longer the one HiGHS solved: put this text in \
             crates/fmine-io/testdata/omf_section_schedule.mps and capture both fixtures again (CONTRACT §9)\n{text}"
        );
        let x = mps::read_values(HIGHS_COLUMNS, Names::Columns, &lp).unwrap();
        let y = mps::read_values(HIGHS_ROWS, Names::Rows, &lp).unwrap();
        let violation = lp.max_violation(&x).unwrap();
        let g = m.guided_schedule(spec, x).unwrap();
        let found = &g.incumbent().searched[g.incumbent().best];
        let c = check(g.problem(), g.incumbent().schedule()).unwrap();
        let bound = m.schedule_bound(spec, &y).unwrap().unwrap();
        let lp_best = 25.0 / 9.0 + 20.0 / 9.9;
        println!(
            r#"{{"test":"omf_outside_lp","violation":{violation},"bound":{bound},"lp_best":{lp_best},"after":{},"feasible":{}}}"#,
            found.after,
            c.feasible()
        );
        assert!(c.feasible(), "{c:?}");
        let f1 = i128::from(DISCOUNT_SCALE) * 10 / 11;
        assert_eq!(found.after, -16 * i128::from(DISCOUNT_SCALE) + 21 * f1);
        assert!(
            bound >= lp_best - 1e-9 && bound <= lp_best + 1e-6,
            "{bound} against {lp_best}"
        );
    });
}

/// A client's schedule (plan §15.4 `UntrustedCandidate`), read from the block model's own "plan" attribute, is
/// checked exactly and improved by local search to the hand optimum, both under the certified bound. Schedules that
/// break a requirement or a limit are reported with their witnesses and not improved.
#[test]
fn a_clients_schedule_in_the_omf_is_checked_then_improved_and_a_broken_one_is_reported() {
    use fmine_lp::pdlp::PdlpOptions;
    use fmine_optimize::toposort::check;
    use fmine_optimize::toposort::search::DISCOUNT_SCALE;

    with_section_schedule(|m, spec| {
        let s = i128::from(DISCOUNT_SCALE);
        let f1 = s * 10 / 11;

        // Years 1 and 2 are periods 0 and 1.
        let plan = m.periods("plan", 1).unwrap();
        let want: Vec<Option<u32>> = (0..15)
            .map(|b| match b {
                10..=14 => Some(0),
                2 | 6..=8 => Some(1),
                _ => None,
            })
            .collect();
        assert_eq!(plan, want);

        // Feasible: the five top blocks fill period 0 (-20 quarter-units), then the middle three and the ore follow
        // (-12 + 37 = 25).
        let c = m.candidate_schedule(spec, &plan).unwrap();
        assert_eq!(m.omf_order(c.schedule()), plan);
        let k = c.check();
        assert!(k.feasible(), "{k:?}");
        assert_eq!(
            (k.mined, k.period_values.clone(), k.used[0].clone()),
            (9, vec![-20, 25, 0], vec![10, 8, 0])
        );
        assert_eq!(c.value(), -20 * s + 25 * f1);
        // The search reaches the hand optimum, one top block later: -16 + 21 / 1.1 against -20 + 25 / 1.1.
        let i = c.improvement().unwrap();
        assert_eq!((i.before, i.after), (c.value(), -16 * s + 21 * f1));
        let improved = check(c.problem(), &i.schedule).unwrap();
        assert!(improved.feasible(), "{improved:?}");
        assert_eq!(improved.period_values, vec![-16, 21, 0]);
        // Both lie under PDLP's certified bound.
        let pdlp = PdlpOptions {
            tolerance: 1e-6,
            ..PdlpOptions::default()
        };
        let bound = m.lp_schedule(spec, &pdlp).unwrap().upper_bound().unwrap();
        let (given, after) = (c.value() as f64 / s as f64, i.after as f64 / s as f64);
        println!(
            r#"{{"test":"omf_candidate","given":{given},"after":{after},"bound":{bound},"gap_given":{},"gap_after":{},"rounds":{},"moves":"{:?}"}}"#,
            (bound - given) / bound,
            (bound - after) / bound,
            i.rounds,
            i.moves
        );
        assert!(given < after && after < bound, "{given} {after} {bound}");

        // The ore alone in year 1 breaks every requirement it has: the three blocks above it are not mined.
        let mut ore_first = vec![None; 15];
        ore_first[2] = Some(0);
        let c = m.candidate_schedule(spec, &ore_first).unwrap();
        let k = c.check();
        let omf = |b: u32| m.omf_index(b as usize).unwrap();
        let witnesses: Vec<(usize, usize)> = k
            .first_unmet
            .iter()
            .map(|&(b, q)| (omf(b), omf(q)))
            .collect();
        assert_eq!((k.unmet, witnesses), (3, vec![(2, 6), (2, 7), (2, 8)]));
        assert!(k.over_limit.is_empty(), "{k:?}");
        assert_eq!((c.value(), c.improvement()), (37 * s, None));

        // The whole cone in year 1 keeps every requirement but uses 18 of period 0's 10.
        let rushed: Vec<Option<u32>> = plan.iter().map(|t| t.map(|_| 0)).collect();
        let c = m.candidate_schedule(spec, &rushed).unwrap();
        let k = c.check();
        assert_eq!(
            (k.unmet, k.over_limit.clone(), k.used[0].clone()),
            (0, vec![(0, 0)], vec![18, 0, 0])
        );
        assert_eq!((c.value(), c.improvement()), (5 * s, None));

        // Integers read too: cost is 2 at the ore and 1 elsewhere.
        let from_cost: Vec<Option<u32>> = (0..15).map(|b| Some(u32::from(b == 2))).collect();
        assert_eq!(m.periods("cost", 1).unwrap(), from_cost);
        assert_eq!(
            [0, 4, 12, 14, 15].map(|b| m.omf_index(b)),
            [Some(10), Some(14), Some(2), Some(4), None]
        );

        refused_as(
            m.periods("value", -1),
            "FMINE-REF-109",
            r#"block [2, 0, 0] of block model "section" has value 9.25, not a whole number from -1"#,
        );
        refused_as(
            m.periods("revenue", 1),
            "FMINE-REF-109",
            "block [0, 0, 0] of block model \"section\" has revenue 0,",
        );
        refused_as(
            m.periods("huge", -1),
            "FMINE-REF-109",
            "has huge 10000000000000000000,",
        );
        refused_as(
            m.periods("cost", -5_000_000_000),
            "FMINE-REF-109",
            "has cost 1, not a whole number from -5000000000 to -5000000000 + 4294967295",
        );
        refused_as(m.periods("cost", i64::MIN), "FMINE-REF-109", "has cost 1,");
        refused_as(
            m.periods("rock", 1),
            "FMINE-REF-506",
            "is not a Number of integers or floats",
        );
        refused_as(
            m.periods("nothing", 1),
            "FMINE-REF-506",
            r#"0 attributes called "nothing""#,
        );
        refused_as(
            m.periods("corner value", 1),
            "FMINE-REF-506",
            "is on Vertices",
        );
        refused_as(
            m.candidate_schedule(spec, &plan[..14]),
            "FMINE-REF-207",
            "14 entries for 15 parent blocks",
        );
        let mut late = plan.clone();
        late[12] = Some(3);
        refused_as(
            m.candidate_schedule(spec, &late),
            "FMINE-REF-207",
            r#"puts block [2, 0, 2] of block model "section" in period 3, but there are 3"#,
        );
    });
}

/// Plan §15.5's shell-to-design loss on the section: designed pits as boxes against the certified ultimate pit, for
/// value and tonnes. With the design's sides on sub-cell faces every enclosure is exact; off them, each encloses the
/// hand value, more tightly at each depth.
#[test]
fn a_designed_pit_against_the_certified_shell_gives_its_loss_in_exact_enclosures() {
    use fmine_optimize::pitflow::slope::{SlopeTable, SlopeTangent};

    let p = section();
    let uniform = SlopeTable::uniform(SlopeTangent::new(1, 1).unwrap());
    let m = pit::PitModel::new(&p, "section", &uniform, None, 2).unwrap();
    let pit = m.ultimate_pit("value", 4).unwrap();
    let shell = pit.in_pit();
    let value = m.weights("value", 4).unwrap();
    let tonnes = m.weights("tonnes", 10).unwrap();

    // At depth 1 each corner-column block is four of its eight sub-cells inside, exactly, and the rest are whole.
    let d = m.design("design", 1).unwrap();
    assert_eq!(d.cells_per_block(), 8);
    let per_block: Vec<[u32; 2]> = (0..15).map(|b| d.cells(b).unwrap()).collect();
    for (b, c) in m.omf_order(&per_block).iter().enumerate() {
        let want = if b % 5 == 0 || b % 5 == 4 { 4 } else { 8 };
        assert_eq!(*c, [want, want], "block {b}");
    }
    assert_eq!(d.cells(15), None);

    // In eighths of a quarter-unit: the pit is worth 5 and the design -7, the nine middle blocks (5) less half of the
    // six corner ones (-12). The design leaves half of the pit's two top corners (-4) and adds the two bottom blocks
    // beside the ore and the lower halves of the corner columns (-16). So it costs 12.
    let s = d.against(shell, &value).unwrap();
    assert_eq!(
        (s.shell, s.design, s.left, s.added, s.loss),
        (40, [-56, -56], [-32, -32], [-128, -128], [96, 96])
    );
    // 14.4 t designed against the pit's 10.8 t, in tenths over eight.
    let t = d.against(shell, &tonnes).unwrap();
    assert_eq!((t.shell, t.design), (864, [1152, 1152]));
    assert_eq!(d.total(&tonnes).unwrap(), [1152, 1152]);

    // Leaving the top layer out breaks the slopes, so the deep design is worth more than the pit, 9 against 5: the
    // loss is negative. The design is not checked against the slopes.
    let deep = m.design("deep design", 1).unwrap();
    let s = deep.against(shell, &value).unwrap();
    assert_eq!(
        (s.design, s.left, s.added, s.loss),
        ([72, 72], [-160, -160], [-128, -128], [-32, -32])
    );

    // The rough design's corner columns are 0.52 inside, so it is worth 5 - 6 x 4 x 0.52 = -7.48 quarter-units. Each
    // depth encloses that with half the width of the one before.
    let mut width = i128::MAX;
    for depth in 1..=4u8 {
        let r = m.design("rough design", depth).unwrap();
        let cells = i128::from(r.cells_per_block());
        let [lo, hi] = r.total(&value).unwrap();
        println!(
            r#"{{"test":"omf_design","depth":{depth},"cells":{cells},"design":[{},{}],"true":-7.48}}"#,
            lo as f64 / cells as f64,
            hi as f64 / cells as f64
        );
        assert!(
            lo * 25 <= -187 * cells && -187 * cells <= hi * 25,
            "depth {depth}: [{lo}, {hi}] / {cells}"
        );
        let w = (hi - lo) * 4096 / cells;
        assert!(
            w * 2 == width || width == i128::MAX,
            "depth {depth}: {w} after {width}"
        );
        width = w;
        let s = r.against(shell, &value).unwrap();
        assert_eq!(s.loss, [s.shell - s.design[1], s.shell - s.design[0]]);
    }

    refused_as(
        m.design("too wide", 1),
        "FMINE-REF-109",
        "beyond its blocks from [0.0, 0.0, 0.0] to [62.5, 12.5, 37.5]",
    );
    refused_as(
        m.design("nothing", 1),
        "FMINE-REF-506",
        r#"no element is called "nothing""#,
    );
    refused_as(
        m.design("section", 1),
        "FMINE-REF-506",
        "is a BlockModel, not a Surface",
    );
    refused_as(m.design("design", 99), "FMINE-REF-109", "depth 99");
    refused_as(
        d.against(&shell[..14], &value),
        "FMINE-REF-207",
        "14 shell flags and 15 weights for 15 blocks",
    );
    refused_as(
        d.total(&value[..3]),
        "FMINE-REF-207",
        "15 shell flags and 3 weights for 15 blocks",
    );
}
