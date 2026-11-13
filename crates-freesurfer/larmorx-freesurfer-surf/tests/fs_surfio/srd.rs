// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 16: `srd`, `MRISread` (with `MRISreadOverAlloc`, the Talairach
//! transform's search, the icosahedron reader and fs.surf's neighbour lists, neighbourhoods and
//! distances) and `MRISfastRead`, in an empty work folder (with a subfolder `d`): Gdiag's bits, the
//! environment (a mask), the case's input files (closed and open meshes, and the other readers'
//! random files, in every format the name chooses), 1 or 2 reads, each surface printed in full: the
//! generator and the port's answers, as `fssurfio.cpp`'s.

use super::cfile::vtk;
use super::sfile::utf8;
use super::sread::{ascii_surface, geo_surface};
use super::stl::{binary_stl, facets, text_stl};
use super::tri::{coord, faces, quad_file, tags, triangle_file, vg_text, vg_values};
use super::*;

use larmorx_freesurfer_surf::readwrite;

/// A mesh: vertex coordinates and faces (vertex numbers from 0).
struct Mesh {
    v: Vec<[f32; 3]>,
    f: Vec<[i64; 3]>,
}

/// The icosahedron's coordinate (the golden ratio as the generator writes it: its float is the case's).
#[allow(clippy::approx_constant)]
const T: f32 = 1.618_034;

const ICOSA_V: [[f32; 3]; 12] = [
    [-1.0, T, 0.0],
    [1.0, T, 0.0],
    [-1.0, -T, 0.0],
    [1.0, -T, 0.0],
    [0.0, -1.0, T],
    [0.0, 1.0, T],
    [0.0, -1.0, -T],
    [0.0, 1.0, -T],
    [T, 0.0, -1.0],
    [T, 0.0, 1.0],
    [-T, 0.0, -1.0],
    [-T, 0.0, 1.0],
];

const ICOSA_F: [[i64; 3]; 20] = [
    [0, 11, 5],
    [0, 5, 1],
    [0, 1, 7],
    [0, 7, 10],
    [0, 10, 11],
    [1, 5, 9],
    [5, 11, 4],
    [11, 10, 2],
    [10, 7, 6],
    [7, 1, 8],
    [3, 9, 4],
    [3, 4, 2],
    [3, 2, 6],
    [3, 6, 8],
    [3, 8, 9],
    [4, 9, 5],
    [2, 4, 11],
    [6, 2, 10],
    [8, 6, 7],
    [9, 8, 1],
];

