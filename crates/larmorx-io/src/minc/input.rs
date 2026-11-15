// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
//! volume_io's volume input (`Volumes/input_volume.c`, `input_mnc.c`; MNI's notice,
//! `LICENSES/LicenseRef-MNI.txt`): the file's dimensions matched to the volume's by name, the
//! geometry read from the dimension variables, the image read slab by slab through a conversion
//! variable into the volume's type.

use larmorx_core::defects::{self as d};

use super::icv::{self, MI_ICV_DO_DIM_CONV, MI_ICV_DO_FILLVALUE, MI_ICV_DO_NORM, MI_ICV_DO_SCALAR};
use super::libsrc::{self as mi, MI_ERROR, MI_MAX_ATTSTR_LEN, Typed};
use super::netcdf::{self as nc, NC_BYTE};
use super::volume::{self, ANY_SPATIAL_DIMENSION, MAX_DIMENSIONS, Volume};
use super::{Ctx, Exit, Session, Status};
use crate::xfm::{GeneralTransform, files};

const INVALID_AXIS: i32 = -1;
const MIN_SLAB_SIZE: i64 = 10000;
const MAX_SLAB_SIZE: i64 = 400000;

/// `minc_input_options`.
#[derive(Clone, Debug, PartialEq)]
pub struct MincInputOptions {
    pub promote_invalid_to_zero: bool,
    pub convert_vector_to_scalar: bool,
    pub convert_vector_to_colour: bool,
    pub dimension_size_for_colour_data: i32,
    pub max_dimension_size_for_colour_data: i32,
    pub rgba_indices: [i32; 4],
    pub user_real_range: [f64; 2],
}

impl Default for MincInputOptions {
    /// `set_default_minc_input_options`.
    fn default() -> MincInputOptions {
        MincInputOptions {
            promote_invalid_to_zero: true,
            convert_vector_to_scalar: true,
            convert_vector_to_colour: false,
            dimension_size_for_colour_data: 3,
            max_dimension_size_for_colour_data: 4,
            rgba_indices: [0, 1, 2, 3],
            user_real_range: [0.0, 0.0],
        }
    }
}

/// `minc_file_struct` for reading.
#[derive(Clone, Debug)]
pub struct MincFile {
    pub cdfid: i32,
    pub img_var: i32,
    pub filename: Vec<u8>,
    pub dim_names: Vec<Vec<u8>>,
    pub sizes_in_file: Vec<i64>,
    pub to_volume_index: Vec<i32>,
    pub to_file_index: Vec<i32>,
    pub spatial_axes: [i32; 3],
    pub valid_file_axes: Vec<usize>,
    pub n_volumes_in_file: i64,
    pub voxel_to_world_transform: Option<GeneralTransform>,
    pub minc_icv: i32,
    pub indices: Vec<i64>,
    pub end_volume_flag: bool,
    pub n_slab_dims: usize,
    pub original_input_options: MincInputOptions,
    /// The cache's own opening of the file (volume_io's volume cache).
    pub cache_file: Option<Box<MincFile>>,
}

/// The input formats of `volume_input_struct`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileFormat {
    Mnc,
    Free,
}

/// `volume_input_struct`.
#[derive(Clone, Debug)]
pub struct VolumeInput {
    pub file_format: FileFormat,
    pub minc_file: Option<MincFile>,
}

/// `filename_extension_matches`: the expanded name, its compressed endings cut, ends in
/// `.<extension>`.
fn filename_extension_matches(c: &mut Ctx, filename: &[u8], extension: &[u8]) -> bool {
    let mut f = files::Ctx::new(c.dialect.xfm(), c.notices).expand(filename);
    let len = f.len();
    for e in [&b".z"[..], b".Z", b".gz"] {
        if f.len() >= e.len() && &f[f.len() - e.len()..] == e {
            f.truncate(len - e.len());
        }
    }
    let mut ending = b".".to_vec();
    ending.extend_from_slice(extension);
    f.len() >= ending.len() && f[f.len() - ending.len()..] == ending[..]
}

fn expand(c: &mut Ctx, name: &[u8]) -> Vec<u8> {
    files::Ctx::new(c.dialect.xfm(), c.notices).expand(name)
}

/// `get_minc_file_n_dimensions`: the image's dimension count, or `-1` (and an error) when the
/// file cannot be opened.
pub fn get_minc_file_n_dimensions(
    s: &mut Session,
    c: &mut Ctx,
    filename: &[u8],
) -> Result<i32, Exit> {
    s.ncopts = nc::NC_VERBOSE;
    let expanded = expand(c, filename);
    let Some(cdfid) = mi::miopen(s, c, &expanded)? else {
        let mut m = b"Error opening ".to_vec();
        m.extend_from_slice(nc::cstr(&expanded));
        m.push(b'\n');
        c.dialect.print_error_bytes(&m, c.streams);
        return Ok(-1);
    };
    let img_var = nc::ncvarid(s, c.streams, cdfid, b"image")?;
    let inq = nc::ncvarinq(s, c.streams, cdfid, img_var)?;
    mi::miclose(s, c, cdfid)?;
    match inq {
        Some(v) if v.dimids.len() <= nc::MAX_VAR_DIMS => Ok(v.dimids.len() as i32),
        Some(v) => {
            c.notices.add(
                &d::FS_783,
                format!(
                    "an image of {} dimensions refused (volume_io's arrays hold {})",
                    v.dimids.len(),
                    nc::MAX_VAR_DIMS
                ),
            );
            Ok(-1)
        }
        None => {
            c.notices.add(
                &d::FS_779,
                "a file without an image variable: no dimension count (volume_io returns one it never wrote)",
            );
            Ok(-1)
        }
    }
}

