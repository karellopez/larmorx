// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI AND NetCDF
//! MINC 1 volumes read and written as MINC 1.5.1's volume_io reads and writes them: a port of
//! volume_io's input path (`start_volume_input`, `input_more_of_volume`, `get_volume_voxel_value`;
//! `Volumes/input_volume.c`, `input_mnc.c`, `volumes.c`) and output path (`output_volume`,
//! `Volumes/output_volume.c`, `output_mnc.c`), the MINC library under them
//! (`libsrc/netcdf_convenience.c`, `minc_convenience.c`, `image_conversion.c`, `dim_conversion.c`,
//! `value_conversion.c`, `minc_error.c`; MNI's notice, `LICENSES/LicenseRef-MNI.txt`) and netCDF
//! 3.6.0-p1's classic-format reader and writer ([`netcdf`], [`ncwrite`]; UCAR/Unidata's notice,
//! `LICENSES/NetCDF.txt`). The files are listed in `PROVENANCE.md`.
//!
//! A MINC 1 file is a netCDF file: an `image` variable over named dimensions (`xspace`, `yspace`,
//! `zspace`, `time`, `vector_dimension`, ...), the dimensions' variables holding `step`, `start`,
//! `direction_cosines` and `spacetype`, and `image-max` / `image-min` variables giving each slice's
//! real range. volume_io reads and writes the image through an image conversion variable ([`icv`]):
//! every slice's values mapped between its own real range and the volume's, stored in the volume's
//! type. MINC 2 files (HDF5) are not netCDF files and fail as such, as they do in MINC 1.5.1.
//!
//! - [`Session`]: the process state the C code keeps in globals (netCDF's `ncopts` and `ncerr`,
//!   the open files and their descriptors, MINC's routine name and call depth, the conversion
//!   variables). One session per program gives the original's messages; a fresh one per read or
//!   write is equivalent (every read and write sets `ncopts` itself).
//! - [`start_volume_input`], [`input_more_of_volume`], [`delete_volume_input`]: the volume input;
//!   [`Volume`] and its functions ([`volume`]).
//! - [`Volume::create_volume`], [`Volume::alloc_volume_data`], [`Volume::set_separations`],
//!   [`Volume::set_direction_cosine`], [`Volume::set_translation`], [`Volume::set_voxel_value`],
//!   [`output_volume`]: the volume output ([`output`], over [`mwrite`] and [`ncwrite`]). The file is
//!   written through posixio's buffer as netCDF writes it, so its bytes are MINC 1.5.1's; the NaNs
//!   that reach it follow volume_io's x86-64 object code on every platform.
//! - [`Dialect`]: the points where a program built on its own copy of MINC behaves differently
//!   (its package errors, decompression, the types and spacings it supports, its 4 x 4 inverse, the
//!   volumes it can write); [`Mni`] is MINC 1.5.1's own behaviour. A replica of such a program
//!   supplies its own dialect, in its own crate.
//!
//! Not ported yet: volume_io's free-format input (a `.fre` name: an error says so), the vector to
//! colour conversion and the dimension conversion of a vector dimension (only reached with a
//! dimension count below the file's), `output_modified_volume` with an original file (its auxiliary
//! data copied), the progress report of a long write (a measure of time). volume_io's volume cache
//! (a volume larger than `VOLUME_CACHE_THRESHOLD` bytes, 80,000,000 by default) is read as the cache
//! reads it: through a second opening of the file on the volume its first opening set up
//! ([`Dialect::cache_threshold`]; a float volume's voxel range is then its real range, which changes
//! how out-of-range voxels are read), into the whole array; written, a cached volume gives the same
//! file as one in memory, so it is allocated whole.
//!
//! The defects (register entries FS-774 to FS-801; `docs/findings/minc-volumes.md`) are corrected by
//! default and reproduced with [`larmorx_core::defects::UpstreamDefects::Reproduce`], each with a
//! notice.

#![allow(clippy::too_many_arguments, clippy::needless_range_loop)]

pub mod icv;
mod input;
pub mod libsrc;
pub mod mwrite;
pub mod ncwrite;
pub mod netcdf;
pub mod output;
pub mod volume;

#[cfg(test)]
mod tests;

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::Notices;

pub use input::{
    MincFile, MincInputOptions, VolumeInput, delete_volume_input, get_minc_file_n_dimensions,
    input_more_of_volume, start_volume_input,
};
pub use output::{MincOutputOptions, output_volume};
pub use volume::{DataType, Volume, VolumeData};

