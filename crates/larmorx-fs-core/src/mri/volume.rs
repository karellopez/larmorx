// SPDX-License-Identifier: Apache-2.0
//! The volume (`MRI`, spec §4): its record, allocation with and without voxels, the volume made
//! from a geometry, freeing, the intents; §3's matrix helpers on a volume (`MRImatrixOf...`,
//! `MRIsetVox2RASFromMatrix`, `MRIp0ToCRAS`), §6's pulse parameters and geometry copies, §11's
//! shear removal.

use larmorx_core::defects::{FS_55, FS_264};

use super::ctab::ColorTable;
use super::geom::{VolGeom, fixed_name};
use super::xform::MincTransform;
use super::{has_size, sizeof};
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// The C library's `errno` for "Cannot allocate memory".
pub const ENOMEM: i32 = 12;

/// A region (`MRI_REGION`): a corner and extents.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub dx: i32,
    pub dy: i32,
    pub dz: i32,
}

/// A frame's parameters (§4): all zero when allocated, with a 4 x 4 zero `m_ras2vox`.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub kind: i32,
    pub te: f32,
    pub tr: f32,
    pub flip: f32,
    pub ti: f32,
    pub td: f32,
    pub sequence_type: i32,
    pub echo_spacing: f32,
    pub echo_train_len: f32,
    pub read_dir: [f32; 3],
    pub pe_dir: [f32; 3],
    pub slice_dir: [f32; 3],
    pub label: i32,
    pub name: Vec<u8>,
    pub dof: i32,
    pub m_ras2vox: Option<Matrix>,
    pub thresh: f32,
    pub units: i32,
    /// DX, DY, DZ, DR, DP, DS, the b-value and TM.
    pub d: [f64; 8],
    pub diffusion_type: i32,
    /// The four gradients' ramp and flat times.
    pub ramp_flat: [i64; 8],
    /// The four gradients' amplitudes.
    pub amp: [f64; 4],
}

impl Default for Frame {
    fn default() -> Frame {
        Frame {
            kind: 0,
            te: 0.0,
            tr: 0.0,
            flip: 0.0,
            ti: 0.0,
            td: 0.0,
            sequence_type: 0,
            echo_spacing: 0.0,
            echo_train_len: 0.0,
            read_dir: [0.0; 3],
            pe_dir: [0.0; 3],
            slice_dir: [0.0; 3],
            label: 0,
            name: Vec::new(),
            dof: 0,
            m_ras2vox: Some(Matrix::zeros(4, 4, MATRIX_REAL)),
            thresh: 0.0,
            units: 0,
            d: [0.0; 8],
            diffusion_type: 0,
            ramp_flat: [0; 8],
            amp: [0.0; 4],
        }
    }
}