/// A closed or open mesh: a tetrahedron, an octahedron, an icosahedron (now and then a face
/// short), a cube, a fan (closed into a cone now and then), a strip, two tetrahedra sharing a
/// vertex, random faces of a few vertices; its coordinates the shape's (scaled and moved), or any
/// now and then; its vertices renumbered and faces turned now and then.
fn mesh(g: &mut Gen, heavy: bool) -> Mesh {
    let mut m = match g.below(10) {
        0 => Mesh {
            v: vec![
                [1.0, 1.0, 1.0],
                [1.0, -1.0, -1.0],
                [-1.0, 1.0, -1.0],
                [-1.0, -1.0, 1.0],
            ],
            f: vec![[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]],
        },
        1 => Mesh {
            v: vec![
                [1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, -1.0],
            ],
            f: vec![
                [0, 2, 4],
                [2, 1, 4],
                [1, 3, 4],
                [3, 0, 4],
                [2, 0, 5],
                [1, 2, 5],
                [3, 1, 5],
                [0, 3, 5],
            ],
        },
        2..=4 => {
            let mut f = ICOSA_F.to_vec();
            if g.chance(5) {
                f.remove(g.below(20) as usize);
            }
            Mesh {
                v: ICOSA_V.to_vec(),
                f,
            }
        }
        5 => {
            let v = (0..8)
                .map(|i| {
                    [
                        if i & 1 != 0 { 1.0 } else { -1.0 },
                        if i & 2 != 0 { 1.0 } else { -1.0 },
                        if i & 4 != 0 { 1.0 } else { -1.0 },
                    ]
                })
                .collect();
            let f = vec![
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
            Mesh { v, f }
        }
        6 => {
            let k = if heavy && g.chance(4) {
                g.int(30, 40)
            } else {
                g.int(3, 8)
            };
            let mut v = vec![[0.0f32, 0.0, 1.0]];
            for i in 0..k {
                v.push([i as f32, (i * i % 7) as f32, 0.0]);
            }
            let closed = g.chance(2);
            let mut f = Vec::new();
            for i in 0..k {
                if i + 1 == k && !closed {
                    break;
                }
                f.push([0, 1 + i, 1 + (i + 1) % k]);
            }
            Mesh { v, f }
        }
        7 => {
            let k = g.int(2, 5);
            let mut v = Vec::new();
            for i in 0..2 * k {
                v.push([(i / 2) as f32, (i % 2) as f32, ((i * 3) % 5) as f32 / 4.0]);
            }
            let mut f = Vec::new();
            for i in 0..k - 1 {
                let a = 2 * i;
                f.push([a, a + 1, a + 2]);
                f.push([a + 1, a + 3, a + 2]);
            }
            Mesh { v, f }
        }
        8 => Mesh {
            v: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [-1.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, 0.0, -1.0],
            ],
            f: vec![
                [0, 1, 2],
                [0, 3, 1],
                [0, 2, 3],
                [1, 3, 2],
                [0, 4, 5],
                [0, 6, 4],
                [0, 5, 6],
                [4, 6, 5],
            ],
        },
        _ => {
            let nv = g.int(1, 8);
            let nf = g.int(0, 8);
            Mesh {
                v: (0..nv)
                    .map(|_| [coord(g, heavy), coord(g, heavy), coord(g, heavy)])
                    .collect(),
                f: faces(g, nv, nf),
            }
        }
    };
    let s = g.pick(&[1.0f32, 10.0, 50.0, 0.5, 100.0]);
    let off = [
        g.int(-40, 40) as f32 / 4.0,
        g.int(-40, 40) as f32 / 4.0,
        g.int(-40, 40) as f32 / 4.0,
    ];
    for p in &mut m.v {
        for k in 0..3 {
            p[k] = p[k] * s + off[k];
        }
    }
    if g.chance(12) && !m.v.is_empty() {
        let k = g.below(m.v.len() as u64) as usize;
        m.v[k][g.below(3) as usize] = coord(g, true);
    }
    if g.chance(5) {
        let n = m.v.len();
        let mut perm: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = g.below(i as u64 + 1) as usize;
            perm.swap(i, j);
        }
        let mut v = vec![[0.0f32; 3]; n];
        for (old, &new) in perm.iter().enumerate() {
            v[new] = m.v[old];
        }
        m.v = v;
        for f in &mut m.f {
            for x in f.iter_mut() {
                *x = perm[*x as usize] as i64;
            }
        }
    }
    if g.chance(6) {
        for f in &mut m.f {
            f.swap(1, 2);
        }
    }
    m
}

/// A vertex tree whose centre has more than 1,024 vertices within 3 hops (the search's list
/// overflows: its message, a search shown vertex by vertex, the check and a failed assertion).
fn big_tree() -> Mesh {
    let mut v = vec![[0.0f32, 0.0, 0.0]];
    let mut f = Vec::new();
    let k = 120i64;
    for i in 0..k {
        v.push([i as f32, 1.0, 0.0]);
    }
    for i in 0..k - 1 {
        f.push([0, 1 + i, 2 + i]);
    }
    for i in 0..k {
        let r = 1 + i;
        let base = v.len() as i64;
        for j in 0..10 {
            v.push([i as f32, 2.0 + j as f32, 1.0]);
        }
        for j in 0..9 {
            f.push([r, base + j, base + j + 1]);
        }
    }
    Mesh { v, f }
}

fn be3(b: &mut Vec<u8>, v: i64) {
    b.extend_from_slice(&(v as u32).to_be_bytes()[1..]);
}

