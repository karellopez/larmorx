// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
//! MNI transform files (`.xfm`), read and written as MINC 1.5.1's volume_io reads and writes them
//! (MNI's notice: `LICENSES/LicenseRef-MNI.txt`; the files in `PROVENANCE.md`).
//!
//! An `.xfm` file is text: the header `MNI Transform File`, comments (`%` or `#` to the end of a
//! line), then one or more transforms, each `Transform_Type = <type>;`, an optional
//! `Invert_Flag = True;` and the type's data; several transforms in one file are concatenated.
//! Volume_io knows three types: `Linear` (a 3 x 4 affine), `Thin_Plate_Spline_Transform` and
//! `Grid_Transform` (a MINC displacement volume).
//!
//! - [`input_transform_file`] / [`input_transform_from_bytes`]: the reader, every message and failure as
//!   volume_io's (`gen_xf_io.c`, `mni_io.c`, `files.c`), with D13's corrections of its defects
//!   ([`defects`]); [`output_transform_file`] / [`output_transform`]: the writer, its numbers
//!   with C's `%.15g`.
//! - [`GeneralTransform`] and its operations (`gen_xfs.c`): concatenation (two linear transforms
//!   in a row are multiplied into one, their inverses too), copies, inverses, the linear parts;
//!   [`Transform`] and `transforms.c`'s arithmetic ([`concat_transforms`], [`transform_point`],
//!   [`make_identity_transform`]); [`compute_transform_inverse`] by `gaussian.c`'s scaled
//!   maximal-pivoting Gaussian elimination.
//! - [`Dialect`]: the points where a program built on volume_io may change it (which transform
//!   types it reads, its 4 x 4 inverse, the wording of its messages, how it reads a token and
//!   expands `~user`); [`Mni`] is volume_io's own behaviour. A replica of such a program supplies
//!   its own dialect, in its own crate.
//!
//! Not read: the displacement volume of a `Grid_Transform` (MINC volumes are not a larmorx format
//! yet: the reader reports it, [`Dialect::read_grid_volume`]); not applied: thin-plate splines and
//! grids ([`general_transform_point`] reports them).

// volume_io's functions return OK or ERROR, their messages already printed: `Result<_, ()>` is that
// status. The index loops are the C code's.
#![allow(clippy::result_unit_err, clippy::needless_range_loop)]

mod files;
mod gaussian;
mod read;
mod write;

pub use files::{
    IoType, OpenFile, expand_filename, expand_filename_in, extract_directory, file_exists,
    open_file, open_file_with_default_suffix, remove_directories_from_filename,
};
pub use gaussian::invert_square_matrix;
pub use read::{input_transform_file, input_transform_from_bytes};
pub use write::{output_transform, output_transform_file, output_transform_to_string};

use larmorx_core::cfmt::Streams;

/// D13 defects of volume_io's `.xfm` code that larmorx corrects (`docs/findings/minc-xfm.md`;
/// their register entries are `larmorx_core::defects::FS_71` to `FS_75` and `FS_79`).
pub mod defects {
    /// The decompression of a compressed file runs `gunzip -c <name> > <temporary>` through the
    /// shell with the name unquoted (`files.c`, `open_file`): a name with a blank fails (or reads
    /// other files) and a name with shell syntax runs it. Corrected: decompressed in the process.
    pub const SHELL_DECOMPRESSION: &str = "FS-71";
    /// A thin-plate spline's `Points` or `Displacements` list that reaches the end of the file
    /// without its `;` makes `mni_input_reals` read the same pushed-back character forever,
    /// growing the list until memory runs out. Corrected: an error at the end of the file.
    pub const READS_FOREVER_AT_EOF: &str = "FS-72";
    /// `Number_Dimensions` of 0 divides by zero (a crash); a negative one allocates negative
    /// sizes. Corrected: an error.
    pub const BAD_DIMENSIONS: &str = "FS-73";
    /// An empty `Points` list frees a pointer that was never set (`input_one_transform`).
    /// Corrected: a spline with no points (its affine part only), which the format allows.
    pub const EMPTY_POINTS: &str = "FS-74";
    /// A `$VAR` (or `~`) that expands to nothing at the start of a file name writes through a
    /// null pointer (`expand_filename` never allocated its result). Corrected: it expands to
    /// nothing.
    pub const EMPTY_EXPANSION: &str = "FS-75";
    /// A backslash in a file name is volume_io's escape (dropped) and only `/` separates
    /// directories, so a Windows path loses its separators. Corrected on Windows: a backslash
    /// separates directories (`\\` and a drive letter also start an absolute name).
    pub const WINDOWS_BACKSLASH: &str = "FS-79";
}