/// A volume (§4): its geometry (whose extent is the volume's) and the rest of its record.
#[derive(Clone, Debug, PartialEq)]
pub struct Mri {
    /// The geometry: the extent, voxel sizes, cosines, centre, shears, name and cached matrices
    /// (FreeSurfer's volume is a geometry).
    pub geom: VolGeom,
    pub nframes: i32,
    /// The voxel type (`super::types`).
    pub kind: i32,
    /// The counts of §4 step 2, as `size_t` (wrapped).
    pub bytes_per_vox: u64,
    pub bytes_total: u64,
    pub vox_per_row: u64,
    pub vox_per_slice: u64,
    pub vox_per_vol: u64,
    pub vox_total: u64,
    pub imnr0: i32,
    pub imnr1: i32,
    pub xstart: f32,
    pub ystart: f32,
    pub zstart: f32,
    pub xend: f32,
    pub yend: f32,
    pub zend: f32,
    pub fov: f32,
    pub thick: f32,
    pub ps: f32,
    pub scale: i32,
    pub roi: Region,
    pub version: i32,
    pub intent: i32,
    pub dof: i32,
    pub brightness: i32,
    pub yinvert: i32,
    pub ptype: i32,
    pub outside_val: f64,
    pub flip_angle: f64,
    pub tr: f32,
    pub te: f32,
    pub ti: f32,
    pub location: f32,
    pub field_strength: f32,
    pub mean: f64,
    /// The phase-encode direction (none at allocation).
    pub pedir: Option<Vec<u8>>,
    pub bvec_space: i32,
    pub bvals: Option<Matrix>,
    pub bvecs: Option<Matrix>,
    pub warp_field_format: i32,
    pub gcamorph_spacing: i32,
    pub gcamorph_exp_k: f64,
    pub cmdlines: Vec<Vec<u8>>,
    pub subject_name: Vec<u8>,
    pub path_to_t1: Vec<u8>,
    pub fname_format: Vec<u8>,
    pub gdf_image_stem: Vec<u8>,
    pub transform_fname: Vec<u8>,
    pub auto_align: Option<Matrix>,
    pub orig_ras2vox: Option<Matrix>,
    /// The MINC transform the volume carries (§14), when it has a linear matrix.
    pub transform: Option<MincTransform>,
    /// Whether the volume owns its transform.
    pub free_transform: i32,
    pub ct: Option<ColorTable>,
    /// Whether the column, row and slice index tables exist (with voxels).
    pub index: bool,
    /// The frames (none only where allocation never made them).
    pub frames: Option<Vec<Frame>>,
    /// The voxels' bytes (little-endian elements), none for a volume without voxels.
    pub voxels: Option<Vec<u8>>,
}

/// The names of the intents (§4).
const INTENTS: [&str; 6] = ["MRI", "LABEL", "SHAPE", "WARP", "WARP_INV", "TIME_SERIES"];

/// `MRI::intentName(code)` (§4).
pub fn intent_name(code: i32) -> &'static str {
    usize::try_from(code)
        .ok()
        .and_then(|i| INTENTS.get(i).copied())
        .unwrap_or("UNKNOWN")
}

/// `MRI::intentCode(name)` (§4): -1 for any other name.
pub fn intent_code(name: &[u8]) -> i32 {
    INTENTS
        .iter()
        .position(|n| n.as_bytes() == name)
        .map_or(-1, |i| i as i32)
}

/// The largest voxel buffer the C library can make (§4 step 6: it fails beyond about 2^47
/// bytes).
const MAX_BUFFER: u64 = 1 << 47;

impl Mri {
    /// The value of the index table of axis `k` (0 columns, 1 rows, 2 slices) at `i` (§4 step 7):
    /// 0 at or below 0, `extent - 1` at or beyond the extent, else `i`.
    pub fn index_value(&self, k: usize, i: i32) -> i32 {
        let e = self.geom.extent(k);
        if i <= 0 {
            0
        } else if i >= e {
            e - 1
        } else {
            i
        }
    }

    /// Whether the volume has voxels.
    pub fn has_voxels(&self) -> bool {
        self.voxels.is_some()
    }
}

/// Whether a voxel buffer of `bytes` could be made (beyond the limit, or not in memory: no).
fn make_buffer(bytes: u64) -> Option<Vec<u8>> {
    if bytes > MAX_BUFFER {
        return None;
    }
    let mut v = Vec::new();
    v.try_reserve_exact(usize::try_from(bytes).ok()?).ok()?;
    v.resize(bytes as usize, 0);
    Some(v)
}

impl Process {
    /// The shape messages of §4 step 1 (one per extent that is not positive).
    pub(crate) fn shape_messages(&mut self, extents: &[i32]) {
        for &e in extents {
            if e <= 0 {
                self.put_out(
                    b"MRI::Shape::Shape(): volume size must be greater than 0 in every dimension\n",
                );
            }
        }
    }

    /// `MRIallocSequence(w, h, d, type, nframes)` (§4): a volume with voxels.
    pub fn mri_alloc_sequence(
        &mut self,
        w: i32,
        h: i32,
        d: i32,
        kind: i32,
        nframes: i32,
    ) -> Result<Mri, Stop> {
        self.mri_alloc(w, h, d, kind, nframes, true)
    }