fn be4(b: &mut Vec<u8>, v: i64) {
    b.extend_from_slice(&(v as i32).to_be_bytes());
}

fn tagless(b: &mut Vec<u8>, g: &mut Gen) {
    match g.below(6) {
        0 => tags(g, b),
        1 => {
            // a surface in scanner coordinates with a valid volume geometry
            be4(b, 2);
            be4(b, 1);
            be4(b, 20);
            let mut vg = vg_values(g);
            vg.valid = 1;
            b.extend_from_slice(&vg_text(g, &vg));
        }
        _ => {}
    }
}

fn text_f(g: &mut Gen, x: f32) -> String {
    match g.below(4) {
        0 => format!("{x:e}"),
        1 => format!("{x:.3}"),
        _ => format!("{x}"),
    }
}

/// The mesh in a format: its file name's extension and its bytes.
fn write(g: &mut Gen, m: &Mesh, heavy: bool) -> (String, Vec<u8>) {
    let nv = m.v.len() as i64;
    let nf = m.f.len() as i64;
    match g.below(12) {
        0..=2 => {
            let mut b = vec![0xff, 0xff, 0xfe];
            b.extend_from_slice(b"created by me on Thu Nov 12 20:55:19 2026\n\n");
            be4(&mut b, nv);
            be4(&mut b, nf);
            for p in &m.v {
                for c in p {
                    b.extend_from_slice(&c.to_be_bytes());
                }
            }
            for f in &m.f {
                for &x in f {
                    be4(&mut b, x);
                }
            }
            tagless(&mut b, g);
            (String::new(), b)
        }
        3 => {
            // quadrangles of a triangle and its third vertex again (new number, old number in
            // hundredths, or the old format)
            let kind = g.below(4);
            let mut b = Vec::new();
            match kind {
                0 => b.extend_from_slice(&[0xff, 0xff, 0xff]),
                1 | 2 => b.extend_from_slice(&[0xff, 0xff, 0xfd]),
                _ => {}
            }
            be3(&mut b, nv);
            be3(&mut b, nf);
            for p in &m.v {
                for c in p {
                    if kind == 0 {
                        b.extend_from_slice(&((c * 100.0) as i16).to_be_bytes());
                    } else {
                        b.extend_from_slice(&c.to_be_bytes());
                    }
                }
                if kind == 3 {
                    let n = g.pick(&[0, 0, 1, 2]);
                    b.push(n as u8);
                    for _ in 0..n {
                        be3(&mut b, g.int(0, (2 * nf - 1).max(0)));
                    }
                }
            }
            for f in &m.f {
                for x in [f[0], f[1], f[2], f[2]] {
                    be3(&mut b, x);
                }
            }
            if g.chance(3) {
                tags(g, &mut b);
            }
            (String::new(), b)
        }
        4 => {
            let mut s = String::new();
            if g.chance(2) {
                s += "#!ascii version of lh.white\n";
            }
            s += &format!("{nv} {nf}\n");
            for p in &m.v {
                let rip = if g.chance(40) { 1 } else { 0 };
                s += &format!(
                    "{}  {}  {}  {rip}\n",
                    text_f(g, p[0]),
                    text_f(g, p[1]),
                    text_f(g, p[2])
                );
            }
            for f in &m.f {
                s += &format!("{} {} {} 0\n", f[0], f[1], f[2]);
            }
            (".asc".into(), s.into_bytes())
        }
        5 => {
            let mut s = format!("1 {nv} {nf} {}\n1 {nf}\n", 3 * nf);
            for (k, p) in m.v.iter().enumerate() {
                s += &format!("{:e} {:e} {:e}", p[0], p[1], p[2]);
                s += if k % 2 == 1 { "\n" } else { " " };
            }
            if nv % 2 == 1 {
                s.push('\n');
            }
            for f in &m.f {
                s += &format!("{} {} -{}\n", f[0] + 1, f[1] + 1, f[2] + 1);
            }
            (".geo".into(), s.into_bytes())
        }
        6 => {
            let fs: Vec<[[f32; 3]; 4]> =
                m.f.iter()
                    .map(|f| {
                        [
                            [0.0, 0.0, 1.0],
                            m.v[f[0] as usize],
                            m.v[f[1] as usize],
                            m.v[f[2] as usize],
                        ]
                    })
                    .collect();
            let b = if g.chance(2) {
                text_stl(g, &fs)
            } else {
                binary_stl(g, &fs)
            };
            (g.pick(&[".stl", ".STL"]).into(), b)
        }
        7 => (".vtk".into(), vtk(g, nv, &m.f)),
        _ => {
            // the icosahedron format: 4- or 6-number vertex lines, 3- or 4-number face lines
            let mut s = String::new();
            if g.chance(6) {
                s += "# icosahedron\n\n";
            }
            s += &format!("{nv}\n");
            let six = g.chance(3);
            let mut order: Vec<usize> = (0..m.v.len()).collect();
            if !six && g.chance(3) {
                order.reverse();
            }
            for &k in &order {
                let p = m.v[k];
                if six {
                    s += &format!(
                        "{} {} {} 0 0 1\n",
                        text_f(g, p[0]),
                        text_f(g, p[1]),
                        text_f(g, p[2])
                    );
                } else {
                    s += &format!(
                        "{:>8} {} {} {}\n",
                        k + 1,
                        text_f(g, p[0]),
                        text_f(g, p[1]),
                        text_f(g, p[2])
                    );
                }
            }
            let mut nfw = nf;
            let mut fl: Vec<String> = Vec::new();
            let four = g.chance(2);
            for (k, f) in m.f.iter().enumerate() {
                let mut q = [f[0] + 1, f[1] + 1, f[2] + 1];
                if heavy && g.chance(40) {
                    q[g.below(3) as usize] = g.pick(&[0, nv + 1, -1]);
                }
                if four {
                    fl.push(format!("{} {} {} {}\n", k + 1, q[0], q[1], q[2]));
                } else {
                    fl.push(format!("{} {} {}\n", q[0], q[1], q[2]));
                }
            }
            match g.below(20) {
                0 => {
                    fl.pop();
                }
                1 => nfw += 1,
                2 if heavy => nfw = g.pick(&[-1, 0]),
                _ => {}
            }
            s += &format!("{nfw}\n");
            s += &fl.concat();
            (g.pick(&[".tri", ".ico", ".TRI"]).into(), s.into_bytes())
        }
    }
}