/// `Transform`: a 4 x 4 matrix, `t[i][j]` = volume_io's `Transform_elem(t, i, j)` (row `i`,
/// column `j`; points are columns, so the translation is column 3).
pub type Transform = [[f64; 4]; 4];

/// volume_io's `Status` (its values, as the functions return them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok = 0,
    Error = 1,
    InternalError = 2,
    EndOfFile = 3,
    Quit = 4,
}

/// The three file types and the in-memory one (`Transform_types`, without user transforms,
/// which have no file form).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformType {
    Linear = 0,
    ThinPlateSpline = 1,
    Concatenated = 3,
    Grid = 4,
}

/// A transform's data (`General_transform` less its type tag and inverse flag).
#[derive(Clone, Debug, PartialEq)]
pub enum TransformData {
    /// The matrix and its inverse (computed when it was made, or multiplied from the inverses of
    /// the transforms it was concatenated from).
    Linear {
        forward: Box<Transform>,
        inverse: Box<Transform>,
    },
    /// A thin-plate spline: `points` (n_points x n_dimensions) and `displacements`
    /// ((n_points + n_dimensions + 1) x n_dimensions).
    ThinPlateSpline {
        n_dimensions: usize,
        points: Vec<Vec<f64>>,
        displacements: Vec<Vec<f64>>,
    },
    /// A grid transform: the displacement volume's file name, as the reader resolved it, and
    /// whatever the dialect read from it.
    Grid {
        volume_filename: Vec<u8>,
        volume: GridVolume,
    },
    Concatenated(Vec<GeneralTransform>),
}

/// volume_io's `General_transform`.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneralTransform {
    pub data: TransformData,
    pub inverse_flag: bool,
}

/// What a [`Dialect`] read from a grid transform's displacement volume (opaque here; two are
/// equal when they are the same object).
#[derive(Clone)]
pub struct GridVolume(pub std::sync::Arc<dyn std::any::Any + Send + Sync>);

impl PartialEq for GridVolume {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for GridVolume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GridVolume(..)")
    }
}

