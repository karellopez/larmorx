// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The GCA atlas's structure (`include/gca.h`: `GCA`, `GCA_NODE`, `GCA_PRIOR`, `GC1D`), its
//! allocation (`gcaAllocMax`), the helper volumes and matrices (`GCAsetup`, `GCAcopyDCToMRI`) and
//! the grid conversions (`gcaNodeToPrior`, `GCApriorToNode`, `GCAvoxelToPrior`, `GCAvoxelToNode`,
//! `getPrior`), from `utils/gca.cpp` of FreeSurfer 8.2.0 (queue items 3.3-01 and 3.3-02, taken for
//! reading atlases as volumes, `readGCA`).
//!
//! The grids are stored x-major (x, then y, then z), the order the file holds them; a grid holds
//! only the nodes or priors read so far (an atlas whose file ended keeps the rest empty, as the
//! allocation's zeros): [`Gca::node`] and [`Gca::prior`] give an empty one past them. `GCAfree`,
//! `GCANfree`, `free_gcs` and `GCAcleanup` are Rust's drop.

use std::io::Write;

use larmorx_core::defects::FS_754;
use larmorx_freesurfer_core::affine::{
    AffineMatrix, AffineVector, affine_mm, affine_mv, get_affine_vector, set_affine_matrix,
    set_affine_vector,
};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NOMEMORY, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::sse::{add64, div, mul};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::ctab::ColorTable;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc_header};
use larmorx_freesurfer_mri::vol_geom::{extract_i_to_r, extract_r_to_i, reinit_cache};

/// `GCA_UCHAR_VERSION`: labels stored as bytes.
pub const GCA_UCHAR_VERSION: f64 = 4.0;
/// `GCA_INT_VERSION`: labels stored as `int`s (the version `GCAwrite` writes).
pub const GCA_INT_VERSION: f64 = 5.0;
/// `MAX_GCA_INPUTS`: the room for the MR parameters of each input.
pub const MAX_GCA_INPUTS: usize = 100;
/// `GCA_NORMAL`, `GCA_FLASH`, `GCA_PARAM`, `GCA_UNKNOWN`: the atlas's type.
pub const GCA_NORMAL: i32 = 0;
pub const GCA_FLASH: i32 = 1;
pub const GCA_PARAM: i32 = 2;
pub const GCA_UNKNOWN: i32 = 99;
/// `GCA_NO_MRF`: the atlas has no neighbour (Markov random field) statistics.
pub const GCA_NO_MRF: i32 = 0x0001;
/// `GIBBS_NEIGHBORS`: the six neighbours of a node.
pub const GIBBS_NEIGHBORS: usize = 6;
/// The tags of the file's tagged section (`FILE_TAG` begins it).
pub const FILE_TAG: i32 = 0xab2c;
pub const TAG_PARAMETERS: i32 = 0x0001;
pub const TAG_GCA_TYPE: i32 = 0x0002;
pub const TAG_GCA_DIRCOS: i32 = 0x0003;
pub const TAG_GCA_COLORTABLE: i32 = 0x0004;
/// `VERY_UNLIKELY`, as `getPrior` returns it (a `double` constant made a `float`).
pub const VERY_UNLIKELY: f32 = 1e-10;

/// `GC1D`: a class's Gaussian (one mean per input, the covariance's upper triangle) and, unless
/// the atlas has no neighbour statistics, the labels and priors of its six neighbours. Laid out
/// compactly (an atlas holds about a million classes): the lists one after another.
#[derive(Clone, Debug, Default)]
pub struct Gc1d {
    pub means: Box<[f32]>,
    pub covars: Box<[f32]>,
    /// `nlabels`, `labels`, `label_priors`: `None` without neighbour statistics (`GCA_NO_MRF`).
    pub nbrs: Option<Box<Nbrs>>,
    pub ntraining: i32,
    pub n_just_priors: i16,
    pub regularized: i8,
}