/// `start_volume_input` for a MINC file: the status, the volume (created unless one is given and
/// `create_volume_flag` is false) and the input to read it with.
pub fn start_volume_input(
    s: &mut Session,
    c: &mut Ctx,
    filename: &[u8],
    mut n_dimensions: i32,
    dim_names: Option<&[Vec<u8>]>,
    nc_type: i32,
    signed: bool,
    voxel_min: f64,
    voxel_max: f64,
    create_volume_flag: bool,
    volume: &mut Option<Volume>,
    options: Option<&MincInputOptions>,
) -> Result<(Status, Option<VolumeInput>), Exit> {
    if create_volume_flag || volume.is_none() {
        if !(1..=MAX_DIMENSIONS as i32).contains(&n_dimensions) {
            n_dimensions = get_minc_file_n_dimensions(s, c, filename)?;
        }
        if n_dimensions < 1 {
            return Ok((Status::Error, None));
        }
        let defaults;
        let names = match dim_names {
            Some(n) => Some(n),
            None if n_dimensions as usize <= MAX_DIMENSIONS => {
                defaults = volume::default_dim_names(n_dimensions as usize);
                Some(&defaults[..])
            }
            None => None,
        };
        if let Some(n) = names
            && n.len() < n_dimensions as usize
            && n_dimensions as usize <= MAX_DIMENSIONS
        {
            // The caller's array of names is shorter than the file's dimension count: C reads
            // past it (FreeSurfer's four names and a five-dimensional file). Treated as empty
            // names.
            c.notices.add(
                &d::FS_779,
                format!(
                    "{} dimension names for a {n_dimensions}-dimensional file (the caller's array is read past its end); the rest empty",
                    n.len()
                ),
            );
        }
        *volume = Volume::create(
            n_dimensions,
            names,
            nc_type,
            signed,
            voxel_min,
            voxel_max,
            c.dialect,
            c.streams,
        )?;
    } else if let Some(v) = volume.as_mut()
        && n_dimensions != v.n_dimensions() as i32
        && v.data.is_some()
    {
        v.data = None;
    }
    let expanded = expand(c, filename);
    let format = if filename_extension_matches(c, &expanded, b"fre") {
        FileFormat::Free
    } else {
        FileFormat::Mnc
    };
    let mut status = Status::Ok;
    let mut input = VolumeInput {
        file_format: format,
        minc_file: None,
    };
    match format {
        FileFormat::Mnc => {
            let mut name = expanded.clone();
            let mut fc = files::Ctx::new(c.dialect.xfm(), c.notices);
            if !fc.file_exists(&name)
                && let Some(z) = fc.file_exists_as_compressed(&name)
            {
                name = z;
            }
            match volume.as_mut() {
                None => {
                    // create_volume failed (more than five dimensions): volume_io goes on with
                    // no volume (a crash).
                    c.notices.add(
                        &d::FS_779,
                        "no volume was created (volume_io reads into a null volume): an error",
                    );
                    status = Status::Error;
                }
                Some(v) => match initialize_minc_input(s, c, &name, v, options)? {
                    None => status = Status::Error,
                    Some(f) => input.minc_file = Some(f),
                },
            }
        }
        FileFormat::Free => {
            c.dialect.print_error(
                "larmorx: volume_io's free-format input (.fre) is not ported yet\n",
                c.streams,
            );
            status = Status::Error;
        }
    }
    if status != Status::Ok && create_volume_flag {
        *volume = None;
    }
    Ok((
        status,
        if status == Status::Ok {
            Some(input)
        } else {
            None
        },
    ))
}

/// `initialize_minc_input`.
pub fn initialize_minc_input(
    s: &mut Session,
    c: &mut Ctx,
    filename: &[u8],
    volume: &mut Volume,
    options: Option<&MincInputOptions>,
) -> Result<Option<MincFile>, Exit> {
    s.ncopts = 0;
    let expanded = expand(c, filename);
    let Some(minc_id) = mi::miopen(s, c, &expanded)? else {
        let mut m = b"Error: opening MINC file \"".to_vec();
        m.extend_from_slice(nc::cstr(&expanded));
        m.extend_from_slice(b"\".\n");
        c.dialect.print_error_bytes(&m, c.streams);
        return Ok(None);
    };
    match initialize_from_id(s, c, minc_id, volume, options)? {
        None => {
            mi::miclose(s, c, minc_id)?;
            Ok(None)
        }
        Some(mut f) => {
            f.filename = expanded;
            Ok(Some(f))
        }
    }
}

