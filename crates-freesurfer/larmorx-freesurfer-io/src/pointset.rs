// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Point sets (`include/pointset.h`, `utils/pointset.cpp`; items 3.1-33 and 3.1-34): the class
//! `fsPointSet` and its writers (`save` as JSON, `save_as_ctrlpoint`, `print`, `printCoords`,
//! `writeCoords`, `writeCentroidTable`) and `loadfsPointSet` (JSON read with `larmorx_core::json`,
//! track C's port of nlohmann/json 3.2.0, with FreeSurfer's copy's `operator[]` message).
//!
//! Numbers written to a stream (`<<` of a `double`) use the stream's default format, `%g` with 6
//! digits; the centroid table's are `%.4f` (`std::fixed`, `setprecision(4)`). Files that do not
//! open are not written, silently (an `ofstream` that failed), except in `writeCentroidTable`.
//!
//! D13: FS-431 (`remove` of an index outside the set: both modes leave the set unchanged), FS-433
//! (a point-set file that is not JSON or not a point set: FreeSurfer's uncaught exception ends the
//! program with `abort`; by default a clear error, `reproduce` the abort).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_431, FS_433};
use larmorx_core::json::{self, Dialect, Value};
use larmorx_freesurfer_core::cstd::{fmt_f, format_double};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;

/// `fsPointSet::Point`.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub value: f64,
    pub index: i32,
    pub count: i32,
    pub labelname: Vec<u8>,
}

impl Default for Point {
    /// `Point()`: (0, 0, 0), value 1, index and count 0, no label name.
    fn default() -> Self {
        Point::new(0.0, 0.0, 0.0, 1.0)
    }
}

impl Point {
    /// `Point(x, y, z, value)`.
    pub fn new(x: f64, y: f64, z: f64, value: f64) -> Point {
        Point {
            x,
            y,
            z,
            value,
            index: 0,
            count: 0,
            labelname: Vec::new(),
        }
    }
}

/// `fsPointSet`.
#[derive(Clone, Debug, PartialEq)]
pub struct PointSet {
    pub include_zero_count: i32,
    pub include_zero_index: i32,
    pub vox2ras: Vec<u8>,
    pub points: Vec<Point>,
}

impl Default for PointSet {
    fn default() -> Self {
        PointSet {
            include_zero_count: 1,
            include_zero_index: 1,
            vox2ras: b"scanner_ras".to_vec(),
            points: Vec::new(),
        }
    }
}

/// `os << x` for a `double` (the stream's default: `%g`).
fn g6(x: f64) -> String {
    format_double("%g", x).expect("%g")
}

fn write_file(dir: Option<&Path>, name: &str, bytes: &[u8]) -> bool {
    let p = larmorx_freesurfer_core::utils::resolve(dir, name);
    std::fs::write(p, bytes).is_ok()
}

impl PointSet {
    /// `npoints()`.
    pub fn npoints(&self) -> i32 {
        self.points.len() as i32
    }

    /// `add(point)`: its place.
    pub fn add(&mut self, p: Point) -> i32 {
        self.points.push(p);
        self.points.len() as i32 - 1
    }

    /// `add(x, y, z, value)`: a point of index and count 0; its place.
    pub fn add_xyz(&mut self, x: f64, y: f64, z: f64, value: f64) -> i32 {
        self.add(Point::new(x, y, z, value))
    }

    /// `set(i, x, y, z, value)`: 0, or 1 for a place outside the set.
    pub fn set_xyz(&mut self, i: i32, x: f64, y: f64, z: f64, value: f64) -> i32 {
        let Some(p) = usize::try_from(i).ok().and_then(|i| self.points.get_mut(i)) else {
            return 1;
        };
        (p.x, p.y, p.z, p.value) = (x, y, z, value);
        0
    }

    /// `set(i, point)`: the coordinates, value, count and index (not the label name); 0, or 1.
    pub fn set(&mut self, i: i32, q: &Point) -> i32 {
        let Some(p) = usize::try_from(i).ok().and_then(|i| self.points.get_mut(i)) else {
            return 1;
        };
        (p.x, p.y, p.z, p.value, p.count, p.index) = (q.x, q.y, q.z, q.value, q.count, q.index);
        0
    }

    /// `get(i)`: the point, or a default point of count -1 for a place outside the set.
    pub fn get(&self, i: i32) -> Point {
        match usize::try_from(i).ok().and_then(|i| self.points.get(i)) {
            Some(p) => p.clone(),
            None => Point {
                count: -1,
                ..Point::default()
            },
        }
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        self.points.clear();
    }