/// Another reader's random file of the kind the name says (its broken forms too).
fn random_file(g: &mut Gen, heavy: bool) -> (String, Vec<u8>) {
    match g.below(6) {
        0 => (String::new(), triangle_file(g, false, heavy).0),
        1 => (String::new(), quad_file(g, heavy).0),
        2 => (".asc".into(), ascii_surface(g)),
        3 => {
            let nv = g.int(1, 6);
            let nf = g.int(0, 4);
            (".geo".into(), geo_surface(g, nv, nf, false))
        }
        4 => {
            let pool: Vec<[f32; 3]> = (0..4)
                .map(|_| [coord(g, heavy), coord(g, heavy), coord(g, heavy)])
                .collect();
            let fs = facets(g, &pool);
            let b = if g.chance(2) {
                text_stl(g, &fs)
            } else {
                binary_stl(g, &fs)
            };
            (".stl".into(), b)
        }
        _ => {
            let nv = g.int(1, 6);
            let nf = g.int(0, 4);
            let f = faces(g, nv, nf);
            (".vtk".into(), vtk(g, nv, &f))
        }
    }
}

/// The hemisphere letters a name starts with.
/// The volume names a transform's comment lines may give: none exists in the work folder (an
/// existing one is read with `MRIreadHeader`, not yet ported), and a name containing `average_305`
/// gives MNI's average brain's geometry.
const XFM_VOLS: [&str; 6] = [
    "average_305.mnc",
    "nofile.mnc",
    "orig.mnc",
    "my_average_305",
    "nothere",
    "srcdst.mnc",
];