/// The points where programs built on volume_io changed it; [`Mni`] is volume_io 1.5.1's own
/// behaviour.
pub trait Dialect {
    /// `print_error`: volume_io formats a message into a C string and prints it on standard
    /// error, so it ends at its first NUL (a `%c` of a NUL byte cuts it there).
    fn print_error(&self, text: &str, streams: &mut dyn Streams) {
        streams.err(text.split('\0').next().unwrap_or(""));
    }
    /// `mni_input_keyword_and_equal_sign` did not find `keyword =` (`got`: the token read, `None`
    /// when the end of the file came in it); volume_io prints only when its caller asks.
    fn keyword_mismatch(
        &self,
        keyword: &[u8],
        got: Option<&[u8]>,
        print_error_message: bool,
        streams: &mut dyn Streams,
    ) {
        let _ = got;
        if print_error_message {
            self.print_error(
                &format!("Expected \"{} =\"\n", String::from_utf8_lossy(keyword)),
                streams,
            );
        }
    }
    /// `get_user_home_directory` (for `~user`): the password file's home directory, `None` for an
    /// unknown user (the `~user` is then kept as written).
    fn user_home_directory(&self, user: &[u8]) -> Option<Vec<u8>> {
        files::user_home_directory(user)
    }
    /// Whether a NUL byte in a token ends the token (the rest of it is read and lost); in
    /// volume_io it adds nothing and the token goes on.
    fn nul_ends_token(&self) -> bool {
        false
    }
    /// `compute_transform_inverse`: the inverse, or `None` when the matrix is singular (the
    /// caller then uses the identity for both, as `create_linear_transform` does).
    fn invert(&self, t: &Transform, _streams: &mut dyn Streams) -> Option<Transform> {
        compute_transform_inverse(t)
    }
    /// The type named after `Transform_Type =`, or `None` after printing the dialect's message.
    fn transform_type(&self, name: &[u8], streams: &mut dyn Streams) -> Option<TransformType> {
        match name {
            b"Linear" => Some(TransformType::Linear),
            b"Thin_Plate_Spline_Transform" => Some(TransformType::ThinPlateSpline),
            b"Grid_Transform" => Some(TransformType::Grid),
            _ => {
                self.print_error("input_transform(): invalid transform type.\n", streams);
                None
            }
        }
    }
    /// Called when a number's token does not convert (`mni_input_real`); volume_io prints
    /// nothing there (its callers do).
    fn bad_real(&self, _token: &[u8], _streams: &mut dyn Streams) {}
    /// Reads a grid transform's displacement volume (`input_volume`); volume_io reads a MINC
    /// file, which larmorx cannot read yet: an error.
    fn read_grid_volume(
        &self,
        filename: &[u8],
        streams: &mut dyn Streams,
    ) -> Result<GridVolume, ()> {
        streams.err(&format!(
            "larmorx: the MINC displacement volume of a Grid_Transform is not read: {}\n",
            String::from_utf8_lossy(filename)
        ));
        Err(())
    }
    /// Writes a grid transform's displacement volume (`output_volume`, a MINC file), which
    /// larmorx cannot do yet: an error.
    fn write_grid_volume(
        &self,
        filename: &[u8],
        _volume: &GridVolume,
        streams: &mut dyn Streams,
    ) -> Result<(), ()> {
        streams.err(&format!(
            "larmorx: the MINC displacement volume of a Grid_Transform is not written: {}\n",
            String::from_utf8_lossy(filename)
        ));
        Err(())
    }
    /// The temporary file's name a failed decompression message names (volume_io's
    /// `micreate_tempfile`: `$TMPDIR/minc-XXXXXX`, random; larmorx decompresses in memory and
    /// names the pattern).
    fn temporary_name(&self) -> Vec<u8> {
        let dir = files::getenv(b"TMPDIR").unwrap_or_else(|| b"/tmp".to_vec());
        let mut n = dir;
        n.extend_from_slice(b"/minc-XXXXXX");
        n
    }
}

/// volume_io 1.5.1's own behaviour.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mni;

impl Dialect for Mni {}

/// `make_identity_transform`.
pub fn make_identity_transform() -> Transform {
    let mut t = [[0.0; 4]; 4];
    for (i, row) in t.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    t
}

/// `close_to_identity`: every element within 0.001 of the identity's.
pub fn close_to_identity(t: &Transform) -> bool {
    const TOLERANCE: f64 = 0.001;
    let mut close = true;
    for (i, row) in t.iter().enumerate() {
        for (j, &v) in row.iter().enumerate() {
            let expected = if i == j { 1.0 } else { 0.0 };
            if v < expected - TOLERANCE || v > expected + TOLERANCE {
                close = false;
            }
        }
    }
    close
}

/// `concat_transforms(result, t1, t2)`: `t2 × t1` (apply `t1`, then `t2`), each element summed
/// over `k` = 0..3 from 0.0 in that order.
pub fn concat_transforms(t1: &Transform, t2: &Transform) -> Transform {
    let mut r = [[0.0; 4]; 4];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, out) in row.iter_mut().enumerate() {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += t2[i][k] * t1[k][j];
            }
            *out = sum;
        }
    }
    r
}