    /// `remove(index)`. FS-431: an index outside the set is undefined behaviour in FreeSurfer;
    /// both modes leave the set unchanged.
    pub fn remove(&mut self, g: &mut Globals, index: i32) {
        match usize::try_from(index) {
            Ok(i) if i < self.points.len() => {
                self.points.remove(i);
            }
            _ => g.notices.add(
                &FS_431,
                format!(
                    "fsPointSet::remove({index}) of a set of {} points: nothing removed",
                    self.points.len()
                ),
            ),
        }
    }

    fn shown(&self, p: &Point) -> bool {
        !(p.count == 0 && self.include_zero_count == 0
            || p.index == 0 && self.include_zero_index == 0)
    }

    /// `save(filename)`: the set as FreeSurfer's JSON (`data_type` `fs_pointset`, each point's
    /// coordinates, label name when not empty, index, value, count and `legacy_stat` 1, then
    /// `vox2ras`); `true` (also when the file does not open).
    pub fn save_bytes(&self) -> Vec<u8> {
        let mut s = b"{\n    \"data_type\": \"fs_pointset\",\n    \"points\": [\n".to_vec();
        let last = self.points.len().wrapping_sub(1);
        for (k, p) in self.points.iter().enumerate() {
            s.extend_from_slice(b"        {\n            \"coordinates\": {\n");
            s.extend_from_slice(
                format!(
                    "                \"x\": {},\n                \"y\": {},\n                \"z\": {}\n",
                    g6(p.x),
                    g6(p.y),
                    g6(p.z)
                )
                .as_bytes(),
            );
            s.extend_from_slice(b"            },\n");
            if !p.labelname.is_empty() {
                s.extend_from_slice(b"            \"labelname\": \"");
                s.extend_from_slice(&p.labelname);
                s.extend_from_slice(b"\",\n");
            }
            s.extend_from_slice(
                format!(
                    "            \"index\": {},\n            \"value\": {},\n            \"count\": {},\n            \"legacy_stat\": 1\n",
                    p.index,
                    g6(p.value),
                    p.count
                )
                .as_bytes(),
            );
            s.extend_from_slice(if k == last {
                b"        }\n"
            } else {
                b"        },\n"
            });
        }
        s.extend_from_slice(b"    ],\n    \"vox2ras\": \"");
        s.extend_from_slice(&self.vox2ras);
        s.extend_from_slice(b"\"\n}\n");
        s
    }

    /// `save(filename)`: [`PointSet::save_bytes`] written to the file; `true`.
    pub fn save(&self, dir: Option<&Path>, filename: &str) -> bool {
        write_file(dir, filename, &self.save_bytes());
        true
    }

    /// `save_as_ctrlpoint(filename)`: each point `x y z` (`%g`), then `info`, `numpoints <n>`,
    /// `useRealRAS 1`; `true`.
    pub fn save_as_ctrlpoint(&self, dir: Option<&Path>, filename: &str) -> bool {
        let mut s = String::new();
        for p in &self.points {
            s.push_str(&format!("{} {} {}\n", g6(p.x), g6(p.y), g6(p.z)));
        }
        s.push_str(&format!(
            "info\nnumpoints {}\nuseRealRAS 1\n",
            self.points.len()
        ));
        write_file(dir, filename, s.as_bytes());
        true
    }

    /// `print(fp)`: each point shown (count 0 and index 0 left out when their flags are 0) as
    /// `%3d %4d %6d %8.4lf %8.4lf %8.4lf %8.4lf` of 0 (FreeSurfer's row number never counts:
    /// kept), index, count, coordinates and value. 0.
    pub fn print(&self, fp: &mut dyn Write) -> i32 {
        for p in self.points.iter().filter(|p| self.shown(p)) {
            let _ = writeln!(
                fp,
                "{:3} {:4} {:6} {} {} {} {}",
                0,
                p.index,
                p.count,
                fmt_f(p.x, 8, 4),
                fmt_f(p.y, 8, 4),
                fmt_f(p.z, 8, 4),
                fmt_f(p.value, 8, 4)
            );
        }
        let _ = fp.flush();
        0
    }