/// The six neighbours' label lists of a class: `nlabels[i]` (`short`), and the labels and priors
/// of neighbour 0, then 1, ... (a list the file cut is shorter than its count).
#[derive(Clone, Debug, Default)]
pub struct Nbrs {
    pub nlabels: [i16; GIBBS_NEIGHBORS],
    pub labels: Box<[u16]>,
    pub label_priors: Box<[f32]>,
}

impl Nbrs {
    /// Neighbour `i`'s labels and priors.
    pub fn of(&self, i: usize) -> (&[u16], &[f32]) {
        let n = self.labels.len();
        let start: usize = self.nlabels[..i]
            .iter()
            .map(|&c| c.max(0) as usize)
            .sum::<usize>()
            .min(n);
        let end = (start + self.nlabels[i].max(0) as usize).min(n);
        (&self.labels[start..end], &self.label_priors[start..end])
    }
}

/// `GCA_NODE`: the classes at one node of the node grid.
#[derive(Clone, Debug, Default)]
pub struct GcaNode {
    pub labels: Box<[u16]>,
    pub gcs: Box<[Gc1d]>,
    pub nlabels: i32,
    pub max_labels: i32,
    pub total_training: i32,
}

/// `GCA_PRIOR`: the label priors at one point of the prior grid.
#[derive(Clone, Debug, Default)]
pub struct GcaPrior {
    pub labels: Box<[u16]>,
    pub priors: Box<[f32]>,
    pub total_training: i32,
    pub nlabels: i16,
    pub max_labels: i16,
}

static EMPTY_NODE: std::sync::LazyLock<GcaNode> = std::sync::LazyLock::new(GcaNode::default);
static EMPTY_PRIOR: std::sync::LazyLock<GcaPrior> = std::sync::LazyLock::new(GcaPrior::default);

/// `GCA` (`GAUSSIAN_CLASSIFIER_ARRAY`). `tissue_parms` is not kept: no function ported here
/// reads or writes it (`GCAread` leaves it zero).
#[derive(Default)]
pub struct Gca {
    pub node_spacing: f32,
    pub prior_spacing: f32,
    pub node_width: i32,
    pub node_height: i32,
    pub node_depth: i32,
    /// The nodes read so far, x-major.
    pub nodes: Vec<GcaNode>,
    pub prior_width: i32,
    pub prior_height: i32,
    pub prior_depth: i32,
    /// The priors read so far, x-major.
    pub priors: Vec<GcaPrior>,
    pub ninputs: i32,
    pub flags: i32,
    /// `type`.
    pub gtype: i32,
    /// `TRs`, `FAs`, `TEs`: room for `MAX_GCA_INPUTS` each (more when the atlas's file holds the
    /// parameters of more inputs: FS-752).
    pub trs: Vec<f64>,
    pub fas: Vec<f64>,
    pub tes: Vec<f64>,
    pub x_r: f32,
    pub x_a: f32,
    pub x_s: f32,
    pub y_r: f32,
    pub y_a: f32,
    pub y_s: f32,
    pub z_r: f32,
    pub z_a: f32,
    pub z_s: f32,
    pub c_r: f32,
    pub c_a: f32,
    pub c_s: f32,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    pub xsize: f32,
    pub ysize: f32,
    pub zsize: f32,
    /// `mri_node__`, `mri_prior__`, `mri_tal__`: headers of the three grids.
    pub mri_node: Option<Mri>,
    pub mri_prior: Option<Mri>,
    pub mri_tal: Option<Mri>,
    pub node_i_to_r: Option<Matrix>,
    pub node_r_to_i: Option<Matrix>,
    pub prior_i_to_r: Option<Matrix>,
    pub prior_r_to_i: Option<AffineMatrix>,
    pub tal_i_to_r: Option<Matrix>,
    pub tal_r_to_i: Option<Matrix>,
    pub tmp: Option<Matrix>,
    pub total_training: i32,
    pub max_label: i32,
    pub ct: Option<ColorTable>,
}

