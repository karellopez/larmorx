// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surf's surface hash table (`utils/mrishash.cpp`, `include/mrishash.h`,
//! `include/mrishash_internals.h`; queue item 3.4-01, the table and its face tests taken early
//! for `mrisMarkIntersections`; relay 39 of track D): the 2000 x 2000 x 2000 grid of buckets of
//! `vres` millimetres centred on the origin, each listing the faces (or vertices) that touch it,
//! in the order they were added; the faces' centroids; the face table's self-intersection tests
//! (`MHTdoesFaceIntersect`, `MHTisVectorFilled`, `MHTtestIsMRISselfIntersecting`) and its updates
//! (`MHTaddAllFaces`, `MHTremoveAllFaces`); the vertex table's construction. The closest-vertex
//! and closest-face searches come with the rest of fs.surf (3.4-01).
//!
//! FreeSurfer's table is an array of 2000 x 2000 column pointers (32 MB) whose buckets are
//! allocated on first use; here a map from the voxel to its list, which gives the same lists in
//! the same order (only lookups by voxel are made). Its locks serve OpenMP and change nothing.
//!
//! Defects (`larmorx_core::defects`):
//! - **FS-909** (undefined conversions): a resolution not above 0, a corner that is not finite or
//!   so far that its voxel number or a face's step count passes the `int` range. FreeSurfer takes
//!   x86-64's `INT_MIN`: the voxels clamp to the table's first bucket when stored and are found
//!   nowhere when looked up, a face is sampled in one step. Rule B: a clear error by default;
//!   `reproduce` as FreeSurfer.
//! - **FS-910** (wrong result): a face stored beyond the table is clamped to its edge, but a
//!   lookup beyond the table finds nothing, so faces more than 1000 voxels from the origin never
//!   meet. Rule A: lookups clamp as the stores do; `reproduce` looks nowhere.
//! - **FS-911** (wrong result): a face that touches more than 10,000 voxels is listed in its first
//!   10,000 only, with two messages for each voxel left out. Rule A: every voxel, noted.
//! - **FS-912** (the program ends): more than 10,000 faces near one face end the program
//!   (status 1). Rule A: any number.
//! - **FS-913** (a hang): a face whose sampling takes more than 2^28 steps, more than 2^22
//!   voxels (by default) or more than a million voxels past the full list (`reproduce`): a
//!   corner that is not finite next to finite ones, a face kilometres across. FreeSurfer runs for
//!   hours, two messages for each voxel past the 10,000th. Refused in both modes (a clear error).
//!
//! `MHTexpandToTouchingFaces`' capacity (10,000 faces) is never reached: a vertex lists at most
//! 255 faces (`num` is an `unsigned char`), so a face touches at most 765.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::io::Write;

use larmorx_core::defects::{FS_909, FS_910, FS_911, FS_912, FS_913};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NO_MEMORY, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::{add, div};

use crate::mris::Mris;
use crate::views::{CURRENT_VERTICES, vertex_coord2xyz_float};

/// `VOXEL_RES`: the default resolution (mm).
pub const VOXEL_RES: f32 = 1.0;
/// `TABLE_SIZE`: the buckets along each axis.
pub const TABLE_SIZE: i32 = 2000;
/// `TABLE_CENTER`: the bucket of the origin.
pub const TABLE_CENTER: i32 = TABLE_SIZE / 2;
/// `MAX_VOXELS`: a face's voxel list's capacity (FS-911).
pub const MAX_VOXELS: usize = 10000;
/// The 64-bit words of the column bits (2000 x 2000 columns).
const COLUMN_WORDS: usize = (TABLE_SIZE as usize * TABLE_SIZE as usize).div_ceil(64);
/// `MHT_MAX_FACES`: the faces near one face (FS-912).
pub const MHT_MAX_FACES: usize = 10000;
/// `MHT_MAX_TOUCHING_FACES`: the faces touching one face (never reached: at most 3 x 255).
pub const MHT_MAX_TOUCHING_FACES: usize = 10000;
/// FS-913: the steps of one face's sampling (main steps, rung steps, path voxels) past which it
/// is refused (2^28: a face kilometres across at 1 mm).
pub const SAMPLING_LIMIT: u64 = 1 << 28;
/// FS-913: the voxels of one face past which it is refused by default (2^22).
pub const VOXEL_LIMIT: usize = 1 << 22;
/// FS-913: the voxels left out of one face's full list (each with FreeSurfer's two messages) past
/// which it is refused with `reproduce`.
pub const DROPPED_LIMIT: u64 = 1_000_000;
/// `RUNGRECIPFACTOR`: slightly less than one, so rungs stop short of a voxel's edge.
const RUNGRECIPFACTOR: f64 = 0.9995;
/// `SamplesPerMHTRes`.
const SAMPLES_PER_MHT_RES: f64 = 2.0;