/// A subject's `talairach.xfm` (relay 17): MINC's text transform, its first line right or not, the
/// comment lines that may name its volumes (FreeSurfer's `%Generated by`, minctracc's command, MINC's
/// GUI lines, other comments), the `Transform_Type` lines or not, and three rows of four numbers
/// (sometimes two, a word among them, a singular matrix), or random bytes.
fn xfm_file(g: &mut Gen) -> Vec<u8> {
    if g.chance(25) {
        let n = g.int(0, 200) as usize;
        return (0..n).map(|_| g.below(256) as u8).collect();
    }
    let mut s = String::from(g.pick(&[
        "MNI Transform File\n",
        "MNI Transform File\n",
        "MNI Transform File\n",
        "MNI Transform File\n",
        "MNI Transform Fil\n",
        "% MNI Transform File\n",
    ]));
    let comments = g.int(0, 3);
    for _ in 0..comments {
        let (a, b) = (g.pick(&XFM_VOLS), g.pick(&XFM_VOLS));
        s += &match g.below(7) {
            0 => "% avi2talxfm\n".to_owned(),
            1 => format!("%Generated by mri_robust_register src {a} dst {b}\n"),
            2 => format!("%Thu Nov 12 20:55:19 2026>>> minctracc {a} {b} out.xfm\n"),
            3 => format!("% {a}\n"),
            4 => "\n".to_owned(),
            5 => format!("%Volume: {a}\n"),
            _ => "%\n".to_owned(),
        };
    }
    match g.below(6) {
        0 => {}
        1 => s += "Transform_Type = Linear;\nLinear_Transform =\n",
        _ => s += "\nTransform_Type = Linear;\nLinear_Transform =\n",
    }
    let rows = if g.chance(12) { 2 } else { 3 };
    let singular = g.chance(20);
    for r in 0..rows {
        let mut row = String::new();
        for c in 0..4 {
            let x = if singular && c < 3 {
                0.0
            } else if c == 3 {
                g.int(-4000, 4000) as f32 / 100.0
            } else if c == r {
                1.0 + g.int(-200, 200) as f32 / 1000.0
            } else {
                g.int(-150, 150) as f32 / 1000.0
            };
            row.push(' ');
            if g.chance(60) {
                row += "abc";
            } else {
                row += &text_f(g, x);
            }
        }
        if r == 2 {
            row.push(';');
        }
        row.push('\n');
        s += &row;
    }
    s.into_bytes()
}

const HEMI: [&str; 8] = ["lh.", "rh.", "Lh.", "R", "b", "B", "x", ""];