/// A grid's flat index of `(x, y, z)` (`None` outside it).
fn index(w: i32, h: i32, d: i32, x: i32, y: i32, z: i32) -> Option<usize> {
    if x < 0 || y < 0 || z < 0 || x >= w || y >= h || z >= d {
        return None;
    }
    let i = ((x as u128 * h as u128) + y as u128) * d as u128 + z as u128;
    usize::try_from(i).ok()
}

impl Gca {
    /// `&gca->nodes[x][y][z]` (inside the grid).
    pub fn node(&self, x: i32, y: i32, z: i32) -> &GcaNode {
        index(self.node_width, self.node_height, self.node_depth, x, y, z)
            .and_then(|i| self.nodes.get(i))
            .unwrap_or(&EMPTY_NODE)
    }

    /// `&gca->priors[x][y][z]`, or `None` outside the prior grid (FreeSurfer reads outside it).
    pub fn prior(&self, x: i32, y: i32, z: i32) -> Option<&GcaPrior> {
        index(
            self.prior_width,
            self.prior_height,
            self.prior_depth,
            x,
            y,
            z,
        )
        .map(|i| self.priors.get(i).unwrap_or(&EMPTY_PRIOR))
    }
}

/// `(int) x` of a value from a `float` or `double` (x86-64's conversion; FS-17 beyond `int`).
pub fn to_int(g: &mut Globals, x: f64, what: &str) -> i32 {
    larmorx_freesurfer_core::affine::float_to_int(g, x, what)
}

/// `(int)(((float)width / spacing) + .99)`: a grid's size along one axis.
pub fn grid_size(g: &mut Globals, width: i32, spacing: f32) -> i32 {
    to_int(
        g,
        add64(f64::from(div(width as f32, spacing)), 0.99),
        "gcaAllocMax",
    )
}

/// The sizes `gcaAllocMax(ninputs, prior_spacing, node_spacing, width, height, depth, ..)` gives
/// the node and prior grids: `[node_width, node_height, node_depth, prior_width, prior_height,
/// prior_depth]`.
pub fn grid_sizes(g: &mut Globals, ps: f32, ns: f32, whd: [i32; 3]) -> [i32; 6] {
    [
        grid_size(g, whd[0], ns),
        grid_size(g, whd[1], ns),
        grid_size(g, whd[2], ns),
        grid_size(g, whd[0], ps),
        grid_size(g, whd[1], ps),
        grid_size(g, whd[2], ps),
    ]
}

/// The allocation error of a grid (`calloc` of a negative count: `ENOMEM`).
fn alloc_exit(g: &mut Globals, err: &mut dyn Write, message: &str) -> Result<(), Fatal> {
    g.errno = libc_enomem();
    log::error_exit(g, err, ERROR_NOMEMORY, message)
}

/// Linux's `ENOMEM` (a failed `calloc`).
pub fn libc_enomem() -> i32 {
    12
}

