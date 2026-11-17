// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surf's closest searches in the surface hash table (`utils/mrishash.cpp`'s
//! `findClosestVertexGeneric`, `findClosestFaceNoGeneric`, their bucket scans, the brute-force
//! searches and the public `MHTfindClosest*` and `MHTBruteForceClosestFace`; queue item 3.4-01,
//! taken early; relay 43 of track D).
//!
//! A search looks at the probe's voxel and its seven nearest neighbours, then (unless the caller
//! limits it to those) the rest of the 3 x 3 x 3 block, then shells of growing radius, stopping as
//! soon as no closer candidate can be left; a candidate is a vertex (its position `which`) or a
//! face (its centroid, computed when the table was made), its squared distance in `double` from
//! the `float` position; the strict `<` keeps the first found of equal distances. The squared
//! distance starts at 10^6, so nothing beyond 1,000 mm is ever returned.
//!
//! The shells end at the table's farthest bucket ([`Mht`]'s box of buckets): a shell beyond it
//! holds no bucket, so the answer is the same as FreeSurfer's, which looks at every voxel up to the
//! caller's distance (1,000 mm at 1 mm: 8 x 10^9 voxels for a probe near no face).
//!
//! Defects (`larmorx_core::defects`):
//! - **FS-909** (undefined conversions): a probe whose voxel number is not an `int` (not finite,
//!   or beyond 2^31 voxels). Rule B: a clear error by default; `reproduce` as FreeSurfer
//!   (`INT_MIN`: nothing is found).
//! - **FS-997** (kept, rule C): `MHTfindClosestVertexNoXYZ` (and `MHTfindClosestVertexNo2`,
//!   `MHTfindClosestVertex2`) test the search's status, which is always 0, for its fallback to a
//!   full search: a probe with no vertex within one voxel gets -1 and 1000 mm. The programs rely
//!   on it (`mri_surf2volseg` takes -1 as "far from the surface"); both modes.
//! - **FS-998** (memory contents): a vertex search of a face table (`MHTfindClosestVertexGeneric`,
//!   `MHTfindClosestVertexNo2`, `MHTfindClosestVertex2`, `MHTfindClosestVertexSet2`,
//!   `MHTfindClosestVertexInTable`) or a face search of a vertex table (`MHTfindClosestFaceGeneric`)
//!   reads the face numbers as vertex numbers (or the other way round): those beyond the
//!   surface's vertices (faces) read memory outside them (**FS-576**, memory contents). Rule B:
//!   FreeSurfer's own check's error by default; `reproduce` takes the numbers within the surface
//!   as FreeSurfer does and skips the others (FS-576, noted).
//! - **FS-999** (wrong result): a vertex or face beyond the table is listed at its edge
//!   (`mhtAddFaceOrVertexAtVoxIx` coerces), but a search beyond the table looks nowhere: a probe at
//!   a vertex beyond the table does not find it. Rule A: such voxels are looked up at the table's
//!   edge, as the stores do (each edge bucket once per search); `reproduce` looks nowhere.
//!
//! `MHTfindReportCounts` (the last search's bucket counts) has no caller in FreeSurfer 8.2.0 and
//! is not ported.

use std::collections::HashSet;
use std::io::Write;

use larmorx_core::defects::{FS_576, FS_909, FS_998, FS_999};
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt, sqrt64, sub, sub64};

use crate::mht::{Mht, MhtFnoUsage, Pt, TABLE_CENTER, TABLE_SIZE, coerce, corner, cvtt, key};
use crate::mris::Mris;
use crate::props2::face_barycentric_coords_raw;
use crate::tritri::narrow;
use crate::views::vertex_coord2xyz_float;

/// `MinDistSq`'s start: nothing beyond 1,000 mm is returned.
const MIN_DIST_SQ: f64 = 1e6;
/// The vertex search's limit on its shells (`max_mhts_MAX`); the face search has none.
const MAX_MHTS_VERTEX: i32 = 5;

/// What a search looks for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Vertex,
    Face { project_into_face: i32 },
}

