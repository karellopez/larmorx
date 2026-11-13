// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrisurf.cpp` (`fs.surfio`, queue item 3.1-46), the functions that need no volume:
//! `MRISaddCommandLine` (FS-112), `mrisVertexHash` (the vertex checksum the topology fixer's
//! save and restore compare) with `include/fnv_hash.h`'s `fnv_init` and `fnv_add`,
//! `MRISdiffSimple`, and `SMALL_SURFACE` with `MRISSfree`.
//!
//! Not here: `MRISripMidline`, `MRIcomputeLabelNormal` and `MRIScopyCoords` (they take fs.mri's
//! `MRI`, `LABEL` and `VOL_GEOM`: with queue item 3.1-55's structure); `mris_hash_init`,
//! `mris_hash_add`, `mris_hash_print`, `mris_print_hash` and `mris_print_diff` (every caller in
//! FreeSurfer 8.2.0's programs is behind `debugNonDeterminism`, a constant 0, or in
//! `utils/test/`); the six `gMRISexternal*` hooks (fs.deform's `INTEGRATION_PARMS`).

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::FS_112;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log;
use larmorx_freesurfer_core::sse;

use crate::aaa::{MAX_CMDS, MrisHash};
use crate::mris::Mris;
use crate::topology::vertex_vsize;

/// `fnv_init()` (`include/fnv_hash.h`): the 32-bit FNV offset basis, held in an `unsigned long`.
pub const FNV_INIT: u64 = 0x811c_9dc5;