/// `gcaAllocMax(ninputs, prior_spacing, node_spacing, width, height, depth, max_labels, flags)`:
/// the atlas with the default geometry (direction cosines x = (-1, 0, 0), y = (0, 0, -1), z =
/// (0, 1, 0), centre 0, voxel sizes 1), type `GCA_UNKNOWN`, its grids' sizes from `width`,
/// `height`, `depth` and the spacings ([`grid_sizes`]; `grid` replaces them: FS-749's default),
/// every node and prior empty, then [`gca_setup`]. A grid of a negative size ends the program as
/// FreeSurfer's allocation fails (`GCAalloc: could not allocate nodes`, `... <x>th **`, `...
/// <x>,<y>th *`, and the same for the priors).
#[allow(clippy::too_many_arguments)]
pub fn gca_alloc_max(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ninputs: i32,
    prior_spacing: f32,
    node_spacing: f32,
    whd: [i32; 3],
    max_labels: i32,
    flags: i32,
    grid: Option<[i32; 6]>,
) -> Result<Gca, Fatal> {
    let mut gca = Gca {
        ninputs,
        prior_spacing,
        node_spacing,
        gtype: GCA_UNKNOWN,
        x_r: -1.0,
        z_a: 1.0,
        y_s: -1.0,
        xsize: 1.0,
        ysize: 1.0,
        zsize: 1.0,
        width: whd[0],
        height: whd[1],
        depth: whd[2],
        trs: vec![0.0; MAX_GCA_INPUTS],
        fas: vec![0.0; MAX_GCA_INPUTS],
        tes: vec![0.0; MAX_GCA_INPUTS],
        flags,
        ..Gca::default()
    };
    let s = match grid {
        Some(s) => s,
        None => grid_sizes(g, prior_spacing, node_spacing, whd),
    };
    [gca.node_width, gca.node_height, gca.node_depth] = [s[0], s[1], s[2]];
    [gca.prior_width, gca.prior_height, gca.prior_depth] = [s[3], s[4], s[5]];
    if max_labels >= 0 {
        for (dims, what) in [(&s[..3], "nodes"), (&s[3..], "priors")] {
            if dims[0] < 0 {
                alloc_exit(g, err, &format!("GCAalloc: could not allocate {what}"))?;
            }
            if dims[0] > 0 && dims[1] < 0 {
                alloc_exit(g, err, "GCAalloc: could not allocate 0th **")?;
            }
            if dims[0] > 0 && dims[1] > 0 && dims[2] < 0 {
                alloc_exit(g, err, "GCAalloc: could not allocate 0,0th *")?;
            }
        }
    }
    gca_setup(g, out, err, &mut gca)?;
    Ok(gca)
}

/// `GCAcopyDCToMRI(gca, mri)`: the atlas's direction cosines and centre into `mri`, RAS good,
/// the cached matrices recomputed (`MRIreInitCache`), then the voxel-to-RAS matrix and its
/// inverse computed again.
pub fn gca_copy_dc_to_mri(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    gca: &Gca,
    mri: &mut Mri,
) -> Result<(), Fatal> {
    let v = &mut mri.vg;
    v.x_r = gca.x_r;
    v.y_r = gca.y_r;
    v.z_r = gca.z_r;
    v.x_a = gca.x_a;
    v.y_a = gca.y_a;
    v.z_a = gca.z_a;
    v.x_s = gca.x_s;
    v.y_s = gca.y_s;
    v.z_s = gca.z_s;
    v.c_r = gca.c_r;
    v.c_a = gca.c_a;
    v.c_s = gca.c_s;
    v.ras_good_flag = 1;
    reinit_cache(g, out, err, &mut mri.vg)?;
    let tmp = extract_i_to_r(g, out, err, &mri.vg)?;
    let mut am = AffineMatrix::default();
    set_affine_matrix(err, &mut am, &tmp)?;
    mri.vg.i_to_r = Some(am);
    mri.vg.r_to_i = extract_r_to_i(g, out, err, &mri.vg)?;
    Ok(())
}

/// A grid's header (`MRIallocHeader(w, h, d, MRI_UCHAR, 1)`) with voxel sizes `xsize *
/// spacing` (`spacing` `None`: the atlas's own) and the atlas's geometry.
fn grid_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    gca: &Gca,
    whd: [i32; 3],
    spacing: Option<f32>,
) -> Result<Mri, Fatal> {
    let mut m = mri_alloc_header(g, out, err, whd[0], whd[1], whd[2], MRI_UCHAR, 1)?;
    match spacing {
        Some(s) => {
            m.vg.xsize = mul(gca.xsize, s);
            m.vg.ysize = mul(gca.ysize, s);
            m.vg.zsize = mul(gca.zsize, s);
        }
        None => {
            m.vg.xsize = gca.xsize;
            m.vg.ysize = gca.ysize;
            m.vg.zsize = gca.zsize;
        }
    }
    gca_copy_dc_to_mri(g, out, err, gca, &mut m)?;
    Ok(m)
}