    /// `printCoords(fp)`: each point shown as `%4d %8.4lf %8.4lf %8.4lf` of its index and
    /// coordinates. 0.
    pub fn print_coords(&self, fp: &mut dyn Write) -> i32 {
        for p in self.points.iter().filter(|p| self.shown(p)) {
            let _ = writeln!(
                fp,
                "{:4} {} {} {}",
                p.index,
                fmt_f(p.x, 8, 4),
                fmt_f(p.y, 8, 4),
                fmt_f(p.z, 8, 4)
            );
        }
        let _ = fp.flush();
        0
    }

    /// `writeCoords(fname)`: [`PointSet::print_coords`] into the file; 0, or 1 when it does not
    /// open.
    pub fn write_coords(&self, dir: Option<&Path>, fname: &str) -> i32 {
        let mut b = Vec::new();
        self.print_coords(&mut b);
        let p = larmorx_freesurfer_core::utils::resolve(dir, fname);
        match std::fs::File::create(p) {
            Ok(mut f) => {
                let _ = f.write_all(&b);
                0
            }
            Err(_) => 1,
        }
    }

    /// `writeCentroidTable(outfile, ltafile)`'s table: `#SegCentroid`, `# Reg <lta>` when given,
    /// a header (`# ID` in the index column's width, `StructName` when a point has a label name,
    /// `R`, `A`, `S` each with four spaces after, right-aligned in 12 characters), `# ` and a rule
    /// of dashes, then each point's index, label name (left-aligned, after four spaces) and
    /// coordinates (`%.4f`, 12 characters). The index column is at least 2 characters wide and the
    /// name column at least 10.
    pub fn centroid_table_bytes(&self, ltafile: &[u8]) -> Vec<u8> {
        let (mut max_id_chars, mut max_label_chars, mut haslabels) = (2usize, 10usize, false);
        for p in &self.points {
            let l = p.labelname.len();
            if l > 0 {
                haslabels = true;
            }
            max_label_chars = max_label_chars.max(l);
            max_id_chars = max_id_chars.max(p.index.to_string().len());
        }
        let precision = 4usize;
        let cwidth = precision + 8;
        let pad_left = |s: &[u8], w: usize| -> Vec<u8> {
            let mut v = vec![b' '; w.saturating_sub(s.len())];
            v.extend_from_slice(s);
            v
        };
        let pad_right = |s: &[u8], w: usize| -> Vec<u8> {
            let mut v = s.to_vec();
            v.resize(w.max(s.len()), b' ');
            v
        };
        let mut t = b"#SegCentroid\n".to_vec();
        if !ltafile.is_empty() {
            t.extend_from_slice(b"# Reg ");
            t.extend_from_slice(ltafile);
            t.push(b'\n');
        }
        t.extend_from_slice(&pad_left(b"# ID", max_id_chars));
        if haslabels {
            t.extend_from_slice(b"    ");
            t.extend_from_slice(&pad_right(b"StructName", max_label_chars));
        }
        for h in [b"R    ", b"A    ", b"S    "] {
            t.push(b' ');
            t.extend_from_slice(&pad_left(h, cwidth));
        }
        t.push(b'\n');
        let mut table_width = max_id_chars + cwidth * 3 + 3;
        if haslabels {
            table_width += max_label_chars + 4;
        }
        t.extend_from_slice(b"# ");
        t.extend(std::iter::repeat_n(b'-', table_width));
        t.push(b'\n');
        for p in &self.points {
            t.extend_from_slice(&pad_left(p.index.to_string().as_bytes(), max_id_chars));
            if haslabels {
                t.extend_from_slice(b"    ");
                t.extend_from_slice(&pad_right(&p.labelname, max_label_chars));
            }
            for v in [p.x, p.y, p.z] {
                t.push(b' ');
                t.extend_from_slice(fmt_f(v, cwidth, precision).as_bytes());
            }
            t.push(b'\n');
        }
        t
    }

    /// `writeCentroidTable(outfile, ltafile)`: `could not open writable file at <outfile>` on
    /// standard error and `false` when the file does not open; else `writing results to <outfile>`
    /// on standard output, [`PointSet::centroid_table_bytes`] into the file, `true`.
    pub fn write_centroid_table(
        &self,
        out: &mut dyn Write,
        err: &mut dyn Write,
        dir: Option<&Path>,
        outfile: &str,
        ltafile: &[u8],
    ) -> bool {
        let p = larmorx_freesurfer_core::utils::resolve(dir, outfile);
        let Ok(mut f) = std::fs::File::create(p) else {
            let _ = writeln!(err, "could not open writable file at {outfile}");
            return false;
        };
        let _ = writeln!(out, "writing results to {outfile}");
        let _ = f.write_all(&self.centroid_table_bytes(ltafile));
        true
    }
}