/// One search's state: the best so far and the edge buckets looked at for voxels beyond the
/// table (FS-999).
struct Search<'a> {
    mht: &'a Mht,
    mris: &'a Mris,
    kind: Kind,
    probe: Pt,
    best: i32,
    best_sq: f64,
    edges: HashSet<u64>,
}

impl<'a> Search<'a> {
    /// The bucket of voxel `(xv, yv, zv)` (in `i64`: FreeSurfer's `int` sums wrap far outside the
    /// table, where nothing is found either way): beyond the table nothing with `reproduce`, the
    /// edge's bucket once per search by default (FS-999).
    fn bucket(&mut self, g: &mut Globals, v: [i64; 3]) -> Option<&'a [i32]> {
        let inside = v.iter().all(|&c| (0..i64::from(TABLE_SIZE)).contains(&c));
        if inside {
            return self
                .mht
                .buckets
                .get(&key(v[0] as i32, v[1] as i32, v[2] as i32))
                .map(Vec::as_slice);
        }
        let c = v.map(|c| coerce(c.clamp(-1, i64::from(TABLE_SIZE)) as i32));
        let k = key(c[0], c[1], c[2]);
        let edge = self.mht.buckets.get(&k).filter(|b| !b.is_empty());
        edge?;
        g.notices.add(
            &FS_999,
            format!(
                "the surface hash table: a search reaches voxel ({}, {}, {}) beyond the table, where FreeSurfer looks for nothing (looked up at the table's edge)",
                v[0], v[1], v[2]
            ),
        );
        if g.notices.reproduce() || !self.edges.insert(k) {
            return None;
        }
        edge.map(Vec::as_slice)
    }

    /// FS-576 (FS-998 with `reproduce`): a number of the table of the other kind beyond the
    /// surface's vertices (faces) is skipped (FreeSurfer reads memory outside them).
    fn beyond(&self, g: &mut Globals, n: i32, count: i32, what: &str) -> bool {
        if (0..count).contains(&n) {
            return false;
        }
        g.notices.add(
            &FS_576,
            format!(
                "the surface hash table: a search of the other kind of table reads {what} {n} beyond the surface's {count} (memory outside them in FreeSurfer; skipped)"
            ),
        );
        true
    }

    /// `mhtfindClosestVertexGenericInBucket` and `mhtfindClosestFaceCentroidGenericInBucket`: each
    /// unripped vertex (each face whose probe's barycentric coordinates, with a positive
    /// `project_into_face`, are not negative) in the bucket's order; `(x - px)^2 + (y - py)^2 +
    /// (z - pz)^2` in `double` from the `float` coordinates; kept when below the best.
    fn scan(&mut self, g: &mut Globals, err: &mut dyn Write, v: [i64; 3]) -> Result<(), Fatal> {
        let Some(bins) = self.bucket(g, v) else {
            return Ok(());
        };
        for &n in bins {
            let p = match self.kind {
                Kind::Vertex => {
                    if self.beyond(g, n, self.mris.nvertices, "vertex") {
                        continue;
                    }
                    let vtx = self.mris.vertex(n as usize);
                    if vtx.ripflag != 0 {
                        continue;
                    }
                    let (x, y, z) = vertex_coord2xyz_float(g, err, vtx, self.mht.which())?;
                    [x, y, z]
                }
                Kind::Face { project_into_face } => {
                    if self.beyond(g, n, self.mht.nfaces.min(self.mris.nfaces), "face") {
                        continue;
                    }
                    let c = self.mht.f[n as usize];
                    if project_into_face > 0 {
                        let mut d = [[0f64; 3]; 3];
                        for (i, row) in d.iter_mut().enumerate() {
                            let vno = self.mris.face(n as usize).v[i];
                            *row = corner(g, err, self.mris, vno, self.mht.which())?;
                        }
                        let (r, _) = face_barycentric_coords_raw(g, d[0], d[1], d[2], self.probe);
                        if r < 0 {
                            continue;
                        }
                    }
                    [c.cx, c.cy, c.cz]
                }
            };
            let d: [f64; 3] = std::array::from_fn(|k| sub64(f64::from(p[k]), self.probe[k]));
            let sq = add64(
                add64(mul64(d[0], d[0]), mul64(d[1], d[1])),
                mul64(d[2], d[2]),
            );
            if sq >= self.best_sq {
                continue;
            }
            self.best_sq = sq;
            self.best = n;
        }
        Ok(())
    }

    /// The best's distance (`sqrt` of its square), when one was found.
    fn found_within(&self, d: f64) -> bool {
        self.best >= 0 && sqrt64(self.best_sq) <= d
    }
}

