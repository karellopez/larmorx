// SPDX-License-Identifier: Apache-2.0
//! Voxel lists (spec §65): the list itself, `VLSTalloc`, `VLSTcreate`, `VLSTcreateInRegion`
//! (`mri_cc`'s hemisphere lists), `VLSTsort`, `VLSTfree`, `VLSTcomputeStats`, `VLSTcopy`,
//! `VLSTcopyInto`, `VLSTadd`, `VLSTaddUnique`, `VLSTaddWithValue` and `VLSTinList` (FS-888 to
//! FS-893).

use larmorx_core::defects::{FS_888, FS_889, FS_890, FS_891};
use larmorx_core::glibc_qsort::qsort;

use super::deriv::Neighbours;
use crate::clib::printf::sprintf_f64;
use crate::mri::Mri;
use crate::mri::Region;
use crate::mri::geom::DIAG_SHOW;
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};
use crate::sse;

/// A voxel list (`VOXEL_LIST`, §65): its type, nine arrays of the length allocated, the count,
/// the capacity, whether its volume is set (the volume it was made from, which the caller
/// keeps), its statistics and the four optional spline arrays.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VoxelList {
    pub kind: i32,
    pub xi: Vec<i32>,
    pub yi: Vec<i32>,
    pub zi: Vec<i32>,
    pub fi: Vec<i32>,
    pub xd: Vec<f32>,
    pub yd: Vec<f32>,
    pub zd: Vec<f32>,
    pub vsrc: Vec<f32>,
    pub vdst: Vec<f32>,
    pub nvox: i32,
    pub max_vox: i32,
    /// Whether the list's volume is set.
    pub has_mri: bool,
    pub mean: f64,
    pub std: f64,
    /// The slopes.
    pub mx: Option<Vec<f32>>,
    pub my: Option<Vec<f32>>,
    pub mz: Option<Vec<f32>>,
    /// The parameterisation.
    pub t: Option<Vec<f32>>,
}

impl VoxelList {
    /// The length of the arrays (what was allocated).
    pub fn alloc_len(&self) -> usize {
        self.xi.len()
    }

    /// Nine arrays of `n` zeros (the count and capacity unchanged).
    fn set_arrays(&mut self, n: usize) {
        self.xi = vec![0; n];
        self.yi = vec![0; n];
        self.zi = vec![0; n];
        self.fi = vec![0; n];
        self.xd = vec![0.0; n];
        self.yd = vec![0.0; n];
        self.zd = vec![0.0; n];
        self.vsrc = vec![0.0; n];
        self.vdst = vec![0.0; n];
    }
}

/// The voxels of a box along one axis: from its first while below `first + size` (32-bit
/// wrapping sum), every `step`-th.
fn axis(first: i32, size: i32, step: i32) -> impl Iterator<Item = i32> {
    let end = first.wrapping_add(size);
    let mut x = first;
    std::iter::from_fn(move || {
        if x < end {
            let v = x;
            x = x.wrapping_add(step);
            // a step past the `int` maximum ends the walk (no voxel lies there)
            if x < v {
                x = end;
            }
            Some(v)
        } else {
            None
        }
    })
}

impl Process {
    /// `VLSTalloc(n)` (§65): nine arrays of `n` zeros, `nvox = max_vox = n`, type 0, no volume,
    /// no spline arrays; a negative `n` ends the program.
    pub fn vlst_alloc(&mut self, n: i32) -> Result<VoxelList, Stop> {
        if n < 0 {
            self.errno = 12;
            self.error_exit(
                codes::NO_MEMORY,
                format!("VLSTalloc({n}): could not allocate VL struct").as_bytes(),
            )?;
        }
        let mut vl = VoxelList {
            nvox: n,
            max_vox: n,
            ..VoxelList::default()
        };
        vl.set_arrays(n.max(0) as usize);
        Ok(vl)
    }