/// `match_dimension_names`: the volume index of each file dimension.
fn match_dimension_names(volume_names: &mut [Vec<u8>], file_names: &[Vec<u8>]) -> Option<Vec<i32>> {
    let nv = volume_names.len();
    let nf = file_names.len();
    let mut to_volume = vec![INVALID_AXIS; nf];
    let mut to_file = vec![-1i32; nv];
    let mut found = vec![false; nv];
    let mut n_matches = 0;
    for iteration in 0..3 {
        for i in (0..nv).rev() {
            if found[i] {
                continue;
            }
            for j in (0..nf).rev() {
                if to_volume[j] != INVALID_AXIS {
                    continue;
                }
                let m = match iteration {
                    0 => volume_names[i] == file_names[j],
                    1 => {
                        volume_names[i] == ANY_SPATIAL_DIMENSION
                            && volume::spatial_axis(&file_names[j]).is_some()
                    }
                    _ => volume_names[i].is_empty(),
                };
                if m {
                    to_volume[j] = i as i32;
                    to_file[i] = j as i32;
                    found[i] = true;
                    n_matches += 1;
                    break;
                }
            }
        }
    }
    if n_matches != nv {
        return None;
    }
    for i in 0..nv {
        if volume_names[i] == ANY_SPATIAL_DIMENSION || volume_names[i].is_empty() {
            volume_names[i] = file_names[to_file[i] as usize].clone();
        }
    }
    Some(to_volume)
}