/// `MHTFNO_t`: what the table lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MhtFnoUsage {
    /// `MHTFNO_FACE`.
    Face = 0,
    /// `MHTFNO_VERTEX`.
    Vertex = 1,
}

/// `MHT_FACE`: a face's centroid, computed once when the table is made.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MhtFace {
    pub cx: f32,
    pub cy: f32,
    pub cz: f32,
}

/// A voxel's key in the map (each coordinate in `0..TABLE_SIZE`).
pub(crate) fn key(xv: i32, yv: i32, zv: i32) -> u64 {
    ((xv as u64) << 32) | ((yv as u64) << 16) | zv as u64
}

/// A multiply-and-shift hash of a voxel's key (deterministic; the map is only looked up).
#[derive(Default)]
pub(crate) struct VoxHasher(u64);

impl Hasher for VoxHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn write_u64(&mut self, x: u64) {
        let h = x.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        self.0 = h ^ (h >> 29);
    }
}

pub(crate) type Buckets = HashMap<u64, Vec<i32>, BuildHasherDefault<VoxHasher>>;

/// `MRIS_HASH_TABLE` of a surface (`MRIS_HASH_TABLE_IMPL` over `MRIS *`). The surface itself is
/// not kept: each call takes it (FreeSurfer's calls pass the table's surface again, and its
/// methods use the one it was made from: pass the same surface).
#[derive(Clone, Debug)]
pub struct Mht {
    fno_usage: MhtFnoUsage,
    vres: f32,
    which: i32,
    pub(crate) buckets: Buckets,
    /// The smallest and largest voxel numbers of the buckets made, along each axis (the closest
    /// searches stop where no bucket can be left: [`crate::mhtfind`]).
    pub(crate) bbox: Option<([i32; 3], [i32; 3])>,
    /// One bit per column `(xv, yv)` that holds a bucket (FreeSurfer's array of column pointers,
    /// as bits): an empty column is known without a map lookup.
    cols: Vec<u64>,
    /// `nfaces`: the surface's faces when the table was made.
    pub nfaces: i32,
    /// `f`: each face's centroid.
    pub f: Vec<MhtFace>,
}

/// `VOXEL_LISTgw`: a face's voxels, each once, in the order found.
struct VoxelList {
    voxels: Vec<[i32; 3]>,
    /// A copy of `voxels` for long lists (the membership test only).
    seen: Option<HashSet<[i32; 3]>>,
    /// The sampling's steps so far (FS-913).
    work: u64,
    /// The voxels left out of the full list (FS-911, FS-913).
    dropped: u64,
}

impl VoxelList {
    fn new() -> VoxelList {
        VoxelList {
            voxels: Vec::new(),
            seen: None,
            work: 0,
            dropped: 0,
        }
    }

    fn contains(&self, v: [i32; 3]) -> bool {
        match &self.seen {
            Some(s) => s.contains(&v),
            None => self.voxels.contains(&v),
        }
    }

    /// `mhtVoxelList_Add`: the voxel appended unless listed. FS-911: a full list (10,000) prints
    /// `mhtVoxelList_Add(x, y, z): complete list too big!` on standard error and as an error
    /// (`ERROR_NOMEMORY`) and drops the voxel with `reproduce`; by default the list grows.
    /// FS-913: past [`DROPPED_LIMIT`] dropped voxels or [`VOXEL_LIMIT`] voxels, refused.
    fn add(&mut self, g: &mut Globals, err: &mut dyn Write, v: [i32; 3]) -> Result<(), Fatal> {
        if self.contains(v) {
            return Ok(());
        }
        if self.voxels.len() >= MAX_VOXELS {
            g.notices.add(
                &FS_911,
                format!(
                    "the surface hash table: a face touches more than {MAX_VOXELS} voxels (FreeSurfer lists only the first {MAX_VOXELS})"
                ),
            );
            if g.notices.reproduce() {
                self.dropped += 1;
                if self.dropped > DROPPED_LIMIT {
                    refuse(
                        g,
                        err,
                        &format!("more than {DROPPED_LIMIT} voxels beyond its list's {MAX_VOXELS}"),
                    )?;
                }
                let msg = format!(
                    "mhtVoxelList_Add({}, {}, {}): complete list too big!",
                    v[0], v[1], v[2]
                );
                let _ = writeln!(err, "{msg}");
                log::error_printf(g, err, ERROR_NO_MEMORY, &msg);
                return Ok(());
            }
            if self.voxels.len() >= VOXEL_LIMIT {
                refuse(g, err, &format!("more than {VOXEL_LIMIT} voxels"))?;
            }
        }
        self.voxels.push(v);
        if let Some(s) = &mut self.seen {
            s.insert(v);
        } else if self.voxels.len() > 256 {
            self.seen = Some(self.voxels.iter().copied().collect());
        }
        Ok(())
    }

