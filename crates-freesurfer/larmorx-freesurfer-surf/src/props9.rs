// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Queue item 3.1-51, its leftovers (relay 23 of track D): `utils/mrisurf_metricProperties.cpp`'s
//! surface-RAS-to-voxel helper (`mriSurfaceRASToVoxel`, 11683-11766, with its global matrix
//! `VoxelFromSRASmatrix`), the vertex-to-voxel table of a volume of one voxel per vertex
//! (`MRIScrsLUT`, `MRIScrsLUTFree`, 12857-12895) and the three functions on such volumes: the
//! spatial AR(1) (`MRISar1`, 12765-12850), the autocorrelation by hops (`MRISarN`, 12508-12604)
//! and the kernel smoothing (`MRISsmoothKernel`, 12610-12758), with fs.surf's hop lists taken
//! early (`utils/mrisurf_vals.cpp` 3134-3220: `SetSurfHopListAlloc`, `SurfHopListFree`,
//! `SetSurfHopList`). Their parallel loops run serially in FreeSurfer (ROMP's `experimental`
//! level), as here.
//!
//! Arithmetic through fs.core's `sse` operations in the official build's operand order (the unit
//! oracle's object code; `docs/findings/freesurfer-surfio.md` §25): the neighbour's square and
//! the cross product as written, the variances' product as `nbrvar * vtxvar`, each term added as
//! `term + sum`, the kernel's weights as `value * weight + sum` and `fabs(weight) + sum`.
//!
//! Defects (`larmorx_core::defects`): FS-525 (volumes of another type read and written as
//! floats), FS-526 (the per-vertex kernel ignored), FS-527 (the closing message's voxel 1031),
//! FS-528 (a table leaked), FS-529 (a given output not cleared: kept), FS-530 (fewer than one
//! hop), FS-531 (the two ways' check reversed), FS-532 (no matrix, or a product refused), FS-533
//! (hop lists of fewer hops than the kernel), FS-534 (FS-525's accesses past a volume's buffer).

use larmorx_core::cfmt;
use larmorx_core::defects::{
    FS_525, FS_526, FS_527, FS_528, FS_529, FS_530, FS_531, FS_532, FS_533, FS_534,
};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sqrt64};
use larmorx_freesurfer_core::timer::Timer;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::mri_clone_by_space;

use crate::aaa::SurfGlobals;
use crate::coords::{Io, metric_properties_cpp};
use crate::mris::Mris;

// ---- direct float access (`MRIFseq_vox`) ------------------------------------------------------

/// Where `MRIFseq_vox(m, c, r, s, f)` lands: a float of the volume, another place in its buffer
/// (a volume of another type, or an index outside its extent), or outside the buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    In(usize),
    Other(usize),
    Outside,
}

/// The row pointer `slices[s + f depth][r]` plus `c` floats (the rows step by the volume's own
/// voxel size).
fn fplace(m: &Mri, c: i32, r: i32, s: i32, f: i32) -> Place {
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    let sl = s.wrapping_add(f.wrapping_mul(d));
    let nslices = d.wrapping_mul(m.nframes);
    if m.chunk.is_none() || !(0..nslices).contains(&sl) || !(0..h).contains(&r) {
        return Place::Outside;
    }
    let row = (i128::from(sl) * i128::from(h) + i128::from(r))
        * i128::from(m.vox_per_row)
        * m.bytes_per_vox as i128;
    let b = row + i128::from(c) * 4;
    if b < 0 || b + 4 > i128::from(m.bytes_total) {
        return Place::Outside;
    }
    let inside = (0..w).contains(&c)
        && (0..d).contains(&s)
        && (0..m.nframes).contains(&f)
        && m.mtype == MRI_FLOAT;
    if inside {
        Place::In(b as usize)
    } else {
        Place::Other(b as usize)
    }
}

fn float_at(m: &Mri, o: usize) -> f32 {
    let chunk = m.chunk.as_ref().expect("voxels");
    f32::from_le_bytes(chunk[o..o + 4].try_into().expect("4 bytes"))
}

/// `MRIFseq_vox(m, c, r, s, f)` read. FS-525: a volume of another type gives, by default, its
/// voxel's value (`MRIgetVoxVal`); with `reproduce` the four bytes FreeSurfer reads there (0
/// outside the volume's buffer: memory it does not own).
#[allow(clippy::too_many_arguments)]
fn fget(
    g: &mut Globals,
    io: &mut Io,
    who: &str,
    m: &Mri,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
) -> Result<f32, Fatal> {
    let p = fplace(m, c, r, s, f);
    if let Place::In(o) = p {
        return Ok(float_at(m, o));
    }
    g.notices.add(
        &FS_525,
        format!(
            "{who} reads a voxel of a volume of type {} as a float",
            m.mtype
        ),
    );
    if p == Place::Outside {
        g.notices.add(
            &FS_534,
            format!("{who} reads a float past a volume's buffer (memory contents; 0 here)"),
        );
    }
    if !g.notices.reproduce() {
        return mri_get_vox_val(g, io.err, m, c, r, s, f, 0);
    }
    Ok(match p {
        Place::Other(o) => float_at(m, o),
        _ => 0.0,
    })
}

