// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 21: `gii`, the GIFTI surface reader (`mrisReadGIFTIdanum`) on
//! generated GIFTI files (surfaces with their metadata and coordinate systems, label tables, shape,
//! label, vector, colour, matrix and statistics arrays, sparse storage, every encoding and both
//! orders, broken forms), with a surface passed in or not, the overlays asked for or not, a colour
//! table given or not; and [`surface_file`], a mesh as a GIFTI file, for `srd`'s `MRISread`.

use super::sfile::utf8;
use super::srd::{Mesh, mesh};
use super::*;

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_surf::alloc;
use larmorx_freesurfer_surf::gifti::{self, GiftiRead, OverlayInfo};

/// A data array to write: its intent, data type, order, dimensions, values (row by row), encoding
/// (0 ASCII, 1 Base64Binary, 2 GZipBase64Binary), metadata and coordinate systems.
pub(super) struct Arr {
    intent: String,
    dtype: &'static str,
    col_major: bool,
    dims: Vec<i64>,
    vals: Vec<f64>,
    enc: u64,
    meta: Vec<(String, String)>,
    cs: Vec<(Option<String>, Option<String>, [f64; 16])>,
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(b: &[u8]) -> String {
    let mut s = String::new();
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..4 {
            if k <= c.len() {
                s.push(B64[((n >> (18 - 6 * k)) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn bytes_of(dtype: &str, v: f64) -> Vec<u8> {
    match dtype {
        "NIFTI_TYPE_FLOAT32" => (v as f32).to_le_bytes().to_vec(),
        "NIFTI_TYPE_FLOAT64" => v.to_le_bytes().to_vec(),
        "NIFTI_TYPE_INT32" => (v as i32).to_le_bytes().to_vec(),
        "NIFTI_TYPE_UINT32" => (v as u32).to_le_bytes().to_vec(),
        "NIFTI_TYPE_INT16" => (v as i16).to_le_bytes().to_vec(),
        "NIFTI_TYPE_UINT16" => (v as u16).to_le_bytes().to_vec(),
        "NIFTI_TYPE_INT8" => (v as i8).to_le_bytes().to_vec(),
        _ => vec![v as u8],
    }
}

fn text_of(dtype: &str, v: f64) -> String {
    match dtype {
        "NIFTI_TYPE_FLOAT32" => format!("{}", v as f32),
        "NIFTI_TYPE_FLOAT64" => format!("{v}"),
        "NIFTI_TYPE_INT32" => format!("{}", v as i32),
        "NIFTI_TYPE_UINT32" => format!("{}", v as u32),
        "NIFTI_TYPE_INT16" => format!("{}", v as i16),
        "NIFTI_TYPE_UINT16" => format!("{}", v as u16),
        "NIFTI_TYPE_INT8" => format!("{}", v as i8),
        _ => format!("{}", v as u8),
    }
}

fn cdata(s: &str) -> String {
    format!("<![CDATA[{s}]]>")
}

impl Arr {
    fn new(intent: &str, dtype: &'static str, rows: i64, cols: i64, vals: Vec<f64>) -> Arr {
        Arr {
            intent: intent.into(),
            dtype,
            col_major: false,
            dims: if cols == 1 {
                vec![rows]
            } else {
                vec![rows, cols]
            },
            vals,
            enc: 0,
            meta: Vec::new(),
            cs: Vec::new(),
        }
    }

    fn xml(&self) -> String {
        let rows = self.dims[0];
        let n = self.vals.len() as i64;
        let cols = if rows > 0 { n / rows } else { 0 };
        let order: Vec<f64> = if self.col_major && cols > 0 {
            (0..cols)
                .flat_map(|j| (0..rows).map(move |i| (i, j)))
                .filter_map(|(i, j)| self.vals.get((i * cols + j) as usize).copied())
                .collect()
        } else {
            self.vals.clone()
        };
        let mut s = format!(
            "<DataArray Intent=\"{}\" DataType=\"{}\" ArrayIndexingOrder=\"{}\" Dimensionality=\"{}\"",
            self.intent,
            self.dtype,
            if self.col_major {
                "ColumnMajorOrder"
            } else {
                "RowMajorOrder"
            },
            self.dims.len()
        );
        for (k, d) in self.dims.iter().enumerate() {
            s += &format!(" Dim{k}=\"{d}\"");
        }
        let enc = ["ASCII", "Base64Binary", "GZipBase64Binary"][self.enc as usize];
        s += &format!(
            " Encoding=\"{enc}\" Endian=\"LittleEndian\" ExternalFileName=\"\" ExternalFileOffset=\"\">\n"
        );
        if !self.meta.is_empty() {
            s += "<MetaData>\n";
            for (k, v) in &self.meta {
                s += &format!(
                    "<MD><Name>{}</Name><Value>{}</Value></MD>\n",
                    cdata(k),
                    cdata(v)
                );
            }
            s += "</MetaData>\n";
        }
        for (ds, xs, m) in &self.cs {
            s += "<CoordinateSystemTransformMatrix>\n";
            if let Some(ds) = ds {
                s += &format!("<DataSpace>{}</DataSpace>\n", cdata(ds));
            }
            if let Some(xs) = xs {
                s += &format!("<TransformedSpace>{}</TransformedSpace>\n", cdata(xs));
            }
            let t: Vec<String> = m.iter().map(|x| format!("{x}")).collect();
            s += &format!("<MatrixData>{}</MatrixData>\n", t.join(" "));
            s += "</CoordinateSystemTransformMatrix>\n";
        }
        let data = match self.enc {
            0 => {
                let t: Vec<String> = order.iter().map(|&v| text_of(self.dtype, v)).collect();
                t.join(" ")
            }
            1 => base64(
                &order
                    .iter()
                    .flat_map(|&v| bytes_of(self.dtype, v))
                    .collect::<Vec<u8>>(),
            ),
            _ => {
                let raw: Vec<u8> = order
                    .iter()
                    .flat_map(|&v| bytes_of(self.dtype, v))
                    .collect();
                let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
                z.write_all(&raw).expect("zlib");
                base64(&z.finish().expect("zlib"))
            }
        };
        s += &format!("<Data>{data}</Data>\n</DataArray>\n");
        s
    }
}

/// A GIFTI file of the arrays, with a label table (key, name, colours or none) and a version.
fn file(
    version: Option<&str>,
    labels: &[(i64, String, Option<[f32; 4]>)],
    arrs: &[Arr],
) -> Vec<u8> {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<GIFTI");
    if let Some(v) = version {
        s += &format!(" Version=\"{v}\"");
    }
    s += &format!(" NumberOfDataArrays=\"{}\">\n", arrs.len());
    if !labels.is_empty() {
        s += "<LabelTable>\n";
        for (k, n, c) in labels {
            s += &format!("<Label Key=\"{k}\"");
            if let Some(c) = c {
                s += &format!(
                    " Red=\"{}\" Green=\"{}\" Blue=\"{}\" Alpha=\"{}\"",
                    c[0], c[1], c[2], c[3]
                );
            }
            s += &format!(">{}</Label>\n", cdata(n));
        }
        s += "</LabelTable>\n";
    }
    for a in arrs {
        s += &a.xml();
    }
    s += "</GIFTI>\n";
    s.into_bytes()
}

fn coord_value(g: &mut Gen, x: f32, heavy: bool) -> f64 {
    if heavy && g.chance(60) {
        g.pick(&[f64::NAN, f64::INFINITY, 1e30, -0.0])
    } else {
        f64::from(x)
    }
}

/// The coordinates' metadata: hemisphere, volume geometry (all 19, some, or odd numbers), centre,
/// group-average area, command lines.
fn coord_meta(g: &mut Gen, heavy: bool) -> Vec<(String, String)> {
    let mut m = Vec::new();
    if g.chance(2) {
        let h = g.pick(&["CortexLeft", "CortexRight", "Cerebellum", "cortexleft"]);
        m.push(("AnatomicalStructurePrimary".into(), h.into()));
    }
    if g.chance(2) {
        let all = g.chance(2);
        for (k, name) in [
            "VolGeomWidth",
            "VolGeomHeight",
            "VolGeomDepth",
            "VolGeomXsize",
            "VolGeomYsize",
            "VolGeomZsize",
            "VolGeomX_R",
            "VolGeomX_A",
            "VolGeomX_S",
            "VolGeomY_R",
            "VolGeomY_A",
            "VolGeomY_S",
            "VolGeomZ_R",
            "VolGeomZ_A",
            "VolGeomZ_S",
            "VolGeomC_R",
            "VolGeomC_A",
            "VolGeomC_S",
            "VolGeomFname",
        ]
        .iter()
        .enumerate()
        {
            if !all && g.chance(4) {
                continue;
            }
            let v = if k == 18 {
                g.pick(&["/subj/mri/orig.mgz", "", "orig.mgz", "a b"])
                    .to_string()
            } else if k < 3 {
                if heavy && g.chance(10) {
                    g.pick(&["x", " 12", "3.5", "99999999999"]).to_string()
                } else {
                    format!("{}", g.int(1, 256))
                }
            } else if heavy && g.chance(10) {
                g.pick(&["nan", "1e40", "-", "0x1p-3"]).to_string()
            } else {
                format!("{}", g.int(-100, 100) as f32 / 8.0)
            };
            m.push(((*name).into(), v));
        }
    }
    for name in ["SurfaceCenterX", "SurfaceCenterY", "SurfaceCenterZ"] {
        if g.chance(3) {
            m.push((name.into(), format!("{}", g.int(-50, 50) as f32 / 4.0)));
        }
    }
    if g.chance(4) {
        m.push((
            "TAG_GROUP_AVG_SURFACE_AREA".into(),
            format!("{}", g.int(1, 9000)),
        ));
    }
    if g.chance(3) {
        let n = g.int(0, 3);
        let mut said = if g.chance(5) { n + g.int(-1, 2) } else { n };
        if heavy && g.chance(40) {
            // more than 1,000 command lines: FreeSurfer never returns (FS-382)
            said = 1000 + g.int(1, 3);
        }
        m.push(("NUM_TAG_CMDLINE".into(), format!("{said}")));
        for k in 0..n {
            m.push((
                format!("TAG_CMDLINE#{k}"),
                format!("mris_make_surfaces -{k} subj lh"),
            ));
        }
    }
    m
}

/// A mesh's coordinate and face arrays.
fn surface_arrays(g: &mut Gen, m: &Mesh, heavy: bool) -> (Arr, Arr) {
    let nv = m.v.len() as i64;
    let nf = m.f.len() as i64;
    let mut cv = Vec::new();
    for p in &m.v {
        for &c in p {
            cv.push(coord_value(g, c, heavy));
        }
    }
    let cdt = g.pick(&[
        "NIFTI_TYPE_FLOAT32",
        "NIFTI_TYPE_FLOAT32",
        "NIFTI_TYPE_FLOAT64",
        "NIFTI_TYPE_INT32",
    ]);
    let mut coords = Arr::new("NIFTI_INTENT_POINTSET", cdt, nv, 3, cv);
    coords.enc = g.below(3);
    coords.col_major = g.chance(12);
    coords.meta = coord_meta(g, heavy);
    for _ in 0..g.pick(&[0, 0, 1, 2]) {
        let ds = if g.chance(12) {
            None
        } else {
            Some(
                g.pick(&[
                    "NIFTI_XFORM_SCANNER_ANAT",
                    "NIFTI_XFORM_UNKNOWN",
                    "NIFTI_XFORM_TALAIRACH",
                ])
                .to_string(),
            )
        };
        let xs = if g.chance(12) {
            None
        } else {
            Some(
                g.pick(&[
                    "NIFTI_XFORM_TALAIRACH",
                    "NIFTI_XFORM_SCANNER_ANAT",
                    "NIFTI_XFORM_MNI_152",
                ])
                .to_string(),
            )
        };
        let mut mx = [0.0f64; 16];
        for (k, x) in mx.iter_mut().enumerate() {
            *x = if k % 5 == 0 {
                1.0
            } else {
                g.int(-8, 8) as f64 / 4.0
            };
        }
        coords.cs.push((ds, xs, mx));
    }
    let mut fv = Vec::new();
    for f in &m.f {
        for &x in f {
            let mut x = x as f64;
            if heavy && g.chance(80) {
                x = g.pick(&[-1.0, nv as f64, 256.0]);
            }
            fv.push(x);
        }
    }
    let fdt = if nv < 256 && g.chance(8) {
        "NIFTI_TYPE_UINT8"
    } else if g.chance(10) {
        "NIFTI_TYPE_FLOAT32"
    } else {
        "NIFTI_TYPE_INT32"
    };
    let mut faces = Arr::new("NIFTI_INTENT_TRIANGLE", fdt, nf, 3, fv);
    faces.enc = g.below(3);
    faces.col_major = g.chance(12);
    if nf == 0 && g.chance(2) {
        faces.dims = vec![0, 3];
    }
    (coords, faces)
}

/// A mesh as a GIFTI surface file (for `srd`'s `MRISread`): its coordinate and face arrays, in
/// either order, now and then a version "1", a shape array after them.
pub(super) fn surface_file(g: &mut Gen, m: &Mesh, heavy: bool) -> Vec<u8> {
    let (c, f) = surface_arrays(g, m, heavy);
    let mut arrs = if g.chance(4) { vec![f, c] } else { vec![c, f] };
    if g.chance(4) {
        let nv = m.v.len() as i64;
        let vals = (0..nv).map(|_| g.int(-20, 20) as f64 / 8.0).collect();
        arrs.push(Arr::new(
            "NIFTI_INTENT_SHAPE",
            "NIFTI_TYPE_FLOAT32",
            nv,
            1,
            vals,
        ));
    }
    let v = g.pick(&["1.0", "1.0", "1", "1.00"]);
    file(Some(v), &[], &arrs)
}

const STAT_INTENTS: [&str; 5] = [
    "NIFTI_INTENT_NONE",
    "NIFTI_INTENT_TTEST",
    "NIFTI_INTENT_ZSCORE",
    "NIFTI_INTENT_TIME_SERIES",
    "NIFTI_INTENT_LOG10PVAL",
];

/// A GIFTI file for `gii`: its bytes and the number of vertices its data arrays use.
fn gii_file(g: &mut Gen, heavy: bool) -> (Vec<u8>, i64, i64) {
    let m = mesh(g, heavy);
    let mut nv = m.v.len() as i64;
    let mut arrs: Vec<Arr> = Vec::new();
    let surface = !g.chance(4);
    let sparse = g.chance(12);
    let mut nidx = nv;
    if sparse {
        // a node index first (FreeSurfer reads it only when it lists every vertex: FS-390)
        nidx = if g.chance(2) { nv } else { g.int(1, nv) };
        let mut vals: Vec<f64> = (0..nidx).map(|k| k as f64).collect();
        if g.chance(3) {
            vals.reverse();
        }
        if heavy && g.chance(5) {
            vals[0] = g.pick(&[-1.0, nv as f64]);
        }
        arrs.push(Arr::new(
            "NIFTI_INTENT_NODE_INDEX",
            "NIFTI_TYPE_INT32",
            nidx,
            1,
            vals,
        ));
    }
    if surface {
        let (c, f) = surface_arrays(g, &m, heavy);
        if g.chance(3) {
            arrs.push(f);
            arrs.push(c);
        } else {
            arrs.push(c);
            arrs.push(f);
        }
    } else if g.chance(3) {
        nv = g.int(1, 8);
        nidx = nv;
    }
    let mut labels: Vec<(i64, String, Option<[f32; 4]>)> = Vec::new();
    let nd = g.int(0, 3);
    for _ in 0..nd {
        let rows = if sparse { nidx } else { nv };
        let rows = if heavy && g.chance(12) {
            rows + g.pick(&[-1, 1])
        } else {
            rows
        };
        match g.below(9) {
            0 | 1 => {
                let vals = (0..rows).map(|_| g.int(-40, 40) as f64 / 8.0).collect();
                let mut a = Arr::new("NIFTI_INTENT_SHAPE", "NIFTI_TYPE_FLOAT32", rows, 1, vals);
                if g.chance(2) {
                    let n = g.pick(&[
                        "lh.thickness",
                        "/s/surf/rh.sulc.gii",
                        "curv.gii",
                        "x/",
                        "",
                        "a.b.gii",
                    ]);
                    a.meta.push(("Name".into(), n.into()));
                }
                if !g.chance(10) {
                    let t = g.pick(&["Thickness", "SulcalDepth", "CurvatureRadial"]);
                    a.meta.push(("ShapeDataType".into(), t.into()));
                }
                a.enc = g.below(3);
                arrs.push(a);
            }
            2 => {
                if labels.is_empty() {
                    let nl = g.int(1, 5);
                    let colours = !g.chance(4);
                    for k in 0..nl {
                        let key = if g.chance(4) { g.int(0, 9) } else { k };
                        let key = if heavy && g.chance(20) { -1 } else { key };
                        let c = colours.then(|| {
                            [
                                g.int(0, 8) as f32 / 8.0,
                                g.int(0, 8) as f32 / 8.0,
                                g.int(0, 8) as f32 / 8.0,
                                g.pick(&[0.0, 1.0]),
                            ]
                        });
                        labels.push((key, format!("label{k}"), c));
                    }
                }
                let keys: Vec<i64> = labels.iter().map(|l| l.0).collect();
                let vals = (0..rows)
                    .map(|_| {
                        if g.chance(6) {
                            g.int(0, 12) as f64
                        } else {
                            g.pick(&keys) as f64
                        }
                    })
                    .collect();
                let dt = g.pick(&[
                    "NIFTI_TYPE_INT32",
                    "NIFTI_TYPE_INT32",
                    "NIFTI_TYPE_UINT32",
                    "NIFTI_TYPE_FLOAT32",
                ]);
                let mut a = Arr::new("NIFTI_INTENT_LABEL", dt, rows, 1, vals);
                a.enc = g.below(3);
                arrs.push(a);
            }
            3 => {
                let vals = (0..rows * 3).map(|_| g.int(-9, 9) as f64 / 4.0).collect();
                arrs.push(Arr::new(
                    "NIFTI_INTENT_VECTOR",
                    "NIFTI_TYPE_FLOAT32",
                    rows,
                    3,
                    vals,
                ));
            }
            4 => {
                let four = g.chance(2);
                let n = if four { 4 } else { 3 };
                let big = g.chance(3);
                let vals = (0..rows * n)
                    .map(|_| {
                        if heavy && g.chance(30) {
                            g.pick(&[f64::NAN, 1e12, -0.5])
                        } else if big {
                            g.int(0, 300) as f64
                        } else {
                            g.int(0, 8) as f64 / 8.0
                        }
                    })
                    .collect();
                let intent = if four {
                    "NIFTI_INTENT_RGBA_VECTOR"
                } else {
                    "NIFTI_INTENT_RGB_VECTOR"
                };
                arrs.push(Arr::new(intent, "NIFTI_TYPE_FLOAT32", rows, n, vals));
            }
            5 => {
                let vals = (0..rows * 9).map(|k| k as f64).collect();
                arrs.push(Arr::new(
                    "NIFTI_INTENT_GENMATRIX",
                    "NIFTI_TYPE_FLOAT32",
                    rows,
                    9,
                    vals,
                ));
            }
            _ => {
                let intent = g.pick(&STAT_INTENTS);
                let runs = g.int(1, 3);
                for _ in 0..runs {
                    let vals = (0..rows).map(|_| g.int(-30, 30) as f64 / 4.0).collect();
                    let dt = g.pick(&[
                        "NIFTI_TYPE_FLOAT32",
                        "NIFTI_TYPE_FLOAT64",
                        "NIFTI_TYPE_INT16",
                    ]);
                    let mut a = Arr::new(intent, dt, rows, 1, vals);
                    a.enc = g.below(3);
                    if g.chance(4) {
                        a.dims = vec![rows, 1];
                    }
                    arrs.push(a);
                }
            }
        }
    }
    if !labels.is_empty() && g.chance(8) {
        let l = labels[0].clone();
        labels.push(l);
    }
    let version = match g.below(12) {
        0 => Some("1"),
        1 => Some("2.0"),
        2 => None,
        _ => Some("1.0"),
    };
    let n_da = arrs.len() as i64;
    let mut b = file(version, &labels, &arrs);
    if heavy && g.chance(15) {
        let cut = g.int(0, b.len() as i64) as usize;
        b.truncate(cut);
    }
    (b, nv, n_da)
}

pub(super) fn gii_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(4);
    ix(t, g.int(0, 3));
    let nfiles = g.int(1, 2);
    let mut info = Vec::new();
    ix(t, nfiles);
    for k in 0..nfiles {
        let (b, nv, nda) = if g.chance(25) {
            (
                (0..g.int(0, 40)).map(|_| g.below(256) as u8).collect(),
                3,
                0,
            )
        } else {
            gii_file(g, heavy)
        };
        let name = format!("{}s{k}.gii", if g.chance(4) { "d/" } else { "" });
        hx(t, name.as_bytes());
        hx(t, &b);
        info.push((name, nv, nda));
    }
    // the kept surface: as many vertices as the first file's arrays use, now and then another
    // count, or none
    let nkeep = match g.below(4) {
        0 => 0,
        1 => g.int(1, 8),
        _ => info[0].1,
    };
    ix(t, nkeep);
    // the colour table: none (-1), or its entries
    if g.chance(3) {
        let n = g.int(0, 6);
        ix(t, n);
        for k in 0..n {
            let present = !g.chance(5);
            ix(t, i64::from(present));
            if present {
                hx(t, format!("user{k}").as_bytes());
                for _ in 0..3 {
                    ix(t, g.int(0, 255));
                }
                ix(t, g.int(0, 1) * 255);
            }
        }
    } else {
        ix(t, -1);
    }
    let nops = g.int(1, 3);
    ix(t, nops);
    for _ in 0..nops {
        let (name, _, nda) = &info[g.below(info.len() as u64) as usize];
        let name = if g.chance(30) {
            "none.gii".to_string()
        } else {
            name.clone()
        };
        hx(t, name.as_bytes());
        let danum = if g.chance(4) && *nda > 0 {
            if heavy && g.chance(6) {
                g.pick(&[*nda, -2])
            } else {
                g.int(0, nda - 1)
            }
        } else {
            -1
        };
        ix(t, danum);
        ix(t, g.int(0, 7));
    }
}

fn dump_gii_surface(o: &mut Out, s: &Mris) {
    for x in [s.nvertices, s.nfaces, s.r#type, s.hemisphere] {
        o.i(i64::from(x));
    }
    o.h(Some(s.fname.as_bytes()));
    o.i(i64::from(s.useRealRAS));
    super::life::dump_vg(&s.vg, o);
    for x in [s.xctr, s.yctr, s.zctr, s.group_avg_surface_area] {
        o.f(x);
    }
    o.i(i64::from(s.ncmds));
    for k in 0..s.ncmds.max(0) as usize {
        o.h(s.cmdlines.get(k).and_then(|c| c.as_deref()));
    }
    match &s.SRASToTalSRAS_ {
        None => o.s("NULL"),
        Some(m) => {
            for r in 1..=4 {
                for c in 1..=4 {
                    o.f(m.relt(r, c));
                }
            }
        }
    }
    match &s.ct {
        None => o.s("NULL"),
        Some(ct) => {
            o.i(i64::from(ct.nentries()));
            o.i(i64::from(ct.version));
            o.h(Some(&ct.fname));
            for e in &ct.entries {
                match e {
                    None => o.s("-"),
                    Some(e) => {
                        // a colour array's table: its alpha is memory contents in FreeSurfer
                        // (FS-393), not printed
                        let alpha = !e.name.starts_with(b"Unknown_Label_");
                        o.h(Some(&e.name));
                        for x in [e.ri, e.gi, e.bi] {
                            o.i(i64::from(x));
                        }
                        if alpha {
                            o.i(i64::from(e.ai));
                        }
                        for x in [e.rf, e.gf, e.bf] {
                            o.f(x);
                        }
                        if alpha {
                            o.f(e.af);
                        }
                    }
                }
            }
        }
    }
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        for x in [v.x, v.y, v.z, v.curv, v.val, v.stat, v.dx, v.dy, v.dz] {
            o.f(x);
        }
        o.i(i64::from(v.annotation));
        o.i(i64::from(s.topology(vno).num));
    }
    for fno in 0..s.nfaces.max(0) as usize {
        for x in s.face(fno).v {
            o.i(i64::from(x));
        }
    }
}

pub(super) fn gii_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    cx.work = Some(dir.to_string_lossy().into_owned());
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let gd = t.i();
    if gd & 1 != 0 {
        cx.g.gdiag |= DIAG_SHOW;
    }
    if gd & 2 != 0 {
        cx.g.gdiag |= DIAG_VERBOSE;
    }
    let nin = t.i();
    for _ in 0..nin {
        let (n, b) = (utf8(t.h()), t.h());
        std::fs::write(dir.join(&n), b).expect("an input file");
    }
    let env = Environment::from_pairs(Vec::<(String, String)>::new());
    let nkeep = t.i();
    let mut keep = if nkeep > 0 {
        let mut out = cx.a.clone();
        let mut err = cx.b.clone();
        Some(alloc::alloc(
            &mut cx.g,
            &mut out,
            &mut err,
            &cx.sg,
            nkeep as i32,
            0,
        )?)
    } else {
        None
    };
    let nct = t.i();
    let ctab = (nct >= 0).then(|| {
        let mut ct = ColorTable {
            fname: b"user.ctab".to_vec(),
            version: 2,
            ..ColorTable::default()
        };
        for _ in 0..nct {
            if t.i() == 0 {
                ct.entries.push(None);
                continue;
            }
            let name = t.h();
            let (ri, gi, bi, ai) = (t.i() as i32, t.i() as i32, t.i() as i32, t.i() as i32);
            ct.entries.push(Some(Box::new(ColorTableEntry {
                name,
                ri,
                gi,
                bi,
                ai,
                rf: ri as f32 / 255.0,
                gf: gi as f32 / 255.0,
                bf: bi as f32 / 255.0,
                af: ai as f32 / 255.0,
                tissue_type: 0,
                count: 0,
            })));
        }
        ct
    });
    let nops = t.i();
    let d = Some(dir);
    for _ in 0..nops {
        let n = utf8(t.h());
        let danum = t.i() as i32;
        let flags = t.i();
        let mut out = cx.a.clone();
        let mut err = cx.b.clone();
        let mut io = coords::Io {
            out: &mut out,
            err: &mut err,
            env: &env,
        };
        let mut ov: Vec<OverlayInfo> = Vec::new();
        let passed = if flags & 1 != 0 { keep.as_mut() } else { None };
        let had = passed.is_some();
        let r = gifti::mris_read_gifti_file(
            &mut cx.g,
            &mut cx.sg,
            &mut io,
            d,
            &n,
            passed,
            (flags & 2 != 0).then_some(&mut ov),
            danum,
            if flags & 4 != 0 { ctab.as_ref() } else { None },
        )?;
        match &r {
            GiftiRead::Null => o.s("NULL"),
            GiftiRead::Passed => {
                o.s("PASSED");
                dump_gii_surface(o, keep.as_ref().expect("the kept surface"));
            }
            GiftiRead::New(m) => {
                o.s("NEW");
                dump_gii_surface(o, m);
            }
        }
        if had {
            o.s("KEPT");
            dump_gii_surface(o, keep.as_ref().expect("the kept surface"));
        }
        o.i(ov.len() as i64);
        for v in &ov {
            o.h(v.foverlay.as_deref());
            o.i(i64::from(v.otype));
            o.i(i64::from(v.gifti_intent));
            o.h(Some(&v.shapedatatype));
            o.i(i64::from(v.format));
            o.i(i64::from(v.n_vals_per_vertex));
            match &v.overlaymri {
                None => o.s("NULL"),
                Some(m) => {
                    for x in [m.width, m.height, m.depth, m.nframes] {
                        o.i(i64::from(x));
                    }
                    for x in 0..m.width {
                        let val = mri_get_vox_val(&mut cx.g, &mut err_sink(), m, x, 0, 0, 0, 0)?;
                        o.f(val);
                    }
                }
            }
        }
    }
    Ok(())
}

fn err_sink() -> Vec<u8> {
    Vec::new()
}