/// `GCAsetup(gca)`: the headers of the node, prior and Talairach grids (each with the atlas's
/// geometry and its voxel sizes times its spacing) and their voxel-to-RAS matrices and inverses.
/// FreeSurfer crashes when the prior grid's matrix has no inverse (FS-754): by default an error
/// that ends the program (`ERROR_BADPARM`'s status, 252); `reproduce` ends as FreeSurfer's crash
/// (status 139).
pub fn gca_setup(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    gca: &mut Gca,
) -> Result<(), Fatal> {
    gca.mri_node = None;
    let node = grid_header(
        g,
        out,
        err,
        gca,
        [gca.node_width, gca.node_height, gca.node_depth],
        Some(gca.node_spacing),
    )?;
    gca.mri_node = Some(node);
    gca.mri_prior = None;
    let prior = grid_header(
        g,
        out,
        err,
        gca,
        [gca.prior_width, gca.prior_height, gca.prior_depth],
        Some(gca.prior_spacing),
    )?;
    gca.mri_prior = Some(prior);
    gca.mri_tal = None;
    let tal = grid_header(g, out, err, gca, [gca.width, gca.height, gca.depth], None)?;
    gca.mri_tal = Some(tal);
    let node = gca.mri_node.as_ref().expect("set above");
    gca.node_i_to_r = Some(extract_i_to_r(g, out, err, &node.vg)?);
    gca.node_r_to_i = extract_r_to_i(g, out, err, &node.vg)?;
    let prior = gca.mri_prior.as_ref().expect("set above");
    gca.prior_i_to_r = Some(extract_i_to_r(g, out, err, &prior.vg)?);
    let Some(tmp) = extract_r_to_i(g, out, err, &prior.vg)? else {
        g.notices.add(
            &FS_754,
            "GCAsetup: the prior grid's voxel-to-RAS matrix has no inverse (FreeSurfer crashes)",
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "GCAsetup: the atlas's prior grid has a voxel-to-RAS matrix with no inverse",
        )?;
        return Ok(());
    };
    let mut am = AffineMatrix::default();
    set_affine_matrix(err, &mut am, &tmp)?;
    gca.prior_r_to_i = Some(am);
    let tal = gca.mri_tal.as_ref().expect("set above");
    gca.tal_i_to_r = Some(extract_i_to_r(g, out, err, &tal.vg)?);
    gca.tal_r_to_i = extract_r_to_i(g, out, err, &tal.vg)?;
    gca.tmp = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?;
    Ok(())
}

/// `gcaNodeToPrior(gca, xn, yn, zn, ..)`: `(int)((double)xn * node_spacing / prior_spacing)` for
/// each axis (no bounds check).
pub fn gca_node_to_prior(g: &mut Globals, gca: &Gca, n: [i32; 3]) -> [i32; 3] {
    let f = |g: &mut Globals, v: i32| {
        to_int(
            g,
            f64::from(v) * f64::from(gca.node_spacing) / f64::from(gca.prior_spacing),
            "gcaNodeToPrior",
        )
    };
    [f(g, n[0]), f(g, n[1]), f(g, n[2])]
}

/// `GCApriorToNode(gca, xp, yp, zp, ..)`: `(int)((double)xp * prior_spacing / node_spacing)`.
pub fn gca_prior_to_node(g: &mut Globals, gca: &Gca, p: [i32; 3]) -> [i32; 3] {
    let f = |g: &mut Globals, v: i32| {
        to_int(
            g,
            f64::from(v) * f64::from(gca.prior_spacing) / f64::from(gca.node_spacing),
            "GCApriorToNode",
        )
    };
    [f(g, p[0]), f(g, p[1]), f(g, p[2])]
}

/// `boundsCheck(&x, &y, &z, mri)`: `NO_ERROR` inside `mri`'s extent, else `ERROR_BADPARM`.
fn bounds_check(c: [i32; 3], mri: &Mri) -> i32 {
    if c[0] < 0
        || c[1] < 0
        || c[2] < 0
        || c[0] > mri.vg.width - 1
        || c[1] > mri.vg.height - 1
        || c[2] > mri.vg.depth - 1
    {
        ERROR_BADPARM
    } else {
        NO_ERROR
    }
}

