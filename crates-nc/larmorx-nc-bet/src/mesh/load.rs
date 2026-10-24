// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! `Mesh::load` (fsl-meshclass 2111.0, `mesh.cpp`): a mesh from a text file, chosen by its first
//! line: Geomview OFF (the line holds `OFF`), VTK polydata (`# vtk DataFile Version 3.0`), else
//! FreeSurfer's ASCII surface. The name `manual_input` asks for a file name on standard input,
//! and `c` cancels (an empty mesh).
//!
//! FSL reads these files without checking them. Where that leaves values it never set (a number
//! that does not read, a file that ends early) or indexes past the vertices (a face naming a
//! vertex that does not exist), its result depends on memory contents or it crashes; the replica
//! refuses such a file with the reason (FSL-52). The values VTK files may carry after the faces
//! are read by FSL into the vertices and never used by BET: the replica ignores them.

use std::io::{BufRead, Write};

use crate::fsl::istream::IStream;
use crate::mesh::{MPoint, Mesh, V3};

/// Why a mesh was not loaded.
#[derive(Debug)]
pub enum LoadError {
    /// FSL prints this to standard error and exits with this status.
    Exit { printed: String, code: u8 },
    /// FSL's result is undefined here (values never set, out-of-range vertices) or it never
    /// returns: the replica refuses, saying why.
    Refused(String),
}

/// The bytes an `std::ifstream` would read from `path`, or `None` when it cannot open it: a
/// directory opens and reads nothing.
pub fn read_like_ifstream(path: &str) -> Option<Vec<u8>> {
    if std::fs::metadata(path).is_ok_and(|m| m.is_dir()) {
        return Some(Vec::new());
    }
    std::fs::read(path).ok()
}