/// `homogenous_transform_point`: `t × (x, y, z, w)`, divided by the result's `w` unless that is
/// 0 or 1.
pub fn homogenous_transform_point(t: &Transform, x: f64, y: f64, z: f64, w: f64) -> [f64; 3] {
    let mut xt = t[0][0] * x + t[0][1] * y + t[0][2] * z + t[0][3] * w;
    let mut yt = t[1][0] * x + t[1][1] * y + t[1][2] * z + t[1][3] * w;
    let mut zt = t[2][0] * x + t[2][1] * y + t[2][2] * z + t[2][3] * w;
    let wt = t[3][0] * x + t[3][1] * y + t[3][2] * z + t[3][3] * w;
    if wt != 0.0 && wt != 1.0 {
        xt /= wt;
        yt /= wt;
        zt /= wt;
    }
    [xt, yt, zt]
}

/// `transform_point`: the point `(x, y, z, 1)`.
pub fn transform_point(t: &Transform, x: f64, y: f64, z: f64) -> [f64; 3] {
    homogenous_transform_point(t, x, y, z, 1.0)
}

/// `compute_transform_inverse` (`inverse.c`): the inverse by [`invert_square_matrix`], or `None`
/// when the elimination meets a zero pivot (volume_io then returns the identity).
pub fn compute_transform_inverse(t: &Transform) -> Option<Transform> {
    let m: Vec<Vec<f64>> = t.iter().map(|r| r.to_vec()).collect();
    let inv = invert_square_matrix(&m)?;
    let mut out = [[0.0; 4]; 4];
    for (i, row) in out.iter_mut().enumerate() {
        row.copy_from_slice(&inv[i]);
    }
    Some(out)
}

/// `print_error`'s internal-error report and `abort_if_allowed` (volume_io's `print.c`): the
/// message, then, when `ABORT_FLAG` is set, the question on the terminal (answered from standard
/// input: `y` aborts the program; the end of the input counts as `n`, where volume_io waits
/// forever).
pub fn handle_internal_error(what: &str, streams: &mut dyn Streams) {
    streams.err(&format!("Internal error:  {what}\n"));
    if files::getenv(b"ABORT_FLAG").is_some() {
        streams.err("Do you wish to abort (y/n): ");
        use std::io::Read;
        let mut stdin = std::io::stdin().lock();
        let mut byte = [0u8; 1];
        let mut answer = b'n';
        loop {
            match stdin.read(&mut byte) {
                Ok(1) if byte[0] == b'y' || byte[0] == b'n' => {
                    answer = byte[0];
                    break;
                }
                Ok(1) => continue,
                _ => break,
            }
        }
        if answer == b'y' || answer == b'n' {
            loop {
                match stdin.read(&mut byte) {
                    Ok(1) if byte[0] != b'\n' => continue,
                    _ => break,
                }
            }
        }
        if answer == b'y' {
            std::process::abort();
        }
    }
}

impl GeneralTransform {
    /// `create_linear_transform`: the matrix and its inverse; a singular matrix (or none) gives
    /// the identity for both.
    pub fn linear(t: Option<&Transform>, dialect: &dyn Dialect, streams: &mut dyn Streams) -> Self {
        let (forward, inverse) = match t.map(|t| (t, dialect.invert(t, streams))) {
            Some((t, Some(inv))) => (*t, inv),
            _ => (make_identity_transform(), make_identity_transform()),
        };
        GeneralTransform {
            data: TransformData::Linear {
                forward: Box::new(forward),
                inverse: Box::new(inverse),
            },
            inverse_flag: false,
        }
    }

    /// `get_transform_type`.
    pub fn transform_type(&self) -> TransformType {
        match self.data {
            TransformData::Linear { .. } => TransformType::Linear,
            TransformData::ThinPlateSpline { .. } => TransformType::ThinPlateSpline,
            TransformData::Grid { .. } => TransformType::Grid,
            TransformData::Concatenated(_) => TransformType::Concatenated,
        }
    }

    /// `get_n_concated_transforms`.
    pub fn n_concated(&self) -> usize {
        match &self.data {
            TransformData::Concatenated(v) => v.len(),
            _ => 1,
        }
    }

