// SPDX-License-Identifier: Apache-2.0
//! The MINC general transform a volume carries (spec §14), as the header copy copies it.

/// The kinds of MINC general transforms (MINC 1.5.1's `Transform_types` codes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformKind {
    /// A linear transform (code 0).
    Linear,
    /// A concatenation (code 1), of no transforms here.
    Concatenated,
    /// Any other code, which FreeSurfer's MINC code does not support.
    Other(i32),
}

impl TransformKind {
    /// The kind of a code.
    pub fn of_code(code: i32) -> TransformKind {
        match code {
            0 => TransformKind::Linear,
            1 => TransformKind::Concatenated,
            c => TransformKind::Other(c),
        }
    }

    /// The code of a kind.
    pub fn code(self) -> i32 {
        match self {
            TransformKind::Linear => 0,
            TransformKind::Concatenated => 1,
            TransformKind::Other(c) => c,
        }
    }
}

/// A volume's MINC transform: its kind, inverse flag and the two linear matrices (row by row);
/// the volume's linear matrix is the effective one (the inverse when the flag is set) and its
/// inverse the other.
#[derive(Clone, Debug, PartialEq)]
pub struct MincTransform {
    pub kind: TransformKind,
    pub inverse_flag: bool,
    pub linear: [f64; 16],
    pub inverse: [f64; 16],
}

impl MincTransform {
    /// The volume's linear matrix (the effective one).
    pub fn effective(&self) -> &[f64; 16] {
        if self.inverse_flag {
            &self.inverse
        } else {
            &self.linear
        }
    }

    /// The volume's inverse linear matrix (the other one).
    pub fn effective_inverse(&self) -> &[f64; 16] {
        if self.inverse_flag {
            &self.linear
        } else {
            &self.inverse
        }
    }
}
