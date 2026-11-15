// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
//! volume_io's volumes (`Volumes/volumes.c`, `multidim_arrays.c`, `evaluate.c`; MNI's notice,
//! `LICENSES/LicenseRef-MNI.txt`): a typed array of up to five dimensions with names, the voxel
//! range its type stores and the real range it maps onto, and the voxel-to-world transform made
//! from each spatial axis's separation, start and direction cosine.

use larmorx_core::cfmt::Streams;

use super::libsrc::{FLT_MAX, Typed};
use super::netcdf::{NC_BYTE, NC_DOUBLE, NC_FLOAT, NC_INT, NC_SHORT};
use crate::xfm::{self, GeneralTransform, Transform, TransformData};

/// `MAX_DIMENSIONS`.
pub const MAX_DIMENSIONS: usize = 5;
/// `N_DIMENSIONS`: the spatial axes x, y, z.
pub const N_DIMENSIONS: usize = 3;
/// `MI_UNKNOWN_SPACE`.
pub const MI_UNKNOWN_SPACE: &[u8] = b"unknown___";
/// `ANY_SPATIAL_DIMENSION`.
pub const ANY_SPATIAL_DIMENSION: &[u8] = b"any_spatial_dimension";
/// `MI_ORIGINAL_TYPE` (`NC_UNSPECIFIED`): the file's own type.
pub const MI_ORIGINAL_TYPE: i32 = 0;

/// volume_io's `Data_types`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataType {
    None = 0,
    UnsignedByte = 1,
    SignedByte = 2,
    UnsignedShort = 3,
    SignedShort = 4,
    UnsignedInt = 5,
    SignedInt = 6,
    Float = 7,
    Double = 8,
}

impl DataType {
    /// `get_type_size`.
    pub fn size(self) -> usize {
        match self {
            DataType::None => 0,
            DataType::UnsignedByte | DataType::SignedByte => 1,
            DataType::UnsignedShort | DataType::SignedShort => 2,
            DataType::UnsignedInt | DataType::SignedInt | DataType::Float => 4,
            DataType::Double => 8,
        }
    }
}

/// A volume's voxels in its type, in C order of its dimensions.
#[derive(Clone, Debug, PartialEq)]
pub enum VolumeData {
    U8(Vec<u8>),
    I8(Vec<i8>),
    U16(Vec<u16>),
    I16(Vec<i16>),
    U32(Vec<u32>),
    I32(Vec<i32>),
    F32(Vec<f32>),
    F64(Vec<f64>),
}