    /// `mhtVoxelList_AddPath(oldvc, newvc)`: every voxel of the box from `oldvc` to `newvc`
    /// (x outermost, each axis stepping towards `newvc`), `oldvc` itself left out.
    fn add_path(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        old: [i32; 3],
        new: [i32; 3],
    ) -> Result<(), Fatal> {
        let span = |k: usize| u64::from(old[k].abs_diff(new[k])) + 1;
        let boxed = u128::from(span(0)) * u128::from(span(1)) * u128::from(span(2));
        self.spend(g, err, u64::try_from(boxed).unwrap_or(u64::MAX))?;
        let inc = |a: i32, b: i32| if b >= a { 1 } else { -1 };
        let (ix, iy, iz) = (
            inc(old[0], new[0]),
            inc(old[1], new[1]),
            inc(old[2], new[2]),
        );
        let within = |v: i32, end: i32, i: i32| if i == 1 { v <= end } else { v >= end };
        let mut count = -1i64;
        let mut xv = old[0];
        while within(xv, new[0], ix) {
            let mut yv = old[1];
            while within(yv, new[1], iy) {
                let mut zv = old[2];
                while within(zv, new[2], iz) {
                    count += 1;
                    if count >= 1 {
                        self.add(g, err, [xv, yv, zv])?;
                    }
                    zv = zv.wrapping_add(iz);
                }
                yv = yv.wrapping_add(iy);
            }
            xv = xv.wrapping_add(ix);
        }
        Ok(())
    }

    /// FS-913: `n` more steps of the sampling; past [`SAMPLING_LIMIT`] the face is refused.
    fn spend(&mut self, g: &mut Globals, err: &mut dyn Write, n: u64) -> Result<(), Fatal> {
        self.work = self.work.saturating_add(n);
        if self.work > SAMPLING_LIMIT {
            refuse(
                g,
                err,
                &format!("more than {SAMPLING_LIMIT} sampling steps"),
            )?;
        }
        Ok(())
    }
}

/// FS-913: a face whose sampling FreeSurfer would spend hours on (or fill the memory with): a
/// clear error (`ERROR_BADPARM`) in both modes.
fn refuse(g: &mut Globals, err: &mut dyn Write, why: &str) -> Result<(), Fatal> {
    g.notices.add(
        &FS_913,
        format!("the surface hash table: a face whose sampling takes {why} (FreeSurfer runs for hours): refused"),
    );
    log::error_exit(
        g,
        err,
        ERROR_BADPARM,
        &format!(
            "surface hash table: a face too large for its voxels ({why}; a resolution above 0 and finite coordinates are needed)\n"
        ),
    )
}

/// C's `(int) x` of a `double` on x86-64 (`cvttsd2si`): toward zero, `INT_MIN` for NaN and values
/// outside the range; `None` for those.
pub(crate) fn cvtt(x: f64) -> Option<i32> {
    if x.is_nan() || !(-2_147_483_649.0 < x && x < 2_147_483_648.0) {
        None
    } else {
        Some(x as i32)
    }
}

/// FS-909: an undefined conversion of the table's arithmetic: by default a clear error (rule B,
/// `ERROR_BADPARM`); `reproduce` x86-64's `INT_MIN`.
pub(crate) fn undefined(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    x: f64,
) -> Result<i32, Fatal> {
    let detail =
        format!("the surface hash table: {what} {x} is not an int (FreeSurfer takes -2147483648)");
    g.notices.add(&FS_909, detail.clone());
    if !g.notices.reproduce() {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "surface hash table: {what} {x} is not finite or too large for its voxels (a resolution above 0 and finite coordinates are needed)\n"
            ),
        )?;
    }
    Ok(i32::MIN)
}

/// `W2VOX(ares, x)`: `(int)(x / ares + TABLE_CENTER)` in `double`.
fn w2vox(g: &mut Globals, err: &mut dyn Write, ares: f32, x: f64) -> Result<i32, Fatal> {
    let v = x / f64::from(ares) + f64::from(TABLE_CENTER);
    match cvtt(v) {
        Some(i) => Ok(i),
        None => undefined(g, err, "a voxel number", v),
    }
}

/// `(int) ceil(x)` of a step count.
fn steps(g: &mut Globals, err: &mut dyn Write, x: f64) -> Result<i32, Fatal> {
    match cvtt(x.ceil()) {
        Some(i) => Ok(i),
        None => undefined(g, err, "a step count", x.ceil()),
    }
}

pub(crate) type Pt = [f64; 3];

fn world2voxel(g: &mut Globals, err: &mut dyn Write, res: f32, p: Pt) -> Result<[i32; 3], Fatal> {
    Ok([
        w2vox(g, err, res, p[0])?,
        w2vox(g, err, res, p[1])?,
        w2vox(g, err, res, p[2])?,
    ])
}