/// `initialize_minc_input_from_minc_id`.
fn initialize_from_id(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    volume: &mut Volume,
    options: Option<&MincInputOptions>,
) -> Result<Option<MincFile>, Exit> {
    let defaults = MincInputOptions::default();
    let options = options.unwrap_or(&defaults);
    let prev_sizes = volume.sizes.clone();
    let prev_nc_type = volume.nc_data_type;
    let img_var = nc::ncvarid(s, c.streams, cdfid, b"image")?;
    let Some(inq) = nc::ncvarinq(s, c.streams, cdfid, img_var)? else {
        c.notices.add(
            &d::FS_779,
            "a file without an image variable: an error (volume_io goes on with a type and dimension count it never wrote)",
        );
        return Ok(None);
    };
    let file_datatype = inq.ty;
    if inq.dimids.len() > nc::MAX_VAR_DIMS {
        c.notices.add(
            &d::FS_783,
            format!(
                "an image of {} dimensions refused (volume_io's arrays hold {})",
                inq.dimids.len(),
                nc::MAX_VAR_DIMS
            ),
        );
        c.dialect.print_error(
            &format!(
                "Error: MINC file has {} dims, can only handle {}.\n",
                inq.dimids.len(),
                nc::MAX_VAR_DIMS
            ),
            c.streams,
        );
        return Ok(None);
    }
    let mut dim_names = Vec::new();
    let mut sizes_in_file = Vec::new();
    for &dimid in &inq.dimids {
        let (name, size) = match nc::ncdiminq(s, c.streams, cdfid, dimid)? {
            Some(x) => x,
            // ncdiminq failed: the name and size buffers keep what they held (memory); an empty
            // name and size 0 here.
            None => (Vec::new(), 0),
        };
        if name.len() > MI_MAX_ATTSTR_LEN {
            c.notices.add(
                &d::FS_782,
                format!(
                    "the dimension name {} of {} bytes read whole (volume_io's buffer holds {})",
                    String::from_utf8_lossy(&name),
                    name.len(),
                    MI_MAX_ATTSTR_LEN
                ),
            );
        }
        dim_names.push(name);
        sizes_in_file.push(size);
    }
    let mut n_file = dim_names.len();
    if n_file > 0 && dim_names[n_file - 1] == b"vector_dimension" {
        let vsize = sizes_in_file[n_file - 1];
        if options.convert_vector_to_colour
            && vsize >= options.dimension_size_for_colour_data as i64
            && vsize <= options.max_dimension_size_for_colour_data as i64
        {
            c.dialect.print_error(
                "larmorx: volume_io's conversion of a vector dimension to colours is not ported yet\n",
                c.streams,
            );
            return Ok(None);
        } else if options.convert_vector_to_scalar {
            dim_names.pop();
            n_file -= 1;
        }
    }
    let n_vol = volume.n_dimensions();
    if n_file < n_vol {
        c.dialect.print_error(
            &format!("Error: MINC file has only {n_file} dims, volume requires {n_vol}.\n"),
            c.streams,
        );
        return Ok(None);
    }
    let Some(to_volume_index) = match_dimension_names(&mut volume.dimension_names, &dim_names)
    else {
        let p = |t: &str, c: &mut Ctx| c.dialect.print_error(t, c.streams);
        p("Error:  dimension names did not match: \n", c);
        p("\n", c);
        p("Requested:\n", c);
        let line = |k: usize, n: &[u8], c: &mut Ctx| {
            let mut m = format!("{}: ", k + 1).into_bytes();
            m.extend_from_slice(nc::cstr(n));
            m.push(b'\n');
            c.dialect.print_error_bytes(&m, c.streams);
        };
        for (k, n) in volume.dimension_names.iter().enumerate() {
            line(k, n, c);
        }
        p("\n", c);
        p("In File:\n", c);
        for (k, n) in dim_names.iter().enumerate() {
            line(k, n, c);
        }
        return Ok(None);
    };
    let mut to_file_index = vec![INVALID_AXIS; n_vol];
    for (dd, &vi) in to_volume_index.iter().enumerate() {
        if vi != INVALID_AXIS {
            to_file_index[vi as usize] = dd as i32;
        }
    }
    let mut n_volumes_in_file: i64 = 1;
    volume.spatial_axes = [INVALID_AXIS; 3];
    let mut file_spatial = [INVALID_AXIS; 3];
    let mut spatial_idx = vec![INVALID_AXIS; n_file];
    let mut valid_file_axes = Vec::new();
    for dd in 0..n_file {
        if let Some(a) = volume::spatial_axis(&dim_names[dd]) {
            spatial_idx[dd] = a as i32;
            file_spatial[a] = dd as i32;
        }
        if to_volume_index[dd] != INVALID_AXIS {
            valid_file_axes.push(dd);
            if spatial_idx[dd] != INVALID_AXIS {
                volume.spatial_axes[spatial_idx[dd] as usize] = to_volume_index[dd];
            }
        }
    }
    // The geometry of each file dimension.
    let mut file_sep = vec![1.0f64; n_file];
    let mut start_pos = vec![0.0f64; n_file];
    let mut dircos = vec![[0.0f64; 3]; n_file];
    let mut irr_starts: Vec<Option<Vec<f64>>> = vec![None; n_file];
    let mut irr_widths: Vec<Option<Vec<f64>>> = vec![None; n_file];
    let mut prev_space: Vec<u8> = Vec::new();
    let mut tmp_cos: [Option<f64>; 3] = [None; 3];
    let mut consensus = true;
    let mut vol_sizes = volume.sizes.clone();
    let mut vol_seps = volume.separations.clone();
    let mut vol_starts = volume.starts.clone();
    for dd in 0..n_file {
        if spatial_idx[dd] != INVALID_AXIS {
            dircos[dd] = [0.0; 3];
            dircos[dd][spatial_idx[dd] as usize] = 1.0;
        }
        let dimvar = nc::ncvarid(s, c.streams, cdfid, &dim_names[dd])?;
        if dimvar != MI_ERROR {
            mi::miattget1(s, c, cdfid, dimvar, b"step", &mut file_sep[dd])?;
            if spatial_idx[dd] != INVALID_AXIS {
                if mi::miattget1(s, c, cdfid, dimvar, b"start", &mut start_pos[dd])?.is_none() {
                    start_pos[dd] = 0.0;
                }
                // volume_io reads every dimension's cosines into one buffer: an attribute of
                // fewer than three values keeps the earlier dimensions' values (FS-791).
                let mut t = [
                    tmp_cos[0].unwrap_or(0.0),
                    tmp_cos[1].unwrap_or(0.0),
                    tmp_cos[2].unwrap_or(0.0),
                ];
                if let Some(len) =
                    mi::miattget(s, c, cdfid, dimvar, b"direction_cosines", 3, &mut t)?
                {
                    let written = (len.max(0) as usize).min(3);
                    for k in 0..written {
                        tmp_cos[k] = Some(t[k]);
                    }
                    if written < 3 {
                        let name = String::from_utf8_lossy(&dim_names[dd]).into_owned();
                        if c.notices.reproduce() {
                            if tmp_cos.iter().any(|x| x.is_none()) {
                                c.notices.add(
                                    &d::FS_792,
                                    format!("the {len} direction cosines of {name}: the rest is memory never written (read as 0)"),
                                );
                            } else {
                                c.notices.add(
                                    &d::FS_791,
                                    format!("the {len} direction cosines of {name}: the rest from the dimension read before (reproduce)"),
                                );
                            }
                            dircos[dd] = [
                                tmp_cos[0].unwrap_or(0.0),
                                tmp_cos[1].unwrap_or(0.0),
                                tmp_cos[2].unwrap_or(0.0),
                            ];
                        } else {
                            c.notices.add(
                                &d::FS_791,
                                format!("the {len} direction cosines of {name} read as none (volume_io keeps the rest from the dimension read before)"),
                            );
                        }
                    } else {
                        dircos[dd] = t;
                    }
                }
                let mut buf = Vec::new();
                if let Some(sp) = mi::miattgetstr(
                    s,
                    c,
                    cdfid,
                    dimvar,
                    b"spacetype",
                    MI_MAX_ATTSTR_LEN + 1,
                    &mut buf,
                )? {
                    if !prev_space.is_empty() && !sp.is_empty() && prev_space != sp {
                        consensus = false;
                    }
                    if !sp.is_empty() {
                        prev_space = sp;
                    }
                }
            } else if dim_names[dd] == b"time" {
                let mut buf = Vec::new();
                let spacing = mi::miattgetstr(
                    s,
                    c,
                    cdfid,
                    dimvar,
                    b"spacing",
                    MI_MAX_ATTSTR_LEN + 1,
                    &mut buf,
                )?;
                let irregular = match &spacing {
                    Some(sp) => sp == b"irregular",
                    None => {
                        c.notices.add(
                            &d::FS_780,
                            "the time dimension has no spacing string: read as regular (volume_io compares a buffer it never wrote)",
                        );
                        false
                    }
                };
                if irregular {
                    let n = sizes_in_file[dd];
                    let st = [0i64];
                    let ct = [n];
                    let starts = mi::mivarget(
                        s,
                        c,
                        cdfid,
                        dimvar,
                        &st,
                        &ct,
                        nc::NC_DOUBLE,
                        Some(mi::MI_SIGNED),
                    )?;
                    irr_starts[dd] = Some(typed_to_vec(starts, n));
                    let wvar = nc::ncvarid(s, c.streams, cdfid, b"time-width")?;
                    if wvar < 0 {
                        irr_widths[dd] = Some(vec![file_sep[dd]; n.max(0) as usize]);
                    } else {
                        let w = mi::mivarget(
                            s,
                            c,
                            cdfid,
                            wvar,
                            &st,
                            &ct,
                            nc::NC_DOUBLE,
                            Some(mi::MI_SIGNED),
                        )?;
                        irr_widths[dd] = Some(typed_to_vec(w, n));
                    }
                } else if mi::miattget1(s, c, cdfid, dimvar, b"start", &mut start_pos[dd])?
                    .is_none()
                {
                    start_pos[dd] = 0.0;
                }
            }
        }
        let vi = to_volume_index[dd];
        if vi == INVALID_AXIS {
            n_volumes_in_file = n_volumes_in_file.wrapping_mul(sizes_in_file[dd] as i32 as i64);
        } else {
            let vi = vi as usize;
            vol_sizes[vi] = sizes_in_file[dd] as i32;
            vol_seps[vi] = file_sep[dd];
            vol_starts[vi] = start_pos[dd];
            volume.set_direction_unit_cosine(vi as i32, &dircos[dd], c.dialect, c.streams);
        }
    }
    volume.set_separations(&vol_seps);
    volume.set_starts(&vol_starts);
    if consensus {
        volume.set_space_type(&prev_space);
    }
    let file_transform = volume::compute_world_transform(
        &file_spatial,
        &file_sep,
        &dircos,
        &start_pos,
        c.dialect,
        c.streams,
    );
    // The type.
    let no_volume_type = volume.data_type == volume::DataType::None;
    let (converted_type, converted_sign);
    if no_volume_type {
        let mut buf = Vec::new();
        let st = mi::miattgetstr(
            s,
            c,
            cdfid,
            img_var,
            b"signtype",
            MI_MAX_ATTSTR_LEN,
            &mut buf,
        )?;
        converted_sign = match st {
            Some(v) => v == mi::MI_SIGNED,
            None => file_datatype != NC_BYTE,
        };
        converted_type = file_datatype;
        if let Some(e) = volume.set_type(
            converted_type,
            converted_sign,
            0.0,
            0.0,
            c.dialect,
            c.streams,
        ) {
            return Err(e);
        }
        if volume.data_type == volume::DataType::None && c.notices.reproduce() {
            c.notices.add(
                &d::FS_781,
                format!(
                    "an image of type {converted_type}: the volume's data type is a value never written, read as none (reproduce)"
                ),
            );
        }
        if volume.data_type == volume::DataType::None && !c.notices.reproduce() {
            c.notices.add(
                &d::FS_781,
                format!(
                    "an image of type {converted_type} refused (volume_io's switch leaves the volume's data type a value it never wrote)"
                ),
            );
            c.dialect.print_error(
                &format!("Error: the MINC image's type ({converted_type}) is not a numeric type volume_io can read.\n"),
                c.streams,
            );
            return Ok(None);
        }
    } else {
        converted_type = volume.nc_data_type;
        converted_sign = volume.signed_flag;
    }
    volume.set_sizes(&vol_sizes);
    for dd in 0..n_file {
        let vi = to_volume_index[dd];
        if vi == INVALID_AXIS {
            continue;
        }
        if let Some(st) = irr_starts[dd].take() {
            if let Some(e) = c.dialect.irregular_spacing(false, c.streams) {
                return Err(e);
            }
            volume.irregular_starts[vi as usize] = Some(st);
        }
        if let Some(w) = irr_widths[dd].take() {
            if let Some(e) = c.dialect.irregular_spacing(true, c.streams) {
                return Err(e);
            }
            volume.irregular_widths[vi as usize] = Some(w);
        }
    }
    // The conversion variable.
    let icvid = icv::miicv_create(s);
    icv::miicv_setint(s, c, icvid, icv::MI_ICV_TYPE, converted_type)?;
    icv::miicv_setstr_sign(
        s,
        c,
        icvid,
        if converted_sign {
            mi::MI_SIGNED
        } else {
            mi::MI_UNSIGNED
        },
    )?;
    icv::miicv_setint(s, c, icvid, MI_ICV_DO_NORM, 1)?;
    icv::miicv_setint(s, c, icvid, MI_ICV_DO_FILLVALUE, 1)?;
    let (vr0, vr1) = volume.voxel_range();
    let range_specified = vr0 < vr1;
    let mut valid = [0.0f64; 2];
    if no_volume_type {
        valid = match mi::miget_valid_range(s, c, cdfid, img_var)? {
            Some(v) => v,
            None => [0.0, 0.0],
        };
    }
    if no_volume_type || !range_specified {
        volume.set_voxel_range(valid[0], valid[1]);
    }
    let (vmin, vmax) = volume.voxel_range();
    valid = [vmin, vmax];
    icv::miicv_setdbl(s, c, icvid, icv::MI_ICV_VALID_MIN, vmin)?;
    icv::miicv_setdbl(s, c, icvid, icv::MI_ICV_VALID_MAX, vmax)?;
    if options.user_real_range[0] < options.user_real_range[1] {
        icv::miicv_setint(s, c, icvid, icv::MI_ICV_USER_NORM, 1)?;
        icv::miicv_setdbl(
            s,
            c,
            icvid,
            icv::MI_ICV_IMAGE_MIN,
            options.user_real_range[0],
        )?;
        icv::miicv_setdbl(
            s,
            c,
            icvid,
            icv::MI_ICV_IMAGE_MAX,
            options.user_real_range[1],
        )?;
    }
    if options.convert_vector_to_scalar {
        icv::miicv_setint(s, c, icvid, MI_ICV_DO_DIM_CONV, 1)?;
        icv::miicv_setint(s, c, icvid, MI_ICV_DO_SCALAR, 1)?;
        icv::miicv_setint(s, c, icvid, icv::MI_ICV_XDIM_DIR, 0)?;
        icv::miicv_setint(s, c, icvid, icv::MI_ICV_YDIM_DIR, 0)?;
        icv::miicv_setint(s, c, icvid, icv::MI_ICV_ZDIM_DIR, 0)?;
        icv::miicv_setint(s, c, icvid, icv::MI_ICV_KEEP_ASPECT, 0)?;
    }
    icv::miicv_attach(s, c, icvid, cdfid, img_var)?;
    let real_min = icv::miicv_inq_norm(s, c, icvid, false)?.unwrap_or(0.0);
    let real_max = icv::miicv_inq_norm(s, c, icvid, true)?.unwrap_or(0.0);
    volume.set_real_range(real_min, real_max);
    if options.promote_invalid_to_zero {
        icv::miicv_detach(s, c, icvid)?;
        let voxel_zero = if real_min == real_max || real_min > 0.0 {
            valid[0]
        } else if real_max < 0.0 {
            valid[1]
        } else {
            valid[0] + (valid[1] - valid[0]) * (0.0 - real_min) / (real_max - real_min)
        };
        icv::miicv_setdbl(s, c, icvid, icv::MI_ICV_FILLVALUE, voxel_zero)?;
        icv::miicv_attach(s, c, icvid, cdfid, img_var)?;
    }
    s.ncopts = nc::NC_VERBOSE | nc::NC_FATAL;
    // How many dimensions to read at a time.
    let mut n_slab_dims = 0usize;
    let mut slab_size: i64 = 1;
    let mut dd = n_file as i64 - 1;
    loop {
        if dd < 0 {
            break;
        }
        if to_volume_index[dd as usize] != INVALID_AXIS {
            n_slab_dims += 1;
            slab_size =
                slab_size.wrapping_mul(sizes_in_file[dd as usize] as i32 as i64) as i32 as i64;
        }
        dd -= 1;
        if !(dd >= 0 && slab_size < MIN_SLAB_SIZE) {
            break;
        }
    }
    if slab_size > MAX_SLAB_SIZE && n_slab_dims > 1 {
        n_slab_dims -= 1;
    }
    let mut different = false;
    for k in 0..n_vol {
        if vol_sizes[k] != prev_sizes[k] {
            different = true;
        }
    }
    if prev_nc_type != converted_type {
        different = true;
    }
    if different && !volume.is_cached_volume {
        volume.data = None;
    }
    Ok(Some(MincFile {
        cdfid,
        img_var,
        filename: Vec::new(),
        dim_names,
        sizes_in_file,
        to_volume_index,
        to_file_index,
        spatial_axes: file_spatial,
        valid_file_axes,
        n_volumes_in_file,
        voxel_to_world_transform: file_transform,
        minc_icv: icvid,
        indices: vec![0; n_file],
        end_volume_flag: false,
        n_slab_dims,
        original_input_options: options.clone(),
        cache_file: None,
    }))
}