/// `AffineMM(&voxelToPrior, gca->prior_r_to_i__, mri->i_to_r__)`: the map of `mri`'s voxels to
/// the prior grid (the same for every voxel of a volume: computed once).
pub fn voxel_to_prior_matrix(gca: &Gca, mri: &Mri) -> AffineMatrix {
    let p = gca.prior_r_to_i.as_ref().expect("GCAsetup computed it");
    let i = mri
        .vg
        .i_to_r
        .as_ref()
        .expect("the volume's cache is filled");
    affine_mm(p, i)
}

/// `GCAvoxelToPrior(gca, mri, xv, yv, zv, ..)` with [`voxel_to_prior_matrix`]'s `m`: the voxel
/// mapped (`AffineMV` in `float`), each coordinate rounded with `nint`, clamped below the prior
/// grid's size, and the bounds checked against the prior grid; the code and the coordinates.
pub fn gca_voxel_to_prior(
    g: &mut Globals,
    gca: &Gca,
    m: &AffineMatrix,
    v: [i32; 3],
) -> (i32, [i32; 3]) {
    let mut vv = AffineVector::default();
    set_affine_vector(&mut vv, v[0] as f32, v[1] as f32, v[2] as f32);
    let pv = affine_mv(m, &vv);
    let (x, y, z) = get_affine_vector(&pv);
    let mut p = [
        nint(g, f64::from(x)),
        nint(g, f64::from(y)),
        nint(g, f64::from(z)),
    ];
    if p[0] >= gca.prior_width {
        p[0] = gca.prior_width - 1;
    }
    if p[1] >= gca.prior_height {
        p[1] = gca.prior_height - 1;
    }
    if p[2] >= gca.prior_depth {
        p[2] = gca.prior_depth - 1;
    }
    let mp = gca.mri_prior.as_ref().expect("GCAsetup made it");
    (bounds_check(p, mp), p)
}

/// `GCAvoxelToNode(gca, mri, xv, yv, zv, ..)`: [`gca_voxel_to_prior`] (its code ignored), then
/// [`gca_prior_to_node`], the bounds checked against the node grid.
pub fn gca_voxel_to_node(
    g: &mut Globals,
    gca: &Gca,
    m: &AffineMatrix,
    v: [i32; 3],
) -> (i32, [i32; 3]) {
    let (_, p) = gca_voxel_to_prior(g, gca, m, v);
    gca_prior_point_to_node(g, gca, p)
}

/// The second half of [`gca_voxel_to_node`]: the node of a prior point (as `gca_voxel_to_prior`
/// gave it), the bounds checked against the node grid.
pub fn gca_prior_point_to_node(g: &mut Globals, gca: &Gca, p: [i32; 3]) -> (i32, [i32; 3]) {
    let n = gca_prior_to_node(g, gca, p);
    let mn = gca.mri_node.as_ref().expect("GCAsetup made it");
    (bounds_check(n, mn), n)
}

/// `getPrior(gcap, label)`: the label's prior; for a label the point has not seen `0.1f /
/// total_training` (`VERY_UNLIKELY` without training), and `VERY_UNLIKELY` for a prior within
/// `1e-15` of 0.
pub fn get_prior(gcap: &GcaPrior, label: i32) -> f32 {
    let n = gcap
        .labels
        .iter()
        .take(gcap.nlabels.max(0) as usize)
        .position(|&l| i32::from(l) == label);
    let Some(n) = n else {
        if gcap.total_training > 0 {
            return div(0.1f32, gcap.total_training as f32);
        }
        return VERY_UNLIKELY;
    };
    if f64::from(gcap.priors[n]).abs() < 1e-15 {
        return VERY_UNLIKELY;
    }
    gcap.priors[n]
}