    /// `MRIallocHeader(w, h, d, type, nframes)` (§4): a volume without voxels.
    pub fn mri_alloc_header(
        &mut self,
        w: i32,
        h: i32,
        d: i32,
        kind: i32,
        nframes: i32,
    ) -> Result<Mri, Stop> {
        self.mri_alloc(w, h, d, kind, nframes, false)
    }

    /// The allocation of §4, with or without voxels.
    pub fn mri_alloc(
        &mut self,
        w: i32,
        h: i32,
        d: i32,
        kind: i32,
        nframes: i32,
        with_voxels: bool,
    ) -> Result<Mri, Stop> {
        // step 1
        self.shape_messages(&[w, h, d, nframes]);
        // step 2
        let half = |e: i32| sse::narrow(f64::from(e) / 2.0);
        let (xend, yend, zend) = (half(w), half(h), half(d));
        let wz = |e: i32| e as i64 as u64;
        let vox_per_row = wz(w);
        let vox_per_slice = vox_per_row.wrapping_mul(wz(h));
        let vox_per_vol = vox_per_slice.wrapping_mul(wz(d));
        let vox_total = vox_per_vol.wrapping_mul(wz(nframes));
        // step 3
        if !has_size(kind) {
            if self.mode().reproduce() {
                self.note(
                    &FS_55,
                    format!(
                        "a volume of type {kind}, which has no size, allocated with the size -1"
                    ),
                );
            } else {
                self.note(
                    &FS_55,
                    format!("a volume of type {kind}, which has no size, refused"),
                );
                self.fatal(1, format!("unsupported MRI data type: {kind}").as_bytes())?;
            }
        }
        let bytes_per_vox = sizeof(kind);
        let bytes_total = vox_total.wrapping_mul(bytes_per_vox);
        let mut geom = VolGeom {
            width: w,
            height: h,
            depth: d,
            ..VolGeom::default()
        };
        // step 4
        if nframes < 0 {
            self.errno = ENOMEM;
            self.error_exit(
                codes::NO_MEMORY,
                format!("MRIalloc: could not allocate {nframes} frame\n").as_bytes(),
            )?;
        }
        let frames = vec![Frame::default(); nframes.max(0) as usize];
        self.reinit_cache(&mut geom)?;
        let mut mri = Mri {
            geom,
            nframes,
            kind,
            bytes_per_vox,
            bytes_total,
            vox_per_row,
            vox_per_slice,
            vox_per_vol,
            vox_total,
            imnr0: 1,
            imnr1: d,
            xstart: -xend,
            ystart: -yend,
            zstart: -zend,
            xend,
            yend,
            zend,
            fov: w as f32,
            thick: 1.0,
            ps: 1.0,
            scale: 1,
            roi: Region {
                dx: w,
                dy: h,
                dz: d,
                ..Region::default()
            },
            version: 1,
            intent: 0,
            dof: 1,
            brightness: 1,
            yinvert: 1,
            ptype: 2,
            outside_val: 0.0,
            flip_angle: 0.0,
            tr: 0.0,
            te: 0.0,
            ti: 0.0,
            location: 0.0,
            field_strength: 0.0,
            mean: 0.0,
            pedir: None,
            bvec_space: 0,
            bvals: None,
            bvecs: None,
            warp_field_format: -1,
            gcamorph_spacing: 1,
            gcamorph_exp_k: 0.0,
            cmdlines: Vec::new(),
            subject_name: Vec::new(),
            path_to_t1: Vec::new(),
            fname_format: Vec::new(),
            gdf_image_stem: Vec::new(),
            transform_fname: Vec::new(),
            auto_align: None,
            orig_ras2vox: None,
            transform: None,
            free_transform: 0,
            ct: None,
            index: false,
            frames: Some(frames),
            voxels: None,
        };
        // step 5
        if !with_voxels {
            return Ok(mri);
        }
        // step 6
        mri.geom.ras_good_flag = 1;
        let buffer = make_buffer(bytes_total);
        let slices = d.wrapping_mul(nframes);
        if slices < 0 {
            self.errno = ENOMEM;
            self.fatal(
                1,
                format!("could not allocate memory for {slices} slices").as_bytes(),
            )?;
        }
        if slices >= 1 && (h < 0 || buffer.is_none()) {
            self.errno = ENOMEM;
            self.fatal(
                1,
                format!("could not allocate memory for slice 1 out of {slices}").as_bytes(),
            )?;
        }
        mri.voxels = Some(buffer.unwrap_or_default());
        // step 7
        for e in [w, h, d] {
            if e + 1000 < 0 {
                self.errno = ENOMEM;
                self.error_exit(
                    codes::NO_MEMORY,
                    format!("could not allocate {} elt index array\n", e + 1000).as_bytes(),
                )?;
            }
        }
        mri.index = true;
        Ok(mri)
    }

