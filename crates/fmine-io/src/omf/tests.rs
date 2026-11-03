//! OMF files built here, member by member, read back whole; then one defect at a time, each refused.

use super::parquet::testkit::{Col, Opts, gzip_stored, nullable as ncol, required as rcol, write};
use super::parquet::{Logical, Physical, Values};
use super::*;
use crate::zip_inflate::{write_stored_zip, write_stored_zip_with_crcs};

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

fn pq(cols: &[Col], rows: usize) -> Vec<u8> {
    write(cols, rows, Opts::default())
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

#[test]
fn a_file_of_every_geometry_and_attribute_reads_back() {
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

    let file = b.file(&[points, lines, surface, grid, blocks, tensor, composite].join(","));
    let p = read(&file).unwrap();

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