/// The program ends: `exit(status)` (netCDF's or MINC's fatal error, or a dialect's), its messages
/// already printed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exit {
    pub status: i32,
}

/// volume_io's `Status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok = 0,
    Error = 1,
}

/// A place where MINC logs a package error (`MI_LOG_PKG_ERROR2`) on the reading path, for a
/// [`Dialect`] that handles them in its own way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PkgSite {
    /// `miexpand_file`: the file could not be decompressed.
    Uncompress,
    /// `miattget_with_sign`: a character attribute read as a number.
    AttNonNumeric,
    /// `miattget1`: an attribute of more than one value read as one.
    AttNonScalar,
    /// `miattgetstr`: a numeric attribute read as a string.
    AttNonChar,
    /// `MI_varaccess`: a character variable read as numbers.
    VarNonNumeric,
    /// `MI_icv_get_type`: a character image attached to a conversion variable.
    IcvNonNumeric,
    /// `MI_icv_get_dim`: an image of fewer dimensions than the conversion's image dimensions.
    IcvTooFewDims,
    /// `MI_icv_get_dim`: a vector image of too few dimensions.
    IcvTooFewDimsVector,
    /// `MI_icv_access`: a read through a conversion variable that is not attached.
    IcvNotAttached,
    /// `miicv_set*`: a property changed while the variable is attached.
    IcvAttached,
    /// `miicv_set*` / `miicv_inq*`: an unknown property, or one of the other kind.
    IcvBadProperty,
    /// `MI_var_loop`: its buffer's allocation failed (`MI_log_sys_error1`; a variable of a type
    /// netCDF does not know sizes it from `nctypelen`'s `-1`).
    VarLoopAlloc,
    /// `miattget_with_sign`: its buffer's allocation failed (`MI_log_sys_error1`).
    AttGetAlloc,
    /// `micreate_std_variable`: a name that is not a standard MINC variable.
    BadStdVar,
    /// `MI_create_dim_variable`, `MI_create_dimwidth_variable`: a dimension variable of more than
    /// one dimension.
    WrongNDims,
    /// `MI_create_dimwidth_variable`: a name without the `-width` suffix.
    BadSuffix,
    /// `MI_verify_maxmin_dims`: `image-max` / `image-min` over the image's fastest dimensions.
    MaxMinDims,
}

/// The points where a program's copy of MINC behaves differently from MINC 1.5.1's. Every
/// method has MINC 1.5.1's behaviour by default ([`Mni`]).
pub trait Dialect {
    /// A package error at `site` (`MI_log_pkg_error2`, its code and message): `None` for MINC's
    /// own handling (`ncerr` set, the message printed with `NC_VERBOSE`, the program ended with
    /// `NC_FATAL`), or the program's own: its messages printed, `Some(exit)` when it ends there.
    fn pkg_error(
        &self,
        _site: PkgSite,
        _code: i32,
        _message: &str,
        _streams: &mut dyn Streams,
    ) -> Option<Exit> {
        None
    }
    /// Whether an error returned to MINC's caller is logged (`MINC package entry point`).
    fn logs_entry_point(&self) -> bool {
        true
    }
    /// Whether `miexpand_file` can decompress a file (MINC 1.5.1 runs `gunzip` and `bunzip2`).
    fn decompresses(&self, _notices: &mut Notices) -> bool {
        true
    }
    /// A volume type `set_volume_type` does not know (a character image): `Some(exit)` after the
    /// program's message, or `None` for MINC 1.5.1's (FS-781).
    fn bad_data_type(&self, _nc_type: i32, _streams: &mut dyn Streams) -> Option<Exit> {
        None
    }
    /// `miget_valid_range` of an image of `nc_type`, after its `valid_range` attribute was read:
    /// `Some(exit)` when the program's copy ends there (a type it has no default range for).
    fn default_range(
        &self,
        _nc_type: i32,
        _notices: &mut Notices,
        _streams: &mut dyn Streams,
    ) -> Option<Exit> {
        None
    }
    /// Irregular spacings of a dimension (`set_volume_irregular_starts`, then `_widths`):
    /// `Some(exit)` when the program does not support them.
    fn irregular_spacing(&self, _widths: bool, _streams: &mut dyn Streams) -> Option<Exit> {
        None
    }
    /// The temporary file `miexpand_file` would decompress into (named in messages).
    fn temporary_name(&self) -> Vec<u8> {
        b"/tmp/minc-XXXXXX".to_vec()
    }
    /// volume_io's `print_error`.
    fn print_error(&self, text: &str, streams: &mut dyn Streams) {
        streams.err(text);
    }
    /// volume_io's `print_error` of a message holding names (bytes as C prints them).
    fn print_error_bytes(&self, text: &[u8], streams: &mut dyn Streams) {
        streams.err_bytes(text);
    }
    /// volume_io's volume cache threshold (`get_n_bytes_cache_threshold`): a volume of more bytes
    /// is read through a second opening of its file, as volume_io's cache reads it; `None` for a
    /// copy without the cache. MINC 1.5.1: `VOLUME_CACHE_THRESHOLD` (`%d`), else 80,000,000.
    fn cache_threshold(&self) -> Option<i64> {
        let v = crate::xfm::files::getenv(b"VOLUME_CACHE_THRESHOLD")
            .and_then(|v| larmorx_core::cscan::sscanf_i32(&v));
        Some(v.unwrap_or(80_000_000) as i64)
    }
    /// `set_volume_direction_cosine` of a vector of length 0, after its warning: `Some(exit)` when
    /// the program's copy ends there.
    fn zero_direction_cosine(&self, _streams: &mut dyn Streams) -> Option<Exit> {
        None
    }
    /// The start of `output_volume`'s dimension names (the volume's names given): `Some(exit)` when
    /// the program's copy ends there.
    fn output_dimension_names(
        &self,
        _names: &[Vec<u8>],
        _notices: &mut Notices,
        _streams: &mut dyn Streams,
    ) -> Option<Exit> {
        None
    }
    /// The `.xfm` dialect of the program's volume_io: its 4 x 4 inverse decides whether a volume's
    /// voxel-to-world matrix is singular (`create_linear_transform` then makes it the identity),
    /// and its file names expand as its `expand_filename` does.
    fn xfm(&self) -> &dyn crate::xfm::Dialect {
        &crate::xfm::Mni
    }
}