pub(super) fn srd_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(4);
    ix(t, g.int(0, 3));
    let mut env = 0;
    // (not TRIANGULARSURFACE_NOEXTRA_READ, 8: the triangle files here have their creator's line,
    // whose text would be read as counts of billions, as `tri` measures)
    for bit in [1, 2, 4, 16, 32, 64, 128] {
        if g.chance(14) {
            env |= bit;
        }
    }
    ix(t, env);
    let nfiles = g.int(1, 2) as usize;
    // relay 17: a third of the cases give the subject a Talairach transform, read for the surfaces
    // in d (two thirds of them then)
    let xfm = g.chance(3);
    let mut inputs: Vec<(String, Vec<u8>)> = Vec::new();
    for k in 0..nfiles {
        let (ext, b) = if g.chance(300) {
            let m = big_tree();
            let mut b = vec![0xff, 0xff, 0xfe];
            b.extend_from_slice(b"created by me on Thu Nov 12 20:55:19 2026\n\n");
            be4(&mut b, m.v.len() as i64);
            be4(&mut b, m.f.len() as i64);
            for p in &m.v {
                for c in p {
                    b.extend_from_slice(&c.to_be_bytes());
                }
            }
            for f in &m.f {
                for &x in f {
                    be4(&mut b, x);
                }
            }
            (String::new(), b)
        } else if g.chance(5) {
            random_file(g, heavy)
        } else {
            let m = mesh(g, heavy);
            write(g, &m, heavy)
        };
        let dir = if g.chance(3) != xfm { "d/" } else { "" };
        let hemi = g.pick(&HEMI);
        let mut name = format!("{dir}{hemi}s{k}{ext}");
        if g.chance(40) {
            // a long name of the same file (FS-356 at 300 bytes; FS-126 at 1,024, not with seed 1)
            let n = if g.1 && g.chance(4) { 520 } else { 150 };
            name = format!("{}{name}", "./".repeat(n));
        }
        inputs.push((name, b));
    }
    let nsurf = inputs.len();
    if xfm {
        let b = xfm_file(g);
        inputs.push(("mri/transforms/talairach.xfm".into(), b));
    }
    // the inputs are written by their short names; a long name reads the same file
    ix(t, inputs.len() as i64);
    for (n, b) in &inputs {
        let short = n.trim_start_matches("./");
        hx(t, short.as_bytes());
        hx(t, b);
    }
    let nops = g.int(1, 2);
    ix(t, nops);
    for _ in 0..nops {
        let mut n = inputs[g.below(nsurf as u64) as usize].0.clone();
        if g.chance(30) {
            n = g
                .pick(&[
                    "s9.asc", "s9.tri", "s.nii", "s.MGH", "s9.stl", "s9.geo", "s9.vtk",
                ])
                .to_owned();
        }
        if g.chance(2) {
            t.push("r".into());
            hx(t, n.as_bytes());
            ix(t, i64::from(!g.chance(4)));
        } else {
            t.push("f".into());
            hx(t, n.as_bytes());
        }
    }
}

fn dump_srd_surface(o: &mut Out, s: Option<&Mris>) {
    let Some(s) = s else {
        o.0 += "NULL ";
        return;
    };
    for x in [
        s.nvertices,
        s.nfaces,
        s.r#type,
        s.max_vertices,
        s.max_faces,
        s.hemisphere,
    ] {
        o.i(i64::from(x));
    }
    o.h(Some(s.fname.as_bytes()));
    o.i(i64::from(s.useRealRAS));
    o.i(i64::from(s.orig_xyzspace));
    o.i(i64::from(s.patch));
    o.i(i64::from(s.status.0));
    for x in [
        s.nsize,
        s.max_nsize,
        s.dist_nsize,
        s.dist_orig_nsize,
        s.dist_alloced_flags,
    ] {
        o.i(i64::from(x));
    }
    o.f(s.avg_nbrs);
    o.d(s.radius);
    o.i(i64::from(s.group_avg_vtxarea_loaded));
    o.i(i64::from(s.lta.is_some()));
    o.i(i64::from(s.free_transform));
    if let Some(l) = &s.lta {
        // relay 17: the transform found and the two matrices cached from it
        o.i(i64::from(l.ltype));
        o.i(i64::from(l.num_xforms));
        let lt = &l.xforms[0];
        o.i(i64::from(lt.ltype));
        super::life::dump_vg(&lt.src, o);
        super::life::dump_vg(&lt.dst, o);
        for mt in [&lt.m_l, &s.SRASToTalSRAS_, &s.TalSRASToSRAS_] {
            match mt {
                None => o.s("NULL"),
                Some(mt) => {
                    for r in 1..=4 {
                        for c in 1..=4 {
                            o.f(mt.relt(r, c));
                        }
                    }
                }
            }
        }
    }
    super::life::dump_vg(&s.vg, o);
    for x in [
        s.xlo,
        s.ylo,
        s.zlo,
        s.xhi,
        s.yhi,
        s.zhi,
        s.xctr,
        s.yctr,
        s.zctr,
        s.total_area,
    ] {
        o.f(x);
    }
    o.d(s.avg_vertex_area);
    o.d(s.avg_vertex_dist);
    o.d(s.std_vertex_dist);
    o.f(s.neg_area);
    o.f(s.neg_orig_area);
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        let vt = s.topology(vno);
        for x in [
            v.x, v.y, v.z, v.nx, v.ny, v.nz, v.area, v.origarea, v.curv, v.tx, v.ty, v.tz,
        ] {
            o.f(x);
        }
        o.i(i64::from(v.border));
        o.i(i64::from(v.ripflag));
        o.i(i64::from(v.marked));
        o.i(i64::from(vt.num));
        for k in 0..usize::from(vt.num) {
            o.i(i64::from(vt.f.as_ref().unwrap()[k]));
            o.i(i64::from(vt.n.as_ref().unwrap()[k]));
        }
        for x in [vt.vnum, vt.v2num, vt.v3num, vt.vtotal] {
            o.i(i64::from(x));
        }
        o.i(i64::from(vt.nsizeMax));
        o.i(i64::from(vt.nsizeCur));
        let vs = topology::vertex_vsize(s, vno).max(0) as usize;
        o.i(vs as i64);
        for k in 0..vs {
            o.i(i64::from(vt.v.as_ref().unwrap()[k]));
        }
        for (d, cap) in [
            (&v.dist, v.dist_capacity),
            (&v.dist_orig, v.dist_orig_capacity),
        ] {
            o.i(i64::from(d.is_some()));
            if let Some(d) = d {
                for k in 0..vs.min(cap.max(0) as usize) {
                    o.f(d[k]);
                }
            }
        }
    }
    for fno in 0..s.nfaces.max(0) as usize {
        let f = s.face(fno);
        for x in f.v {
            o.i(i64::from(x));
        }
        o.i(i64::from(f.ripflag));
        o.f(f.area);
        for x in f.angle {
            o.f(x);
        }
        let e = s.faceNormCacheEntries.as_ref().unwrap()[fno];
        for x in [e.nx, e.ny, e.nz, e.orig_area] {
            o.f(x);
        }
        o.i(i64::from(
            s.faceNormDeferredEntries.as_ref().unwrap()[fno].deferred,
        ));
    }
}