    /// The end of the program where a voxel list's arrays would be read or written past what
    /// was allocated (FS-890, both modes).
    pub(crate) fn vlst_fs890(&mut self, function: &str, i: i64, n: usize) -> Result<(), Stop> {
        self.note(
            &FS_890,
            format!("{function}: voxel {i} of an array of {n}: the program ended (FreeSurfer reads or writes past the list's arrays)"),
        );
        self.error_exit(
            codes::BAD_PARM,
            format!("{function}: voxel {i} of an array of {n} (FreeSurfer reads or writes past the list's arrays here)")
                .as_bytes(),
        )
    }

    /// `VLSTcreateInRegion(mri, low, hi, vl, skip, border, box)` (§65): the voxels of the box
    /// (every `skip + 1`-th along each axis) whose values lie in `[low, hi]`, the list sized by
    /// frame 0's count, then every frame's voxels appended (with `border` only those with a face
    /// neighbour of frame 0 rounding to the frame's number). Returns the list (`vl` itself when
    /// given) and whether it is the given one.
    #[allow(clippy::too_many_arguments)]
    pub fn vlst_create_in_region(
        &mut self,
        mri: &Mri,
        low: f32,
        hi: f32,
        vl: Option<VoxelList>,
        skip: i32,
        border: i32,
        b: &Region,
    ) -> Result<VoxelList, Stop> {
        let name = "VLSTcreateInRegion";
        if skip < 0 {
            self.note(
                &FS_889,
                format!("{name}: a negative skip: the program ended (FreeSurfer never ends)"),
            );
            self.error_exit(
                codes::BAD_PARM,
                format!("{name}: skip {skip} is negative (FreeSurfer never ends here)").as_bytes(),
            )?;
        }
        let step = skip.wrapping_add(1);
        let (lo, hi) = (sse::widen(low), sse::widen(hi));
        let mut count = 0i32;
        for x in axis(b.x, b.dx, step) {
            for y in axis(b.y, b.dy, step) {
                for z in axis(b.z, b.dz, step) {
                    let v = sse::widen(self.mri_get_vox_val(mri, x, y, z, 0, 0)?);
                    if lo <= v && v <= hi {
                        count = count.wrapping_add(1);
                    }
                }
            }
        }
        let mut vl = match vl {
            Some(mut l) => {
                if l.max_vox < count {
                    self.vlst_allocating(count);
                    l.set_arrays(count.max(0) as usize);
                }
                l
            }
            None => {
                self.vlst_allocating(count);
                let mut l = VoxelList::default();
                l.set_arrays(count.max(0) as usize);
                l
            }
        };
        vl.nvox = count;
        vl.has_mri = true;
        let n = vl.alloc_len();
        let mut i = 0usize;
        for f in 0..mri.nframes {
            for x in axis(b.x, b.dx, step) {
                for y in axis(b.y, b.dy, step) {
                    for z in axis(b.z, b.dz, step) {
                        let v = sse::widen(self.mri_get_vox_val(mri, x, y, z, f, 0)?);
                        if !(lo <= v && v <= hi) {
                            continue;
                        }
                        if border != 0
                            && self.mri_neighbours(mri, x, y, z, Neighbours::Value(f))? == 0
                        {
                            continue;
                        }
                        if i >= n {
                            self.vlst_fs890(name, i as i64, n)?;
                        }
                        vl.xi[i] = x;
                        vl.yi[i] = y;
                        vl.zi[i] = z;
                        vl.fi[i] = f;
                        vl.vsrc[i] = sse::narrow(v);
                        i += 1;
                    }
                }
            }
        }
        Ok(vl)
    }

    /// `allocating <n> voxel indices...` under `DIAG_SHOW` and `DIAG_VERBOSE` (§65).
    fn vlst_allocating(&mut self, n: i32) {
        if self.gdiag & DIAG_SHOW != 0 && self.gdiag & DIAG_VERBOSE != 0 {
            self.put_out(format!("allocating {n} voxel indices...\n").as_bytes());
        }
    }

