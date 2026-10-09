// SPDX-License-Identifier: Apache-2.0
//! Voxel element types.

use std::fmt;

use ndarray::ArrayD;
use num_complex::Complex;

use crate::array::DynArray;

/// The element type of an array, named as in numpy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DataType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
    /// Complex with f32 components (numpy `complex64`).
    C64,
    /// Complex with f64 components (numpy `complex128`).
    C128,
}

impl DataType {
    /// Every supported type.
    pub const ALL: [DataType; 12] = [
        DataType::U8,
        DataType::I8,
        DataType::U16,
        DataType::I16,
        DataType::U32,
        DataType::I32,
        DataType::U64,
        DataType::I64,
        DataType::F32,
        DataType::F64,
        DataType::C64,
        DataType::C128,
    ];

    /// Size of one element in bytes.
    pub const fn size(self) -> usize {
        match self {
            DataType::U8 | DataType::I8 => 1,
            DataType::U16 | DataType::I16 => 2,
            DataType::U32 | DataType::I32 | DataType::F32 => 4,
            DataType::U64 | DataType::I64 | DataType::F64 | DataType::C64 => 8,
            DataType::C128 => 16,
        }
    }

    /// The numpy name (`"uint8"`, `"float32"`, `"complex64"`, ...).
    pub const fn name(self) -> &'static str {
        match self {
            DataType::U8 => "uint8",
            DataType::I8 => "int8",
            DataType::U16 => "uint16",
            DataType::I16 => "int16",
            DataType::U32 => "uint32",
            DataType::I32 => "int32",
            DataType::U64 => "uint64",
            DataType::I64 => "int64",
            DataType::F32 => "float32",
            DataType::F64 => "float64",
            DataType::C64 => "complex64",
            DataType::C128 => "complex128",
        }
    }

    /// The type with numpy name `name`.
    pub fn from_name(name: &str) -> Option<DataType> {
        DataType::ALL.into_iter().find(|t| t.name() == name)
    }

    pub const fn is_integer(self) -> bool {
        !self.is_float() && !self.is_complex()
    }

    pub const fn is_float(self) -> bool {
        matches!(self, DataType::F32 | DataType::F64)
    }

    pub const fn is_complex(self) -> bool {
        matches!(self, DataType::C64 | DataType::C128)
    }
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A voxel element type: plain old data with a fixed [`DataType`].
pub trait Element:
    bytemuck::Pod + Default + PartialEq + fmt::Debug + Send + Sync + 'static
{
    const DATA_TYPE: DataType;

    /// The value with its bytes reversed (for converting between byte orders).
    fn swap_bytes(self) -> Self;

    /// Wraps a typed array into a [`DynArray`].
    fn into_dyn_array(array: ArrayD<Self>) -> DynArray;

    /// The typed array inside `array`, if it holds this type.
    fn from_dyn_array(array: &DynArray) -> Option<&ArrayD<Self>>;

    /// Takes the typed array out of `array`, or gives `array` back if it holds another type.
    fn try_from_dyn_array(array: DynArray) -> Result<ArrayD<Self>, DynArray>;
}

/// Real (non-complex) element types, convertible to floating point.
pub trait RealElement: Element + PartialOrd {
    /// Conversion with round-to-nearest, like a numpy `astype(float64)`.
    fn to_f64(self) -> f64;
    /// Conversion with round-to-nearest, like a numpy `astype(float32)`.
    fn to_f32(self) -> f32;
}

macro_rules! impl_element {
    ($t:ty, $variant:ident, $swap:expr) => {
        impl Element for $t {
            const DATA_TYPE: DataType = DataType::$variant;

            fn swap_bytes(self) -> Self {
                ($swap)(self)
            }

            fn into_dyn_array(array: ArrayD<Self>) -> DynArray {
                DynArray::$variant(array)
            }

            fn from_dyn_array(array: &DynArray) -> Option<&ArrayD<Self>> {
                match array {
                    DynArray::$variant(a) => Some(a),
                    _ => None,
                }
            }

            fn try_from_dyn_array(array: DynArray) -> Result<ArrayD<Self>, DynArray> {
                match array {
                    DynArray::$variant(a) => Ok(a),
                    other => Err(other),
                }
            }
        }
    };
}

macro_rules! impl_real {
    ($($t:ty => $variant:ident),*) => {$(
        impl_element!($t, $variant, |v: $t| <$t>::from_ne_bytes({
            let mut b = v.to_ne_bytes();
            b.reverse();
            b
        }));

        impl RealElement for $t {
            #[allow(clippy::cast_precision_loss, clippy::cast_lossless)]
            fn to_f64(self) -> f64 {
                self as f64
            }

            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_lossless)]
            fn to_f32(self) -> f32 {
                self as f32
            }
        }
    )*};
}

impl_real!(u8 => U8, i8 => I8, u16 => U16, i16 => I16, u32 => U32, i32 => I32, u64 => U64,
           i64 => I64, f32 => F32, f64 => F64);

impl_element!(Complex<f32>, C64, |v: Complex<f32>| Complex::new(
    v.re.swap_bytes(),
    v.im.swap_bytes()
));
impl_element!(Complex<f64>, C128, |v: Complex<f64>| Complex::new(
    v.re.swap_bytes(),
    v.im.swap_bytes()
));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_sizes_and_kinds() {
        for t in DataType::ALL {
            assert_eq!(DataType::from_name(t.name()), Some(t));
            assert_eq!(
                t.is_integer() as u8 + t.is_float() as u8 + t.is_complex() as u8,
                1
            );
        }
        assert_eq!(DataType::C128.size(), 16);
        assert_eq!(DataType::from_name("float16"), None);
        assert_eq!(f32::DATA_TYPE, DataType::F32);
    }

    #[test]
    fn swap_bytes_reverses_and_is_an_involution() {
        assert_eq!(0x0102u16.swap_bytes(), Element::swap_bytes(0x0102u16));
        assert_eq!(
            Element::swap_bytes(1.5f32).to_bits(),
            1.5f32.to_bits().swap_bytes()
        );
        let c = Complex::new(1.25f64, -3.5);
        assert_eq!(Element::swap_bytes(Element::swap_bytes(c)), c);
        assert_eq!(Element::swap_bytes(-7i8), -7i8);
    }
}