/// `fnv_add(hash, p, size)`: for each byte, `hash ^= byte; hash *= 16777619` in 64-bit unsigned
/// arithmetic (FNV-1a's step with the 32-bit prime, never reduced to 32 bits).
pub fn fnv_add(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

/// `MRISaddCommandLine(mris, cmdline)`: the command line (its bytes before a NUL) becomes the
/// surface's next one. A surface that holds `MAX_CMDS` (1,000) already gets the message `error:
/// can't add cmd to surface since max cmds (<n>) has been reached` on standard error (and in the
/// error log); FreeSurfer then stores it past its array, over the count itself (FS-112): larmorx
/// keeps the 1,000 and drops the line, with FS-112's notice, in both modes (the original's count
/// would be a memory address). Returns `NO_ERROR`.
pub fn add_command_line(
    g: &mut Globals,
    err: &mut dyn Write,
    mris: &mut Mris,
    cmdline: &[u8],
) -> i32 {
    if mris.ncmds >= MAX_CMDS as i32 {
        log::error(
            g,
            err,
            &format!(
                "can't add cmd to surface since max cmds ({}) has been reached",
                mris.ncmds
            ),
        );
        g.notices.add(
            &FS_112,
            "a command line past the surface's 1,000 is dropped (the original writes it past its array)",
        );
        return 0;
    }
    let i = mris.ncmds as usize;
    mris.ncmds += 1;
    let end = cmdline
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(cmdline.len());
    mris.cmdlines[i] = Some(cmdline[..end].to_vec());
    0
}

/// `mrisVertexHash(hash, mris, vno)`: the FNV hash ([`fnv_add`] from [`FNV_INIT`]) of the vertex's
/// neighbourhood: the bytes (little-endian) of `vnum`, `v2num`, `v3num`, `vtotal` (`short`),
/// `nsizeMax`, `nsizeCur`, `num` (`uchar`); then, each only when its array exists, the first
/// `vsize` ([`vertex_vsize`]) neighbour numbers (`int`), the `num` face numbers (`int`) followed
/// by the `num` positions in them (`uchar`), the first `vsize` distances and the first `vsize`
/// original distances (`float` bits). No vertex coordinate or value enters it (the source
/// defines the per-field step for `VERTEX` but never applies it). An array shorter than what is
/// hashed is memory FreeSurfer reads but does not own: here a panic.
pub fn vertex_hash(mris: &Mris, vno: usize) -> MrisHash {
    let vt = mris.topology(vno);
    let v = mris.vertex(vno);
    let mut h = FNV_INIT;
    for s in [vt.vnum, vt.v2num, vt.v3num, vt.vtotal] {
        h = fnv_add(h, &s.to_le_bytes());
    }
    h = fnv_add(h, &[vt.nsizeMax, vt.nsizeCur, vt.num]);
    let vsize = usize::try_from(vertex_vsize(mris, vno)).unwrap_or(0);
    let num = usize::from(vt.num);
    if let Some(a) = &vt.v {
        for x in &a[..vsize] {
            h = fnv_add(h, &x.to_le_bytes());
        }
    }
    if let Some(f) = &vt.f {
        for x in &f[..num] {
            h = fnv_add(h, &x.to_le_bytes());
        }
        let n = vt.n.as_ref().expect("VERTEX_TOPOLOGY::n is NULL");
        h = fnv_add(h, &n[..num]);
    }
    for d in [&v.dist, &v.dist_orig].into_iter().flatten() {
        for x in &d[..vsize] {
            h = fnv_add(h, &x.to_bits().to_le_bytes());
        }
    }
    MrisHash { hash: h }
}

/// `%g` of a `double`, as `printf` writes it.
fn g6(x: f64) -> String {
    cfmt::g(x, 6)
}

/// `MRISdiffSimple(surf1, surf2, ndiffmin, rmsthresh, verbosity)`: compares two surfaces, writing
/// to standard output as it goes, and returns 0 (no difference beyond `ndiffmin`), 1 (vertex
/// counts), 2 (face counts), 3 (1-hop neighbour counts), 4 (rip flags, or positions). Each
/// unripped pair's distance `sqrt((dx² + dy²) + dz²)` (the differences in `float`, the rest in
/// `double`) is stored in `surf1`'s `val` (as `float`); a distance above `rmsthresh` counts as a
/// difference; `Gdiag_no`'s vertex gets a line of its own.
pub fn diff_simple(
    g: &Globals,
    out: &mut dyn Write,
    surf1: &mut Mris,
    surf2: &Mris,
    ndiffmin: i32,
    rmsthresh: f64,
    verbosity: i32,
) -> i32 {
    let _ = writeln!(
        out,
        "Entering MRISdiffSimple(): ndiffmin={ndiffmin}, rmsthresh={}, verbosity={verbosity}",
        g6(rmsthresh)
    );
    if surf1.nvertices != surf2.nvertices {
        let _ = writeln!(
            out,
            "Surfaces differ in the number of vertices {} {}",
            surf1.nvertices, surf2.nvertices
        );
        return 1;
    }
    if surf1.nfaces != surf2.nfaces {
        let _ = writeln!(
            out,
            "Surfaces differ in the number of faces {} {}",
            surf1.nfaces, surf2.nfaces
        );
        return 2;
    }
    let nv = usize::try_from(surf1.nvertices).unwrap_or(0);

    let mut ndiff = 0;
    for n in 0..nv {
        let (a, b) = (surf1.topology(n).vnum, surf2.topology(n).vnum);
        if a != b {
            if verbosity > 0 {
                let _ = writeln!(
                    out,
                    "{ndiff} Surfaces differ in number of neighbors at vertex {n}   {a} {b}"
                );
            }
            ndiff += 1;
        }
    }
    if ndiff > ndiffmin {
        let _ = writeln!(
            out,
            "Surfaces differ in number of neighbors at a vertex ndiff={ndiff}"
        );
        return 3;
    }

    ndiff = 0;
    for n in 0..nv {
        if surf1.vertex(n).ripflag != surf2.vertex(n).ripflag {
            ndiff += 1;
        }
    }
    if ndiff > ndiffmin {
        let _ = writeln!(out, "Surfaces differ in ripflags ndiff={ndiff}");
        return 4;
    }

    ndiff = 0;
    let mut rmsmax = 0.0f64;
    let (mut vnomax, mut nhits) = (0usize, 0i32);
    let mut rmssum = 0.0f64;
    for n in 0..nv {
        let v2 = surf2.vertex(n);
        let (x2, y2, z2, rip2) = (v2.x, v2.y, v2.z, v2.ripflag);
        let v1 = surf1.vertex_mut(n);
        if v1.ripflag != 0 || rip2 != 0 {
            continue;
        }
        nhits += 1;
        // SSE's NaNs (the expression's order): the same bits on every platform
        let dx = f64::from(sse::sub(v1.x, x2));
        let dy = f64::from(sse::sub(v1.y, y2));
        let dz = f64::from(sse::sub(v1.z, z2));
        let rms = sse::sqrt64(sse::add64(
            sse::add64(sse::mul64(dx, dx), sse::mul64(dy, dy)),
            sse::mul64(dz, dz),
        ));
        v1.val = rms as f32;
        let (x1, y1, z1) = (f64::from(v1.x), f64::from(v1.y), f64::from(v1.z));
        let (x2, y2, z2) = (f64::from(x2), f64::from(y2), f64::from(z2));
        if g.gdiag_no >= 0 && g.gdiag_no as usize == n {
            let _ = writeln!(
                out,
                "vtxno {n}  [{},{},{}] [{},{},{}] {}",
                g6(x1),
                g6(y1),
                g6(z1),
                g6(x2),
                g6(y2),
                g6(z2),
                g6(rms)
            );
        }
        rmssum = sse::add64(rmssum, rms);
        if rmsmax < rms {
            rmsmax = rms;
            vnomax = n;
        }
        if rms > rmsthresh {
            if verbosity > 0 {
                let _ = writeln!(
                    out,
                    "{ndiff} Surfaces differ in xyz at vertex {n}  rms={} ({},{},{}) ({},{},{})",
                    g6(rms),
                    g6(x1),
                    g6(y1),
                    g6(z1),
                    g6(x2),
                    g6(y2),
                    g6(z2)
                );
            }
            ndiff += 1;
        }
    }
    if ndiff > ndiffmin {
        let _ = writeln!(
            out,
            "Surfaces differ in vertex xyz ndiff={ndiff}, rmsmax = {} at {vnomax}, rmsmean = {}",
            g6(rmsmax),
            g6(sse::div64(rmssum, f64::from(nhits)))
        );
        return 4;
    }
    if rmsmax > 0.0 {
        let _ = writeln!(out, "rmsmax = {}", g6(rmsmax));
    }
    let _ = writeln!(out, "surfaces do not differ");
    0
}

/// `SMALL_VERTEX`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SmallVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// `SMALL_SURFACE`: a surface's vertex positions only (`MRISreadVerticesOnly`, queue item 3.1-48).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SmallSurface {
    pub nvertices: i32,
    pub vertices: Vec<SmallVertex>,
}

/// `MRISSfree(&mriss)`: frees the surface and sets the caller's pointer to `NULL`; returns
/// `NO_ERROR`.
pub fn small_surface_free(mriss: &mut Option<SmallSurface>) -> i32 {
    *mriss = None;
    0
}