    /// `VLSTcreate(mri, low, hi, vl, skip, border)` (§65): every `skip + 1`-th frame, slice, row
    /// and column (frames outermost) whose value lies in `[low, hi]`, with `border` only those
    /// with fewer than six face neighbours in the range (in the voxel's frame). The count sizes
    /// the list (none given, or a given one whose `max_vox` is below it, gets nine new arrays of
    /// the count, at least 1); `max_vox` is never set (FS-892, both modes). FS-888: with `border`,
    /// `reproduce` counts the interior voxels instead (the border voxels are stored past the
    /// count, FS-890 past the arrays); FS-889: a negative skip ends the program (both modes).
    /// Returns the list (the given one when given), its volume `mri`.
    pub fn vlst_create(
        &mut self,
        mri: &Mri,
        low: f32,
        hi: f32,
        vl: Option<VoxelList>,
        skip: i32,
        border: i32,
    ) -> Result<VoxelList, Stop> {
        let name = "VLSTcreate";
        if skip < 0 {
            self.note(
                &FS_889,
                format!("{name}: a negative skip: the program ended (FreeSurfer never ends)"),
            );
            self.error_exit(
                codes::BAD_PARM,
                format!("{name}: skip {skip} is negative (FreeSurfer never ends here)").as_bytes(),
            )?;
        }
        let step = skip.wrapping_add(1);
        let (lo, hiw) = (sse::widen(low), sse::widen(hi));
        let ext = [mri.geom.width, mri.geom.height, mri.geom.depth];
        let walk = |e: i32| axis(0, e, step);
        // the voxels in the range (the frame, slice, row and column order) and their face
        // neighbours in the range (with `border` only)
        let mut found: Vec<([i32; 4], f64, i32)> = Vec::new();
        for f in walk(mri.nframes) {
            for z in walk(ext[2]) {
                for y in walk(ext[1]) {
                    for x in walk(ext[0]) {
                        let v = sse::widen(self.mri_get_vox_val(mri, x, y, z, f, 0)?);
                        if !(lo <= v && v <= hiw) {
                            continue;
                        }
                        let nb = if border != 0 {
                            self.mri_neighbours(mri, x, y, z, Neighbours::InRange(f, low, hi))?
                        } else {
                            0
                        };
                        found.push(([x, y, z, f], v, nb));
                    }
                }
            }
        }
        let stored: Vec<&([i32; 4], f64, i32)> =
            found.iter().filter(|e| border == 0 || e.2 < 6).collect();
        let mut count = stored.len() as i32;
        if border != 0 {
            let interior = found.iter().filter(|e| e.2 >= 6).count() as i32;
            if interior != count {
                if self.mode().reproduce() {
                    self.note(
                        &FS_888,
                        format!("{name}: the list's count is the {interior} interior voxels, not the {count} border voxels it holds"),
                    );
                    count = interior;
                } else {
                    self.note(
                        &FS_888,
                        format!("{name}: the list's count is the {count} border voxels it holds (FreeSurfer counts the {interior} interior voxels)"),
                    );
                }
            }
        }
        // the allocation line whether or not the arrays are made (observed: `vlst`)
        self.vlst_allocating(count);
        let mut vl = match vl {
            Some(mut l) => {
                if l.max_vox < count {
                    l.set_arrays(count.max(1) as usize);
                }
                l
            }
            None => {
                let mut l = VoxelList::default();
                l.set_arrays(count.max(1) as usize);
                l
            }
        };
        vl.nvox = count;
        vl.has_mri = true;
        let n = vl.alloc_len();
        for (i, &&([x, y, z, f], v, _)) in stored.iter().enumerate() {
            if i >= n {
                self.vlst_fs890(name, i as i64, n)?;
            }
            vl.xi[i] = x;
            vl.yi[i] = y;
            vl.zi[i] = z;
            vl.fi[i] = f;
            vl.vsrc[i] = sse::narrow(v);
            if [x, y, z] == self.gvoxel {
                let mut b = format!("voxel ({x}, {y}, {z}) = ").into_bytes();
                b.extend_from_slice(&sprintf_f64(b"%2.1f", v));
                b.extend_from_slice(format!(" added to voxlist at {i}\n").as_bytes());
                self.put_out(&b);
            }
        }
        Ok(vl)
    }