    /// The volume made from a geometry (`MRI(vg, type, nframes, HeaderOnly)`, §4): its extent,
    /// then `ras_good_flag = 1`, its voxel sizes, cosines, centre and file name; the cache
    /// recomputed.
    pub fn mri_from_vol_geom(
        &mut self,
        vg: &VolGeom,
        kind: i32,
        nframes: i32,
        header_only: bool,
    ) -> Result<Mri, Stop> {
        let mut m = self.mri_alloc(vg.width, vg.height, vg.depth, kind, nframes, !header_only)?;
        self.use_vol_geom(vg, &mut m)?;
        Ok(m)
    }

    /// `useVolGeomToMRI(vg, mri)` (§4): the extent, `ras_good_flag = 1`, the voxel sizes,
    /// cosines, centre and file name; the cache recomputed.
    pub fn use_vol_geom(&mut self, vg: &VolGeom, m: &mut Mri) -> Result<(), Stop> {
        let g = &mut m.geom;
        g.width = vg.width;
        g.height = vg.height;
        g.depth = vg.depth;
        g.ras_good_flag = 1;
        g.xsize = vg.xsize;
        g.ysize = vg.ysize;
        g.zsize = vg.zsize;
        g.x = vg.x;
        g.y = vg.y;
        g.z = vg.z;
        g.c = vg.c;
        g.fname = vg.fname.clone();
        self.reinit_cache(&mut m.geom)?;
        Ok(())
    }

    /// `MRIfree(&mri)` (§4): 0, the caller left without it; a volume the caller no longer has:
    /// the C-style error `MRIfree: null pointer` and -4.
    pub fn mri_free(&mut self, m: &mut Option<Mri>) -> i32 {
        if m.take().is_none() {
            self.error_printf(codes::BAD_PARM, b"MRIfree: null pointer\n");
            return codes::BAD_PARM;
        }
        0
    }

    /// FS-264's refusal (both modes): a matrix that is not at least 4 x 4.
    pub(crate) fn not_4x4(&mut self, function: &str, m: &Matrix) -> Result<(), Stop> {
        if m.rows >= 4 && m.cols >= 4 {
            return Ok(());
        }
        self.note(
            &FS_264,
            format!(
                "{function} given a {} x {} matrix: refused (FreeSurfer addresses it as 4 x 4)",
                m.rows, m.cols
            ),
        );
        self.error_exit(
            codes::BAD_PARM,
            format!("{function}: the matrix is not 4 x 4").as_bytes(),
        )
    }

    /// `MRImatrixOfDirectionCosines(mri, m)` (§3): the cosines as columns 1-3 and (4, 4) = 1,
    /// into `m` (other elements kept) or a new matrix of zeros.
    pub fn matrix_of_direction_cosines(
        &mut self,
        g: &VolGeom,
        given: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        self.fill_helper("MRImatrixOfDirectionCosines", given, |m| {
            for j in 0..3 {
                for (r, &x) in g.cosine(j).iter().enumerate() {
                    m.set(r as i32 + 1, j as i32 + 1, x);
                }
            }
            m.set(4, 4, 1.0);
        })
    }