impl Mht {
    /// `checkConstructedWithVertices`: a face table is `checkConstructedWithVertices: mht not
    /// initialized for vertices` (fatal, `ERROR_BADPARM`).
    fn check_vertices(&self, g: &mut Globals, err: &mut dyn Write) -> Result<(), Fatal> {
        if self.fno_usage() != MhtFnoUsage::Vertex {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "checkConstructedWithVertices: mht not initialized for vertices\n",
            )?;
        }
        Ok(())
    }

    /// FS-998: a search of the other kind of table, unchecked in FreeSurfer: FreeSurfer's own
    /// check's error by default (rule B); `reproduce` goes on (the numbers read as the other kind).
    fn other_kind(&self, g: &mut Globals, err: &mut dyn Write, want: MhtFnoUsage) -> Result<(), Fatal> {
        if self.fno_usage() == want {
            return Ok(());
        }
        let (asked, table) = match want {
            MhtFnoUsage::Vertex => ("vertex", "face"),
            MhtFnoUsage::Face => ("face", "vertex"),
        };
        g.notices.add(
            &FS_998,
            format!(
                "the surface hash table: a {asked} search of a {table} table reads its numbers as {asked} numbers (FreeSurfer does not check)"
            ),
        );
        if g.notices.reproduce() {
            return Ok(());
        }
        match want {
            MhtFnoUsage::Vertex => self.check_vertices(g, err),
            MhtFnoUsage::Face => self.check_faces(g, err),
        }
    }

    /// The shared search of `findClosestVertexGeneric` and `findClosestFaceNoGeneric` (their code is
    /// the same but for the bucket scan and the vertex search's limit of 5 shells): `(number,
    /// distance)`, or `(-1, 1000)`.
    #[allow(clippy::too_many_arguments)]
    fn generic(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        probe: Pt,
        in_max_distance_mm: f64,
        in_max_mhts: i32,
        kind: Kind,
    ) -> Result<(i32, f64), Fatal> {
        let mhtres = f64::from(self.vres());
        let (mut max_distance_mm, mut max_mhts);
        if in_max_mhts == -1 {
            max_distance_mm = in_max_distance_mm;
            if max_distance_mm * 2.0 <= mhtres {
                max_mhts = 0;
            } else {
                let c = (max_distance_mm / mhtres).ceil();
                // C's implicit conversion of a double to int (cvttsd2si: INT_MIN when undefined;
                // the shells are then skipped, as in FreeSurfer, in both modes)
                max_mhts = cvtt(c).unwrap_or(i32::MIN);
            }
        } else {
            max_mhts = in_max_mhts;
            max_distance_mm = in_max_distance_mm;
            let tempdbl = if max_mhts >= 1 {
                mul64(f64::from(max_mhts), mhtres)
            } else {
                mul64(mhtres, 0.5)
            };
            if max_distance_mm > tempdbl {
                max_distance_mm = tempdbl;
            }
        }
        if kind == Kind::Vertex && max_mhts > MAX_MHTS_VERTEX {
            max_mhts = MAX_MHTS_VERTEX;
        }
        let mut s = Search {
            mht: self,
            mris,
            kind,
            probe,
            best: -1,
            best_sq: MIN_DIST_SQ,
            edges: HashSet::new(),
        };
        let mut vox = [0i64; 3];
        let mut near8 = [0i64; 3];
        for k in 0..3 {
            let vol = add64(div64(probe[k], mhtres), f64::from(TABLE_CENTER));
            let v = match cvtt(vol) {
                Some(v) => v,
                None => {
                    let detail = format!(
                        "the surface hash table: a search's probe voxel number {vol} is not an int (FreeSurfer takes -2147483648)"
                    );
                    g.notices.add(&FS_909, detail);
                    if !g.notices.reproduce() {
                        log::error_exit(
                            g,
                            err,
                            ERROR_BADPARM,
                            &format!(
                                "surface hash table: a search's probe voxel number {vol} is not finite or too large for its voxels (a resolution above 0 and finite coordinates are needed)\n"
                            ),
                        )?;
                    }
                    i32::MIN
                }
            };
            let m = sub64(vol, f64::from(v));
            vox[k] = i64::from(v);
            near8[k] = if m <= 0.5 { -1 } else { 0 };
        }
        let at = |o: [i64; 3]| [vox[0] + o[0], vox[1] + o[1], vox[2] + o[2]];
        let mut central27 = [[[false; 3]; 3]; 3];
        for xvi in 0..=1 {
            let xv = xvi + near8[0];
            for yvi in 0..=1 {
                let yv = yvi + near8[1];
                for zvi in 0..=1 {
                    let zv = zvi + near8[2];
                    s.scan(g, err, at([xv, yv, zv]))?;
                    central27[(xv + 1) as usize][(yv + 1) as usize][(zv + 1) as usize] = true;
                }
            }
        }
        'done: {
            if max_mhts == 0 {
                break 'done;
            }
            let remaining = mul64(0.5, mhtres);
            if max_distance_mm <= remaining {
                break 'done;
            }
            if s.found_within(remaining) {
                break 'done;
            }
            for xv in -1i64..=1 {
                for yv in -1i64..=1 {
                    for zv in -1i64..=1 {
                        if !central27[(xv + 1) as usize][(yv + 1) as usize][(zv + 1) as usize] {
                            s.scan(g, err, at([xv, yv, zv]))?;
                            central27[(xv + 1) as usize][(yv + 1) as usize][(zv + 1) as usize] =
                                true;
                        }
                    }
                }
            }
            // The farthest bucket's shell from the probe's voxel (Chebyshev distance): shells
            // beyond it hold no bucket. Not used when a probe beyond the table reads the edge's
            // buckets (FS-999 by default).
            let probe_inside = vox.iter().all(|&c| (0..i64::from(TABLE_SIZE)).contains(&c));
            let last = match self.bbox {
                None => -1,
                Some((lo, hi)) => (0..3)
                    .map(|k| {
                        (vox[k] - i64::from(lo[k]))
                            .abs()
                            .max((vox[k] - i64::from(hi[k])).abs())
                    })
                    .max()
                    .unwrap_or(-1),
            };
            let bounded = g.notices.reproduce() || probe_inside;
            let mut rvox: i32 = 2;
            while rvox <= max_mhts {
                let remaining = mul64(mhtres, f64::from(rvox - 1));
                if s.found_within(remaining) {
                    break 'done;
                }
                if max_distance_mm <= remaining {
                    break 'done;
                }
                if bounded && i64::from(rvox) > last {
                    // nothing left to find: the same answer as the rest of FreeSurfer's shells
                    break 'done;
                }
                let r = i64::from(rvox);
                let wall_jump = r + r;
                for xv in -r..=r {
                    for yv in -r..=r {
                        let is_wall = xv == -r || xv == r || yv == -r || yv == r;
                        let mut zv = -r;
                        while zv <= r {
                            s.scan(g, err, at([xv, yv, zv]))?;
                            zv += if is_wall { 1 } else { wall_jump };
                        }
                    }
                }
                rvox += 1;
            }
        }
        let (mut best, mut dist) = (s.best, 1e3);
        if best >= 0 {
            dist = sqrt64(s.best_sq);
            if dist > max_distance_mm {
                best = -1;
                dist = 1e3;
            }
        }
        Ok((best, dist))
    }

    /// `MHTfindClosestVertexGeneric(mht, probex, probey, probez, in_max_distance_mm,
    /// in_max_mhts, &vtxnum, &vtx_distance)` (`findClosestVertexGeneric`): the closest unripped
    /// vertex (its position `which`) within the distance and the shells asked for: with
    /// `in_max_mhts` -1, `in_max_distance_mm` alone sets them (`ceil(d / vres)` shells, none when
    /// `2 d <= vres`); otherwise `in_max_mhts` shells (0: the probe's voxel and its 7 nearest; 1:
    /// the 3 x 3 x 3 block; at most 5) and a distance of at most `in_max_mhts x vres` (`vres / 2`
    /// for 0). `(-1, 1000)` when none is found. FS-998: a face table's numbers are read as vertex
    /// numbers (an error by default).
    pub fn find_closest_vertex_generic(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        probe: Pt,
        in_max_distance_mm: f64,
        in_max_mhts: i32,
    ) -> Result<(i32, f64), Fatal> {
        self.other_kind(g, err, MhtFnoUsage::Vertex)?;
        self.generic(
            g,
            err,
            mris,
            probe,
            in_max_distance_mm,
            in_max_mhts,
            Kind::Vertex,
        )
    }

    /// `mhtBruteForceClosestVertex(x, y, z, which, &dmin)`: the unripped vertex closest to the
    /// point at its position `which`, every vertex in turn, the squared distance in `float` (from
    /// 10^8: none beyond 10 km), the first of equal distances; `(vno, sqrt)` or `(-1, 10000)`.
    pub fn brute_force_closest_vertex(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        p: [f32; 3],
        which: i32,
    ) -> Result<(i32, f32), Fatal> {
        let (mut min_vno, mut min_dsq) = (-1, 1e8f32);
        for vno in 0..mris.nvertices.max(0) {
            let v = mris.vertex(vno as usize);
            if v.ripflag != 0 {
                continue;
            }
            let (x, y, z) = vertex_coord2xyz_float(g, err, v, which)?;
            let (dx, dy, dz) = (sub(x, p[0]), sub(y, p[1]), sub(z, p[2]));
            let dsq = add(add(mul(dx, dx), mul(dy, dy)), mul(dz, dz));
            if dsq >= min_dsq {
                continue;
            }
            min_dsq = dsq;
            min_vno = vno;
        }
        Ok((min_vno, sqrt(min_dsq)))
    }

    /// `findClosestVertexNoXYZ(x, y, z, &min_dist)`: the closest vertex in the 3 x 3 x 3 block
    /// within one voxel (`vres`), its distance rounded to `float`; `(-1, 1000)` when there is none
    /// (FS-997: FreeSurfer's fallback to a full search is never taken; kept).
    fn no_xyz(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        p: [f32; 3],
    ) -> Result<(i32, f32), Fatal> {
        let probe = p.map(f64::from);
        let (vno, d) = self.generic(g, err, mris, probe, 1000.0, 1, Kind::Vertex)?;
        Ok((vno, narrow(d)))
    }

    /// `MHTfindClosestVertexNoXYZ(mht, mris, x, y, z, &min_dist)`: (a vertex table only) as
    /// `findClosestVertexNoXYZ` (FS-997).
    pub fn find_closest_vertex_no_xyz(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        p: [f32; 3],
    ) -> Result<(i32, f32), Fatal> {
        self.check_vertices(g, err)?;
        self.no_xyz(g, err, mris, p)
    }

    /// `findClosestSetVertexNo(x, y, z)`: the closest vertex within 3 shells (7 x 7 x 7) and
    /// 1,000 mm; else the brute-force search's.
    fn set_vertex_no(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        p: [f32; 3],
    ) -> Result<i32, Fatal> {
        let (mut vno, _) = self.generic(g, err, mris, p.map(f64::from), 1000.0, 3, Kind::Vertex)?;
        if vno < 0 {
            vno = self.brute_force_closest_vertex(g, err, mris, p, self.which())?.0;
        }
        Ok(vno)
    }

    /// `MHTfindClosestSetVertexNo(mht, mris, x, y, z)`: (a vertex table only) as
    /// `findClosestSetVertexNo`.
    pub fn find_closest_set_vertex_no(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        p: [f32; 3],
    ) -> Result<i32, Fatal> {
        self.check_vertices(g, err)?;
        self.set_vertex_no(g, err, mris, p)
    }

    /// `findVnoOfClosestVertexInTable(x, y, z, do_global_search)`: the closest vertex within one
    /// voxel; when there is none and `do_global_search` is not 0, within 2, 3, then 4 shells, then
    /// the brute-force search's.
    fn vno_in_table(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        p: [f32; 3],
        do_global_search: i32,
    ) -> Result<i32, Fatal> {
        let probe = p.map(f64::from);
        let (mut vno, _) = self.generic(g, err, mris, probe, 1000.0, 1, Kind::Vertex)?;
        if vno < 0 && do_global_search != 0 {
            for i in 2..=4 {
                vno = self.generic(g, err, mris, probe, 1000.0, i, Kind::Vertex)?.0;
                if vno >= 0 {
                    break;
                }
            }
            if vno < 0 {
                vno = self.brute_force_closest_vertex(g, err, mris, p, self.which())?.0;
            }
        }
        Ok(vno)
    }

    /// `MHTfindVnoOfClosestVertexInTable(mht, mris, x, y, z, do_global_search)`: (a vertex table
    /// only) as `findVnoOfClosestVertexInTable`.
    pub fn find_vno_of_closest_vertex_in_table(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        p: [f32; 3],
        do_global_search: i32,
    ) -> Result<i32, Fatal> {
        self.check_vertices(g, err)?;
        self.vno_in_table(g, err, mris, p, do_global_search)
    }

    /// `MHTfindClosestVertexInTable(mht, mris, x, y, z, do_global_search)`: as
    /// `findVnoOfClosestVertexInTable`, unchecked (FS-998); the vertex or `None`.
    pub fn find_closest_vertex_in_table(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        p: [f32; 3],
        do_global_search: i32,
    ) -> Result<Option<usize>, Fatal> {
        self.other_kind(g, err, MhtFnoUsage::Vertex)?;
        let vno = self.vno_in_table(g, err, mris, p, do_global_search)?;
        Ok(usize::try_from(vno).ok())
    }

    /// The position `which` (the table's) of vertex `vno` of `mris_for_v`, after
    /// `cheapAssert(0 <= vno && vno < mris_for_v->nvertices)` (`mrishash.cpp` line `line`).
    fn vertex_of(
        &self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        mris_for_v: &Mris,
        vno: i32,
        line: i32,
    ) -> Result<[f32; 3], Fatal> {
        if !(0 <= vno && vno < mris_for_v.nvertices) {
            return Err(diag::assert_failed(
                g,
                out,
                err,
                &format!("{}/utils/mrishash.cpp", diag::BUILD_ROOT),
                line,
                "0 <= vno && vno < mris_for_v->nvertices",
            ));
        }
        let (x, y, z) = vertex_coord2xyz_float(g, err, mris_for_v.vertex(vno as usize), self.which())?;
        Ok([x, y, z])
    }

    /// `MHTfindClosestVertexNo2(mht, mris, mris_for_v, v, &min_dist)`: vertex `vno` of
    /// `mris_for_v` at the table's position set, then `findClosestVertexNoXYZ` in the table's
    /// surface `mris` (FS-997), unchecked (FS-998).
    #[allow(clippy::too_many_arguments)]
    pub fn find_closest_vertex_no2(
        &self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        mris: &Mris,
        mris_for_v: &Mris,
        vno: i32,
    ) -> Result<(i32, f32), Fatal> {
        let p = self.vertex_of(g, out, err, mris_for_v, vno, 2501)?;
        self.other_kind(g, err, MhtFnoUsage::Vertex)?;
        self.no_xyz(g, err, mris, p)
    }

    /// `MHTfindClosestVertex2(mht, mris, mris_for_v, v, &min_dist)`: as
    /// [`Mht::find_closest_vertex_no2`]; the vertex or `None`.
    #[allow(clippy::too_many_arguments)]
    pub fn find_closest_vertex2(
        &self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        mris: &Mris,
        mris_for_v: &Mris,
        vno: i32,
    ) -> Result<(Option<usize>, f32), Fatal> {
        let (v, d) = self.find_closest_vertex_no2(g, out, err, mris, mris_for_v, vno)?;
        Ok((usize::try_from(v).ok(), d))
    }

    /// `MHTfindClosestVertexSet2(mht, mris, mris_for_v, v)`: vertex `vno` of `mris_for_v` at the
    /// table's position set, then `findClosestSetVertexNo` (unchecked: FS-998); the vertex or
    /// `None`.
    #[allow(clippy::too_many_arguments)]
    pub fn find_closest_vertex_set2(
        &self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        mris: &Mris,
        mris_for_v: &Mris,
        vno: i32,
    ) -> Result<Option<usize>, Fatal> {
        let p = self.vertex_of(g, out, err, mris_for_v, vno, 2527)?;
        self.other_kind(g, err, MhtFnoUsage::Vertex)?;
        Ok(usize::try_from(self.set_vertex_no(g, err, mris, p)?).ok())
    }

    /// `findClosestFaceNoGeneric(probe, in_max_distance_mm, in_max_mhts, project_into_face)`:
    /// the face whose centroid (computed when the table was made) is closest, within the distance
    /// and shells as [`Mht::find_closest_vertex_generic`] sets them (no limit on the shells); with
    /// a positive `project_into_face`, only faces whose barycentric coordinates of the probe (at
    /// the table's position set) are not negative. `(-1, 1000)` when none is found.
    #[allow(clippy::too_many_arguments)]
    fn face_no_generic(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        probe: Pt,
        in_max_distance_mm: f64,
        in_max_mhts: i32,
        project_into_face: i32,
    ) -> Result<(i32, f64), Fatal> {
        self.generic(
            g,
            err,
            mris,
            probe,
            in_max_distance_mm,
            in_max_mhts,
            Kind::Face { project_into_face },
        )
    }

    /// `MHTfindClosestFaceNoGeneric(mht, mris, ...)`: (a face table only) as
    /// `findClosestFaceNoGeneric`.
    #[allow(clippy::too_many_arguments)]
    pub fn find_closest_face_no_generic(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        probe: Pt,
        in_max_distance_mm: f64,
        in_max_mhts: i32,
        project_into_face: i32,
    ) -> Result<(i32, f64), Fatal> {
        self.check_faces(g, err)?;
        self.face_no_generic(
            g,
            err,
            mris,
            probe,
            in_max_distance_mm,
            in_max_mhts,
            project_into_face,
        )
    }

    /// `MHTfindClosestFaceGeneric(mht, mris, ..., &face, &fno, &dist)`: as
    /// `findClosestFaceNoGeneric`, unchecked (FS-998); the face number (or -1) and the distance.
    #[allow(clippy::too_many_arguments)]
    pub fn find_closest_face_generic(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        mris: &Mris,
        probe: Pt,
        in_max_distance_mm: f64,
        in_max_mhts: i32,
        project_into_face: i32,
    ) -> Result<(i32, f64), Fatal> {
        self.other_kind(g, err, MhtFnoUsage::Face)?;
        self.face_no_generic(
            g,
            err,
            mris,
            probe,
            in_max_distance_mm,
            in_max_mhts,
            project_into_face,
        )
    }
}