/// How FSL's `manual_input` reads a file name: lines from standard input until one is not
/// empty (`None`: the input ended first, where FSL waits forever).
fn prompt(out: &mut dyn Write, input: &mut dyn BufRead) -> Option<String> {
    let _ = write!(out, "loading mesh : enter file name / c to cancel: ");
    let _ = out.flush();
    loop {
        let mut line = String::new();
        match input.read_line(&mut line) {
            Ok(0) | Err(_) => return None,
            Ok(_) => {
                let s = line.strip_suffix('\n').unwrap_or(&line).to_owned();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
}

fn never_returns() -> LoadError {
    LoadError::Refused(
        "standard input ended before a file name: FSL's betsurf asks for one forever here".into(),
    )
}

/// `Mesh::load(name)`; `out` takes what FSL prints to standard output (the prompt,
/// `cancelled`).
pub fn load(name: &str, out: &mut dyn Write, input: &mut dyn BufRead) -> Result<Mesh, LoadError> {
    let mut s = name.to_owned();
    if s == "manual_input" {
        s = prompt(out, input).ok_or_else(never_returns)?;
    }
    if s == "c" {
        let _ = writeln!(out, "cancelled");
        return Ok(Mesh::default());
    }
    let Some(bytes) = read_like_ifstream(&s) else {
        return Err(LoadError::Exit {
            printed: format!(
                "error opening file\ncould not open the following file: {s}\nterminating.\n"
            ),
            code: 255,
        });
    };
    let mut f = IStream::new(&bytes);
    let header = f.getline().unwrap_or_default();
    if header.contains("OFF") {
        load_off(&s, out, input)
    } else if header.contains("# vtk DataFile Version 3.0") {
        load_vtk(&s, out, input)
    } else {
        load_fs(&s, out, input)
    }
}

/// The file a `load_*` function reads (each asks again for `manual_input`, as FSL's do).
fn reopen(
    name: &str,
    out: &mut dyn Write,
    input: &mut dyn BufRead,
) -> Result<Option<Vec<u8>>, LoadError> {
    let mut s = name.to_owned();
    if s == "manual_input" {
        s = prompt(out, input).ok_or_else(never_returns)?;
    }
    if s == "c" {
        let _ = writeln!(out, "cancelled");
        return Ok(None);
    }
    match read_like_ifstream(&s) {
        Some(b) => Ok(Some(b)),
        None => {
            // FSL prints this one to standard output
            let _ = writeln!(out, "error opening file");
            Err(LoadError::Exit {
                printed: String::new(),
                code: 255,
            })
        }
    }
}

fn unread(what: &str) -> LoadError {
    LoadError::Refused(format!(
        "the mesh file's {what} does not read as a number, so FSL computes with values it never \
         set (FSL-52)"
    ))
}

/// The vertices and faces read so far.
struct Builder {
    mesh: Mesh,
}

impl Builder {
    fn point(&mut self, x: f64, y: f64, z: f64, value: f32) {
        let i = self.mesh.points.len();
        self.mesh
            .points
            .push(MPoint::new(V3::new(x, y, z), i as i32, value));
    }

    fn face(&mut self, p: [i32; 3], value: f32) -> Result<(), LoadError> {
        let n = self.mesh.points.len();
        let mut v = [0usize; 3];
        for (k, &q) in p.iter().enumerate() {
            if q < 0 || q as usize >= n {
                return Err(LoadError::Refused(format!(
                    "a face of the mesh names vertex {q}, and the mesh has {n} (FSL reads past \
                     its vertices: FSL-52)"
                )));
            }
            v[k] = q as usize;
        }
        let id = self.mesh.new_triangle(v[0], v[1], v[2], value);
        self.mesh.tri_list.push(id);
        Ok(())
    }
}

fn int(f: &mut IStream, what: &str) -> Result<i32, LoadError> {
    f.i32().ok().ok_or_else(|| unread(what))
}

fn float(f: &mut IStream, what: &str) -> Result<f64, LoadError> {
    f.f64().ok().ok_or_else(|| unread(what))
}

fn points(f: &mut IStream, b: &mut Builder, n: i32, with_value: bool) -> Result<(), LoadError> {
    for i in 0..n.max(0) {
        let what = format!("vertex {i}");
        let x = float(f, &what)?;
        let y = float(f, &what)?;
        let z = float(f, &what)?;
        let value = if with_value {
            f.f32().ok().ok_or_else(|| unread(&what))?
        } else {
            0.0
        };
        b.point(x, y, z, value);
    }
    Ok(())
}

/// `Mesh::load_off`.
fn load_off(name: &str, out: &mut dyn Write, input: &mut dyn BufRead) -> Result<Mesh, LoadError> {
    let Some(bytes) = reopen(name, out, input)? else {
        return Ok(Mesh::default());
    };
    let mut f = IStream::new(&bytes);
    let header = f.getline().unwrap_or_default();
    if !header.contains("OFF") {
        return Err(LoadError::Exit {
            printed: "error in the header\n".into(),
            code: 255,
        });
    }
    if header.contains('n') {
        let n = int(&mut f, "dimension")?;
        if n != 3 {
            return Err(LoadError::Exit {
                printed: "this program only handles triangles meshes\n".into(),
                code: 255,
            });
        }
    }
    let nv = int(&mut f, "vertex count")?;
    let nf = int(&mut f, "face count")?;
    // the edge count: FSL initialises it, and never uses it
    let _ = f.i32();
    let mut b = Builder {
        mesh: Mesh::default(),
    };
    points(&mut f, &mut b, nv, false)?;
    for i in 0..nf.max(0) {
        let what = format!("face {i}");
        let _j = int(&mut f, &what)?;
        let p = [
            int(&mut f, &what)?,
            int(&mut f, &what)?,
            int(&mut f, &what)?,
        ];
        b.face(p, 0.0)?;
    }
    Ok(b.mesh)
}

/// `Mesh::load_vtk_ASCII`.
fn load_vtk(name: &str, out: &mut dyn Write, input: &mut dyn BufRead) -> Result<Mesh, LoadError> {
    let Some(bytes) = reopen(name, out, input)? else {
        return Ok(Mesh::default());
    };
    let mut f = IStream::new(&bytes);
    let header = f.getline().unwrap_or_default();
    if !header.contains("vtk DataFile Version 3.0") {
        return Err(LoadError::Exit {
            printed: "error in the header\n".into(),
            code: 255,
        });
    }
    for _ in 0..3 {
        let _ = f.getline();
    }
    let _ = f.word();
    let nv = int(&mut f, "vertex count")?;
    let _ = f.word();
    let mut b = Builder {
        mesh: Mesh::default(),
    };
    points(&mut f, &mut b, nv, false)?;
    let _ = f.word();
    let nf = int(&mut f, "face count")?;
    let _ = f.word();
    for i in 0..nf.max(0) {
        let what = format!("face {i}");
        let _j = int(&mut f, &what)?;
        let p = [
            int(&mut f, &what)?,
            int(&mut f, &what)?,
            int(&mut f, &what)?,
        ];
        b.face(p, 0.0)?;
    }
    Ok(b.mesh)
}

/// `Mesh::load_fs`.
fn load_fs(name: &str, out: &mut dyn Write, input: &mut dyn BufRead) -> Result<Mesh, LoadError> {
    let Some(bytes) = reopen(name, out, input)? else {
        return Ok(Mesh::default());
    };
    let mut f = IStream::new(&bytes);
    let _ = f.getline();
    let nv = int(&mut f, "vertex count")?;
    let nf = int(&mut f, "face count")?;
    let mut b = Builder {
        mesh: Mesh::default(),
    };
    points(&mut f, &mut b, nv, true)?;
    for i in 0..nf.max(0) {
        let what = format!("face {i}");
        let p = [
            int(&mut f, &what)?,
            int(&mut f, &what)?,
            int(&mut f, &what)?,
        ];
        let value = f.f32().ok().ok_or_else(|| unread(&what))?;
        b.face(p, value)?;
    }
    Ok(b.mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_text(text: &str) -> Result<Mesh, LoadError> {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("m.txt");
        std::fs::write(&p, text).unwrap();
        let mut out = Vec::new();
        load(p.to_str().unwrap(), &mut out, &mut std::io::empty())
    }

    #[test]
    fn the_three_formats() {
        let vtk = Mesh::icosa(2).vtk_text();
        let m = load_text(&vtk).unwrap();
        assert_eq!(m.points.len(), 42);
        assert_eq!(m.tri_list.len(), 80);
        assert_eq!(m.vtk_text(), vtk);
        let off = load_text("OFF\n3 1 0\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n").unwrap();
        assert_eq!(off.points.len(), 3);
        assert_eq!(off.points[0].neighbours, vec![1, 2]);
        let fs = load_text("#!ascii\n3 1\n0 0 0 0\n1 0 0 0\n0 1 0 0\n0 1 2 0\n").unwrap();
        assert_eq!(fs.tri_list.len(), 1);
    }

    #[test]
    fn what_fsl_cannot_read() {
        assert!(matches!(
            load_text("OFF\n3 1 0\n0 0 0\n1 0 0\n0 1 0\n3 0 1 7\n"),
            Err(LoadError::Refused(_))
        ));
        assert!(matches!(
            load_text("# vtk DataFile Version 3.0\na\nb\nc\nPOINTS 3 float\n0 0 0\n1 0\n"),
            Err(LoadError::Refused(_))
        ));
        assert!(matches!(
            load_text("nOFF\n4\n"),
            Err(LoadError::Exit { code: 255, .. })
        ));
        let mut out = Vec::new();
        let r = load("/nonexistent/x.vtk", &mut out, &mut std::io::empty());
        let Err(LoadError::Exit { printed, code }) = r else {
            panic!()
        };
        assert_eq!(code, 255);
        assert!(printed.starts_with("error opening file\ncould not open"));
        let r = load("manual_input", &mut out, &mut std::io::empty());
        assert!(matches!(r, Err(LoadError::Refused(_))));
        let m = load("c", &mut out, &mut std::io::empty()).unwrap();
        assert!(m.points.is_empty());
    }
}