    /// `VLSTsort(src, dst)` (§65): without `dst` a copy of `src` (`VLSTcopy(src, NULL, 0,
    /// nvox)`); the destination's first `nvox` voxels sorted (glibc's merge order) by larger
    /// `vsrc`, then larger `xi`, `yi`, `zi`. FS-891: `reproduce` leaves the frame indices in
    /// place. FS-890: a destination of more voxels than the source's count, or past its arrays.
    /// Returns the destination and whether it is the given one.
    pub fn vlst_sort(
        &mut self,
        src: &VoxelList,
        dst: Option<VoxelList>,
    ) -> Result<(VoxelList, bool), Stop> {
        let name = "VLSTsort";
        let (mut d, given) = match dst {
            Some(d) => (d, true),
            None => (self.vlst_copy(src, None, 0, src.nvox)?.0, false),
        };
        if given && src.nvox < 0 {
            self.errno = 12;
            self.error_exit(
                codes::NO_MEMORY,
                b"MRIorderIndices: could not allocate sort table",
            )?;
        }
        if d.nvox > src.nvox {
            self.vlst_fs890(name, i64::from(src.nvox), src.nvox.max(0) as usize)?;
        }
        let n = d.nvox.max(0) as usize;
        if n > d.alloc_len() {
            self.vlst_fs890(name, d.alloc_len() as i64, d.alloc_len())?;
        }
        let mut order: Vec<u32> = (0..n as u32).collect();
        {
            let dd = &d;
            qsort(&mut order, |&a, &b| {
                let (a, b) = (a as usize, b as usize);
                let (va, vb) = (dd.vsrc[a], dd.vsrc[b]);
                if va > vb {
                    return -1;
                }
                if va < vb {
                    return 1;
                }
                for k in [&dd.xi, &dd.yi, &dd.zi] {
                    if k[a] > k[b] {
                        return -1;
                    }
                    if k[a] < k[b] {
                        return 1;
                    }
                }
                0
            });
        }
        let moved = order.iter().enumerate().any(|(i, &j)| j as usize != i);
        let keep_frames = self.mode().reproduce();
        if moved {
            let frames_differ = order
                .iter()
                .enumerate()
                .any(|(i, &j)| d.fi[j as usize] != d.fi[i]);
            if frames_differ {
                if keep_frames {
                    self.note(
                        &FS_891,
                        format!(
                            "{name}: the frame indices stayed in place while their voxels moved"
                        ),
                    );
                } else {
                    self.note(
                        &FS_891,
                        format!("{name}: the frame indices moved with their voxels (FreeSurfer leaves them in place)"),
                    );
                }
            }
            let perm = |v: &mut Vec<f32>| {
                let old = v[..n].to_vec();
                for (i, &j) in order.iter().enumerate() {
                    v[i] = old[j as usize];
                }
            };
            let permi = |v: &mut Vec<i32>| {
                let old = v[..n].to_vec();
                for (i, &j) in order.iter().enumerate() {
                    v[i] = old[j as usize];
                }
            };
            permi(&mut d.xi);
            permi(&mut d.yi);
            permi(&mut d.zi);
            if !keep_frames {
                permi(&mut d.fi);
            }
            perm(&mut d.xd);
            perm(&mut d.yd);
            perm(&mut d.zd);
            perm(&mut d.vsrc);
            perm(&mut d.vdst);
        }
        Ok((d, given))
    }

    /// `VLSTfree(&vl)` (§65): 0, the caller left without it; -4 (no message) for none.
    pub fn vlst_free(&mut self, vl: &mut Option<VoxelList>) -> i32 {
        if vl.take().is_none() {
            return codes::BAD_PARM;
        }
        codes::NONE
    }