fn typed_to_vec(t: Option<Typed>, n: i64) -> Vec<f64> {
    let mut v = vec![0.0; n.max(0) as usize];
    if let Some(t) = t {
        for (i, x) in v.iter_mut().enumerate() {
            if i < t.len() {
                *x = t.get(i);
            }
        }
    }
    v
}

/// `input_more_of_volume`: whether more remains, and the fraction read.
pub fn input_more_of_volume(
    s: &mut Session,
    c: &mut Ctx,
    volume: &mut Volume,
    input: &mut VolumeInput,
) -> Result<(bool, f64), Exit> {
    match input.minc_file.as_mut() {
        Some(f) => input_more_minc_file(s, c, f, volume),
        None => Ok((false, 0.0)),
    }
}

/// `input_more_minc_file`.
fn input_more_minc_file(
    s: &mut Session,
    c: &mut Ctx,
    file: &mut MincFile,
    volume: &mut Volume,
) -> Result<(bool, f64), Exit> {
    if file.end_volume_flag {
        c.dialect
            .print_error("End of file in input_more_minc_file()\n", c.streams);
        return Ok((false, 0.0));
    }
    if volume.data.is_none() {
        let total = volume
            .sizes
            .iter()
            .fold(1u128, |n, &x| n * (x.max(0) as u128));
        if total >= 1u128 << 32 {
            c.notices.add(
                &d::FS_789,
                format!("a volume of {total} voxels allocated whole (volume_io counts its voxels in 32 bits)"),
            );
        }
        if volume.sizes.contains(&0) {
            // An empty dimension (a record dimension without records): volume_io allocates
            // nothing and reads through null pointers (a crash). Read as an empty volume.
            c.notices.add(
                &d::FS_790,
                "a volume with a dimension of size 0 read as empty (volume_io reads into data it never allocated)",
            );
        }
        let data_size = (volume.total_voxels() as u32 as u64) * volume.data_type.size() as u64;
        let cached = c
            .dialect
            .cache_threshold()
            .is_some_and(|t| t >= 0 && data_size as i128 > t as i128);
        // No data type (FS-781 with reproduce): the slab is read into nothing.
        let _ = volume.alloc(c.dialect, c.streams);
        if cached && volume.data.is_some() {
            // volume_io's cache: a second opening of the file on this volume, its blocks read
            // through that opening's conversion.
            volume.is_cached_volume = true;
            let name = file.filename.clone();
            let opts = file.original_input_options.clone();
            if let Some(mut f2) = initialize_minc_input(s, c, &name, volume, Some(&opts))? {
                loop {
                    let (more, _) = input_more_minc_file(s, c, &mut f2, volume)?;
                    if !more {
                        break;
                    }
                }
                file.cache_file = Some(Box::new(f2));
            }
            file.end_volume_flag = true;
            return Ok((false, 1.0));
        }
    }
    let n_file = file.dim_names.len();
    let mut count = vec![1i64; n_file];
    let mut n_slab = 0;
    let mut dd = n_file as i64 - 1;
    while dd >= 0 && n_slab < file.n_slab_dims {
        if file.to_volume_index[dd as usize] != INVALID_AXIS {
            count[dd as usize] = file.sizes_in_file[dd as usize];
            n_slab += 1;
        }
        dd -= 1;
    }
    let start = file.indices.clone();
    input_slab(s, c, file, volume, &start, &count)?;
    let mut increment = true;
    let mut n_slab = 0;
    let mut total: i64 = 1;
    let mut n_done: i64 = 0;
    for dd in (0..n_file).rev() {
        if n_slab >= file.n_slab_dims && file.to_volume_index[dd] != INVALID_AXIS {
            if increment {
                file.indices[dd] += 1;
                if file.indices[dd] < file.sizes_in_file[dd] {
                    increment = false;
                } else {
                    file.indices[dd] = 0;
                }
            }
            n_done = n_done.wrapping_add(total.wrapping_mul(file.indices[dd] as i32 as i64)) as i32
                as i64;
            total = total.wrapping_mul(file.sizes_in_file[dd] as i32 as i64) as i32 as i64;
        }
        if file.to_volume_index[dd] != INVALID_AXIS {
            n_slab += 1;
        }
    }
    let fraction = if increment {
        file.end_volume_flag = true;
        if let Some(mask) = volume.written.take()
            && mask.iter().any(|w| !w)
        {
            c.notices.add(
                &d::FS_796,
                format!(
                    "{}: voxels no slab wrote read as 0 (volume_io leaves the memory its array was given)",
                    String::from_utf8_lossy(&file.filename)
                ),
            );
        }
        1.0
    } else {
        n_done as f64 / total as f64
    };
    Ok((!file.end_volume_flag, fraction))
}