/// MINC 1.5.1's own behaviour.
pub struct Mni;

impl Dialect for Mni {}

/// What the reading functions need besides the session: the program's streams, the defect
/// notices (and so the `reproduce` choice) and the dialect.
pub struct Ctx<'a> {
    pub streams: &'a mut dyn Streams,
    pub notices: &'a mut Notices,
    pub dialect: &'a dyn Dialect,
}

/// The state MINC and netCDF keep in globals for a program.
pub struct Session {
    /// netCDF's `ncopts` (`NC_FATAL | NC_VERBOSE` when a program starts).
    pub ncopts: i32,
    /// netCDF's `ncerr`.
    pub ncerr: i32,
    pub(crate) files: Vec<netcdf::NcFile>,
    /// The first file descriptor a file opens on (3 in a program whose only open files are its
    /// standard streams): netCDF's ids are descriptors and appear in its messages.
    pub first_fd: i32,
    fds: Vec<i32>,
    /// MINC's `minc_routine_name`, `minc_call_depth` and `minc_callers_ncopts`.
    pub(crate) routine: &'static str,
    pub(crate) depth: i32,
    pub(crate) callers_ncopts: i32,
    pub(crate) icvs: Vec<Option<icv::Icv>>,
    /// Notices netCDF's reader raised, passed on to the caller's notices by `miopen`.
    pub(crate) pending: Vec<(&'static larmorx_core::defects::Defect, String)>,
    /// Whether files opened now reproduce posixio's reads past their end (`reproduce`).
    pub(crate) reproduce: bool,
}

impl Default for Session {
    fn default() -> Session {
        Session::new()
    }
}

impl Session {
    pub fn new() -> Session {
        Session {
            ncopts: netcdf::NC_FATAL | netcdf::NC_VERBOSE,
            ncerr: 0,
            files: Vec::new(),
            first_fd: 3,
            fds: Vec::new(),
            routine: "MINC",
            depth: 0,
            callers_ncopts: 0,
            icvs: Vec::new(),
            pending: Vec::new(),
            reproduce: false,
        }
    }

    /// The notices netCDF's reader raised, moved to `notices`.
    pub(crate) fn drain_notices(&mut self, notices: &mut Notices) {
        for (d, t) in self.pending.drain(..) {
            notices.add(d, t);
        }
    }

    /// The lowest free descriptor (`open`).
    pub(crate) fn alloc_fd(&mut self) -> i32 {
        let mut fd = self.first_fd;
        while self.fds.contains(&fd) {
            fd += 1;
        }
        self.fds.push(fd);
        fd
    }

