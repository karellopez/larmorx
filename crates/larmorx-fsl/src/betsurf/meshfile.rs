// SPDX-License-Identifier: Apache-2.0
//! The mesh files betsurf reads (`specs/bet.md` §6.2): VTK polydata as bet2 writes it, OFF and
//! FreeSurfer's ASCII surfaces, read as C++'s streams read numbers.
//!
//! FSL reads them without checks (FSL-52): a number that does not read is stored as 0 and every
//! later read sets nothing; a face may name a vertex the file does not have. [`read`] keeps
//! what the stream defines (0 for what it never sets), leaves out the faces that name missing
//! vertices, and lists each problem, so that the caller refuses (the default) or goes on with
//! FSL's defined values (`reproduce`).

use crate::bet::V3;

/// A mesh as read, with the problems FSL would not have noticed (FSL-52).
#[derive(Clone, Debug, Default)]
pub struct Read {
    pub points: Vec<V3>,
    pub triangles: Vec<[u32; 3]>,
    /// What does not read, in FSL-52's words (empty for a sound file).
    pub problems: Vec<String>,
}

/// A text read as C++'s `>>` reads it: a failed read stores 0 and stops every later one.
struct Stream<'a> {
    text: &'a [u8],
    at: usize,
    failed: bool,
}

impl<'a> Stream<'a> {
    fn new(text: &'a str) -> Stream<'a> {
        Stream {
            text: text.as_bytes(),
            at: 0,
            failed: false,
        }
    }

    /// One line (`std::getline`), without its newline.
    fn line(&mut self) -> &'a str {
        let rest = &self.text[self.at..];
        let end = rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len());
        self.at += (end + 1).min(rest.len());
        std::str::from_utf8(&rest[..end]).unwrap_or("")
    }

    fn skip_blanks(&mut self) {
        while self.at < self.text.len() && self.text[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }

    /// A word (`>> std::string`).
    fn word(&mut self) -> Option<&'a str> {
        if self.failed {
            return None;
        }
        self.skip_blanks();
        let start = self.at;
        while self.at < self.text.len() && !self.text[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
        if self.at == start {
            self.failed = true;
            return None;
        }
        std::str::from_utf8(&self.text[start..self.at]).ok()
    }

    /// The longest prefix that reads as a number of the given kind; the bytes it took.
    fn number_prefix(&self, from: usize, real: bool) -> usize {
        let b = self.text;
        let mut i = from;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let digits = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let mut n = i - digits;
        if real {
            if i < b.len() && b[i] == b'.' {
                let f = i + 1;
                let mut j = f;
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                n += j - f;
                i = j;
            }
            if n > 0 && i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                let mut j = i + 1;
                if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                    j += 1;
                }
                let e = j;
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                if j > e {
                    i = j;
                } else {
                    // an exponent without digits does not read (libstdc++)
                    return 0;
                }
            }
        }
        if n == 0 { 0 } else { i - from }
    }

    /// A number (`>> double`, `>> int`): `None` once a read failed (the value never set).
    fn number(&mut self, real: bool) -> Option<f64> {
        if self.failed {
            return None;
        }
        self.skip_blanks();
        let used = self.number_prefix(self.at, real);
        if used == 0 {
            self.failed = true;
            return None;
        }
        let text = std::str::from_utf8(&self.text[self.at..self.at + used]).unwrap_or("0");
        self.at += used;
        text.parse::<f64>().ok().or_else(|| {
            self.failed = true;
            None
        })
    }
}

/// The largest vertex or face count read (a guard: no file of this size holds more).
fn plausible(count: f64, text_len: usize) -> usize {
    if !(count > 0.0) {
        0
    } else {
        count.min(text_len as f64 + 1.0) as usize
    }
}