/// `MRIFseq_vox(m, c, r, s, f) = v`. FS-525: a volume of another type gets, by default, the value
/// as its type (`MRIsetVoxVal`); with `reproduce` the float's four bytes where FreeSurfer writes
/// them (nowhere outside the volume's buffer).
#[allow(clippy::too_many_arguments)]
fn fset(
    g: &mut Globals,
    io: &mut Io,
    who: &str,
    m: &mut Mri,
    c: i32,
    r: i32,
    s: i32,
    f: i32,
    v: f32,
) -> Result<(), Fatal> {
    let p = fplace(m, c, r, s, f);
    let at = match p {
        Place::In(o) => Some(o),
        _ => {
            g.notices.add(
                &FS_525,
                format!(
                    "{who} writes a voxel of a volume of type {} as a float",
                    m.mtype
                ),
            );
            if p == Place::Outside {
                g.notices.add(
                    &FS_534,
                    format!("{who} writes a float past a volume's buffer (memory corruption; nothing written here)"),
                );
            }
            if !g.notices.reproduce() {
                mri_set_vox_val(g, io.err, m, c, r, s, f, v, 0)?;
                return Ok(());
            }
            match p {
                Place::Other(o) => Some(o),
                _ => None,
            }
        }
    };
    if let Some(o) = at {
        let chunk = m.chunk.as_mut().expect("voxels");
        chunk[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    Ok(())
}

/// `src->width * src->height * src->depth` in C's `int` arithmetic.
fn nvox(src: &Mri) -> i32 {
    src.vg
        .width
        .wrapping_mul(src.vg.height)
        .wrapping_mul(src.vg.depth)
}

/// `MRIgetVoxVal(mask, c, r, s, 0) < 0.5`: the vertex is masked out.
fn masked(
    g: &mut Globals,
    io: &mut Io,
    mask: Option<&Mri>,
    c: i32,
    r: i32,
    s: i32,
) -> Result<bool, Fatal> {
    match mask {
        Some(m) => Ok(f64::from(mri_get_vox_val(g, io.err, m, c, r, s, 0, 0)?) < 0.5),
        None => Ok(false),
    }
}

/// A vertex's neighbours (the first `vnum` of its list).
fn neighbours(surf: &Mris, vno: usize) -> Vec<usize> {
    let vt = surf.topology(vno);
    let n = usize::try_from(vt.vnum).unwrap_or(0);
    vt.v.as_deref().unwrap_or(&[])[..n]
        .iter()
        .map(|&x| x as usize)
        .collect()
}

// ---- the vertex-to-voxel table --------------------------------------------------------------

/// `MRIScrsLUT`'s table: each vertex's column, row and slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrsLut {
    pub c: Vec<i32>,
    pub r: Vec<i32>,
    pub s: Vec<i32>,
}

/// `MRIScrsLUT(surf, src)`: for a volume of as many voxels as the surface has vertices, each
/// vertex's column, row and slice, the vertices numbered column fastest, then row, then slice;
/// `None` after `ERROR: MRIScrsLUT: surf/src dimension mismatch.` (standard output) otherwise.
/// FS-528: FreeSurfer then leaks the table it began (noted; nothing is kept here).
pub fn mris_crs_lut(g: &mut Globals, io: &mut Io, surf: &Mris, src: &Mri) -> Option<CrsLut> {
    let n = nvox(src);
    if surf.nvertices != n {
        let _ = io
            .out
            .write_all(b"ERROR: MRIScrsLUT: surf/src dimension mismatch.\n");
        g.notices.add(
            &FS_528,
            "MRIScrsLUT: the table begun for a mismatched volume is freed (FreeSurfer leaks it)",
        );
        return None;
    }
    let cap = usize::try_from(n).unwrap_or(0);
    let mut lut = CrsLut {
        c: Vec::with_capacity(cap),
        r: Vec::with_capacity(cap),
        s: Vec::with_capacity(cap),
    };
    for s in 0..src.vg.depth {
        for r in 0..src.vg.height {
            for c in 0..src.vg.width {
                lut.c.push(c);
                lut.r.push(r);
                lut.s.push(s);
            }
        }
    }
    Some(lut)
}

/// `MRIScrsLUTFree(crslut)`: 0 (the table is dropped).
pub fn mris_crs_lut_free(lut: CrsLut) -> i32 {
    drop(lut);
    0
}

// ---- fs.surf's hop lists, taken early -------------------------------------------------------

/// `SURFHOPLIST` (`mrisurf.h` 155-163): the vertices 0, 1, ... `nhops - 1` links away from a
/// centre vertex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfHopList {
    /// `cvtx`: the centre vertex.
    pub cvtx: i32,
    /// `nhops`.
    pub nhops: i32,
    /// `hit`: whether each vertex of the surface is in a list.
    pub hit: Vec<u8>,
    /// `vtxlist` (with `nperhop` its lengths): the vertices of each hop, in the order found.
    pub vtxlist: Vec<Vec<i32>>,
}