    pub(crate) fn free_fd(&mut self, fd: i32) {
        self.fds.retain(|&f| f != fd);
    }
}

/// The element count of a file's `image` variable as its netCDF header announces it (record
/// dimensions counted with the header's record count), or `None` when the file is not netCDF or has
/// no image variable: what a read would allocate before it reads anything.
pub fn header_image_size(path: &[u8]) -> Option<u128> {
    let mut s = Session::new();
    let id = netcdf::nc_open(&mut s, path, None).ok()?;
    let f = s.file(id)?;
    let v = f.vars.iter().find(|v| netcdf::cstr(&v.name) == b"image")?;
    let mut n: u128 = 1;
    for &d in &v.dimids {
        let size = f.dims.get(d as usize)?.size;
        n = n.saturating_mul(if size == 0 { f.numrecs } else { size } as u128);
    }
    Some(n)
}

/// The defect IDs of this module (`larmorx_core::defects::FS_774` to `FS_801`).
pub mod defects {
    /// `miexpand_file` decompresses through the shell with the name unquoted.
    pub const SHELL_DECOMPRESSION: &str = "FS-774";
    /// A variable whose begin is a 32-bit offset of 2 GiB or more is read at a negative offset.
    pub const NEGATIVE_OFFSET: &str = "FS-776";
    /// A variable whose element count wraps to 0 divides by zero.
    pub const ZERO_PRODUCT: &str = "FS-777";
    /// A header list longer than the reader can hold.
    pub const HUGE_LIST: &str = "FS-778";
    /// A file without an `image` variable: its dimension count is never written.
    pub const NO_IMAGE: &str = "FS-779";
    /// A `time` dimension without `spacing`: a buffer never written decides its spacing.
    pub const NO_SPACING: &str = "FS-780";
    /// A volume type volume_io does not know (a character image): its data type is never written.
    pub const BAD_TYPE: &str = "FS-781";
    /// A dimension name longer than 64 bytes overflows volume_io's buffer.
    pub const LONG_DIM_NAME: &str = "FS-782";
    /// An image of more than 512 dimensions overflows MINC's arrays.
    pub const TOO_MANY_DIMS: &str = "FS-783";
    /// An empty string attribute is checked one byte before its buffer.
    pub const EMPTY_STRING: &str = "FS-784";
    /// `image-max` / `image-min` over a dimension the image does not have.
    pub const FOREIGN_MAX_DIM: &str = "FS-785";
    /// A value that is not a number converted to an integer type.
    pub const NAN_TO_INT: &str = "FS-786";
    /// A valid range outside its type's converted to it.
    pub const RANGE_CAST: &str = "FS-787";
    /// FreeSurfer's copy cannot decompress a MINC file.
    pub const NO_DECOMPRESSION: &str = "FS-788";
    /// A volume of 2^32 voxels or more counts its voxels in 32 bits.
    pub const VOXEL_COUNT: &str = "FS-789";
    /// A dimension of size 0 (a record dimension without records).
    pub const EMPTY_DIMENSION: &str = "FS-790";
    /// Values past the end of a file shorter than its header.
    pub const PAST_END: &str = "FS-793";
    /// A slab read straight into the volume where a dimension of size 1 hides the order.
    pub const SCRAMBLED_SLAB: &str = "FS-794";
    /// ... and a run past the end of the volume's array.
    pub const SLAB_PAST_ARRAY: &str = "FS-795";
    /// ... and voxels no slab wrote (memory contents).
    pub const SLAB_UNWRITTEN: &str = "FS-796";
    /// FreeSurfer's copy refuses a MINC image of type double.
    pub const NO_DOUBLE: &str = "FS-775";
    /// A `direction_cosines` attribute of fewer than three values keeps the earlier dimension's.
    pub const SHORT_COSINES: &str = "FS-791";
    /// ... and for the first dimension read, memory never written.
    pub const SHORT_COSINES_UNSET: &str = "FS-792";
    /// FreeSurfer's copy cannot write a volume with a time dimension.
    pub const NO_TIME_OUTPUT: &str = "FS-797";
    /// A volume with a dimension of size 0 allocated or written.
    pub const EMPTY_ALLOCATION: &str = "FS-798";
    /// A volume whose voxels were never allocated, written.
    pub const UNALLOCATED_OUTPUT: &str = "FS-799";
    /// A volume without spatial axes: its voxel-to-world transform from memory never written.
    pub const NO_AXES_TRANSFORM: &str = "FS-800";
    /// A volume created without a type: its voxel range and sign never written.
    pub const NO_TYPE: &str = "FS-801";
}