    /// `VLSTcomputeStats(vl)` (§65) with the list's volume `mri`: the mean and deviation of
    /// frame 0's values at the first `nvox` voxels (0 and 0 without a volume or voxels).
    pub fn vlst_compute_stats(
        &mut self,
        vl: &mut VoxelList,
        mri: Option<&Mri>,
    ) -> Result<(), Stop> {
        let Some(mri) = mri else {
            vl.mean = 0.0;
            vl.std = 0.0;
            return Ok(());
        };
        if vl.nvox <= 0 {
            vl.mean = 0.0;
            vl.std = 0.0;
            return Ok(());
        }
        let n = vl.nvox as usize;
        if n > vl.alloc_len() {
            self.vlst_fs890("VLSTcomputeStats", vl.alloc_len() as i64, vl.alloc_len())?;
        }
        let (mut s, mut s2) = (0.0f64, 0.0f64);
        for i in 0..n {
            let v = sse::widen(self.mri_get_vox_val(mri, vl.xi[i], vl.yi[i], vl.zi[i], 0, 0)?);
            s = sse::add64(s, v);
            s2 = sse::add64(s2, sse::mul64(v, v));
        }
        let nf = n as f64;
        let mean = sse::div64(s, nf);
        vl.mean = mean;
        vl.std = sse::sqrt64(sse::sub64(sse::div64(s2, nf), sse::mul64(mean, mean)));
        Ok(())
    }

    /// `VLSTcopy(src, dst, start, num)` (§65, `into` false) and `VLSTcopyInto` (`into` true):
    /// the source's voxels `start..start + num` into `0..num` (or `0..num` into `start..start +
    /// num`) of the destination (a new list of `num`, or `start + num`, when none is given); the
    /// capacity checked, the type copied, a new zero slope array of `num` for each the source
    /// has (FS-893, both modes: the program ends when the source has `mx` and the destination
    /// then lacks one of the four); `nvox` raised to the copy's end. FS-890 past the arrays.
    /// Returns the destination and whether it is the given one.
    pub fn vlst_copy_any(
        &mut self,
        src: &VoxelList,
        dst: Option<VoxelList>,
        start: i32,
        num: i32,
        into: bool,
    ) -> Result<(VoxelList, bool), Stop> {
        let name = if into { "VLSTcopyInto" } else { "VLSTcopy" };
        let end = start.wrapping_add(num);
        let need = if into { end } else { num };
        let (mut d, given) = match dst {
            Some(d) => (d, true),
            None => (self.vlst_alloc(need)?, false),
        };
        if d.max_vox < need {
            self.error_exit(
                codes::NO_MEMORY,
                format!(
                    "VLSTcopy: destination voxlist not big enough ({}, but must be {end})",
                    d.max_vox
                )
                .as_bytes(),
            )?;
        }
        d.kind = src.kind;
        let slopes = [&src.mx, &src.my, &src.mz, &src.t];
        let mut news: [Option<Option<Vec<f32>>>; 4] = [None, None, None, None];
        for (k, s) in slopes.into_iter().enumerate() {
            if s.is_some() {
                news[k] = Some(if num < 0 {
                    self.errno = 12;
                    None
                } else {
                    Some(vec![0.0f32; num as usize])
                });
            }
        }
        let [a, b, c, e] = news;
        for (slot, new) in [
            (&mut d.mx, a),
            (&mut d.my, b),
            (&mut d.mz, c),
            (&mut d.t, e),
        ] {
            if let Some(n) = new {
                *slot = n;
            }
        }
        if src.mx.is_some() && (d.mx.is_none() || d.my.is_none() || d.mz.is_none() || d.t.is_none())
        {
            self.error_exit(
                codes::NO_MEMORY,
                format!("VLSTcopy: could not allocate {num}-len slope arrays").as_bytes(),
            )?;
        }
        let (ns, nd) = (src.alloc_len(), d.alloc_len());
        for i in 0..num.max(0) {
            let (si, di) = if into {
                (i64::from(i), i64::from(start) + i64::from(i))
            } else {
                (i64::from(start) + i64::from(i), i64::from(i))
            };
            if si < 0 || si as usize >= ns {
                self.vlst_fs890(name, si, ns)?;
            }
            if di < 0 || di as usize >= nd {
                self.vlst_fs890(name, di, nd)?;
            }
            let (si, di) = (si as usize, di as usize);
            d.xi[di] = src.xi[si];
            d.yi[di] = src.yi[si];
            d.zi[di] = src.zi[si];
            d.fi[di] = src.fi[si];
            d.xd[di] = src.xd[si];
            d.yd[di] = src.yd[si];
            d.zd[di] = src.zd[si];
            d.vsrc[di] = src.vsrc[si];
            d.vdst[di] = src.vdst[si];
        }
        if d.nvox < need {
            d.nvox = need;
        }
        Ok((d, given))
    }