pub(super) fn srd_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    std::fs::create_dir_all(dir.join("mri").join("transforms")).expect("the folder mri/transforms");
    cx.g.progname = "fssurfio".into();
    let gd = t.i();
    if gd & 1 != 0 {
        cx.g.gdiag |= DIAG_SHOW;
    }
    if gd & 2 != 0 {
        cx.g.gdiag |= DIAG_VERBOSE;
    }
    let mask = t.i();
    let mut pairs: Vec<(String, String)> = Vec::new();
    for (bit, name, value) in [
        (1, "mrisCompleteTopology_new", "1"),
        (2, "MRISfindNeighborsAtVertex_new", "1"),
        (4, "MRISfindNeighborsAtVertex_old", "1"),
        (8, "TRIANGULARSURFACE_NOEXTRA_READ", "1"),
        (16, "FS_SKIP_TAGS", "1"),
        (32, "FS_GII", ".gii"),
        (64, "FREESURFER_always_check_vv", "1"),
        (128, "FS_FASTER_MP", "1"),
    ] {
        if mask & bit != 0 {
            pairs.push((name.into(), value.into()));
        }
    }
    pairs.push(("SURFER_SIDEDOOR".into(), "1".into()));
    let env = Environment::from_pairs(pairs);
    let nin = t.i();
    for _ in 0..nin {
        let (n, b) = (utf8(t.h()), t.h());
        std::fs::write(dir.join(&n), b).expect("an input file");
    }
    let nops = t.i();
    let d = Some(dir);
    for _ in 0..nops {
        let op = t.next();
        let n = utf8(t.h());
        let mut io = coords::Io {
            out: &mut cx.a.clone(),
            err: &mut cx.b.clone(),
            env: &env,
        };
        let g = &mut cx.g;
        let s = match op {
            "r" => {
                let dotkr = t.i() != 0;
                readwrite::read(g, &mut cx.sg, &mut io, d, &n, dotkr)?
            }
            "f" => readwrite::fast_read(g, &mut cx.sg, &mut io, d, &n)?,
            _ => panic!("srd op {op}"),
        };
        dump_srd_surface(o, s.as_ref());
    }
    Ok(())
}