/// Reads a mesh file's text (§6.2): VTK when its first line contains `# vtk DataFile Version
/// 3.0`, OFF when it contains `OFF`, else FreeSurfer's ASCII surface.
pub fn read(text: &str) -> Read {
    let mut s = Stream::new(text);
    let first = s.line();
    let mut problems = Vec::new();
    let mut never = false;
    let mut get = |s: &mut Stream<'_>, real: bool| -> f64 {
        match s.number(real) {
            Some(v) => v,
            None => {
                never = true;
                0.0
            }
        }
    };
    let (points, faces): (Vec<V3>, Vec<[i64; 3]>);
    if first.contains("# vtk DataFile Version 3.0") {
        for _ in 0..3 {
            s.line();
        }
        let _ = s.word();
        let n = plausible(get(&mut s, false), text.len());
        let _ = s.word();
        points = (0..n)
            .map(|_| [get(&mut s, true), get(&mut s, true), get(&mut s, true)])
            .collect();
        let _ = s.word();
        let m = plausible(get(&mut s, false), text.len());
        let _ = get(&mut s, false);
        faces = (0..m)
            .map(|_| {
                let _ = get(&mut s, false);
                [0, 0, 0].map(|_| get(&mut s, false) as i64)
            })
            .collect();
    } else if first.contains("OFF") {
        if first.contains('n') {
            let dim = get(&mut s, false);
            if dim != 3.0 {
                problems.push(format!("the OFF file's dimension is {dim}, not 3"));
            }
        }
        let n = plausible(get(&mut s, false), text.len());
        let m = plausible(get(&mut s, false), text.len());
        let _ = get(&mut s, false);
        points = (0..n)
            .map(|_| [get(&mut s, true), get(&mut s, true), get(&mut s, true)])
            .collect();
        faces = (0..m)
            .map(|_| {
                let _ = get(&mut s, false);
                [0, 0, 0].map(|_| get(&mut s, false) as i64)
            })
            .collect();
    } else {
        let n = plausible(get(&mut s, false), text.len());
        let m = plausible(get(&mut s, false), text.len());
        points = (0..n)
            .map(|_| {
                let p = [get(&mut s, true), get(&mut s, true), get(&mut s, true)];
                let _ = get(&mut s, true);
                p
            })
            .collect();
        faces = (0..m)
            .map(|_| {
                let f = [0, 0, 0].map(|_| get(&mut s, false) as i64);
                let _ = get(&mut s, true);
                f
            })
            .collect();
    }
    if never {
        problems.push(
            "a number does not read (FSL stores 0 there and leaves every later value unset)".into(),
        );
    }
    let n = points.len() as i64;
    let mut missing = 0usize;
    let triangles: Vec<[u32; 3]> = faces
        .into_iter()
        .filter_map(|f| {
            if f.iter().all(|&v| (0..n).contains(&v)) {
                Some(f.map(|v| v as u32))
            } else {
                missing += 1;
                None
            }
        })
        .collect();
    if missing > 0 {
        problems.push(format!(
            "{missing} face(s) name a vertex the mesh does not have (FSL reads past its vertices)"
        ));
    }
    if points.is_empty() {
        problems.push("the mesh has no vertices".into());
    }
    Read {
        points,
        triangles,
        problems,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VTK: &str = "# vtk DataFile Version 3.0\nsurface file\nASCII\nDATASET POLYDATA\nPOINTS 3  float\n0.5 0 0\n0 1.25 0\n0 0 -2e1\nPOLYGONS 1 4\n3 0 1 2 \n";

    #[test]
    fn the_three_formats() {
        let r = read(VTK);
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        assert_eq!(
            r.points,
            vec![[0.5, 0.0, 0.0], [0.0, 1.25, 0.0], [0.0, 0.0, -20.0]]
        );
        assert_eq!(r.triangles, vec![[0, 1, 2]]);
        let off = read("OFF\n3 1 0\n0.5 0 0\n0 1.25 0\n0 0 -2e1\n3 0 1 2\n");
        assert_eq!(
            (off.points, off.triangles),
            (r.points.clone(), r.triangles.clone())
        );
        let fs = read("#!ascii\n3 1\n0.5 0 0 0\n0 1.25 0 0\n0 0 -2e1 0\n0 1 2 0\n");
        assert_eq!((fs.points, fs.triangles), (r.points, r.triangles));
    }

    #[test]
    fn what_fsl_leaves_unset() {
        let bad = VTK.replace("0 1.25 0", "0 x 0");
        let r = read(&bad);
        assert_eq!(r.points[1], [0.0, 0.0, 0.0]);
        assert_eq!(r.points[2], [0.0, 0.0, 0.0]);
        assert!(r.triangles.is_empty());
        assert_eq!(r.problems.len(), 1);
        let missing = VTK.replace("3 0 1 2", "3 0 1 7");
        let r = read(&missing);
        assert!(r.triangles.is_empty());
        assert_eq!(r.problems.len(), 1);
    }
}