    /// `MRImatrixOfVoxelSizes(mri, m)` (§3): the sizes on the diagonal, (4, 4) = 1.
    pub fn matrix_of_voxel_sizes(
        &mut self,
        g: &VolGeom,
        given: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        self.fill_helper("MRImatrixOfVoxelSizes", given, |m| {
            for j in 0..3 {
                m.set(j as i32 + 1, j as i32 + 1, g.size(j));
            }
            m.set(4, 4, 1.0);
        })
    }

    /// `MRImatrixOfTranslations(mri, m)` (§3): the scanner matrix's last column, 1 on the whole
    /// diagonal.
    pub fn matrix_of_translations(
        &mut self,
        g: &VolGeom,
        given: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        let s = self.extract_i_to_r(g)?;
        self.fill_helper("MRImatrixOfTranslations", given, |m| {
            for r in 1..=3 {
                m.set(r, 4, s.at(r, 4));
            }
            for k in 1..=4 {
                m.set(k, k, 1.0);
            }
        })
    }

    /// §3's helpers' output: a given matrix (FS-264 when it is not at least 4 x 4), filled in
    /// place, or a new 4 x 4 of zeros.
    fn fill_helper(
        &mut self,
        function: &str,
        given: Option<&mut Matrix>,
        fill: impl FnOnce(&mut Matrix),
    ) -> Result<Returned<Matrix>, Stop> {
        if let Some(g) = given {
            self.not_4x4(function, g)?;
            fill(g);
            return Ok(Returned::Output);
        }
        let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
        fill(&mut m);
        Ok(Returned::New(m))
    }

    /// `MRIsetVox2RASFromMatrix(mri, m)` (§3): the cosines from the columns (divided by their
    /// lengths), the centre from the last column; the sizes kept (a warning when they differ
    /// from the lengths by more than 0.001); returns 0.
    pub fn set_vox2ras_from_matrix(&mut self, g: &mut VolGeom, m: &Matrix) -> Result<i32, Stop> {
        self.not_4x4("MRIsetVox2RASFromMatrix", m)?;
        let mut len = [0.0f64; 3];
        for (j, l) in len.iter_mut().enumerate() {
            let c = j as i32 + 1;
            let (x, y, z) = (
                sse::widen(m.at(1, c)),
                sse::widen(m.at(2, c)),
                sse::widen(m.at(3, c)),
            );
            let s = sse::add64(
                sse::add64(sse::mul64(x, x), sse::mul64(y, y)),
                sse::mul64(z, z),
            );
            *l = sse::sqrt64(s);
        }
        let sizes = [g.xsize, g.ysize, g.zsize];
        let off = (0..3).any(|j| (sse::widen(sizes[j]) - len[j]).abs() > 0.001);
        if off {
            let mut b =
                b"WARNING: MRIsetRas2VoxFromMatrix(): voxels sizes are inconsistent\n   ".to_vec();
            for j in 0..3 {
                b.push(b'(');
                b.extend_from_slice(&sprintf_g(sse::widen(sizes[j])));
                b.push(b',');
                b.extend_from_slice(&sprintf_g(len[j]));
                b.extend_from_slice(b") ");
            }
            b.extend_from_slice(
                b"\nThis is probably due to shear in the vox2ras matrix\nInput Vox2RAS ------\n",
            );
            self.put_out(&b);
            self.matrix_print(Some(false), Some(m), 5);
        }
        for (j, &l) in len.iter().enumerate() {
            let c = j as i32 + 1;
            let v = [
                sse::narrow(sse::div64(sse::widen(m.at(1, c)), l)),
                sse::narrow(sse::div64(sse::widen(m.at(2, c)), l)),
                sse::narrow(sse::div64(sse::widen(m.at(3, c)), l)),
            ];
            match j {
                0 => g.x = v,
                1 => g.y = v,
                _ => g.z = v,
            }
        }
        let (r0, a0, s0) = (
            sse::widen(m.at(1, 4)),
            sse::widen(m.at(2, 4)),
            sse::widen(m.at(3, 4)),
        );
        self.p0_to_cras(g, r0, a0, s0)?;
        Ok(0)
    }