    /// `VLSTcopy(src, dst, start, num)` (§65): [`Process::vlst_copy_any`].
    pub fn vlst_copy(
        &mut self,
        src: &VoxelList,
        dst: Option<VoxelList>,
        start: i32,
        num: i32,
    ) -> Result<(VoxelList, bool), Stop> {
        self.vlst_copy_any(src, dst, start, num, false)
    }

    /// `VLSTadd(vl, x, y, z, xd, yd, zd)` (§65): the indices and coordinates at `nvox`, 0; a full
    /// list (`nvox >= max_vox`): the C-style error `VLSTadd: too many voxels (<max_vox>)`, -2.
    #[allow(clippy::too_many_arguments)]
    pub fn vlst_add(
        &mut self,
        vl: &mut VoxelList,
        x: i32,
        y: i32,
        z: i32,
        xd: f32,
        yd: f32,
        zd: f32,
    ) -> Result<i32, Stop> {
        if vl.nvox >= vl.max_vox {
            self.error_printf(
                codes::NO_MEMORY,
                format!("VLSTadd: too many voxels ({})", vl.max_vox).as_bytes(),
            );
            return Ok(codes::NO_MEMORY);
        }
        let n = vl.alloc_len();
        if vl.nvox < 0 || vl.nvox as usize >= n {
            self.vlst_fs890("VLSTadd", i64::from(vl.nvox), n)?;
        }
        let i = vl.nvox as usize;
        vl.xi[i] = x;
        vl.yi[i] = y;
        vl.zi[i] = z;
        vl.xd[i] = xd;
        vl.yd[i] = yd;
        vl.zd[i] = zd;
        vl.nvox += 1;
        Ok(codes::NONE)
    }

    /// `VLSTinList(vl, x, y, z)` (§65): 1 when one of the first `nvox` voxels has these indices.
    pub fn vlst_in_list(&mut self, vl: &VoxelList, x: i32, y: i32, z: i32) -> Result<i32, Stop> {
        let n = vl.alloc_len();
        for i in 0..vl.nvox.max(0) as usize {
            if i >= n {
                self.vlst_fs890("VLSTinList", i as i64, n)?;
            }
            if vl.xi[i] == x && vl.yi[i] == y && vl.zi[i] == z {
                return Ok(1);
            }
        }
        Ok(0)
    }

    /// `VLSTaddUnique` (§65): 0 when the voxel is in the list, else [`Process::vlst_add`] (its
    /// error printed) and 1.
    #[allow(clippy::too_many_arguments)]
    pub fn vlst_add_unique(
        &mut self,
        vl: &mut VoxelList,
        x: i32,
        y: i32,
        z: i32,
        xd: f32,
        yd: f32,
        zd: f32,
    ) -> Result<i32, Stop> {
        if self.vlst_in_list(vl, x, y, z)? != 0 {
            return Ok(0);
        }
        self.vlst_add(vl, x, y, z, xd, yd, zd)?;
        Ok(1)
    }

    /// `VLSTaddWithValue` (§65): [`Process::vlst_add`], then the new voxel's `vdst` and `vsrc`,
    /// 0; a failed add: the code it recorded.
    #[allow(clippy::too_many_arguments)]
    pub fn vlst_add_with_value(
        &mut self,
        vl: &mut VoxelList,
        x: i32,
        y: i32,
        z: i32,
        xd: f32,
        yd: f32,
        zd: f32,
        vsrc: f32,
        vdst: f32,
    ) -> Result<i32, Stop> {
        if self.vlst_add(vl, x, y, z, xd, yd, zd)? != codes::NONE {
            return Ok(self.gerror);
        }
        let i = (vl.nvox - 1) as usize;
        vl.vdst[i] = vdst;
        vl.vsrc[i] = vsrc;
        Ok(codes::NONE)
    }
}