fn lensq(a: Pt) -> f64 {
    a[0] * a[0] + a[1] * a[1] + a[2] * a[2]
}

/// `mhtVoxelList_SampleTriangle(mhtres, a, b, c, voxlist)`: the voxels a triangle touches. Main
/// steps along `b -> a` and `c -> a` (`ceil(2 L / mhtres)` of the longer side's length `L`, 1 at
/// least; `1 / mhtres` taken as 1 for a resolution of 0), each crossing of a voxel boundary
/// adding its path ([`VoxelList::add_path`]); at each main step, unless both ends are in one
/// voxel, a rung from the `b -> a` point to the `c -> a` point in `ceil(2 l / mhtres)` steps
/// (1 at least) of `0.9995 / steps` of it, each crossing adding its path. The first main step adds
/// the corners `b` and `c`.
fn sample_triangle(
    g: &mut Globals,
    err: &mut dyn Write,
    mhtres: f32,
    a: Pt,
    b: Pt,
    c: Pt,
    list: &mut VoxelList,
) -> Result<(), Fatal> {
    let recip = if mhtres != 0.0 {
        1.0 / f64::from(mhtres)
    } else {
        1.0
    };
    if g.notices.reproduce() {
        if mhtres.is_nan() || mhtres <= 0.0 {
            g.notices.add(
                &FS_909,
                format!("the surface hash table: a resolution of {mhtres} (not above 0)"),
            );
        }
    } else if mhtres.is_nan() || mhtres <= 0.0 {
        undefined(g, err, "the resolution", f64::from(mhtres))?;
    }
    let dif_b2a = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dif_c2a = [a[0] - c[0], a[1] - c[1], a[2] - c[2]];
    let (lb, lc) = (lensq(dif_b2a), lensq(dif_c2a));
    // MAX(a, b) of macros.h: (a >= b ? a : b)
    let temp = if lb >= lc { lb } else { lc };
    let main_dbl = temp.sqrt() * SAMPLES_PER_MHT_RES * recip;
    let mut mainsteps = steps(g, err, main_dbl)?;
    if 0 >= mainsteps {
        mainsteps = 1;
    }
    list.spend(g, err, mainsteps as u64 + 1)?;
    let frac = 1.0 / f64::from(mainsteps);
    let delta_b2a = dif_b2a.map(|d| d * frac);
    let delta_c2a = dif_c2a.map(|d| d * frac);
    let mut posn_b2a = b;
    let mut posn_c2a = c;
    let (mut old_b2a, mut old_c2a) = ([0i32; 3], [0i32; 3]);
    for mainstep in 0..=mainsteps {
        if mainstep >= 1 {
            for k in 0..3 {
                posn_b2a[k] += delta_b2a[k];
                posn_c2a[k] += delta_c2a[k];
            }
        }
        let vox_b2a = world2voxel(g, err, mhtres, posn_b2a)?;
        let vox_c2a = world2voxel(g, err, mhtres, posn_c2a)?;
        let mut changed = false;
        if mainstep == 0 {
            list.add(g, err, vox_b2a)?;
            list.add(g, err, vox_c2a)?;
            changed = true;
        } else {
            if old_b2a != vox_b2a {
                list.add_path(g, err, old_b2a, vox_b2a)?;
                changed = true;
            }
            if old_c2a != vox_c2a {
                list.add_path(g, err, old_c2a, vox_c2a)?;
                changed = true;
            }
        }
        if changed {
            old_b2a = vox_b2a;
            old_c2a = vox_c2a;
        }
        if vox_c2a == vox_b2a {
            continue;
        }
        let dif_b2c = [
            posn_c2a[0] - posn_b2a[0],
            posn_c2a[1] - posn_b2a[1],
            posn_c2a[2] - posn_b2a[2],
        ];
        let rung_dbl = lensq(dif_b2c).sqrt() * SAMPLES_PER_MHT_RES * recip;
        let mut rungsteps = steps(g, err, rung_dbl)?;
        if rungsteps < 1 {
            rungsteps = 1;
        }
        list.spend(g, err, rungsteps as u64)?;
        let rfrac = RUNGRECIPFACTOR / f64::from(rungsteps);
        let delta_b2c = dif_b2c.map(|d| d * rfrac);
        let mut posn_b2c = posn_b2a;
        let mut old_b2c = vox_b2a;
        for _ in 1..=rungsteps {
            for k in 0..3 {
                posn_b2c[k] += delta_b2c[k];
            }
            let vox_b2c = world2voxel(g, err, mhtres, posn_b2c)?;
            if old_b2c != vox_b2c {
                list.add_path(g, err, old_b2c, vox_b2c)?;
                old_b2c = vox_b2c;
            }
        }
    }
    Ok(())
}