/// FreeSurfer's copy of nlohmann/json 3.2.0 words `operator[]`'s `type_error.305` with the
/// argument's kind.
#[derive(Clone, Copy, Debug, Default)]
pub struct FsJson;

impl Dialect for FsJson {
    fn operator_index_message(&self, numeric: bool, type_name: &str) -> String {
        let arg = if numeric { "numeric" } else { "string" };
        format!("cannot use operator[] with a {arg} argument with {type_name}")
    }
}

/// The point set of a JSON value as `loadfsPointSet` takes it: `vox2ras` a string, then each of
/// `points` (an array's elements, an object's values in key order, nothing for null, any other
/// value itself) with `legacy_stat` and the `coordinates`' `z`, `y` and `x` numbers (taken in
/// that order: the first that is not a number names the exception).
fn point_set_of(mut j: Value) -> Result<PointSet, json::Error> {
    let mut ps = PointSet {
        vox2ras: j.index_key(b"vox2ras", &FsJson)?.get_string()?.to_vec(),
        ..PointSet::default()
    };
    let points = j.index_key(b"points", &FsJson)?.clone();
    for point in points.iter() {
        let mut point = point.clone();
        let mut coord = point.index_key(b"coordinates", &FsJson)?.clone();
        let value = point.index_key(b"legacy_stat", &FsJson)?.get_f64()?;
        let z = coord.index_key(b"z", &FsJson)?.get_f64()?;
        let y = coord.index_key(b"y", &FsJson)?.get_f64()?;
        let x = coord.index_key(b"x", &FsJson)?.get_f64()?;
        ps.add_xyz(x, y, z, value);
    }
    Ok(ps)
}

/// `loadfsPointSet(filename)`: the point set a JSON file holds (as [`PointSet::save`] writes it).
/// A file that does not open reads as empty. FS-433: a file that is not JSON (nlohmann's
/// `parse_error`) or not a point set (`type_error`: `vox2ras` not a string, a point or its
/// coordinates not an object, a coordinate or `legacy_stat` not a number) ends FreeSurfer with
/// `terminate called after throwing an instance of 'nlohmann::detail::<kind>'` and the
/// exception's text on standard error (a directory: libstdc++'s `std::__ios_failure`, `basic_filebuf::underflow
/// error reading the file: Is a directory`), then `abort` (`reproduce`: the same, status 134); by
/// default `loadfsPointSet(<file>): <text>` as an error and status 1.
pub fn load_fs_point_set(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    filename: &str,
) -> Result<PointSet, Fatal> {
    let path = larmorx_freesurfer_core::utils::resolve(dir, filename);
    let read = if path.is_dir() {
        // a directory opens, and its first read fails: libstdc++'s stream throws
        Err(json::Error {
            kind: "__ios_failure",
            id: 0,
            message: b"basic_filebuf::underflow error reading the file: Is a directory".to_vec(),
        })
    } else {
        let bytes = std::fs::read(&path).unwrap_or_default();
        json::parse(&bytes, false).and_then(|(j, _)| point_set_of(j))
    };
    match read {
        Ok(ps) => Ok(ps),
        Err(e) => {
            let class = if e.kind == "__ios_failure" {
                "std::__ios_failure".to_owned()
            } else {
                format!("nlohmann::detail::{}", e.kind)
            };
            if g.notices.reproduce() {
                let _ = write!(
                    err,
                    "terminate called after throwing an instance of '{class}'\n  what():  "
                );
                let _ = err.write_all(&e.message);
                let _ = err.write_all(b"\n");
                g.notices.add(
                    &FS_433,
                    format!(
                        "loadfsPointSet({filename}): an uncaught {}: aborted",
                        e.kind
                    ),
                );
                return Err(Fatal::Exit(134));
            }
            larmorx_freesurfer_core::log::error(
                g,
                err,
                &format!(
                    "loadfsPointSet({filename}): {}",
                    String::from_utf8_lossy(&e.message)
                ),
            );
            g.notices.add(
                &FS_433,
                format!(
                    "loadfsPointSet({filename}): an uncaught {}: a clear error, not an abort",
                    e.kind
                ),
            );
            Err(Fatal::Exit(1))
        }
    }
}
