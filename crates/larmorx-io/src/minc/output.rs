// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
//! volume_io's MINC output (`Volumes/output_volume.c`, `output_mnc.c`, `volumes.c`; MNI's
//! notice, `LICENSES/LicenseRef-MNI.txt`): `output_volume` and what it calls. The file gets the
//! root variable, a dimension per volume dimension (in the volume's order, or the options'), a
//! dimension variable per spatial dimension (its step, start, direction cosines when they are
//! not the axis's own, units `mm` and the space type), `image-min` and `image-max` (one value
//! for the whole volume, or one per slice when no global range is known), the history, then the
//! image itself, written through an image conversion variable that maps the volume's voxel range
//! onto the file's valid range.
//!
//! Not ported: copying another MINC file's auxiliary data (`output_modified_volume` with an
//! original file: the volume is written without it), the volume cache's output file, and the
//! progress report (volume_io prints one when a write has taken more than 5 seconds: a measure
//! of time, not reproduced).

use super::icv::{self as icv, MI_ICV_DO_NORM, MI_ICV_IMAGE_MAX, MI_ICV_IMAGE_MIN};
use super::icv::{MI_ICV_TYPE, MI_ICV_USER_NORM, MI_ICV_VALID_MAX, MI_ICV_VALID_MIN};
use super::libsrc::{self as mi, MI_ERROR, MI_SIGNED, MI_UNSIGNED, Typed};
use super::mwrite::{self as mw, MI_FALSE, MI_TRUE};
use super::ncwrite::{self as ncw, NC_CLOBBER};
use super::netcdf::{self as nc, NC_DOUBLE, NC_FLOAT, NC_GLOBAL, Values};
use super::volume::{
    self as vol, DataType, MAX_DIMENSIONS, MI_ORIGINAL_TYPE, MI_UNKNOWN_SPACE, N_DIMENSIONS,
    Volume, VolumeData,
};
use super::{Ctx, Exit, Session, Status};
use crate::xfm::{GeneralTransform, TransformData, TransformType};

const INVALID_AXIS: i32 = -1;
/// `MIN_SLAB_SIZE` / `MAX_SLAB_SIZE` of `output_mnc.c`.
const MIN_SLAB_SIZE: i32 = 10000;
const MAX_SLAB_SIZE: i32 = 200000;

/// `minc_output_options`.
#[derive(Clone, Debug)]
pub struct MincOutputOptions {
    /// The file's dimension order (`set_minc_output_dimensions_order`); `None` entries are C's
    /// `NULL` (an empty name).
    pub dimension_names: Vec<Option<Vec<u8>>>,
    /// `global_image_range`: unset while `[0] >= [1]`.
    pub global_image_range: [f64; 2],
    pub use_volume_starts_and_steps: bool,
    pub use_starts_set: bool,
}

impl Default for MincOutputOptions {
    /// `set_default_minc_output_options`.
    fn default() -> Self {
        MincOutputOptions {
            dimension_names: vec![None; MAX_DIMENSIONS],
            global_image_range: [0.0, -1.0],
            use_volume_starts_and_steps: false,
            use_starts_set: false,
        }
    }
}

impl MincOutputOptions {
    /// `set_minc_output_dimensions_order`.
    pub fn set_dimensions_order(&mut self, names: &[Vec<u8>]) {
        for (i, n) in names.iter().enumerate().take(MAX_DIMENSIONS) {
            self.dimension_names[i] = Some(n.clone());
        }
    }
    /// `set_minc_output_real_range`.
    pub fn set_real_range(&mut self, real_min: f64, real_max: f64) {
        self.global_image_range = [real_min, real_max];
    }
    /// `set_minc_output_use_volume_starts_and_steps_flag`.
    pub fn set_use_volume_starts_and_steps_flag(&mut self, flag: bool) {
        self.use_volume_starts_and_steps = flag;
        self.use_starts_set = true;
    }
}

/// An open MINC file being written (`minc_file_struct` for output).
#[derive(Clone, Debug)]
pub struct MincOutput {
    pub filename: Vec<u8>,
    pub cdfid: i32,
    pub n_file_dimensions: usize,
    pub dim_names: Vec<Vec<u8>>,
    pub sizes_in_file: Vec<i64>,
    pub indices: Vec<i64>,
    pub to_volume_index: Vec<i32>,
    pub to_file_index: Vec<i32>,
    pub image_dims: Vec<i32>,
    pub dim_ids: Vec<i32>,
    pub img_var_id: i32,
    pub min_id: i32,
    pub max_id: i32,
    pub nc_data_type: i32,
    pub signed_flag: bool,
    pub valid_range: [f64; 2],
    pub image_range: [f64; 2],
    pub end_def_done: bool,
    pub variables_written: bool,
    pub outputting_in_order: bool,
    pub entire_file_written: bool,
    pub minc_icv: i32,
    pub n_slab_dims: usize,
}

/// `convert_dim_name_to_spatial_axis`.
fn spatial(name: &[u8]) -> Option<usize> {
    vol::spatial_axis(nc::cstr(name))
}

fn perr(c: &mut Ctx, t: &str) {
    c.dialect.print_error(t, c.streams);
}

fn perr_bytes(c: &mut Ctx, t: &[u8]) {
    c.dialect.print_error_bytes(t, c.streams);
}