/// A vertex's coordinates `which` (`MRISvertexCoord2XYZ_float`: an unsupported `which` is its
/// fatal error), widened.
pub(crate) fn corner(
    g: &mut Globals,
    err: &mut dyn Write,
    mris: &Mris,
    vno: i32,
    which: i32,
) -> Result<Pt, Fatal> {
    let (x, y, z) = vertex_coord2xyz_float(g, err, mris.vertex(vno as usize), which)?;
    Ok([f64::from(x), f64::from(y), f64::from(z)])
}

/// Coerced into `0..TABLE_SIZE`, as the stores coerce.
pub(crate) fn coerce(v: i32) -> i32 {
    v.clamp(0, TABLE_SIZE - 1)
}

impl Mht {
    fn new(
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        fno_usage: MhtFnoUsage,
        vres: f32,
        which: i32,
    ) -> Result<Mht, Fatal> {
        let nfaces = mris.nfaces;
        let mut f = vec![MhtFace::default(); usize::try_from(nfaces).unwrap_or(0)];
        for (fno, c) in f.iter_mut().enumerate() {
            let (mut xt, mut yt, mut zt) = (0f32, 0f32, 0f32);
            for n in 0..3 {
                let vno = mris.face(fno).v[n];
                let (x, y, z) = vertex_coord2xyz_float(g, err, mris.vertex(vno as usize), which)?;
                xt = add(xt, x);
                yt = add(yt, y);
                zt = add(zt, z);
            }
            *c = MhtFace {
                cx: div(xt, 3.0),
                cy: div(yt, 3.0),
                cz: div(zt, 3.0),
            };
        }
        Ok(Mht {
            fno_usage,
            vres,
            which,
            buckets: Buckets::default(),
            bbox: None,
            cols: vec![0; COLUMN_WORDS],
            nfaces,
            f,
        })
    }

    /// `MHTcreateFaceTable_Resolution(mris, which, res)`: the centroids of every face, then each
    /// unripped face in turn added to every voxel its triangle touches ([`Mht::add_face`]).
    pub fn create_face_table_resolution(
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        which: i32,
        res: f32,
    ) -> Result<Mht, Fatal> {
        let mut mht = Mht::new(g, err, mris, MhtFnoUsage::Face, res, which)?;
        for fno in 0..mris.nfaces {
            if mris.face(fno as usize).ripflag != 0 {
                continue;
            }
            mht.face_to_mht(g, err, mris, fno, true)?;
        }
        Ok(mht)
    }

    /// `MHTcreateFaceTable(mris)`: the current positions at 1 mm.
    pub fn create_face_table(
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
    ) -> Result<Mht, Fatal> {
        Mht::create_face_table_resolution(g, err, mris, CURRENT_VERTICES, VOXEL_RES)
    }

    /// `MHTcreateVertexTable_Resolution(mris, which, res)`: the faces' centroids, then each
    /// unripped vertex added to the voxel of its `which` position (`(int)(x / res + 1000)` in
    /// `float`, coerced into the table).
    pub fn create_vertex_table_resolution(
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        which: i32,
        res: f32,
    ) -> Result<Mht, Fatal> {
        let mut mht = Mht::new(g, err, mris, MhtFnoUsage::Vertex, res, which)?;
        for vno in 0..mris.nvertices {
            if mris.vertex(vno as usize).ripflag != 0 {
                continue;
            }
            let (x, y, z) = vertex_coord2xyz_float(g, err, mris.vertex(vno as usize), which)?;
            let xv = mht.world_to_voxel_f32(g, err, x)?;
            let yv = mht.world_to_voxel_f32(g, err, y)?;
            let zv = mht.world_to_voxel_f32(g, err, z)?;
            mht.add_at_vox(xv, yv, zv, vno);
        }
        Ok(mht)
    }

    /// `MHTcreateVertexTable(mris, which)`: at 1 mm.
    pub fn create_vertex_table(
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        which: i32,
    ) -> Result<Mht, Fatal> {
        Mht::create_vertex_table_resolution(g, err, mris, which, VOXEL_RES)
    }

    /// `which()`.
    pub fn which(&self) -> i32 {
        self.which
    }

    /// `vres()`.
    pub fn vres(&self) -> f32 {
        self.vres
    }

    /// `fno_usage()`.
    pub fn fno_usage(&self) -> MhtFnoUsage {
        self.fno_usage
    }

    /// The bucket of a voxel (`MHTacqBucketAtVoxIx`): its list, or `None` when the voxel is
    /// outside the table or has no bucket.
    pub fn bucket(&self, xv: i32, yv: i32, zv: i32) -> Option<&[i32]> {
        if !(0..TABLE_SIZE).contains(&xv)
            || !(0..TABLE_SIZE).contains(&yv)
            || !(0..TABLE_SIZE).contains(&zv)
            || !self.has_column(xv, yv)
        {
            return None;
        }
        self.buckets.get(&key(xv, yv, zv)).map(Vec::as_slice)
    }