/// `MHTBruteForceClosestFace(mris, x, y, z, which, &dmin)`: the face (ripped ones too) whose
/// centroid at the position set `which` (the corners summed in `float` from 0, divided by 3) is
/// closest, every face in turn, the squared distance in `float` (from 10^8), the first of equal
/// distances; `(fno, sqrt)` or `(-1, 10000)`.
pub fn brute_force_closest_face(
    g: &mut Globals,
    err: &mut dyn Write,
    mris: &Mris,
    p: [f32; 3],
    which: i32,
) -> Result<(i32, f32), Fatal> {
    let (mut min_fno, mut min_dsq) = (-1, 1e8f32);
    for fno in 0..mris.nfaces.max(0) {
        let mut t = [0f32; 3];
        for n in 0..3 {
            let vno = mris.face(fno as usize).v[n];
            let (x, y, z) = vertex_coord2xyz_float(g, err, mris.vertex(vno as usize), which)?;
            t = [add(t[0], x), add(t[1], y), add(t[2], z)];
        }
        let c = t.map(|s| div(s, 3.0));
        let (dx, dy, dz) = (sub(c[0], p[0]), sub(c[1], p[1]), sub(c[2], p[2]));
        let dsq = add(add(mul(dx, dx), mul(dy, dy)), mul(dz, dz));
        if min_dsq <= dsq {
            continue;
        }
        min_dsq = dsq;
        min_fno = fno;
    }
    Ok((min_fno, sqrt(min_dsq)))
}