    /// `get_nth_general_transform`: the `n`-th of a concatenation, or the transform itself when
    /// it is not one; out of range: the internal-error report and `None`.
    pub fn nth(&self, n: usize, streams: &mut dyn Streams) -> Option<&GeneralTransform> {
        if n >= self.n_concated() {
            handle_internal_error("get_nth_general_transform", streams);
            return None;
        }
        match &self.data {
            TransformData::Concatenated(v) => Some(&v[n]),
            _ => Some(self),
        }
    }

    /// `get_linear_transform_ptr`: the matrix that maps forward (the stored inverse when the
    /// inverse flag is set); not linear: the internal-error report and `None`.
    pub fn linear_transform(&self, streams: &mut dyn Streams) -> Option<&Transform> {
        match &self.data {
            TransformData::Linear { forward, inverse } => {
                Some(if self.inverse_flag { inverse } else { forward })
            }
            _ => {
                handle_internal_error("get_linear_transform_ptr", streams);
                None
            }
        }
    }

    /// `get_inverse_linear_transform_ptr`.
    pub fn inverse_linear_transform(&self, streams: &mut dyn Streams) -> Option<&Transform> {
        match &self.data {
            TransformData::Linear { forward, inverse } => {
                Some(if self.inverse_flag { forward } else { inverse })
            }
            _ => {
                handle_internal_error("get_inverse_linear_transform_ptr", streams);
                None
            }
        }
    }

    /// `copy_and_invert_transform`: a copy, inverted when `invert_it`; a linear copy takes the
    /// inverse flag into its matrices (it comes out with the flag clear), the others toggle it.
    pub fn copy_and_invert(&self, invert_it: bool) -> GeneralTransform {
        match &self.data {
            TransformData::Linear { forward, inverse } => {
                let invert = invert_it != self.inverse_flag;
                let (f, i) = if invert {
                    (inverse.clone(), forward.clone())
                } else {
                    (forward.clone(), inverse.clone())
                };
                GeneralTransform {
                    data: TransformData::Linear {
                        forward: f,
                        inverse: i,
                    },
                    inverse_flag: false,
                }
            }
            TransformData::Concatenated(v) => GeneralTransform {
                data: TransformData::Concatenated(
                    v.iter().map(|t| t.copy_and_invert(false)).collect(),
                ),
                inverse_flag: self.inverse_flag != invert_it,
            },
            other => GeneralTransform {
                data: other.clone(),
                inverse_flag: self.inverse_flag != invert_it,
            },
        }
    }

    /// `create_inverse_general_transform`.
    pub fn inverse(&self) -> GeneralTransform {
        self.copy_and_invert(true)
    }

    /// `invert_general_transform`: toggles the inverse flag.
    pub fn invert_in_place(&mut self) {
        self.inverse_flag = !self.inverse_flag;
    }