/// `get_dimension_ordering`: the file's dimension of each volume dimension and the reverse, or
/// `None` after volume_io's message.
fn dimension_ordering(
    c: &mut Ctx,
    vol_names: &[Vec<u8>],
    file_names: &[Vec<u8>],
) -> Option<(Vec<i32>, Vec<i32>)> {
    let mut to_volume = vec![-1i32; file_names.len()];
    let mut to_file = vec![-1i32; vol_names.len()];
    let mut n_found = 0;
    for (v, vn) in vol_names.iter().enumerate() {
        for (f, fname) in file_names.iter().enumerate() {
            if to_volume[f] < 0 && nc::cstr(vn) == nc::cstr(fname) {
                to_volume[f] = v as i32;
                to_file[v] = f as i32;
                n_found += 1;
            }
        }
    }
    if n_found != vol_names.len() {
        perr(
            c,
            "Unsuccessful matching of volume and output dimension names.\n",
        );
        return None;
    }
    Some((to_volume, to_file))
}

/// `create_output_dim_names` without an original file: the options' names when they give a first
/// one, else the volume's; with the file's sizes, or `None` after volume_io's message when the
/// names are not the volume's in some order.
fn create_output_dim_names(
    c: &mut Ctx,
    volume: &Volume,
    options: Option<&MincOutputOptions>,
) -> Result<Option<(Vec<Vec<u8>>, Vec<i32>)>, Exit> {
    if let Some(e) = c
        .dialect
        .output_dimension_names(&volume.dimension_names, c.notices, c.streams)
    {
        return Err(e);
    }
    let n = volume.n_dimensions();
    let vol_names = volume.dimension_names_canonical();
    let dim_names: Vec<Vec<u8>> = match options {
        Some(o)
            if o.dimension_names[0]
                .as_ref()
                .is_some_and(|x| !nc::cstr(x).is_empty()) =>
        {
            (0..n)
                .map(|i| o.dimension_names[i].clone().unwrap_or_default())
                .collect()
        }
        _ => vol_names.clone(),
    };
    let mut file_sizes = vec![0i32; n];
    let mut n_found = 0;
    for i in 0..n {
        for j in 0..n {
            if nc::cstr(&dim_names[j]) == nc::cstr(&vol_names[i]) {
                file_sizes[j] = volume.sizes[i];
                n_found += 1;
            }
        }
    }
    if n_found != n {
        perr(c, "create_output_dim_names: dimension name mismatch.\n");
        return Ok(None);
    }
    Ok(Some((dim_names, file_sizes)))
}

/// `is_default_direction_cosine`.
fn is_default_direction_cosine(axis: usize, dir: &[f64; 3]) -> bool {
    (0..N_DIMENSIONS).all(|i| {
        if i == axis {
            dir[i] == 1.0
        } else {
            dir[i] == 0.0
        }
    })
}