impl VolumeData {
    fn new(t: DataType, n: usize) -> Option<VolumeData> {
        Some(match t {
            DataType::None => return None,
            DataType::UnsignedByte => VolumeData::U8(vec![0; n]),
            DataType::SignedByte => VolumeData::I8(vec![0; n]),
            DataType::UnsignedShort => VolumeData::U16(vec![0; n]),
            DataType::SignedShort => VolumeData::I16(vec![0; n]),
            DataType::UnsignedInt => VolumeData::U32(vec![0; n]),
            DataType::SignedInt => VolumeData::I32(vec![0; n]),
            DataType::Float => VolumeData::F32(vec![0.0; n]),
            DataType::Double => VolumeData::F64(vec![0.0; n]),
        })
    }
    pub fn len(&self) -> usize {
        match self {
            VolumeData::U8(v) => v.len(),
            VolumeData::I8(v) => v.len(),
            VolumeData::U16(v) => v.len(),
            VolumeData::I16(v) => v.len(),
            VolumeData::U32(v) => v.len(),
            VolumeData::I32(v) => v.len(),
            VolumeData::F32(v) => v.len(),
            VolumeData::F64(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The voxel at `i` as `(Real)` of its stored value (a float widened as x86 widens it).
    pub fn get(&self, i: usize) -> f64 {
        match self {
            VolumeData::U8(v) => v[i] as f64,
            VolumeData::I8(v) => v[i] as f64,
            VolumeData::U16(v) => v[i] as f64,
            VolumeData::I16(v) => v[i] as f64,
            VolumeData::U32(v) => v[i] as f64,
            VolumeData::I32(v) => v[i] as f64,
            VolumeData::F32(v) => super::libsrc::f32_to_f64(v[i]),
            VolumeData::F64(v) => v[i],
        }
    }
    /// Copies `src[j]` to the voxel `dst` (same types: the ICV reads in the volume's type).
    pub(crate) fn put_from(&mut self, dst: usize, src: &Typed, j: usize) {
        match (self, src) {
            (VolumeData::U8(a), Typed::U8(b)) => a[dst] = b[j],
            (VolumeData::I8(a), Typed::I8(b)) => a[dst] = b[j],
            (VolumeData::U16(a), Typed::U16(b)) => a[dst] = b[j],
            (VolumeData::I16(a), Typed::I16(b)) => a[dst] = b[j],
            (VolumeData::U32(a), Typed::U32(b)) => a[dst] = b[j],
            (VolumeData::I32(a), Typed::I32(b)) => a[dst] = b[j],
            (VolumeData::F32(a), Typed::F32(b)) => a[dst] = b[j],
            (VolumeData::F64(a), Typed::F64(b)) => a[dst] = b[j],
            _ => unreachable!("an ICV reads in the volume's type"),
        }
    }
}

/// volume_io's `volume_struct`.
#[derive(Clone, Debug)]
pub struct Volume {
    pub dimension_names: Vec<Vec<u8>>,
    pub data_type: DataType,
    pub nc_data_type: i32,
    pub signed_flag: bool,
    pub voxel_min: f64,
    pub voxel_max: f64,
    pub real_range_set: bool,
    pub real_value_scale: f64,
    pub real_value_translation: f64,
    pub sizes: Vec<i32>,
    pub separations: Vec<f64>,
    pub starts: Vec<f64>,
    pub direction_cosines: Vec<[f64; 3]>,
    pub spatial_axes: [i32; 3],
    pub irregular_starts: Vec<Option<Vec<f64>>>,
    pub irregular_widths: Vec<Option<Vec<f64>>>,
    pub(crate) voxel_to_world: GeneralTransform,
    pub(crate) uptodate: bool,
    pub coordinate_system_name: Vec<u8>,
    pub is_rgba_data: bool,
    /// Whether volume_io keeps the volume in its cache ([`super::Dialect::cache_threshold`]).
    pub is_cached_volume: bool,
    pub data: Option<VolumeData>,
    /// The voxels slabs wrote, kept while a scrambled read (FS-794, `reproduce`) leaves some unset.
    pub(crate) written: Option<Vec<bool>>,
}

/// `convert_dim_name_to_spatial_axis`.
pub fn spatial_axis(name: &[u8]) -> Option<usize> {
    match name {
        b"xspace" => Some(0),
        b"yspace" => Some(1),
        b"zspace" => Some(2),
        _ => None,
    }
}

/// `get_default_dim_names`.
pub fn default_dim_names(n: usize) -> Vec<Vec<u8>> {
    let t: [&[&[u8]]; 5] = [
        &[b"xspace"],
        &[b"yspace", b"xspace"],
        &[b"zspace", b"yspace", b"xspace"],
        &[b"", b"zspace", b"yspace", b"xspace"],
        &[b"", b"", b"zspace", b"yspace", b"xspace"],
    ];
    t[n - 1].iter().map(|s| s.to_vec()).collect()
}

/// The data type of a netCDF type and sign (`set_volume_type`'s switch); `None` for a type it
/// has no case for.
pub fn data_type_of(nc_type: i32, signed: bool) -> Option<DataType> {
    Some(match nc_type {
        NC_BYTE => {
            if signed {
                DataType::SignedByte
            } else {
                DataType::UnsignedByte
            }
        }
        NC_SHORT => {
            if signed {
                DataType::SignedShort
            } else {
                DataType::UnsignedShort
            }
        }
        NC_INT => {
            if signed {
                DataType::SignedInt
            } else {
                DataType::UnsignedInt
            }
        }
        NC_FLOAT => DataType::Float,
        NC_DOUBLE => DataType::Double,
        _ => return None,
    })
}

impl Volume {
    /// `create_volume` (the type set by [`Volume::set_type`] as `create_volume` does); `None`
    /// with volume_io's message for a dimension count out of 1 to 5. A type volume_io's switch
    /// has no case for leaves the data type unset (`NO_DATA_TYPE`).
    pub fn create(
        n_dimensions: i32,
        names: Option<&[Vec<u8>]>,
        nc_type: i32,
        signed: bool,
        voxel_min: f64,
        voxel_max: f64,
        dialect: &dyn super::Dialect,
        streams: &mut dyn Streams,
    ) -> Result<Option<Volume>, super::Exit> {
        if !(1..=MAX_DIMENSIONS as i32).contains(&n_dimensions) {
            dialect.print_error(
                &format!(
                    "create_volume(): n_dimensions ({n_dimensions}) not in range 1 to {MAX_DIMENSIONS}.\n"
                ),
                streams,
            );
            return Ok(None);
        }
        let n = n_dimensions as usize;
        let defaults = default_dim_names(n);
        let mut v = Volume {
            dimension_names: Vec::with_capacity(n),
            data_type: DataType::None,
            nc_data_type: MI_ORIGINAL_TYPE,
            signed_flag: false,
            voxel_min: 0.0,
            voxel_max: 0.0,
            real_range_set: false,
            real_value_scale: 1.0,
            real_value_translation: 0.0,
            sizes: vec![0; n],
            separations: vec![1.0; n],
            starts: vec![0.0; n],
            direction_cosines: vec![[0.0; 3]; n],
            spatial_axes: [-1; 3],
            irregular_starts: vec![None; n],
            irregular_widths: vec![None; n],
            voxel_to_world: GeneralTransform::linear(
                Some(&xfm::make_identity_transform()),
                &xfm::Mni,
                streams,
            ),
            uptodate: true,
            coordinate_system_name: MI_UNKNOWN_SPACE.to_vec(),
            is_rgba_data: false,
            is_cached_volume: false,
            data: None,
            written: None,
        };
        for i in 0..n {
            let name = match names {
                Some(ns) => ns.get(i).cloned().unwrap_or_default(),
                None => defaults[i].clone(),
            };
            if let Some(axis) = spatial_axis(&name) {
                v.spatial_axes[axis] = i as i32;
                v.direction_cosines[i][axis] = 1.0;
            }
            v.dimension_names.push(name);
        }
        if let Some(e) = v.set_type(nc_type, signed, voxel_min, voxel_max, dialect, streams) {
            return Err(e);
        }
        Ok(Some(v))
    }

    /// `get_volume_n_dimensions`.
    pub fn n_dimensions(&self) -> usize {
        self.sizes.len()
    }

    /// `set_volume_type`: `Some(exit)` when the dialect ends the program on a type volume_io does
    /// not know.
    pub fn set_type(
        &mut self,
        nc_type: i32,
        signed: bool,
        voxel_min: f64,
        voxel_max: f64,
        dialect: &dyn super::Dialect,
        streams: &mut dyn Streams,
    ) -> Option<super::Exit> {
        if nc_type != MI_ORIGINAL_TYPE {
            match data_type_of(nc_type, signed) {
                Some(t) => self.data_type = t,
                None => {
                    if let Some(e) = dialect.bad_data_type(nc_type, streams) {
                        return Some(e);
                    }
                    // The switch has no case: the data type is a variable never written
                    // (FS-781); left unset, which makes the data never allocated.
                    self.data_type = DataType::None;
                }
            }
            self.signed_flag = signed;
            self.set_voxel_range(voxel_min, voxel_max);
        }
        self.nc_data_type = nc_type;
        None
    }

    /// `set_volume_voxel_range`.
    pub fn set_voxel_range(&mut self, mut vmin: f64, mut vmax: f64) {
        if vmin >= vmax {
            let r = match self.data_type {
                DataType::UnsignedByte => Some((0.0, u8::MAX as f64)),
                DataType::SignedByte => Some((i8::MIN as f64, i8::MAX as f64)),
                DataType::UnsignedShort => Some((0.0, u16::MAX as f64)),
                DataType::SignedShort => Some((i16::MIN as f64, i16::MAX as f64)),
                DataType::UnsignedInt => Some((0.0, u32::MAX as f64)),
                DataType::SignedInt => Some((i32::MIN as f64, i32::MAX as f64)),
                DataType::Float => Some((-FLT_MAX, FLT_MAX)),
                DataType::Double => Some((-f64::MAX, f64::MAX)),
                DataType::None => None,
            };
            if let Some((a, b)) = r {
                vmin = a;
                vmax = b;
            }
        }
        let real = if self.real_range_set {
            Some(self.real_range())
        } else {
            None
        };
        self.voxel_min = vmin;
        self.voxel_max = vmax;
        if let Some((a, b)) = real {
            self.set_real_range(a, b);
        }
    }

    /// `get_volume_voxel_range`.
    pub fn voxel_range(&self) -> (f64, f64) {
        (self.voxel_min, self.voxel_max)
    }

    /// `convert_voxel_to_value`.
    pub fn voxel_to_value(&self, voxel: f64) -> f64 {
        if self.real_range_set {
            self.real_value_scale * voxel + self.real_value_translation
        } else {
            voxel
        }
    }

    /// `get_volume_real_range`.
    pub fn real_range(&self) -> (f64, f64) {
        (
            self.voxel_to_value(self.voxel_min),
            self.voxel_to_value(self.voxel_max),
        )
    }

    /// `set_volume_real_range`.
    pub fn set_real_range(&mut self, rmin: f64, rmax: f64) {
        if matches!(self.data_type, DataType::Float | DataType::Double) {
            self.set_voxel_range(rmin, rmax);
            self.real_value_scale = 1.0;
            self.real_value_translation = 0.0;
        } else {
            let (vmin, vmax) = self.voxel_range();
            if vmin < vmax {
                self.real_value_scale = (rmax - rmin) / (vmax - vmin);
                self.real_value_translation = rmin - vmin * self.real_value_scale;
            } else {
                self.real_value_scale = 0.0;
                self.real_value_translation = rmin;
            }
            self.real_range_set = true;
        }
    }

    /// `set_volume_separations`.
    pub fn set_separations(&mut self, s: &[f64]) {
        let n = self.n_dimensions();
        self.separations[..n].copy_from_slice(&s[..n]);
        self.uptodate = false;
    }

    /// `set_volume_starts`.
    pub fn set_starts(&mut self, s: &[f64]) {
        let n = self.n_dimensions();
        self.starts[..n].copy_from_slice(&s[..n]);
        self.uptodate = false;
    }

    /// `set_volume_direction_unit_cosine`: ignored for an axis that is not spatial; `false` (with
    /// volume_io's message) for an axis out of range.
    pub fn set_direction_unit_cosine(
        &mut self,
        axis: i32,
        dir: &[f64; 3],
        dialect: &dyn super::Dialect,
        streams: &mut dyn Streams,
    ) {
        if axis < 0 || axis as usize >= self.n_dimensions() {
            dialect.print_error(
                &format!("set_volume_direction_cosine:  cannot set dir cosine for axis {axis}\n"),
                streams,
            );
            return;
        }
        if !self.spatial_axes.contains(&axis) {
            return;
        }
        self.direction_cosines[axis as usize] = *dir;
        self.uptodate = false;
    }

    /// `set_volume_direction_cosine`: the direction made a unit vector (a zero vector refused with
    /// volume_io's warning), then [`Volume::set_direction_unit_cosine`].
    pub fn set_direction_cosine(
        &mut self,
        axis: i32,
        dir: &[f64; 3],
        dialect: &dyn super::Dialect,
        streams: &mut dyn Streams,
    ) {
        let mut len = dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2];
        if len == 0.0 {
            dialect.print_error(
                "Warning: zero length direction cosine in set_volume_direction_cosine()\n",
                streams,
            );
            return;
        }
        if len <= 0.0 {
            len = 1.0;
        }
        let len = len.sqrt();
        let unit = [dir[0] / len, dir[1] / len, dir[2] / len];
        self.set_direction_unit_cosine(axis, &unit, dialect, streams);
    }

    /// `set_volume_translation`: the starts that put `voxel` at `world` with the volume's
    /// separations and direction cosines (missing spatial axes completed orthogonally; a singular
    /// set of directions solved by the identity, as volume_io's inverse gives it).
    pub fn set_translation(
        &mut self,
        voxel: &[f64],
        world: &[f64; 3],
        dialect: &dyn super::Dialect,
        streams: &mut dyn Streams,
    ) {
        let mut t = xfm::make_identity_transform();
        let mut origin = [0.0f64; 3];
        for dim in 0..N_DIMENSIONS {
            origin[dim] = world[dim];
            for dim2 in 0..N_DIMENSIONS {
                let axis = self.spatial_axes[dim2];
                if axis >= 0 {
                    let a = axis as usize;
                    origin[dim] -= self.separations[a] * self.direction_cosines[a][dim] * voxel[a];
                    t[dim][dim2] = self.direction_cosines[a][dim];
                }
            }
        }
        let n_axes = self.spatial_axes.iter().filter(|&&a| a >= 0).count();
        let normalize = |t: &mut xfm::Transform, col: usize| {
            let mut len = t[0][col] * t[0][col] + t[1][col] * t[1][col] + t[2][col] * t[2][col];
            if len == 0.0 {
                len = 1.0;
            } else {
                len = len.sqrt();
            }
            t[0][col] /= len;
            t[1][col] /= len;
            t[2][col] /= len;
        };
        if n_axes == 1 {
            let dim = if self.spatial_axes[0] >= 0 {
                0
            } else if self.spatial_axes[1] >= 0 {
                1
            } else {
                2
            };
            let a1 = if dim == 0 { 1 } else { 0 };
            t[0][a1] = t[1][dim] + t[2][dim];
            t[1][a1] = -t[0][dim] - t[2][dim];
            t[2][a1] = t[1][dim] - t[0][dim];
            normalize(&mut t, a1);
        }
        if n_axes == 1 || n_axes == 2 {
            let dim = if self.spatial_axes[2] < 0 {
                2
            } else if self.spatial_axes[1] < 0 {
                1
            } else {
                0
            };
            let a1 = (dim + 1) % N_DIMENSIONS;
            let a2 = (dim + 2) % N_DIMENSIONS;
            t[0][dim] = t[1][a1] * t[2][a2] - t[1][a2] * t[2][a1];
            t[1][dim] = t[2][a1] * t[0][a2] - t[2][a2] * t[0][a1];
            t[2][dim] = t[0][a1] * t[1][a2] - t[0][a2] * t[1][a1];
            normalize(&mut t, dim);
        }
        let inverse = dialect
            .xfm()
            .invert(&t, streams)
            .unwrap_or_else(xfm::make_identity_transform);
        let s3 = xfm::transform_point(&inverse, origin[0], origin[1], origin[2]);
        let mut starts = vec![0.0; self.n_dimensions()];
        for dim in 0..N_DIMENSIONS {
            let axis = self.spatial_axes[dim];
            if axis >= 0 {
                starts[axis as usize] = s3[dim];
            }
        }
        self.set_starts(&starts);
    }

    /// `set_volume_voxel_value`: `value` stored in the volume's type as C's cast stores it on x86
    /// (an integer type through a 32-bit truncation, an unsigned int through a 64-bit one; a
    /// float rounded). Indices past the volume's dimensions are ignored; a voxel outside the
    /// volume is not written.
    pub fn set_voxel_value(&mut self, v: &[usize], value: f64) {
        let n = self.n_dimensions();
        if (0..n).any(|k| v[k] >= self.sizes[k].max(0) as usize) {
            return;
        }
        let i = self.index(v);
        let Some(d) = self.data.as_mut() else {
            return;
        };
        if i >= d.len() {
            return;
        }
        match d {
            VolumeData::U8(a) => a[i] = super::libsrc::cvt_i32(value) as u8,
            VolumeData::I8(a) => a[i] = super::libsrc::cvt_i32(value) as i8,
            VolumeData::U16(a) => a[i] = super::libsrc::cvt_i32(value) as u16,
            VolumeData::I16(a) => a[i] = super::libsrc::cvt_i32(value) as i16,
            VolumeData::U32(a) => a[i] = super::libsrc::cvt_i64(value) as u32,
            VolumeData::I32(a) => a[i] = super::libsrc::cvt_i32(value),
            VolumeData::F32(a) => a[i] = super::libsrc::f64_to_f32(value),
            VolumeData::F64(a) => a[i] = value,
        }
    }

    /// `get_volume_dimension_names`: the volume's names with each spatial axis's canonical
    /// (`xspace`, `yspace`, `zspace`).
    pub fn dimension_names_canonical(&self) -> Vec<Vec<u8>> {
        let mut names = self.dimension_names.clone();
        let canon: [&[u8]; 3] = [b"xspace", b"yspace", b"zspace"];
        for (i, c) in canon.iter().enumerate() {
            let a = self.spatial_axes[i];
            if a >= 0 {
                names[a as usize] = c.to_vec();
            }
        }
        names
    }

    /// `set_volume_sizes`.
    pub fn set_sizes(&mut self, sizes: &[i32]) {
        let n = self.n_dimensions();
        self.sizes[..n].copy_from_slice(&sizes[..n]);
    }

    /// `set_volume_space_type`.
    pub fn set_space_type(&mut self, name: &[u8]) {
        self.coordinate_system_name = name.to_vec();
    }

    /// `get_volume_total_n_voxels` (an `unsigned int`: a count of 2^32 or more wraps, FS-789).
    pub fn total_voxels(&self) -> u64 {
        self.sizes
            .iter()
            .fold(1u64, |n, &s| n.wrapping_mul(s as u32 as u64))
    }

    /// The voxel-to-world transform (`get_voxel_to_world_transform`).
    pub fn voxel_to_world_transform(
        &mut self,
        dialect: &dyn super::Dialect,
        streams: &mut dyn Streams,
    ) -> &GeneralTransform {
        self.check_recompute(dialect, streams);
        &self.voxel_to_world
    }

    /// The linear part of the voxel-to-world transform.
    pub fn voxel_to_world_matrix(
        &mut self,
        dialect: &dyn super::Dialect,
        streams: &mut dyn Streams,
    ) -> Transform {
        self.check_recompute(dialect, streams);
        match &self.voxel_to_world.data {
            TransformData::Linear { forward, .. } => **forward,
            _ => xfm::make_identity_transform(),
        }
    }

    fn check_recompute(&mut self, dialect: &dyn super::Dialect, streams: &mut dyn Streams) {
        if !self.uptodate {
            self.uptodate = true;
            let t = compute_world_transform(
                &self.spatial_axes,
                &self.separations,
                &self.direction_cosines,
                &self.starts,
                dialect,
                streams,
            );
            if let Some(t) = t {
                self.voxel_to_world = t;
            }
        }
    }

    /// `convert_voxel_to_world`.
    pub fn convert_voxel_to_world(
        &mut self,
        voxel: &[f64],
        dialect: &dyn super::Dialect,
        streams: &mut dyn Streams,
    ) -> [f64; 3] {
        self.check_recompute(dialect, streams);
        let mut xyz = [0.0; 3];
        for c in 0..N_DIMENSIONS {
            let a = self.spatial_axes[c];
            xyz[c] = if a >= 0 { voxel[a as usize] } else { 0.0 };
        }
        let _ = streams;
        xfm::general_transform_point(&self.voxel_to_world, false, xyz[0], xyz[1], xyz[2])
            .unwrap_or(xyz)
    }

    /// The index of a voxel in [`Volume::data`].
    pub fn index(&self, v: &[usize]) -> usize {
        let mut i = 0usize;
        for (k, &s) in self.sizes.iter().enumerate() {
            i = i * s.max(0) as usize + v[k];
        }
        i
    }

    /// `get_volume_voxel_value` (indices past the volume's dimensions ignored).
    pub fn voxel_value(&self, v: &[usize]) -> f64 {
        match &self.data {
            Some(d) => d.get(self.index(v)),
            None => 0.0,
        }
    }

    /// `alloc_volume_data`: `false` (with volume_io's message) for a volume without a data type.
    /// (volume_io keeps a volume of more than its cache threshold in its volume cache; written, a
    /// cached volume gives the same file, so it is allocated whole here.)
    pub fn alloc(&mut self, dialect: &dyn super::Dialect, streams: &mut dyn Streams) -> bool {
        if self.data_type == DataType::None {
            dialect.print_error(
                "Error: cannot allocate array data until size specified.\n",
                streams,
            );
            return false;
        }
        let n = self.total_voxels() as usize;
        self.data = VolumeData::new(self.data_type, n);
        true
    }
}

/// `compute_world_transform`: `None` (with volume_io's message) when no axis is spatial.
pub fn compute_world_transform(
    spatial_axes: &[i32; 3],
    separations: &[f64],
    direction_cosines: &[[f64; 3]],
    starts: &[f64],
    dialect: &dyn super::Dialect,
    streams: &mut dyn Streams,
) -> Option<GeneralTransform> {
    let mut sep3 = [1.0f64; 3];
    let mut start3 = [0.0f64; 3];
    let mut dirs = [[0.0f64; 3]; 3];
    let mut axis_list = Vec::new();
    for c in 0..N_DIMENSIONS {
        let a = spatial_axes[c];
        if a >= 0 {
            let a = a as usize;
            sep3[c] = separations[a];
            start3[c] = starts[a];
            dirs[c] = direction_cosines[a];
            axis_list.push(c);
        }
    }
    if axis_list.is_empty() {
        dialect.print_error("error compute_world_transform:  no axes.\n", streams);
        return None;
    }
    let cross = |a: &[f64; 3], b: &[f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let normalize = |v: &[f64; 3]| {
        let mut mag = 0.0;
        for x in v {
            mag += x * x;
        }
        if mag <= 0.0 {
            mag = 1.0;
        }
        let mag = mag.sqrt();
        [v[0] / mag, v[1] / mag, v[2] / mag]
    };
    if axis_list.len() == 1 {
        let a0 = axis_list[0];
        let a1 = (a0 + 1) % N_DIMENSIONS;
        let a2 = (a0 + 2) % N_DIMENSIONS;
        let d = dirs[a0];
        dirs[a1] = [d[1] + d[2], -d[0] - d[2], d[1] - d[0]];
        dirs[a2] = cross(&dirs[a0], &dirs[a1]);
        dirs[a1] = normalize(&dirs[a1]);
        dirs[a2] = normalize(&dirs[a2]);
    } else if axis_list.len() == 2 {
        let a2 = N_DIMENSIONS - axis_list[0] - axis_list[1];
        dirs[a2] = cross(&dirs[axis_list[0]], &dirs[axis_list[1]]);
        dirs[a2] = normalize(&dirs[a2]);
    }
    let mut singular = false;
    for dim in 0..N_DIMENSIONS {
        let n = cross(&dirs[dim], &dirs[(dim + 1) % N_DIMENSIONS]);
        if n[0] == 0.0 && n[1] == 0.0 && n[2] == 0.0 {
            singular = true;
            break;
        }
    }
    if singular {
        dirs = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    }
    let mut t = xfm::make_identity_transform();
    for c in 0..N_DIMENSIONS {
        for dim in 0..N_DIMENSIONS {
            t[dim][c] = dirs[c][dim] * sep3[c];
            t[dim][3] += dirs[c][dim] * start3[c];
        }
    }
    Some(GeneralTransform::linear(Some(&t), dialect.xfm(), streams))
}
