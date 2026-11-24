// SPDX-License-Identifier: Apache-2.0
//! Voxel lists (spec §65): the list itself, `VLSTalloc` and `VLSTcreateInRegion` (`mri_cc`'s
//! hemisphere lists; FS-889, FS-890, FS-892).

use larmorx_core::defects::{FS_889, FS_890};

use super::deriv::Neighbours;
use crate::mri::Region;
use crate::mri::geom::DIAG_SHOW;
use crate::mri::Mri;
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
}