    /// `MHTacqBucket(mht, x, y, z)`: the bucket of the voxel that holds the point (each `float`
    /// coordinate's `WORLD_TO_VOXEL`; FS-909 for one that is not an `int`): its key (the bucket's
    /// identity, for a caller that compares buckets) and its list, or `None` when the voxel has no
    /// bucket. FS-910: a voxel beyond the table is looked up at the table's edge by default (as a
    /// face there is stored), nowhere with `reproduce` (FreeSurfer's).
    pub fn acq_bucket(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        x: f32,
        y: f32,
        z: f32,
    ) -> Result<Option<(u64, &[i32])>, Fatal> {
        let xv = self.world_to_voxel_f32(g, err, x)?;
        let yv = self.world_to_voxel_f32(g, err, y)?;
        let zv = self.world_to_voxel_f32(g, err, z)?;
        let inside = (0..TABLE_SIZE).contains(&xv)
            && (0..TABLE_SIZE).contains(&yv)
            && (0..TABLE_SIZE).contains(&zv);
        let (xv, yv, zv) = if inside {
            (xv, yv, zv)
        } else {
            g.notices.add(
                &FS_910,
                format!(
                    "the surface hash table: a point at voxel ({xv}, {yv}, {zv}) beyond the table (FreeSurfer looks for nothing there; looked up at the table's edge)"
                ),
            );
            if g.notices.reproduce() {
                return Ok(None);
            }
            (coerce(xv), coerce(yv), coerce(zv))
        };
        if !self.has_column(xv, yv) {
            return Ok(None);
        }
        let k = key(xv, yv, zv);
        Ok(self.buckets.get(&k).map(|b| (k, b.as_slice())))
    }

    /// Whether column `(xv, yv)` (each in `0..TABLE_SIZE`) holds a bucket.
    pub(crate) fn has_column(&self, xv: i32, yv: i32) -> bool {
        let col = (xv * TABLE_SIZE + yv) as usize;
        self.cols[col / 64] & (1 << (col % 64)) != 0
    }

    /// The number of buckets made so far.
    pub fn bucket_count(&self) -> usize {
        self.buckets.len()
    }

    /// `WORLD_TO_VOXEL(float x)`: `(int)(x / vres + 1000)` in `float`.
    fn world_to_voxel_f32(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        x: f32,
    ) -> Result<i32, Fatal> {
        let v = add(div(x, self.vres), TABLE_CENTER as f32);
        match cvtt(f64::from(v)) {
            Some(i) => Ok(i),
            None => undefined(g, err, "a voxel number", f64::from(v)),
        }
    }

    /// `mhtAddFaceOrVertexAtVoxIx(xv, yv, zv, forvnum)`: the voxel coerced into the table, its
    /// bucket made when there is none, `forvnum` appended unless listed.
    fn add_at_vox(&mut self, xv: i32, yv: i32, zv: i32, forvnum: i32) {
        let c = [coerce(xv), coerce(yv), coerce(zv)];
        self.bbox = Some(match self.bbox {
            None => (c, c),
            Some((lo, hi)) => (
                [lo[0].min(c[0]), lo[1].min(c[1]), lo[2].min(c[2])],
                [hi[0].max(c[0]), hi[1].max(c[1]), hi[2].max(c[2])],
            ),
        });
        let col = (c[0] * TABLE_SIZE + c[1]) as usize;
        self.cols[col / 64] |= 1 << (col % 64);
        let b = self.buckets.entry(key(c[0], c[1], c[2])).or_default();
        if !b.contains(&forvnum) {
            b.push(forvnum);
        }
    }

    /// `mhtRemoveFaceOrVertexAtVoxIx(xv, yv, zv, forvnum)`: the voxel coerced, `forvnum` taken out
    /// of its bucket (the rest kept in order).
    fn remove_at_vox(&mut self, xv: i32, yv: i32, zv: i32, forvnum: i32) {
        if let Some(b) = self
            .buckets
            .get_mut(&key(coerce(xv), coerce(yv), coerce(zv)))
            && let Some(i) = b.iter().position(|&x| x == forvnum)
        {
            b.remove(i);
        }
    }