impl SurfHopList {
    /// `nperhop[hop]`.
    pub fn nperhop(&self, hop: usize) -> i32 {
        self.vtxlist.get(hop).map_or(0, |l| l.len() as i32)
    }
}

/// `SetSurfHopListAlloc(surf, nhops)`: empty lists of `nhops` hops for the surface's vertices.
pub fn set_surf_hop_list_alloc(surf: &Mris, nhops: i32) -> SurfHopList {
    SurfHopList {
        cvtx: 0,
        nhops,
        hit: vec![0; usize::try_from(surf.nvertices).unwrap_or(0)],
        vtxlist: vec![Vec::new(); usize::try_from(nhops).unwrap_or(0)],
    }
}

/// `SurfHopListFree(&shl)`: 0 (the lists are dropped).
pub fn surf_hop_list_free(shl: SurfHopList) -> i32 {
    drop(shl);
    0
}

/// `SetSurfHopList(center, surf, nhops)`: the vertices 0 (the centre), 1, ... `nhops - 1` links
/// away, each hop's in the order its predecessors' neighbour lists give them, each vertex once;
/// every listed vertex's `undefval` set to its hop. FS-530: fewer than one hop crashes FreeSurfer
/// (its lists are empty); `reproduce` ends the program so, the default refuses it
/// (`SetSurfHopList: <n> hops: at least 1 is needed` on standard error, `None`).
pub fn set_surf_hop_list(
    g: &mut Globals,
    io: &mut Io,
    center: i32,
    surf: &mut Mris,
    nhops: i32,
) -> Result<Option<SurfHopList>, Fatal> {
    if nhops < 1 {
        g.notices.add(
            &FS_530,
            format!(
                "SetSurfHopList: {nhops} hops (FreeSurfer writes into empty lists and crashes)"
            ),
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        let _ = writeln!(io.err, "SetSurfHopList: {nhops} hops: at least 1 is needed");
        return Ok(None);
    }
    let mut shl = set_surf_hop_list_alloc(surf, nhops);
    shl.cvtx = center;
    shl.vtxlist[0].push(center);
    shl.hit[center as usize] = 1;
    for hop in 1..nhops as usize {
        let mut list = Vec::new();
        for k in 0..shl.vtxlist[hop - 1].len() {
            let vno = shl.vtxlist[hop - 1][k] as usize;
            for nbr in neighbours(surf, vno) {
                if shl.hit[nbr] != 0 {
                    continue;
                }
                shl.hit[nbr] = 1;
                list.push(nbr as i32);
            }
        }
        shl.vtxlist[hop] = list;
    }
    for (hop, list) in shl.vtxlist.iter().enumerate() {
        for &vno in list {
            surf.vertex_mut(vno as usize).undefval = hop as i32;
        }
    }
    Ok(Some(shl))
}

// ---- the autocorrelations -------------------------------------------------------------------

/// The cross term of a vertex and a neighbour over the frames: `None` when the neighbour's sum of
/// squares is 0, else `(sumsqx / n) / sqrt(nbrvar * vtxvar)`.
#[allow(clippy::too_many_arguments)]
fn cross(
    g: &mut Globals,
    io: &mut Io,
    who: &str,
    src: &Mri,
    v: (i32, i32, i32),
    nb: (i32, i32, i32),
    vtxvar: f64,
) -> Result<Option<f64>, Fatal> {
    let (mut sumsqnbr, mut sumsqx) = (0.0f64, 0.0f64);
    for frame in 0..src.nframes {
        let valnbr = f64::from(fget(g, io, who, src, nb.0, nb.1, nb.2, frame)?);
        let valvtx = f64::from(fget(g, io, who, src, v.0, v.1, v.2, frame)?);
        sumsqnbr = add64(sumsqnbr, mul64(valnbr, valnbr));
        sumsqx = add64(sumsqx, mul64(valvtx, valnbr));
    }
    if sumsqnbr == 0.0 {
        return Ok(None);
    }
    let n = f64::from(src.nframes);
    let nbrvar = div64(sumsqnbr, n);
    let term = div64(div64(sumsqx, n), sqrt64(mul64(nbrvar, vtxvar)));
    Ok(Some(term))
}

/// A vertex's variance over the frames (`sumsq / nframes`), `None` when its sum of squares is 0.
fn variance(
    g: &mut Globals,
    io: &mut Io,
    who: &str,
    src: &Mri,
    v: (i32, i32, i32),
) -> Result<Option<f64>, Fatal> {
    let mut sumsq = 0.0f64;
    for frame in 0..src.nframes {
        let x = f64::from(fget(g, io, who, src, v.0, v.1, v.2, frame)?);
        sumsq = add64(sumsq, mul64(x, x));
    }
    if sumsq == 0.0 {
        return Ok(None);
    }
    Ok(Some(div64(sumsq, f64::from(src.nframes))))
}

/// `MRISar1(surf, src, mask, ar1)`: for a volume of one voxel per vertex (`src`, frames the
/// samples), each unripped, unmasked vertex of non-zero sum of squares gets the mean over its
/// unripped, unmasked neighbours of non-zero sum of squares of `(sum of products / n) /
/// sqrt(variance products)` (frame 0 of `ar1`, a new one-frame `MRI_FLOAT` volume like `src` when
/// none is given; other voxels as they were). `None` after `ERROR: MRISar1: Surf/Src dimension
/// mismatch.` (standard output). FS-525.
pub fn mris_ar1(
    g: &mut Globals,
    io: &mut Io,
    surf: &Mris,
    src: &Mri,
    mask: Option<&Mri>,
    ar1: Option<Mri>,
) -> Result<Option<Mri>, Fatal> {
    const WHO: &str = "MRISar1";
    if surf.nvertices != nvox(src) {
        let _ = io
            .out
            .write_all(b"ERROR: MRISar1: Surf/Src dimension mismatch.\n");
        return Ok(None);
    }
    let mut ar1 = match ar1 {
        Some(m) => m,
        None => mri_clone_by_space(g, io.out, io.err, src, MRI_FLOAT, 1)?,
    };
    let lut = mris_crs_lut(g, io, surf, src).expect("the sizes were checked");
    for vtx in 0..usize::try_from(surf.nvertices).unwrap_or(0) {
        if surf.vertex(vtx).ripflag != 0 {
            continue;
        }
        let v = (lut.c[vtx], lut.r[vtx], lut.s[vtx]);
        if masked(g, io, mask, v.0, v.1, v.2)? {
            continue;
        }
        let Some(vtxvar) = variance(g, io, WHO, src, v)? else {
            continue;
        };
        let (mut sum, mut n) = (0.0f64, 0i32);
        for nbr in neighbours(surf, vtx) {
            if surf.vertex(nbr).ripflag != 0 {
                continue;
            }
            let nb = (lut.c[nbr], lut.r[nbr], lut.s[nbr]);
            if masked(g, io, mask, nb.0, nb.1, nb.2)? {
                continue;
            }
            if let Some(term) = cross(g, io, WHO, src, v, nb, vtxvar)? {
                sum = add64(term, sum);
                n += 1;
            }
        }
        if n != 0 {
            let x = div64(sum, f64::from(n)) as f32;
            fset(g, io, WHO, &mut ar1, v.0, v.1, v.2, 0, x)?;
        }
    }
    Ok(Some(ar1))
}

/// `MRISarN(surf, src, mask, arN, N)`: as [`mris_ar1`] by hops: for each unripped, unmasked vertex
/// of non-zero sum of squares, frame 0 is 1 and frame `h` (1 to `N - 1`) the mean term over the
/// vertices `h` links away ([`set_surf_hop_list`], which sets every listed vertex's `undefval`);
/// `arN` a new `N`-frame `MRI_FLOAT` volume like `src` when none is given. `None` after `ERROR:
/// MRISarN: Surf/Src dimension mismatch.`. FS-530: `N` below 1 crashes FreeSurfer at the first
/// vertex it handles (`reproduce`); the default refuses it (`ERROR: MRISarN: <N> hops: at least 1
/// is needed`, `None`). FS-525.
pub fn mris_arn(
    g: &mut Globals,
    io: &mut Io,
    surf: &mut Mris,
    src: &Mri,
    mask: Option<&Mri>,
    arn: Option<Mri>,
    nhops: i32,
) -> Result<Option<Mri>, Fatal> {
    const WHO: &str = "MRISarN";
    if surf.nvertices != nvox(src) {
        let _ = io
            .out
            .write_all(b"ERROR: MRISarN: Surf/Src dimension mismatch.\n");
        return Ok(None);
    }
    if nhops < 1 {
        g.notices.add(
            &FS_530,
            format!("MRISarN: {nhops} hops (FreeSurfer crashes at the first vertex it handles)"),
        );
        if !g.notices.reproduce() {
            let _ = writeln!(io.out, "ERROR: MRISarN: {nhops} hops: at least 1 is needed");
            return Ok(None);
        }
    }
    let mut arn = match arn {
        Some(m) => m,
        None => mri_clone_by_space(g, io.out, io.err, src, MRI_FLOAT, nhops)?,
    };
    let lut = mris_crs_lut(g, io, surf, src).expect("the sizes were checked");
    for vtx in 0..usize::try_from(surf.nvertices).unwrap_or(0) {
        if surf.vertex(vtx).ripflag != 0 {
            continue;
        }
        let v = (lut.c[vtx], lut.r[vtx], lut.s[vtx]);
        if masked(g, io, mask, v.0, v.1, v.2)? {
            continue;
        }
        let Some(vtxvar) = variance(g, io, WHO, src, v)? else {
            continue;
        };
        if nhops < 1 {
            // reproduce: FreeSurfer writes into a volume or lists of no hops
            return Err(Fatal::Exit(139));
        }
        fset(g, io, WHO, &mut arn, v.0, v.1, v.2, 0, 1.0)?;
        let shl = set_surf_hop_list(g, io, vtx as i32, surf, nhops)?.expect("at least one hop");
        for hop in 1..nhops as usize {
            let (mut sum, mut n) = (0.0f64, 0i32);
            for &nbr in &shl.vtxlist[hop] {
                let nbr = nbr as usize;
                if surf.vertex(nbr).ripflag != 0 {
                    continue;
                }
                let nb = (lut.c[nbr], lut.r[nbr], lut.s[nbr]);
                if masked(g, io, mask, nb.0, nb.1, nb.2)? {
                    continue;
                }
                if let Some(term) = cross(g, io, WHO, src, v, nb, vtxvar)? {
                    sum = add64(term, sum);
                    n += 1;
                }
            }
            if n != 0 {
                let x = div64(sum, f64::from(n)) as f32;
                fset(g, io, WHO, &mut arn, v.0, v.1, v.2, hop as i32, x)?;
            }
        }
        surf_hop_list_free(shl);
    }
    Ok(Some(arn))
}

// ---- the kernel smoothing -------------------------------------------------------------------

/// `MRISsmoothKernel`'s output: a new volume (`NULL`), the caller's, or the input itself (refused).
#[derive(Debug)]
pub enum KernelOutput {
    New,
    Given(Box<Mri>),
    Src,
}

/// `MRISsmoothKernel(surf, src, mask, mrikern, globkern, &shl, out)`: each unripped, unmasked
/// vertex's frames become the weighted sum of its unripped, unmasked vertices 0 to `nhops - 1`
/// links away (each hop's weight from `globkern`'s first column, row `hop + 1`, or from the
/// vertex's own kernel `mrikern`, voxel `(vertex, 0, 0, hop)`), divided by the sum of the weights'
/// absolute values (counted over frame 0); `nhops` the kernel's frames or rows. The hop lists are
/// made once ([`set_surf_hop_list`] for every vertex, setting `undefval`) and kept in `shl` for the
/// next call. Messages (standard output): `nhops = <n>`, `Allocating shl <nv>`, `Done allocating
/// shl <nv>, <s> sec`, `Starting loop over <nv> vertices`, an empty line, `Finished loop over <nv>
/// vertices, <n> hops, t=<s> sec`, `vtxval <x>` (column 1031 of the output's first row). `None`
/// after `ERROR: MRISsmoothKernel(): cannot spec both mrikern and globkern`, `... Surf/Src dimension
/// mismatch.`, `... cannot run in-place`. FS-525, FS-526, FS-527, FS-529, FS-530, FS-533.
#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
pub fn mris_smooth_kernel(
    g: &mut Globals,
    io: &mut Io,
    surf: &mut Mris,
    src: &Mri,
    mask: Option<&Mri>,
    mrikern: Option<&Mri>,
    globkern: Option<&Matrix>,
    shl: &mut Option<Vec<SurfHopList>>,
    out: KernelOutput,
) -> Result<Option<Mri>, Fatal> {
    const WHO: &str = "MRISsmoothKernel";
    if mrikern.is_some() && globkern.is_some() {
        let _ = io
            .out
            .write_all(b"ERROR: MRISsmoothKernel(): cannot spec both mrikern and globkern\n");
        return Ok(None);
    }
    if surf.nvertices != nvox(src) {
        let _ = io
            .out
            .write_all(b"ERROR: MRISsmoothKernel(): Surf/Src dimension mismatch.\n");
        return Ok(None);
    }
    let mut out = match out {
        KernelOutput::New => mri_clone_by_space(g, io.out, io.err, src, MRI_FLOAT, src.nframes)?,
        KernelOutput::Src => {
            let _ = io
                .out
                .write_all(b"ERROR: MRISsmoothKernel(): cannot run in-place\n");
            return Ok(None);
        }
        KernelOutput::Given(m) => {
            let m = *m;
            g.notices.add(
                &FS_529,
                "MRISsmoothKernel: the given output is not cleared (the vertices skipped keep its values)",
            );
            m
        }
    };
    let mut nhops = 0;
    if let Some(k) = mrikern {
        nhops = k.nframes;
    }
    if let Some(k) = globkern {
        nhops = k.rows;
    }
    let _ = writeln!(io.out, "nhops = {nhops}");
    let mut kern = vec![0.0f64; usize::try_from(nhops).unwrap_or(0)];
    if let Some(k) = globkern {
        for (n, w) in kern.iter_mut().enumerate() {
            *w = f64::from(k.relt(n as i32 + 1, 1));
        }
    }
    if nhops < 1 {
        g.notices.add(
            &FS_530,
            format!("MRISsmoothKernel: {nhops} hops (FreeSurfer crashes making the hop lists)"),
        );
        if !g.notices.reproduce() {
            let _ = writeln!(
                io.out,
                "ERROR: MRISsmoothKernel(): {nhops} hops: at least 1 is needed"
            );
            return Ok(None);
        }
    }
    let lut = mris_crs_lut(g, io, surf, src).expect("the sizes were checked");
    let nv = surf.nvertices;
    let mut timer = Timer::new();
    if shl.is_none() {
        timer.reset();
        let _ = writeln!(io.out, "Allocating shl {nv}");
        let mut lists = Vec::with_capacity(usize::try_from(nv).unwrap_or(0));
        for vtx in 0..nv {
            // FS-530 with `reproduce`: no hops crash here
            let l = set_surf_hop_list(g, io, vtx, surf, nhops)?.expect("at least one hop");
            lists.push(l);
        }
        *shl = Some(lists);
        let ms = timer.milliseconds();
        let _ = writeln!(
            io.out,
            "Done allocating shl {nv}, {} sec",
            cfmt::g(ms as f64 / 1000.0, 6)
        );
    }
    let lists = shl.as_ref().expect("the hop lists");
    let _ = writeln!(io.out, "Starting loop over {nv} vertices");
    timer.reset();
    let own_kernel = mrikern.is_some();
    if own_kernel {
        g.notices.add(
            &FS_526,
            "MRISsmoothKernel: a per-vertex kernel (FreeSurfer weighs by the global kernel, all zeros then: NaN)",
        );
    }
    let fix526 = own_kernel && !g.notices.reproduce();
    let nh = usize::try_from(nhops).unwrap_or(0);
    for vtx in 0..usize::try_from(nv).unwrap_or(0) {
        if surf.vertex(vtx).ripflag != 0 {
            continue;
        }
        let v = (lut.c[vtx], lut.r[vtx], lut.s[vtx]);
        if masked(g, io, mask, v.0, v.1, v.2)? {
            continue;
        }
        let mut vkern = Vec::new();
        if let Some(k) = mrikern {
            for hop in 0..nh {
                let x = mri_get_vox_val(g, io.err, k, vtx as i32, 0, 0, hop as i32, 0)?;
                vkern.push(f64::from(x));
            }
        }
        let weights: &[f64] = if fix526 { &vkern } else { &kern };
        let list = &lists[vtx];
        if (list.nhops as usize) < nh {
            g.notices.add(
                &FS_533,
                format!("MRISsmoothKernel: hop lists of {} hops for a kernel of {nhops}: the missing hops count as empty", list.nhops),
            );
        }
        let mut ksum = 0.0f64;
        for frame in 0..src.nframes {
            if frame == 0 {
                ksum = 0.0;
            }
            let (mut kvsum, mut n) = (0.0f64, 0i32);
            for (hop, &w) in weights.iter().enumerate().take(nh) {
                let Some(nbrs) = list.vtxlist.get(hop) else {
                    continue;
                };
                for &nbr in nbrs {
                    let nbr = nbr as usize;
                    if surf.vertex(nbr).ripflag != 0 {
                        continue;
                    }
                    let nb = (lut.c[nbr], lut.r[nbr], lut.s[nbr]);
                    if masked(g, io, mask, nb.0, nb.1, nb.2)? {
                        continue;
                    }
                    let x = f64::from(fget(g, io, WHO, src, nb.0, nb.1, nb.2, frame)?);
                    kvsum = add64(mul64(x, w), kvsum);
                    if frame == 0 {
                        ksum = add64(w.abs(), ksum);
                    }
                    n += 1;
                }
            }
            if n != 0 {
                let y = div64(kvsum, ksum) as f32;
                fset(g, io, WHO, &mut out, v.0, v.1, v.2, frame, y)?;
            }
        }
    }
    let _ = io.out.write_all(b"\n");
    let ms = timer.milliseconds();
    let _ = writeln!(
        io.out,
        "Finished loop over {nv} vertices, {nhops} hops, t={} sec",
        cfmt::g(ms as f64 / 1000.0, 6)
    );
    let x = vtxval(g, io, &out)?;
    let _ = writeln!(io.out, "vtxval {}", cfmt::g(f64::from(x), 6));
    Ok(Some(out))
}

/// `MRIFseq_vox(out, 1031, 0, 0, 0)` for the closing message. FS-527: outside the volume's extent
/// FreeSurfer reads another voxel (`reproduce`; 0 by default) or memory outside the buffer (0 in
/// both modes); FS-525 for a volume of another type.
fn vtxval(g: &mut Globals, io: &mut Io, out: &Mri) -> Result<f32, Fatal> {
    let p = fplace(out, 1031, 0, 0, 0);
    let in_extent = 1031 < out.vg.width && out.vg.height > 0 && out.vg.depth > 0 && out.nframes > 0;
    match p {
        Place::In(o) => Ok(float_at(out, o)),
        Place::Other(_) if in_extent => fget(g, io, "MRISsmoothKernel", out, 1031, 0, 0, 0),
        Place::Other(o) => {
            g.notices.add(
                &FS_527,
                "MRISsmoothKernel's closing message reads column 1031 of a narrower volume: another voxel",
            );
            Ok(if g.notices.reproduce() {
                float_at(out, o)
            } else {
                0.0
            })
        }
        Place::Outside => {
            g.notices.add(
                &FS_527,
                "MRISsmoothKernel's closing message reads column 1031 of a narrower volume: memory outside it (0)",
            );
            Ok(0.0)
        }
    }
}

// ---- surface RAS to voxel -------------------------------------------------------------------

/// A voxel's coordinates (column, row, slice).
pub type Voxel = (f64, f64, f64);

/// FreeSurfer's `closeEnough(a, b)` (`mrisurf_base.h` 179-184): the larger magnitude below 1e-6,
/// or the difference below 1 % of it.
fn close_enough(a: f32, b: f32) -> bool {
    let (fa, fb) = (a.abs(), b.abs());
    // maxss: the second operand when the first is not greater (NaN included)
    let mag = if fa > fb { fa } else { fb };
    if f64::from(mag) < 1.0e-6 {
        return true;
    }
    f64::from((a - b).abs()) < mul64(f64::from(mag), 1.0e-2)
}

/// `mriSurfaceRASToVoxel(xr, yr, zr, &xv, &yv, &zv)` over [`SurfGlobals::voxel_from_sras`]: a real
/// 4 x 4 matrix gives each voxel coordinate as `m[i][1] x + m[i][2] y + m[i][3] z + m[i][4]` in
/// `double` with the point's coordinates made `float`, rounded to `float` (the new way); another
/// matrix, or `FREESURFER_mriSurfaceRASToVoxel_old` set, the matrix times the `float` vector `(x, y,
/// z, 1)` (`MatrixMultiply`), its first three rows (the old way); with
/// `FREESURFER_mriSurfaceRASToVoxel_both` set both, the old way's answer returned (the variables
/// read at the first call). `(NO_ERROR, Some(voxel))`. FS-531: with both ways FreeSurfer prints
/// `<file>:11755 not same answer` (standard error) and crashes when the two agree in any
/// coordinate; by default the message is printed when they differ in some coordinate, and the old
/// way's answer returned. FS-532: no matrix, or a product refused (`MatrixMultiply`'s messages) or
/// of fewer than three rows: FreeSurfer crashes or reads outside the product (`reproduce`: the
/// crash; with the old way alone and no matrix, `MatrixMultiply`'s refusal ends the program first;
/// rows missing read as 0); by default `(ERROR_BADPARM, None)` after
/// `mriSurfaceRASToVoxel: no surface RAS to voxel matrix` or the refusal (missing rows 0).
pub fn mri_surface_ras_to_voxel(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    xr: f64,
    yr: f64,
    zr: f64,
) -> Result<(i32, Option<Voxel>), Fatal> {
    let (try_new, try_old) = match sg.sras_ways {
        Some(w) => w,
        None => {
            let w = if io.env.get("FREESURFER_mriSurfaceRASToVoxel_both").is_some() {
                (true, true)
            } else if io.env.get("FREESURFER_mriSurfaceRASToVoxel_old").is_some() {
                (false, true)
            } else {
                (true, false)
            };
            sg.sras_ways = Some(w);
            w
        }
    };
    let Some(m) = sg.voxel_from_sras.as_ref() else {
        g.notices.add(
            &FS_532,
            "mriSurfaceRASToVoxel without a surface-RAS-to-voxel matrix (FreeSurfer reads a null pointer)",
        );
        if g.notices.reproduce() {
            if try_new {
                // the matrix's type read through the null pointer
                return Err(Fatal::Exit(139));
            }
            // the old way alone: MatrixMultiply refuses the null matrix (its messages; it ends the
            // program), else the product's first element is read through a null pointer
            let mut sr =
                matrix_alloc(g, io.out, io.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
            sr.set_relt(1, 1, xr as f32);
            sr.set_relt(2, 1, yr as f32);
            sr.set_relt(3, 1, zr as f32);
            sr.set_relt(4, 1, 1.0);
            matrix_multiply(g, io.out, io.err, None, Some(&sr), None)?;
            return Err(Fatal::Exit(139));
        }
        let _ = writeln!(
            io.err,
            "mriSurfaceRASToVoxel: no surface RAS to voxel matrix"
        );
        return Ok((ERROR_BADPARM, None));
    };
    let new_way = try_new && m.mtype == MATRIX_REAL && m.rows == 4 && m.cols == 4;
    let old_way = try_old || !new_way;
    let (mut nx, mut ny, mut nz) = (0.0f32, 0.0f32, 0.0f32);
    if new_way {
        let (fx, fy, fz) = (
            f64::from(xr as f32),
            f64::from(yr as f32),
            f64::from(zr as f32),
        );
        let row = |i: i32| -> f32 {
            let e = |j: i32| f64::from(m.relt(i, j));
            add64(
                add64(add64(mul64(e(1), fx), mul64(e(2), fy)), mul64(e(3), fz)),
                e(4),
            ) as f32
        };
        (nx, ny, nz) = (row(1), row(2), row(3));
    }
    if !old_way {
        return Ok((
            NO_ERROR,
            Some((f64::from(nx), f64::from(ny), f64::from(nz))),
        ));
    }
    let mut sr = matrix_alloc(g, io.out, io.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    sr.set_relt(1, 1, xr as f32);
    sr.set_relt(2, 1, yr as f32);
    sr.set_relt(3, 1, zr as f32);
    sr.set_relt(4, 1, 1.0);
    let vv = match matrix_multiply(g, io.out, io.err, Some(m), Some(&sr), None)? {
        Ret::New(vv) => Some(vv),
        _ => None,
    };
    let Some(vv) = vv else {
        g.notices.add(
            &FS_532,
            "mriSurfaceRASToVoxel: the matrix's product with the point was refused (FreeSurfer reads a null pointer)",
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        return Ok((ERROR_BADPARM, None));
    };
    let mut elt = |r: i32| -> f32 {
        if r <= vv.rows {
            vv.relt(r, 1)
        } else {
            g.notices.add(
                &FS_532,
                "mriSurfaceRASToVoxel: a product of fewer than three rows (FreeSurfer reads outside it; 0 here)",
            );
            0.0
        }
    };
    let (ox, oy, oz) = (elt(1), elt(2), elt(3));
    if new_way {
        let agree = [
            close_enough(ox, nx),
            close_enough(oy, ny),
            close_enough(oz, nz),
        ];
        let line = format!("{}:11755 not same answer\n", metric_properties_cpp());
        let (any, all) = (agree.iter().any(|&a| a), agree.iter().all(|&a| a));
        if any {
            g.notices.add(
                &FS_531,
                "mriSurfaceRASToVoxel: the two ways agree (FreeSurfer reports them different and crashes)",
            );
            if g.notices.reproduce() {
                let _ = io.err.write_all(line.as_bytes());
                return Err(Fatal::Exit(139));
            }
        }
        if !all {
            if !any {
                g.notices.add(
                    &FS_531,
                    "mriSurfaceRASToVoxel: the two ways differ in every coordinate (FreeSurfer says nothing)",
                );
            }
            if !g.notices.reproduce() {
                let _ = io.err.write_all(line.as_bytes());
            }
        }
    }
    Ok((
        NO_ERROR,
        Some((f64::from(ox), f64::from(oy), f64::from(oz))),
    ))
}