    /// `MRIp0ToCRAS(mri, r0, a0, s0)` (§3): the centre from the first voxel's position; the cache
    /// recomputed; returns 0.
    pub fn p0_to_cras(&mut self, g: &mut VolGeom, r0: f64, a0: f64, s0: f64) -> Result<i32, Stop> {
        let mut m = self.extract_i_to_r(g)?;
        m.set(1, 4, sse::narrow(r0));
        m.set(2, 4, sse::narrow(a0));
        m.set(3, 4, sse::narrow(s0));
        let mut p = Matrix::zeros(4, 1, MATRIX_REAL);
        for j in 0..3 {
            p.set(j as i32 + 1, 1, sse::narrow(f64::from(g.extent(j)) / 2.0));
        }
        p.set(4, 1, 1.0);
        if let Returned::New(c) = self.matrix_multiply(Some(&m), Some(&p), None)? {
            g.c = [c.at(1, 1), c.at(2, 1), c.at(3, 1)];
        }
        self.reinit_cache(g)?;
        Ok(0)
    }

    /// `MRIcopyPulseParameters(src, dst)` (§6): flip angle, TR, TE, TI; returns 0.
    pub fn copy_pulse_parameters(&mut self, src: &Mri, dst: &mut Mri) -> i32 {
        dst.flip_angle = src.flip_angle;
        dst.tr = src.tr;
        dst.te = src.te;
        dst.ti = src.ti;
        0
    }

    /// `MRIcopyVolGeomToMRI(mri, vg)` (§6): sizes, cosines and centre (no extent, no name, the
    /// cache not recomputed); returns 0.
    pub fn copy_vol_geom_to_mri(&mut self, m: &mut Mri, vg: &VolGeom) -> i32 {
        let g = &mut m.geom;
        g.xsize = vg.xsize;
        g.ysize = vg.ysize;
        g.zsize = vg.zsize;
        g.x = vg.x;
        g.y = vg.y;
        g.z = vg.z;
        g.c = vg.c;
        0
    }

    /// `MRIcopyVolGeomFromMRI(mri, vg)` (§6): sizes, cosines, centre, extent, name and
    /// `valid = ras_good_flag`; returns 0.
    pub fn copy_vol_geom_from_mri(&mut self, m: &Mri, vg: &mut VolGeom) -> i32 {
        let g = &m.geom;
        vg.xsize = g.xsize;
        vg.ysize = g.ysize;
        vg.zsize = g.zsize;
        vg.x = g.x;
        vg.y = g.y;
        vg.z = g.z;
        vg.c = g.c;
        vg.width = g.width;
        vg.height = g.height;
        vg.depth = g.depth;
        vg.fname = fixed_name(&g.fname, 1024);
        vg.valid = g.ras_good_flag;
        0
    }

    /// `shearless_components` (§11): with a shear above 1e-5, the geometry set from its sheared
    /// scanner matrix and the shears set to 0.
    pub fn shearless_components(&mut self, g: &mut VolGeom) -> Result<(), Stop> {
        if !g.has_shear() {
            return Ok(());
        }
        let m = self.vox2ras_scanner(g, 0, true)?;
        self.set_vox2ras_from_matrix(g, &m)?;
        g.s = [0.0; 3];
        Ok(())
    }
}

/// `%g` of a double.
fn sprintf_g(v: f64) -> Vec<u8> {
    crate::clib::printf::sprintf_f64(b"%g", v)
}