    /// `concat_general_transforms(first, second)`: apply `first`, then `second`. The last of
    /// `first`'s transforms and the first of `second`'s are multiplied into one when both are
    /// linear (and their inverses, in the other order); a single transform left is linear.
    pub fn concat(first: &GeneralTransform, second: &GeneralTransform) -> GeneralTransform {
        let mut quiet = larmorx_core::cfmt::Silent;
        let first_inverted_concat =
            first.transform_type() == TransformType::Concatenated && first.inverse_flag;
        let second_inverted_concat =
            second.transform_type() == TransformType::Concatenated && second.inverse_flag;
        let range = |t: &GeneralTransform| -> (i64, i64, i64) {
            let n = t.n_concated() as i64;
            if t.inverse_flag {
                (n - 1, 0, -1)
            } else {
                (0, n - 1, 1)
            }
        };
        let (first_start, mut first_end, first_step) = range(first);
        let (mut second_start, second_end, second_step) = range(second);
        let nth = |t: &'_ GeneralTransform, i: i64| -> GeneralTransform {
            t.nth(i as usize, &mut larmorx_core::cfmt::Silent)
                .expect("in range")
                .clone()
        };
        let mut n_transforms =
            (first_end - first_start).abs() + 1 + (second_end - second_start).abs() + 1;
        let mut crunching_linear = false;
        if nth(first, first_end).transform_type() == TransformType::Linear
            && nth(second, second_start).transform_type() == TransformType::Linear
        {
            n_transforms -= 1;
            crunching_linear = true;
            first_end -= first_step;
            second_start += second_step;
        }
        let mut parts: Vec<GeneralTransform> = Vec::with_capacity(n_transforms as usize);
        let mut i = first_start;
        while i != first_end + first_step {
            parts.push(nth(first, i).copy_and_invert(first_inverted_concat));
            i += first_step;
        }
        if crunching_linear {
            let f = nth(first, first_end + first_step);
            let s = nth(second, second_start - second_step);
            let (first_transform, first_inverse) = if first_inverted_concat {
                (
                    *f.inverse_linear_transform(&mut quiet).expect("linear"),
                    *f.linear_transform(&mut quiet).expect("linear"),
                )
            } else {
                (
                    *f.linear_transform(&mut quiet).expect("linear"),
                    *f.inverse_linear_transform(&mut quiet).expect("linear"),
                )
            };
            let (second_transform, second_inverse) = if second_inverted_concat {
                (
                    *s.inverse_linear_transform(&mut quiet).expect("linear"),
                    *s.linear_transform(&mut quiet).expect("linear"),
                )
            } else {
                (
                    *s.linear_transform(&mut quiet).expect("linear"),
                    *s.inverse_linear_transform(&mut quiet).expect("linear"),
                )
            };
            parts.push(GeneralTransform {
                data: TransformData::Linear {
                    forward: Box::new(concat_transforms(&first_transform, &second_transform)),
                    inverse: Box::new(concat_transforms(&second_inverse, &first_inverse)),
                },
                inverse_flag: false,
            });
        }
        let mut i = second_start;
        while i != second_end + second_step {
            parts.push(nth(second, i).copy_and_invert(second_inverted_concat));
            i += second_step;
        }
        debug_assert_eq!(parts.len() as i64, n_transforms);
        if n_transforms == 1 {
            // volume_io marks the result linear and writes the one transform over it: the linear
            // copy (its inverse flag cleared by copy_and_invert, or the crunched product).
            let mut only = parts.pop().expect("one transform");
            only.inverse_flag = false;
            only
        } else {
            GeneralTransform {
                data: TransformData::Concatenated(parts),
                inverse_flag: false,
            }
        }
    }
}

/// `general_transform_point` (and, with `inverse`, `general_inverse_transform_point`): linear
/// transforms and concatenations of them; thin-plate splines and grids are not applied by larmorx
/// yet (`Err` with the type).
pub fn general_transform_point(
    t: &GeneralTransform,
    inverse: bool,
    x: f64,
    y: f64,
    z: f64,
) -> Result<[f64; 3], TransformType> {
    transform_or_invert_point(t, t.inverse_flag != inverse, x, y, z)
}

fn transform_or_invert_point(
    t: &GeneralTransform,
    inverse_flag: bool,
    x: f64,
    y: f64,
    z: f64,
) -> Result<[f64; 3], TransformType> {
    match &t.data {
        TransformData::Linear { forward, inverse } => Ok(transform_point(
            if inverse_flag { inverse } else { forward },
            x,
            y,
            z,
        )),
        TransformData::Concatenated(v) => {
            let mut p = [x, y, z];
            if inverse_flag {
                for sub in v.iter().rev() {
                    p = transform_or_invert_point(sub, !sub.inverse_flag, p[0], p[1], p[2])?;
                }
            } else {
                for sub in v {
                    p = transform_or_invert_point(sub, sub.inverse_flag, p[0], p[1], p[2])?;
                }
            }
            Ok(p)
        }
        other => Err(match other {
            TransformData::ThinPlateSpline { .. } => TransformType::ThinPlateSpline,
            _ => TransformType::Grid,
        }),
    }
}

#[cfg(test)]
mod tests;