    /// `checkConstructedWithFaces`: a vertex table is `checkConstructedWithFaces:
    /// MRIS_HASH_TABLE_NoSurface not initialized for faces` (fatal, `ERROR_BADPARM`).
    pub(crate) fn check_faces(&self, g: &mut Globals, err: &mut dyn Write) -> Result<(), Fatal> {
        if self.fno_usage != MhtFnoUsage::Face {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "checkConstructedWithFaces: MRIS_HASH_TABLE_NoSurface not initialized for faces\n",
            )?;
        }
        Ok(())
    }

    /// The triangle of face `fno` at `which`.
    fn triangle(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        fno: i32,
    ) -> Result<[Pt; 3], Fatal> {
        let v = mris.face(fno as usize).v;
        Ok([
            corner(g, err, mris, v[0], self.which)?,
            corner(g, err, mris, v[1], self.which)?,
            corner(g, err, mris, v[2], self.which)?,
        ])
    }

    /// `mhtFaceToMHT(face, on)`: an unripped face added to (or removed from) every voxel its
    /// triangle touches, in the voxel list's order.
    fn face_to_mht(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        fno: i32,
        on: bool,
    ) -> Result<(), Fatal> {
        if mris.face(fno as usize).ripflag != 0 {
            return Ok(());
        }
        let [p0, p1, p2] = self.triangle(g, err, mris, fno)?;
        let mut list = VoxelList::new();
        sample_triangle(g, err, self.vres, p0, p1, p2, &mut list)?;
        for &[i, j, k] in &list.voxels {
            if on {
                self.add_at_vox(i, j, k, fno);
            } else {
                self.remove_at_vox(i, j, k, fno);
            }
        }
        Ok(())
    }

    /// `MHTaddAllFaces(mht, mris, vno)`: (a face table only) the vertex's faces added again.
    pub fn add_all_faces(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        vno: i32,
    ) -> Result<i32, Fatal> {
        self.check_faces(g, err)?;
        let t = mris.topology(vno as usize);
        let faces: Vec<i32> = t.f.as_deref().unwrap_or(&[])[..usize::from(t.num)].to_vec();
        for fno in faces {
            self.face_to_mht(g, err, mris, fno, true)?;
        }
        Ok(NO_ERROR)
    }

    /// `MHTremoveAllFaces(mht, mris, vno)`: (a face table only) the vertex's faces taken out of
    /// the voxels their triangles now touch.
    pub fn remove_all_faces(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        vno: i32,
    ) -> Result<i32, Fatal> {
        self.check_faces(g, err)?;
        let t = mris.topology(vno as usize);
        let faces: Vec<i32> = t.f.as_deref().unwrap_or(&[])[..usize::from(t.num)].to_vec();
        for fno in faces {
            self.face_to_mht(g, err, mris, fno, false)?;
        }
        Ok(NO_ERROR)
    }

    /// `MHTexpandToTouchingFaces(fno, ...)`: `fno`, then each face of its corners' faces (in the
    /// corners' and their faces' order) not yet listed (at most 1 + 3 x 255: its capacity of
    /// 10,000 is never reached).
    fn touching_faces(mris: &Mris, fno: i32) -> Vec<i32> {
        let mut list = vec![fno];
        for &vno in &mris.face(fno as usize).v {
            let t = mris.topology(vno as usize);
            for &fno2 in &t.f.as_deref().unwrap_or(&[])[..usize::from(t.num)] {
                if fno2 == fno {
                    continue;
                }
                if !list.contains(&fno2) {
                    list.push(fno2);
                }
            }
        }
        list
    }

    /// `MHTdoesTriangleIntersect` and `mhtDoesTriangleVoxelListIntersect`: the faces listed in
    /// the voxels the triangle touches (each once, in the voxels' and buckets' order; those in
    /// `ignore` left out) tested in turn against it ([`crate::tritri::tri_tri_intersect`]): 1 at
    /// the first that intersects, else 0. FS-910: a voxel beyond the table is looked up at its
    /// edge by default, nowhere with `reproduce`. FS-912: more than 10,000 faces print
    /// `mhtDoesTriangleVoxelListIntersect: MHT_MAX_FACES exceeded!` as an error
    /// (`ERROR_NO_MEMORY`) and end the program (status 1) with `reproduce`.
    fn does_triangle_intersect(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        tri: [Pt; 3],
        ignore: &[i32],
    ) -> Result<i32, Fatal> {
        let mut list = VoxelList::new();
        sample_triangle(g, err, self.vres, tri[0], tri[1], tri[2], &mut list)?;
        let ignore_set: Option<HashSet<i32>> =
            (ignore.len() > 64).then(|| ignore.iter().copied().collect());
        let mut facelist: Vec<i32> = Vec::new();
        let mut seen: Option<HashSet<i32>> = None;
        for &[xv, yv, zv] in &list.voxels {
            let inside = (0..TABLE_SIZE).contains(&xv)
                && (0..TABLE_SIZE).contains(&yv)
                && (0..TABLE_SIZE).contains(&zv);
            let bins = if inside {
                self.buckets.get(&key(xv, yv, zv))
            } else {
                g.notices.add(
                    &FS_910,
                    format!(
                        "the surface hash table: a face reaches voxel ({xv}, {yv}, {zv}) beyond the table (FreeSurfer looks for nothing there; looked up at the table's edge)"
                    ),
                );
                if g.notices.reproduce() {
                    None
                } else {
                    self.buckets.get(&key(coerce(xv), coerce(yv), coerce(zv)))
                }
            };
            let Some(bins) = bins else {
                continue;
            };
            for &fno in bins {
                let ignored = match &ignore_set {
                    Some(s) => s.contains(&fno),
                    None => ignore.contains(&fno),
                };
                if ignored {
                    continue;
                }
                let listed = match &seen {
                    Some(s) => s.contains(&fno),
                    None => facelist.contains(&fno),
                };
                if listed {
                    continue;
                }
                if facelist.len() >= MHT_MAX_FACES {
                    g.notices.add(
                        &FS_912,
                        format!(
                            "the surface hash table: more than {MHT_MAX_FACES} faces near one face (FreeSurfer ends the program)"
                        ),
                    );
                    if g.notices.reproduce() {
                        log::error_printf(
                            g,
                            err,
                            ERROR_NO_MEMORY,
                            "mhtDoesTriangleVoxelListIntersect: MHT_MAX_FACES exceeded!",
                        );
                        return Err(Fatal::Exit(1));
                    }
                }
                facelist.push(fno);
                if let Some(s) = &mut seen {
                    s.insert(fno);
                } else if facelist.len() > 256 {
                    seen = Some(facelist.iter().copied().collect());
                }
            }
        }
        for &fno in &facelist {
            let [u0, u1, u2] = self.triangle(g, err, mris, fno)?;
            if crate::tritri::tri_tri_intersect(tri[0], tri[1], tri[2], u0, u1, u2) != 0 {
                return Ok(1);
            }
        }
        Ok(0)
    }

    /// `MHTdoesFaceIntersect(mht, mris, fno)`: (a face table only) 0 for a ripped face; else
    /// whether its triangle intersects a face of the table that touches none of its corners.
    pub fn does_face_intersect(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        fno: i32,
    ) -> Result<i32, Fatal> {
        self.check_faces(g, err)?;
        self.check_faces(g, err)?;
        if mris.face(fno as usize).ripflag != 0 {
            return Ok(0);
        }
        let tri = self.triangle(g, err, mris, fno)?;
        let touching = Mht::touching_faces(mris, fno);
        self.does_triangle_intersect(g, err, mris, tri, &touching)
    }

    /// `MHTisVectorFilled(mht, vno, dx, dy, dz)`: (a face table of the current positions only:
    /// else `isVectorFilled: mht not loaded using CURRENT_VERTICES`, fatal) whether moving the
    /// vertex by `(dx, dy, dz)` (in `float`) makes one of its faces, in turn, intersect a face of
    /// the table that touches none of that face's corners. 1 or 0.
    #[allow(clippy::too_many_arguments)]
    pub fn is_vector_filled(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        vno: i32,
        dx: f32,
        dy: f32,
        dz: f32,
    ) -> Result<i32, Fatal> {
        self.check_faces(g, err)?;
        if self.which != CURRENT_VERTICES {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "isVectorFilled: mht not loaded using CURRENT_VERTICES\n",
            )?;
        }
        let v = mris.vertex(vno as usize);
        let moved = [
            f64::from(add(v.x, dx)),
            f64::from(add(v.y, dy)),
            f64::from(add(v.z, dz)),
        ];
        let t = mris.topology(vno as usize);
        let faces: Vec<i32> = t.f.as_deref().unwrap_or(&[])[..usize::from(t.num)].to_vec();
        for fno in faces {
            let fv = mris.face(fno as usize).v;
            let mut tri = [[0f64; 3]; 3];
            for (c, &cv) in fv.iter().enumerate() {
                tri[c] = if cv == vno {
                    moved
                } else {
                    corner(g, err, mris, cv, self.which)?
                };
            }
            let touching = Mht::touching_faces(mris, fno);
            let r = self.does_triangle_intersect(g, err, mris, tri, &touching)?;
            if r != 0 {
                return Ok(r);
            }
        }
        Ok(0)
    }
}

/// `MHTtestIsMRISselfIntersecting(mris, res)`: a face table of the current positions at `res`;
/// 1 at the first face that intersects another, else 0.
pub fn test_is_mris_self_intersecting(
    g: &mut Globals,
    err: &mut dyn Write,
    mris: &Mris,
    res: f32,
) -> Result<i32, Fatal> {
    let mht = Mht::create_face_table_resolution(g, err, mris, CURRENT_VERTICES, res)?;
    for fno in 0..mris.nfaces {
        if mht.does_face_intersect(g, err, mris, fno)? != 0 {
            return Ok(1);
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert_eq!(cvtt(-0.5), Some(0));
        assert_eq!(cvtt(2_147_483_647.9), Some(i32::MAX));
        assert_eq!(cvtt(-2_147_483_648.9), Some(i32::MIN));
        assert_eq!(cvtt(2_147_483_648.0), None);
        assert_eq!(cvtt(f64::NAN), None);
        assert_eq!(coerce(-3), 0);
        assert_eq!(coerce(5000), TABLE_SIZE - 1);
    }
}