/// `GCAsourceVoxelToPrior(gca, mri, transform, xv, yv, zv, ..)` (`utils/gca.cpp` 1052-1127,
/// `USE_ROUND` 1; track D, queue item 3.2-24, for `mri_normalize -atlas`): the source voxel to
/// the atlas's template volume (a voxel-to-voxel linear transform: `TransformWithMatrix` of its
/// first matrix, each coordinate then narrowed to `float`; a morph: `TransformSample`; any other
/// type ends the program, `GCAsourceVoxelToPrior: needs vox-to-vox transform`), then to the prior
/// grid as `GCAvoxelToPriorReal` maps it (`tal`: [`voxel_to_prior_matrix`] of `mri_tal__`, the
/// same for every voxel) with each coordinate rounded by `nint`. The code is `ERROR_BADPARM` when
/// the point falls outside the prior grid, and the point is clamped into it.
#[allow(clippy::too_many_arguments)]
pub fn gca_source_voxel_to_prior(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &larmorx_freesurfer_core::run::Environment,
    gca: &Gca,
    tal: &AffineMatrix,
    transform: &mut crate::transform::Transform,
    v: [i32; 3],
) -> Result<(i32, [i32; 3]), Fatal> {
    use larmorx_freesurfer_mri::lta::{LINEAR_VOX_TO_VOX, MORPH_3D_TYPE};
    let (xt, yt, zt) = if transform.ttype != MORPH_3D_TYPE {
        if transform.ttype == LINEAR_VOX_TO_VOX {
            let lta = transform.lta();
            let m = lta.xforms[0]
                .m_l
                .as_ref()
                .expect("a linear transform read from a file has its matrix");
            let p = larmorx_freesurfer_volume::talairachex::transform_with_matrix(
                m,
                f64::from(v[0]),
                f64::from(v[1]),
                f64::from(v[2]),
            );
            (p[0] as f32, p[1] as f32, p[2] as f32)
        } else {
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                "GCAsourceVoxelToPrior: needs vox-to-vox transform",
            )?;
            (0.0, 0.0, 0.0)
        }
    } else {
        let (_, p) = crate::transform::transform_sample(
            g, out, err, env, transform, v[0] as f32, v[1] as f32, v[2] as f32,
        )?;
        (p[0], p[1], p[2])
    };
    let mut vv = AffineVector::default();
    set_affine_vector(&mut vv, xt, yt, zt);
    let pv = affine_mv(tal, &vv);
    let (x, y, z) = get_affine_vector(&pv);
    let mut p = [
        nint(g, f64::from(x)),
        nint(g, f64::from(y)),
        nint(g, f64::from(z)),
    ];
    let dims = [gca.prior_width, gca.prior_height, gca.prior_depth];
    let mut code = NO_ERROR;
    for k in 0..3 {
        if p[k] < 0 || p[k] >= dims[k] {
            code = ERROR_BADPARM;
        }
    }
    for k in 0..3 {
        if p[k] < 0 {
            p[k] = 0;
        }
        if p[k] >= dims[k] {
            p[k] = dims[k] - 1;
        }
    }
    Ok((code, p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn grids_of_the_official_spacings() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        // the official atlases: 256 voxels, nodes every 4, priors every 2
        assert_eq!(
            grid_sizes(&mut g, 2.0, 4.0, [256, 256, 256]),
            [64, 64, 64, 128, 128, 128]
        );
        // a width that is not a multiple of the node spacing rounds up
        assert_eq!(grid_size(&mut g, 253, 4.0), 64);
        assert_eq!(grid_size(&mut g, 253, 2.0), 127);
        assert!(g.notices.as_slice().is_empty());
    }

    #[test]
    fn priors_of_unseen_labels() {
        let p = GcaPrior {
            nlabels: 2,
            labels: Box::new([3, 7]),
            priors: Box::new([0.25, 0.0]),
            total_training: 4,
            ..GcaPrior::default()
        };
        assert_eq!(get_prior(&p, 3), 0.25);
        assert_eq!(get_prior(&p, 7), VERY_UNLIKELY);
        assert_eq!(get_prior(&p, 9), 0.1f32 / 4.0);
        let none = GcaPrior::default();
        assert_eq!(get_prior(&none, 9), VERY_UNLIKELY);
    }
}