/// `input_slab` and `input_minc_hyperslab`: the hyperslab read through the ICV and copied into
/// the volume (the copy volume_io makes directly or through a buffer).
fn input_slab(
    s: &mut Session,
    c: &mut Ctx,
    file: &MincFile,
    volume: &mut Volume,
    start: &[i64],
    count: &[i64],
) -> Result<(), Exit> {
    let n_file = file.dim_names.len();
    // Whether volume_io reads straight into the volume's array (direct_to_array).
    let n_array = volume.n_dimensions();
    let mut direct = true;
    let mut expected = n_array as i64 - 1;
    let mut non_full = false;
    for fi in (0..n_file).rev() {
        let ind = file.to_volume_index[fi];
        if ind != INVALID_AXIS {
            if !non_full && count[fi] < file.sizes_in_file[fi] {
                non_full = true;
            } else if non_full && count[fi] > 1 {
                direct = false;
            }
            if count[fi] > 1 && ind as i64 != expected {
                direct = false;
            }
            expected -= 1;
        }
    }
    if volume.data.is_none() && !direct {
        // The buffer of the volume's (unset) type is allocated with the same message.
        c.dialect.print_error(
            "Error: cannot allocate array data until size specified.\n",
            c.streams,
        );
    }
    // The image's own dimensions (with a vector dimension the ICV converts).
    let mut st = start.to_vec();
    let mut ct = count.to_vec();
    if let Some(inq) = s
        .file(file.cdfid)
        .and_then(|f| f.vars.get(file.img_var as usize))
        && inq.dimids.len() > n_file
    {
        st.resize(inq.dimids.len(), 0);
        ct.resize(inq.dimids.len(), 1);
    }
    let Some(vals) = icv::miicv_get(s, c, file.minc_icv, &st, &ct)? else {
        return Ok(());
    };
    let reproduce = c.notices.reproduce();
    let Some(data) = volume.data.as_mut() else {
        return Ok(());
    };
    let written = &mut volume.written;
    // Each value's volume index, in the hyperslab's C order.
    let sizes: Vec<usize> = volume.sizes.iter().map(|&x| x.max(0) as usize).collect();
    let total = count.iter().product::<i64>().max(0) as usize;
    let mut places = Vec::with_capacity(total);
    let mut coord = vec![0i64; n_file];
    for _ in 0..total {
        let mut vv = vec![0usize; n_array];
        for fi in 0..n_file {
            let vi = file.to_volume_index[fi];
            if vi != INVALID_AXIS {
                vv[vi as usize] = (start[fi] + coord[fi]) as usize;
            }
        }
        let mut vidx = 0usize;
        for (k, &x) in vv.iter().enumerate() {
            vidx = vidx * sizes[k] + x;
        }
        places.push(vidx);
        for fi in (0..n_file).rev() {
            coord[fi] += 1;
            if coord[fi] < count[fi] {
                break;
            }
            coord[fi] = 0;
        }
    }
    // A read volume_io makes straight into the volume's array puts the values one after the
    // other from the slab's first voxel: where a dimension of size 1 hid the volume's order from
    // its check, that run is not the slab's place (FS-794).
    if direct && let Some(&first) = places.first() {
        let scrambled = places.iter().enumerate().any(|(j, &p)| p != first + j);
        if scrambled {
            let name = String::from_utf8_lossy(&file.filename).into_owned();
            if reproduce {
                if first + total > data.len() {
                    c.notices.add(
                        &d::FS_795,
                        format!("{name}: a slab written past the end of the volume's array (memory beside it); stopped at its end"),
                    );
                } else {
                    c.notices.add(
                        &d::FS_794,
                        format!("{name}: a slab read as one run into the volume (reproduce)"),
                    );
                }
                for (j, p) in places.iter_mut().enumerate() {
                    *p = first + j;
                }
                let n = data.len();
                let mask = written.get_or_insert_with(|| vec![false; n]);
                for &p in &places {
                    if p < n {
                        mask[p] = true;
                    }
                }
            } else {
                c.notices.add(
                    &d::FS_794,
                    format!("{name}: a slab put in its place (volume_io writes it as one run where a dimension of size 1 hides the volume's order)"),
                );
            }
        }
    }
    for (j, &p) in places.iter().enumerate() {
        if j < vals.len() && p < data.len() {
            data.put_from(p, &vals, j);
        }
    }
    Ok(())
}

/// `delete_volume_input` (`close_minc_input`).
pub fn delete_volume_input(s: &mut Session, c: &mut Ctx, input: VolumeInput) -> Result<(), Exit> {
    if let Some(f) = input.minc_file {
        mi::miclose(s, c, f.cdfid)?;
        icv::miicv_free(s, c, f.minc_icv)?;
        if let Some(cf) = f.cache_file {
            mi::miclose(s, c, cf.cdfid)?;
            icv::miicv_free(s, c, cf.minc_icv)?;
        }
    }
    Ok(())
}
