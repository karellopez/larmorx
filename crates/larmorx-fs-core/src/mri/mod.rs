// SPDX-License-Identifier: Apache-2.0
//! FreeSurfer 8.2.0's volume structure (`fs.mri`), a clean room of `specs/freesurfer-mri.md`: the
//! geometry and its voxel-to-RAS matrices (§2, §3), the volume and its allocation (§4), voxel
//! access (§5), the header and colour-table copies (§6, §7), the tkregister and FSL matrices (§9),
//! world and voxel coordinates (§10), the geometry's printing and comparisons (§11), the
//! interpolation and precision names (§12), the full copy (§13), the volume's MINC transform
//! (§14), shapes (§15), the bounding box (§17) and the linear transform arrays (§18-22).
//!
//! Every function that prints, ends the program or meets a defect is a method of the program's
//! [`Process`](crate::Process); a call that ends the program in FreeSurfer returns
//! [`Stop`](crate::Stop). Where FreeSurfer reads memory it does not own, the clean room gives
//! the values the spec defines for both modes (`outside_val`, 0, nothing written).

pub mod coords;
pub mod copy;
pub mod ctab;
pub mod geom;
pub mod lta;
pub mod names;
pub mod tkreg;
pub mod volume;
pub mod voxels;
pub mod xform;

pub use ctab::{ColorTable, ColorTableEntry};
pub use geom::VolGeom;
pub use lta::{LinearTransform, Lta};
pub use volume::{Frame, Mri, Region};
pub use xform::{MincTransform, TransformKind};

/// The voxel types (§4).
pub mod types {
    pub const UCHAR: i32 = 0;
    pub const INT: i32 = 1;
    pub const LONG: i32 = 2;
    pub const FLOAT: i32 = 3;
    pub const SHORT: i32 = 4;
    pub const BITMAP: i32 = 5;
    pub const TENSOR: i32 = 6;
    pub const FLOAT_COMPLEX: i32 = 7;
    pub const DOUBLE_COMPLEX: i32 = 8;
    pub const RGB: i32 = 9;
    pub const USHRT: i32 = 10;
}

/// `MRIsizeof(type)` (§4): the bytes of one voxel, `size_t` -1 (as `u64::MAX`) for the types
/// without a size (bitmap, double complex, unknown codes).
pub fn sizeof(kind: i32) -> u64 {
    match kind {
        types::UCHAR => 1,
        types::INT => 4,
        types::LONG => 8,
        types::FLOAT => 4,
        types::SHORT => 2,
        types::TENSOR => 4,
        types::FLOAT_COMPLEX => 8,
        types::RGB => 4,
        types::USHRT => 2,
        _ => u64::MAX,
    }
}

/// Whether a voxel type has a size.
pub fn has_size(kind: i32) -> bool {
    sizeof(kind) != u64::MAX
}
