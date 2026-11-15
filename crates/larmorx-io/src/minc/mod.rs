// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI AND NetCDF
//! MINC 1 volumes read as MINC 1.5.1's volume_io reads them (`start_volume_input`,
//! `input_more_of_volume`, `get_volume_voxel_value`): a port of volume_io's input path
//! (`Volumes/input_volume.c`, `input_mnc.c`, `volumes.c`), the MINC library it reads through
//! (`libsrc/netcdf_convenience.c`, `minc_convenience.c`, `image_conversion.c`, `dim_conversion.c`,
//! `value_conversion.c`, `minc_error.c`; MNI's notice, `LICENSES/LicenseRef-MNI.txt`) and netCDF
//! 3.6.0-p1's classic-format reader under it ([`netcdf`]; UCAR/Unidata's notice,
//! `LICENSES/NetCDF.txt`). The files are listed in `PROVENANCE.md`.
//!
//! A MINC 1 file is a netCDF file: an `image` variable over named dimensions (`xspace`, `yspace`,
//! `zspace`, `time`, `vector_dimension`, ...), the dimensions' variables holding `step`, `start`,
//! `direction_cosines` and `spacetype`, and `image-max` / `image-min` variables giving each slice's
//! real range. volume_io reads the image through an image conversion variable ([`icv`]): every
//! slice's values mapped onto one real range for the whole volume, stored in the volume's type.
//! MINC 2 files (HDF5) are not netCDF files and fail as such, as they do in MINC 1.5.1.
//!
//! - [`Session`]: the process state the C code keeps in globals (netCDF's `ncopts` and `ncerr`,
//!   the open files and their descriptors, MINC's routine name and call depth, the conversion
//!   variables). One session per program gives the original's messages; a fresh one per read is
//!   equivalent for the reading path (every read sets `ncopts` itself).
//! - [`start_volume_input`], [`input_more_of_volume`], [`delete_volume_input`]: the volume input;
//!   [`Volume`] and its functions ([`volume`]).
//! - [`Dialect`]: the points where a program built on its own copy of MINC behaves differently
//!   (its package errors, decompression, the types and spacings it supports); [`Mni`] is MINC
//!   1.5.1's own behaviour. A replica of such a program supplies its own dialect, in its own crate.
//!
//! Not ported yet: volume_io's free-format input (a `.fre` name: an error says so), the vector to
//! colour conversion and the dimension conversion of a vector dimension (only reached with a
//! dimension count below the file's), and the volume cache (volume_io keeps a volume larger than
//! `VOLUME_CACHE_THRESHOLD` bytes, 80,000,000 by default, in a cache read on demand: the values are
//! those of the whole read, which this port always does).
//!
//! The defects (register entries FS-774 to FS-790; `docs/findings/minc-volumes.md`) are corrected by
//! default and reproduced with [`larmorx_core::defects::UpstreamDefects::Reproduce`], each with a
//! notice.

#![allow(clippy::too_many_arguments, clippy::needless_range_loop)]

pub mod icv;
mod input;
pub mod libsrc;
pub mod netcdf;
pub mod volume;

#[cfg(test)]
mod tests;

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::Notices;

pub use input::{
    MincFile, MincInputOptions, VolumeInput, delete_volume_input, get_minc_file_n_dimensions,
    input_more_of_volume, start_volume_input,
};
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

/// The defect IDs of this module (`larmorx_core::defects::FS_774` to `FS_790`).
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
}