/// `convert_transform_origin_to_starts`.
fn origin_to_starts(
    c: &mut Ctx,
    origin: &[f64; 3],
    n_volume_dimensions: usize,
    spatial_axes: &[i32; 3],
    dir_cosines: &[[f64; 3]],
    starts: &mut [f64],
) {
    for s in starts.iter_mut().take(n_volume_dimensions) {
        *s = 0.0;
    }
    let which: Vec<usize> = spatial_axes
        .iter()
        .filter(|&&a| a >= 0)
        .map(|&a| a as usize)
        .collect();
    let dot = |a: &[f64; 3], b: &[f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    match which.len() {
        1 => {
            let o_dot_c = dot(origin, &dir_cosines[which[0]]);
            let c_dot_c = dot(&dir_cosines[which[0]], &dir_cosines[which[0]]);
            if c_dot_c != 0.0 {
                starts[which[0]] = o_dot_c / c_dot_c;
            }
        }
        2 => {
            let (x, y) = (&dir_cosines[which[0]], &dir_cosines[which[1]]);
            let x_dot_x = dot(x, x);
            let x_dot_v = dot(x, origin);
            let x_dot_y = dot(x, y);
            let y_dot_y = dot(y, y);
            let y_dot_v = dot(y, origin);
            let bottom = x_dot_x * y_dot_y - x_dot_y * x_dot_y;
            if bottom != 0.0 {
                starts[which[0]] = (x_dot_v * y_dot_y - x_dot_y * y_dot_v) / bottom;
                starts[which[1]] = (y_dot_v * x_dot_x - x_dot_y * x_dot_v) / bottom;
            }
        }
        3 => {
            let m: Vec<Vec<f64>> = (0..N_DIMENSIONS)
                .map(|i| {
                    (0..N_DIMENSIONS)
                        .map(|j| dir_cosines[which[j]][i])
                        .collect()
                })
                .collect();
            if let Some(sol) = crate::xfm::solve_linear_system(&m, origin) {
                starts[which[0]] = sol[0];
                starts[which[1]] = sol[1];
                starts[which[2]] = sol[2];
            }
        }
        _ => perr(
            c,
            "Invalid number of axes in convert_transform_origin_to_starts\n",
        ),
    }
}

/// `convert_transform_to_starts_and_steps` with no sign wishes: each spatial axis's step (the
/// length of its column, negative when its own component is) and direction cosine, and the starts
/// of the transform's origin.
fn transform_to_starts_and_steps(
    c: &mut Ctx,
    transform: &GeneralTransform,
    n_volume_dimensions: usize,
    spatial_axes: &[i32; 3],
    starts: &mut [f64],
    steps: &mut [f64],
    dir_cosines: &mut [[f64; 3]],
) {
    let TransformData::Linear { forward, .. } = &transform.data else {
        perr(
            c,
            "convert_transform_to_starts_and_steps(): non-linear transform found.\n",
        );
        return;
    };
    let t = &**forward;
    let origin = [t[0][3], t[1][3], t[2][3]];
    let axes = [
        [t[0][0], t[1][0], t[2][0]],
        [t[0][1], t[1][1], t[2][1]],
        [t[0][2], t[1][2], t[2][2]],
    ];
    for s in steps.iter_mut().take(n_volume_dimensions) {
        *s = 1.0;
    }
    for dim in 0..N_DIMENSIONS {
        let axis = spatial_axes[dim];
        if axis >= 0 {
            let a = axis as usize;
            let ax = &axes[dim];
            let mut mag = ax[0] * ax[0] + ax[1] * ax[1] + ax[2] * ax[2];
            if mag <= 0.0 {
                mag = 1.0;
            }
            let mag = mag.sqrt();
            let sign = if ax[dim] < 0.0 { -1.0 } else { 1.0 };
            steps[a] = sign * mag;
            dir_cosines[a] = [ax[0] / steps[a], ax[1] / steps[a], ax[2] / steps[a]];
        }
    }
    origin_to_starts(
        c,
        &origin,
        n_volume_dimensions,
        spatial_axes,
        dir_cosines,
        starts,
    );
}

/// `output_world_transform`: the spatial dimensions' variables with their step, start, direction
/// cosines, units and space type; `false` (`ERROR`) when a variable cannot be created.
fn output_world_transform(
    s: &mut Session,
    c: &mut Ctx,
    file: &mut MincOutput,
    volume: &Volume,
    space_type: &[u8],
    transform: &GeneralTransform,
    use_volume_starts_and_steps: bool,
) -> Result<bool, Exit> {
    let n = file.n_file_dimensions;
    let mut start = vec![0.0f64; n.max(MAX_DIMENSIONS)];
    let mut step = vec![1.0f64; n.max(MAX_DIMENSIONS)];
    let mut dir = vec![[0.0f64; 3]; n.max(MAX_DIMENSIONS)];
    if use_volume_starts_and_steps {
        for d in 0..n {
            if let Some(axis) = spatial(&file.dim_names[d]) {
                let vi = file.to_volume_index[d];
                if vi == INVALID_AXIS {
                    dir[d][axis] = 1.0;
                } else {
                    let vi = vi as usize;
                    start[d] = volume.starts[vi];
                    step[d] = volume.separations[vi];
                    dir[d] = volume.direction_cosines[vi];
                }
            }
        }
    } else if transform.transform_type() != TransformType::Linear {
        perr(
            c,
            "Cannot output null or non-linear transforms.  Using identity.\n",
        );
        for d in 0..n {
            if let Some(axis) = spatial(&file.dim_names[d]) {
                dir[d][axis] = 1.0;
            }
        }
    } else {
        let mut spatial_axes = [INVALID_AXIS; 3];
        for d in 0..n {
            if let Some(axis) = spatial(&file.dim_names[d]) {
                spatial_axes[axis] = d as i32;
            }
        }
        transform_to_starts_and_steps(
            c,
            transform,
            n,
            &spatial_axes,
            &mut start,
            &mut step,
            &mut dir,
        );
    }
    for d in 0..n {
        if let Some(axis) = spatial(&file.dim_names[d]) {
            let name = file.dim_names[d].clone();
            let id = mw::micreate_std_variable(s, c, file.cdfid, &name, NC_DOUBLE, 0, &[])?;
            file.dim_ids[d] = id;
            if id < 0 {
                return Ok(false);
            }
            mw::miattputdbl(s, c, file.cdfid, id, b"step", step[d])?;
            mw::miattputdbl(s, c, file.cdfid, id, b"start", start[d])?;
            if !is_default_direction_cosine(axis, &dir[d]) {
                ncw::ncattput(
                    s,
                    c.streams,
                    file.cdfid,
                    id,
                    b"direction_cosines",
                    NC_DOUBLE,
                    &Values::Double(dir[d].to_vec()),
                )?;
            }
            mw::miattputstr(s, c, file.cdfid, id, b"units", b"mm")?;
            if nc::cstr(space_type) != MI_UNKNOWN_SPACE {
                mw::miattputstr(s, c, file.cdfid, id, b"spacetype", space_type)?;
            }
        } else {
            file.dim_ids[d] = -1;
        }
    }
    Ok(true)
}

/// `initialize_minc_output`: the file created and its header defined (the image itself is
/// defined when the first volume is written), or `None` after volume_io's message.
pub fn initialize_minc_output(
    s: &mut Session,
    c: &mut Ctx,
    filename: &[u8],
    n_dimensions: usize,
    dim_names: Option<&[Vec<u8>]>,
    sizes: &[i32],
    file_nc_data_type: i32,
    file_signed_flag: bool,
    file_voxel_min: f64,
    file_voxel_max: f64,
    voxel_to_world: &GeneralTransform,
    volume: &Volume,
    options: Option<&MincOutputOptions>,
) -> Result<Option<MincOutput>, Exit> {
    let default_options = MincOutputOptions::default();
    let options = options.unwrap_or(&default_options);
    let default_names: Vec<Vec<u8>> =
        vec![b"zspace".to_vec(), b"yspace".to_vec(), b"xspace".to_vec()];
    let dim_names: Vec<Vec<u8>> = match dim_names {
        Some(d) => d.to_vec(),
        None => {
            if n_dimensions != 3 {
                perr(c, "initialize_minc_output: ");
                perr(c, "can't use NULL dim_names except with 3 dimensions.\n");
                return Ok(None);
            }
            default_names
        }
    };
    let (mut nc_type, mut signed, mut vmin, mut vmax) = (
        file_nc_data_type,
        file_signed_flag,
        file_voxel_min,
        file_voxel_max,
    );
    if nc_type == MI_ORIGINAL_TYPE {
        nc_type = volume.nc_data_type;
        signed = volume.signed_flag;
        (vmin, vmax) = volume.voxel_range();
    } else if (nc_type == NC_FLOAT || nc_type == NC_DOUBLE) && vmin >= vmax {
        (vmin, vmax) = volume.real_range();
    }
    let n_volume_dims = volume.n_dimensions();
    if n_volume_dims > n_dimensions {
        perr(c, "initialize_minc_output:");
        perr(
            c,
            &format!(" volume ({n_volume_dims}) has more dimensions than file ({n_dimensions}).\n"),
        );
        return Ok(None);
    }
    let filename = crate::xfm::files::Ctx::new(c.dialect.xfm(), c.notices).expand(filename);
    let vol_names = volume.dimension_names_canonical();
    let Some((to_volume, to_file)) = dimension_ordering(c, &vol_names, &dim_names[..n_dimensions])
    else {
        return Ok(None);
    };
    if options.global_image_range[0] >= options.global_image_range[1] {
        let mut n_range_dims = n_dimensions as i64 - 2;
        if nc::cstr(&dim_names[n_dimensions - 1]) == b"vector_dimension" {
            n_range_dims -= 1;
        }
        for d in n_range_dims.max(0) as usize..n_dimensions {
            if to_volume[d] == INVALID_AXIS {
                perr(c, "initialize_minc_output: ");
                perr(c, "if outputting volumes which don't contain all image\n");
                perr(c, "dimensions, then must specify global image range.\n");
                return Ok(None);
            }
        }
    }
    for d in 0..n_dimensions {
        let vi = to_volume[d];
        if vi >= 0 && volume.sizes[vi as usize] != sizes[d] {
            perr(c, "initialize_minc_output: ");
            perr(
                c,
                &format!(
                    "volume size[{vi}]={} does not match file[{d}]={}.\n",
                    volume.sizes[vi as usize], sizes[d]
                ),
            );
            return Ok(None);
        }
    }
    s.ncopts = nc::NC_VERBOSE;
    let cdfid = mw::micreate(s, c, &filename, NC_CLOBBER)?;
    if cdfid == MI_ERROR {
        let mut m = b"Error: opening MINC file \"".to_vec();
        m.extend_from_slice(nc::cstr(&filename));
        m.extend_from_slice(b"\".\n");
        perr_bytes(c, &m);
        return Ok(None);
    }
    mw::micreate_std_variable(s, c, cdfid, b"rootvariable", nc::NC_INT, 0, &[])?;
    let mut file = MincOutput {
        filename,
        cdfid,
        n_file_dimensions: n_dimensions,
        dim_names: Vec::with_capacity(n_dimensions),
        sizes_in_file: Vec::with_capacity(n_dimensions),
        indices: vec![0; n_dimensions],
        to_volume_index: to_volume,
        to_file_index: to_file,
        image_dims: Vec::with_capacity(n_dimensions),
        dim_ids: vec![-1; n_dimensions],
        img_var_id: MI_ERROR,
        min_id: MI_ERROR,
        max_id: MI_ERROR,
        nc_data_type: nc_type,
        signed_flag: signed,
        valid_range: [vmin, vmax],
        image_range: options.global_image_range,
        end_def_done: false,
        variables_written: false,
        outputting_in_order: true,
        entire_file_written: false,
        minc_icv: MI_ERROR,
        n_slab_dims: 0,
    };
    for d in 0..n_dimensions {
        file.sizes_in_file.push(sizes[d] as i64);
        file.dim_names.push(nc::cstr(&dim_names[d]).to_vec());
        let id = ncw::ncdimdef(s, c.streams, cdfid, &dim_names[d], sizes[d] as i64)?;
        file.image_dims.push(id);
    }
    let space = volume.coordinate_system_name.clone();
    if !output_world_transform(
        s,
        c,
        &mut file,
        volume,
        &space,
        voxel_to_world,
        options.use_volume_starts_and_steps,
    )? {
        return Ok(None);
    }
    let dims = file.image_dims.clone();
    if file.image_range[0] < file.image_range[1] {
        file.min_id = mw::micreate_std_variable(s, c, cdfid, b"image-min", NC_DOUBLE, 0, &[])?;
        file.max_id = mw::micreate_std_variable(s, c, cdfid, b"image-max", NC_DOUBLE, 0, &[])?;
    } else {
        let mut n_range_dims = n_dimensions as i32 - 2;
        if nc::cstr(&dim_names[n_dimensions - 1]) == b"vector_dimension" {
            n_range_dims -= 1;
        }
        file.min_id =
            mw::micreate_std_variable(s, c, cdfid, b"image-min", NC_DOUBLE, n_range_dims, &dims)?;
        file.max_id =
            mw::micreate_std_variable(s, c, cdfid, b"image-max", NC_DOUBLE, n_range_dims, &dims)?;
    }
    s.ncopts = nc::NC_VERBOSE | nc::NC_FATAL;
    Ok(Some(file))
}

/// `add_minc_history`: `history` appended to the file's `history` attribute (in define mode).
pub fn add_minc_history(
    s: &mut Session,
    c: &mut Ctx,
    file: &mut MincOutput,
    history: &[u8],
) -> Result<Status, Exit> {
    if file.end_def_done {
        perr(c, "Cannot call add_minc_history when not in define mode\n");
        return Ok(Status::Error);
    }
    s.ncopts = 0;
    let old_len = match nc::ncattinq(s, c.streams, file.cdfid, NC_GLOBAL, b"history")? {
        Some((nc::NC_CHAR, n)) => n.max(0) as usize,
        _ => 0,
    };
    let mut buf = vec![0u8; old_len + 1];
    let got = mi::miattgetstr(
        s,
        c,
        file.cdfid,
        NC_GLOBAL,
        b"history",
        old_len + 1,
        &mut buf,
    )?;
    let mut new_history = match got {
        Some(v) => v,
        None => nc::cstr(&buf).to_vec(),
    };
    new_history.extend_from_slice(nc::cstr(history));
    s.ncopts = nc::NC_VERBOSE | nc::NC_FATAL;
    mw::miattputstr(s, c, file.cdfid, NC_GLOBAL, b"history", &new_history)?;
    Ok(Status::Ok)
}

/// `create_image_variable`: the image, last in the file, with `complete` false, its sign and its
/// valid range (when the range is not empty).
fn create_image_variable(s: &mut Session, c: &mut Ctx, file: &mut MincOutput) -> Result<(), Exit> {
    let dims = file.image_dims.clone();
    file.img_var_id = mw::micreate_std_variable(
        s,
        c,
        file.cdfid,
        b"image",
        file.nc_data_type,
        dims.len() as i32,
        &dims,
    )?;
    mw::miattputstr(s, c, file.cdfid, file.img_var_id, b"complete", MI_FALSE)?;
    let sign = if file.signed_flag {
        MI_SIGNED
    } else {
        MI_UNSIGNED
    };
    mw::miattputstr(s, c, file.cdfid, file.img_var_id, b"signtype", sign)?;
    if file.valid_range[0] < file.valid_range[1] {
        mw::miset_valid_range(s, c, file.cdfid, file.img_var_id, file.valid_range)?;
    }
    Ok(())
}

/// `end_file_def`.
fn end_file_def(s: &mut Session, c: &mut Ctx, file: &mut MincOutput) -> Result<Status, Exit> {
    create_image_variable(s, c, file)?;
    let r = ncw::ncendef(s, c.streams, file.cdfid)?;
    Ok(if r == MI_ERROR {
        Status::Error
    } else {
        Status::Ok
    })
}

/// `check_minc_output_variables`: the definitions ended and, once, the dimension variables'
/// values, the image conversion variable and the global image range written.
fn check_minc_output_variables(
    s: &mut Session,
    c: &mut Ctx,
    file: &mut MincOutput,
    volume: &Volume,
) -> Result<Status, Exit> {
    if !file.end_def_done {
        s.ncopts = nc::NC_VERBOSE;
        let status = end_file_def(s, c, file)?;
        s.ncopts = nc::NC_VERBOSE | nc::NC_FATAL;
        file.end_def_done = true;
        if status != Status::Ok {
            perr(c, "Error outputting volume: possibly disk full?\n");
            return Ok(status);
        }
    }
    if !file.variables_written {
        file.variables_written = true;
        s.ncopts = nc::NC_VERBOSE;
        let mindex = vec![0i64; file.n_file_dimensions.max(1)];
        let zero = Typed::F64(vec![0.0]);
        for d in 0..file.n_file_dimensions {
            if spatial(&file.dim_names[d]).is_some() {
                mw::mivarput1(
                    s,
                    c,
                    file.cdfid,
                    file.dim_ids[d],
                    &mindex,
                    NC_DOUBLE,
                    Some(MI_SIGNED),
                    &zero,
                )?;
            }
        }
        let icvid = icv::miicv_create(s);
        file.minc_icv = icvid;
        icv::miicv_setint(s, c, icvid, MI_ICV_TYPE, volume.nc_data_type)?;
        let sign = if volume.signed_flag {
            MI_SIGNED
        } else {
            MI_UNSIGNED
        };
        icv::miicv_setstr_sign(s, c, icvid, sign)?;
        icv::miicv_setint(s, c, icvid, MI_ICV_DO_NORM, 1)?;
        icv::miicv_setint(s, c, icvid, MI_ICV_USER_NORM, 1)?;
        let (imin, imax) = if file.image_range[0] < file.image_range[1] {
            (file.image_range[0], file.image_range[1])
        } else {
            volume.real_range()
        };
        icv::miicv_setdbl(s, c, icvid, MI_ICV_IMAGE_MIN, imin)?;
        icv::miicv_setdbl(s, c, icvid, MI_ICV_IMAGE_MAX, imax)?;
        let (vmin, vmax) = volume.voxel_range();
        if vmin < vmax {
            icv::miicv_setdbl(s, c, icvid, MI_ICV_VALID_MIN, vmin)?;
            icv::miicv_setdbl(s, c, icvid, MI_ICV_VALID_MAX, vmax)?;
        } else {
            perr(c, "Volume has invalid min and max voxel value\n");
        }
        icv::miicv_attach(s, c, icvid, file.cdfid, file.img_var_id)?;
        if file.image_range[0] < file.image_range[1] {
            let start = [0i64];
            for (id, v) in [
                (file.min_id, file.image_range[0]),
                (file.max_id, file.image_range[1]),
            ] {
                mw::mivarput1(
                    s,
                    c,
                    file.cdfid,
                    id,
                    &start,
                    NC_DOUBLE,
                    Some(MI_SIGNED),
                    &Typed::F64(vec![v]),
                )?;
            }
        }
        s.ncopts = nc::NC_VERBOSE | nc::NC_FATAL;
    }
    Ok(Status::Ok)
}

/// The volume's values as a buffer of its type.
fn volume_values(d: &VolumeData, idx: &[usize]) -> Typed {
    macro_rules! g {
        ($v:expr, $t:ident) => {
            Typed::$t(
                idx.iter()
                    .map(|&i| $v.get(i).copied().unwrap_or_default())
                    .collect(),
            )
        };
    }
    match d {
        VolumeData::U8(v) => g!(v, U8),
        VolumeData::I8(v) => g!(v, I8),
        VolumeData::U16(v) => g!(v, U16),
        VolumeData::I16(v) => g!(v, I16),
        VolumeData::U32(v) => g!(v, U32),
        VolumeData::I32(v) => g!(v, I32),
        VolumeData::F32(v) => g!(v, F32),
        VolumeData::F64(v) => g!(v, F64),
    }
}

/// An empty buffer of a volume's type.
fn empty_values(t: DataType) -> Typed {
    match t {
        DataType::UnsignedByte | DataType::None => Typed::U8(Vec::new()),
        DataType::SignedByte => Typed::I8(Vec::new()),
        DataType::UnsignedShort => Typed::U16(Vec::new()),
        DataType::SignedShort => Typed::I16(Vec::new()),
        DataType::UnsignedInt => Typed::U32(Vec::new()),
        DataType::SignedInt => Typed::I32(Vec::new()),
        DataType::Float => Typed::F32(Vec::new()),
        DataType::Double => Typed::F64(Vec::new()),
    }
}

/// `output_minc_hyperslab` of the volume's array from the voxel `base` (its index): the
/// hyperslab's values taken as one run of the array when it is one (`direct_from_array`), else
/// gathered in the file's order (`copy_multidim_data_reordered`), and written through the image
/// conversion variable.
fn output_minc_hyperslab(
    s: &mut Session,
    c: &mut Ctx,
    file: &mut MincOutput,
    volume: &Volume,
    base: usize,
    to_array: &[i32],
    file_start: &[i32],
    file_count: &[i32],
) -> Result<Status, Exit> {
    let st = check_minc_output_variables(s, c, file, volume)?;
    if st != Status::Ok {
        return Ok(st);
    }
    let n_file = file.n_file_dimensions;
    let n_array = volume.n_dimensions();
    let array_sizes: Vec<usize> = volume.sizes.iter().map(|&x| x.max(0) as usize).collect();
    let mut expected_ind = n_array as i64 - 1;
    let mut non_full = false;
    let mut direct = true;
    let mut long_start = vec![0i64; n_file];
    let mut long_count = vec![0i64; n_file];
    for fi in (0..n_file).rev() {
        long_start[fi] = file_start[fi] as i64;
        long_count[fi] = file_count[fi] as i64;
        let ind = to_array[fi];
        if ind != INVALID_AXIS {
            if !non_full && (file_count[fi] as i64) < file.sizes_in_file[fi] {
                non_full = true;
            } else if non_full && file_count[fi] > 1 {
                direct = false;
            }
            if file_count[fi] > 1 && ind as i64 != expected_ind {
                direct = false;
            }
            expected_ind -= 1;
        }
    }
    let total: usize = file_count.iter().map(|&x| x.max(0) as usize).product();
    let Some(data) = volume.data.as_ref() else {
        // Voxels never allocated (FS-799): volume_io follows the array's pointer table (a crash)
        // for an allocated type of two or more dimensions; else miicv_put reads the values from a
        // null or unset pointer unless it fails first.
        let crashes = (volume.data_type != DataType::None && n_array >= 2)
            || (total > 0 && icv::put_reads_values(s, file.minc_icv));
        if crashes {
            c.notices.add(
                &larmorx_core::defects::FS_799,
                "a volume whose voxels were never allocated not written (volume_io reads them through a null or unset pointer and crashes)",
            );
            return Ok(Status::Error);
        }
        let vals = empty_values(volume.data_type);
        let r = icv::miicv_put(s, c, file.minc_icv, &long_start, &long_count, &vals)?;
        return Ok(if r == MI_ERROR {
            Status::Error
        } else {
            Status::Ok
        });
    };
    let idx: Vec<usize> = if direct {
        (base..base + total).collect()
    } else {
        // The file's C order over the hyperslab, each element's voxel in the array.
        let mut strides = vec![1usize; n_array];
        for k in (0..n_array.saturating_sub(1)).rev() {
            strides[k] = strides[k + 1] * array_sizes[k + 1];
        }
        let mut out = Vec::with_capacity(total);
        let mut coord = vec![0i32; n_file];
        if total > 0 {
            loop {
                let mut at = base;
                for fi in 0..n_file {
                    let ind = to_array[fi];
                    if ind != INVALID_AXIS {
                        at += coord[fi] as usize * strides[ind as usize];
                    }
                }
                out.push(at);
                let mut k = n_file;
                loop {
                    if k == 0 {
                        break;
                    }
                    k -= 1;
                    coord[k] += 1;
                    if coord[k] < file_count[k] {
                        break;
                    }
                    coord[k] = 0;
                    if k == 0 {
                        k = usize::MAX;
                        break;
                    }
                }
                if k == usize::MAX || n_file == 0 {
                    break;
                }
            }
        }
        out
    };
    let mut vals = volume_values(data, &idx);
    if total == 0 {
        vals = empty_values(volume.data_type);
    }
    let r = icv::miicv_put(s, c, file.minc_icv, &long_start, &long_count, &vals)?;
    Ok(if r == MI_ERROR {
        Status::Error
    } else {
        Status::Ok
    })
}

/// `output_slab`: the slab at `file_start` / `file_count` from the volume.
fn output_slab(
    s: &mut Session,
    c: &mut Ctx,
    file: &mut MincOutput,
    volume: &Volume,
    to_volume: &[i32],
    file_start: &[i64],
    file_count: &[i64],
) -> Result<(), Exit> {
    let n_file = file.n_file_dimensions;
    let mut int_start = vec![0i32; n_file];
    let mut int_count = vec![0i32; n_file];
    let mut volume_start = vec![0usize; volume.n_dimensions()];
    for fi in 0..n_file {
        int_start[fi] = file_start[fi] as i32;
        int_count[fi] = file_count[fi] as i32;
        let ind = to_volume[fi];
        if ind != INVALID_AXIS {
            volume_start[ind as usize] = int_start[fi].max(0) as usize;
        }
    }
    let base = volume.index(&volume_start);
    output_minc_hyperslab(s, c, file, volume, base, to_volume, &int_start, &int_count)?;
    Ok(())
}

/// `output_the_volume`: the volume (`volume_count` voxels of each dimension) written at
/// `file_start` in slabs of at least `MIN_SLAB_SIZE` voxels (at most `MAX_SLAB_SIZE` when that
/// takes more than one dimension), after the per-slice image ranges when no global range is set.
fn output_the_volume(
    s: &mut Session,
    c: &mut Ctx,
    file: &mut MincOutput,
    volume: &mut Volume,
    volume_count: &[i32],
    file_start: &[i64],
) -> Result<Status, Exit> {
    let st = check_minc_output_variables(s, c, file, volume)?;
    if st != Status::Ok {
        return Ok(st);
    }
    let n_file = file.n_file_dimensions;
    let n_volume_dims = volume.n_dimensions();
    if n_volume_dims > n_file {
        perr(c, "output_volume_to_minc_file_position:");
        perr(
            c,
            &format!(" volume ({n_volume_dims}) has more dimensions than file ({n_file}).\n"),
        );
        return Ok(Status::Error);
    }
    let vol_names = volume.dimension_names_canonical();
    let fnames = file.dim_names.clone();
    let Some((to_volume, _to_file)) = dimension_ordering(c, &vol_names, &fnames) else {
        return Ok(Status::Error);
    };
    let sizes = volume.sizes.clone();
    for d in 0..n_file {
        let vi = to_volume[d];
        let this_count = if vi >= 0 {
            let vi = vi as usize;
            if volume_count[vi] < 0 || volume_count[vi] > sizes[vi] {
                perr(c, "output_the_volume: invalid volume count.\n");
                perr(c, &format!("    count[{vi}] = {}\n", volume_count[vi]));
                return Ok(Status::Error);
            }
            volume_count[vi]
        } else {
            1
        };
        if file_start[d] < 0 || file_start[d] + this_count as i64 > file.sizes_in_file[d] {
            perr(c, "output_the_volume:  invalid minc file position.\n");
            perr(
                c,
                &format!(
                    "    start[{d}] = {}     count[{d}] = {this_count}\n",
                    file_start[d] as i32
                ),
            );
            return Ok(Status::Error);
        }
    }
    if file.image_range[0] >= file.image_range[1] {
        let mut n_range_dims = n_file as i64 - 2;
        if nc::cstr(&file.dim_names[n_file - 1]) == b"vector_dimension" {
            n_range_dims -= 1;
        }
        let nr = n_range_dims.max(0) as usize;
        let mut n_ranges: i64 = 1;
        let mut range_start = vec![0i64; nr];
        let mut range_count = vec![0i64; nr];
        for d in 0..nr {
            let vi = to_volume[d];
            if vi == INVALID_AXIS {
                range_count[d] = 1;
                range_start[d] = file_start[d];
            } else {
                n_ranges = n_ranges.wrapping_mul(volume_count[vi as usize] as i64);
                range_count[d] = volume_count[vi as usize] as i64;
                range_start[d] = 0;
            }
        }
        let (rmin, rmax) = volume.real_range();
        let n = n_ranges.max(0) as usize;
        for (id, v) in [(file.min_id, rmin), (file.max_id, rmax)] {
            mw::mivarput(
                s,
                c,
                file.cdfid,
                id,
                &range_start,
                &range_count,
                NC_DOUBLE,
                Some(MI_UNSIGNED),
                &Typed::F64(vec![v; n]),
            )?;
        }
    }
    // The slabs.
    let mut n_slab_dims = 0usize;
    let mut slab_size: i32 = 1;
    let mut d = n_file as i64 - 1;
    loop {
        let vi = to_volume[d as usize];
        if vi != INVALID_AXIS {
            n_slab_dims += 1;
            slab_size = slab_size.wrapping_mul(volume_count[vi as usize]);
        }
        d -= 1;
        if !(d >= 0 && slab_size < MIN_SLAB_SIZE) {
            break;
        }
    }
    if slab_size > MAX_SLAB_SIZE && n_slab_dims > 1 {
        n_slab_dims -= 1;
    }
    file.n_slab_dims = n_slab_dims;
    let mut file_indices = vec![0i64; n_file];
    for d in (0..n_file).rev() {
        file_indices[d] = file_start[d];
    }
    let mut count = vec![1i64; n_file];
    loop {
        let mut n_slab = 0usize;
        for d in (0..n_file).rev() {
            let vi = to_volume[d];
            count[d] = if vi == INVALID_AXIS || n_slab >= n_slab_dims {
                1
            } else {
                volume_count[vi as usize] as i64
            };
            if vi != INVALID_AXIS {
                n_slab += 1;
            }
        }
        output_slab(s, c, file, volume, &to_volume, &file_indices, &count)?;
        let mut increment = true;
        let mut d = n_file as i64 - 1;
        let mut n_slab = 0usize;
        while increment && d >= 0 {
            let du = d as usize;
            let vi = to_volume[du];
            if vi != INVALID_AXIS && n_slab >= n_slab_dims {
                file_indices[du] += 1;
                if file_indices[du] < file_start[du] + volume_count[vi as usize] as i64 {
                    increment = false;
                } else {
                    file_indices[du] = file_start[du];
                }
            }
            if vi != INVALID_AXIS {
                n_slab += 1;
            }
            d -= 1;
        }
        if increment {
            break;
        }
    }
    Ok(Status::Ok)
}

/// `output_minc_volume`: the attached volume written at the file's current position, which then
/// moves to the next volume of the dimensions the volume does not have.
pub fn output_minc_volume(
    s: &mut Session,
    c: &mut Ctx,
    file: &mut MincOutput,
    volume: &mut Volume,
) -> Result<Status, Exit> {
    let n_file = file.n_file_dimensions;
    let mut d = 0;
    while d < n_file && file.to_volume_index[d] != INVALID_AXIS {
        d += 1;
    }
    if d < n_file && file.indices[d] >= file.sizes_in_file[d] {
        perr(
            c,
            "output_minc_volume: attempted to write too many subvolumes.\n",
        );
        return Ok(Status::Error);
    }
    let volume_count = volume.sizes.clone();
    let indices = file.indices.clone();
    if output_the_volume(s, c, file, volume, &volume_count, &indices)? != Status::Ok {
        return Ok(Status::Error);
    }
    let mut increment = true;
    let mut d = n_file as i64 - 1;
    while increment && d >= 0 {
        let du = d as usize;
        if file.to_volume_index[du] == INVALID_AXIS {
            file.indices[du] += 1;
            if file.indices[du] < file.sizes_in_file[du] {
                increment = false;
            } else {
                file.indices[du] = 0;
            }
        }
        d -= 1;
    }
    if increment {
        file.entire_file_written = true;
    }
    Ok(Status::Ok)
}

/// `close_minc_output`: `complete` set true, the file closed, the conversion variable freed.
pub fn close_minc_output(s: &mut Session, c: &mut Ctx, file: MincOutput) -> Result<Status, Exit> {
    if file.outputting_in_order && !file.entire_file_written {
        perr(c, "Warning:  the MINC file has been ");
        perr(c, "closed without writing part of it.\n");
    }
    mw::miattputstr(s, c, file.cdfid, file.img_var_id, b"complete", MI_TRUE)?;
    mi::miclose(s, c, file.cdfid)?;
    icv::miicv_free(s, c, file.minc_icv)?;
    Ok(Status::Ok)
}

/// `output_volume`: the volume written as a new MINC file `filename` of the type `file_nc_type`
/// with `file_signed`, whose voxels `file_voxel_min` .. `file_voxel_max` are the volume's real
/// range (`MI_ORIGINAL_TYPE`: the volume's type and voxel range), with `history` appended to the
/// file's history. `Status::Error` after volume_io's message.
pub fn output_volume(
    s: &mut Session,
    c: &mut Ctx,
    filename: &[u8],
    file_nc_type: i32,
    file_signed: bool,
    file_voxel_min: f64,
    file_voxel_max: f64,
    volume: &mut Volume,
    history: Option<&[u8]>,
    options: Option<&MincOutputOptions>,
) -> Result<Status, Exit> {
    let Some((dim_names, sizes)) = create_output_dim_names(c, volume, options)? else {
        return Ok(Status::Error);
    };
    let n_dims = volume.n_dimensions();
    let mut used = options.cloned().unwrap_or_default();
    if used.global_image_range[0] >= used.global_image_range[1] {
        let (rmin, rmax) = volume.real_range();
        used.set_real_range(rmin, rmax);
    }
    if !volume.uptodate && volume.spatial_axes.iter().all(|&a| a < 0) {
        c.notices.add(
            &larmorx_core::defects::FS_800,
            "a volume without spatial axes written with its earlier voxel-to-world transform (volume_io's compute_world_transform returns none: it takes memory never written)",
        );
    }
    let linear = volume
        .voxel_to_world_transform(c.dialect, c.streams)
        .transform_type()
        == TransformType::Linear;
    if !used.use_starts_set && !used.use_volume_starts_and_steps && linear {
        used.set_use_volume_starts_and_steps_flag(true);
    }
    let transform = volume
        .voxel_to_world_transform(c.dialect, c.streams)
        .clone();
    let Some(mut file) = initialize_minc_output(
        s,
        c,
        filename,
        n_dims,
        Some(&dim_names),
        &sizes,
        file_nc_type,
        file_signed,
        file_voxel_min,
        file_voxel_max,
        &transform,
        volume,
        Some(&used),
    )?
    else {
        return Ok(Status::Error);
    };
    let mut status = Status::Ok;
    if let Some(h) = history {
        status = add_minc_history(s, c, &mut file, h)?;
    }
    if status == Status::Ok {
        status = output_minc_volume(s, c, &mut file, volume)?;
    }
    if status == Status::Ok {
        status = close_minc_output(s, c, file)?;
    }
    Ok(status)
}
