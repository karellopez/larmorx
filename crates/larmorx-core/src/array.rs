//! Arrays whose element type is known only at run time (e.g. after reading a file).

use ndarray::ArrayD;
use num_complex::Complex;

use crate::element::{DataType, Element};

/// An n-dimensional array of any supported element type.
#[derive(Clone, Debug, PartialEq)]
pub enum DynArray {
    U8(ArrayD<u8>),
    I8(ArrayD<i8>),
    U16(ArrayD<u16>),
    I16(ArrayD<i16>),
    U32(ArrayD<u32>),
    I32(ArrayD<i32>),
    U64(ArrayD<u64>),
    I64(ArrayD<i64>),
    F32(ArrayD<f32>),
    F64(ArrayD<f64>),
    C64(ArrayD<Complex<f32>>),
    C128(ArrayD<Complex<f64>>),
}

/// Runs `$body` with `$a` bound to the typed array inside `$value`, for every variant.
#[macro_export]
macro_rules! dispatch_dyn_array {
    ($value:expr, $a:ident => $body:expr) => {
        match $value {
            $crate::DynArray::U8($a) => $body,
            $crate::DynArray::I8($a) => $body,
            $crate::DynArray::U16($a) => $body,
            $crate::DynArray::I16($a) => $body,
            $crate::DynArray::U32($a) => $body,
            $crate::DynArray::I32($a) => $body,
            $crate::DynArray::U64($a) => $body,
            $crate::DynArray::I64($a) => $body,
            $crate::DynArray::F32($a) => $body,
            $crate::DynArray::F64($a) => $body,
            $crate::DynArray::C64($a) => $body,
            $crate::DynArray::C128($a) => $body,
        }
    };
}

impl DynArray {
    pub fn data_type(&self) -> DataType {
        fn of<T: Element>(_: &ArrayD<T>) -> DataType {
            T::DATA_TYPE
        }
        dispatch_dyn_array!(self, a => of(a))
    }

    pub fn shape(&self) -> &[usize] {
        dispatch_dyn_array!(self, a => a.shape())
    }

    pub fn ndim(&self) -> usize {
        self.shape().len()
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        dispatch_dyn_array!(self, a => a.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The typed array, if this holds elements of type `T`.
    pub fn as_typed<T: Element>(&self) -> Option<&ArrayD<T>> {
        T::from_dyn_array(self)
    }

    /// The typed array, or `self` back if it holds another type.
    pub fn into_typed<T: Element>(self) -> Result<ArrayD<T>, DynArray> {
        T::try_from_dyn_array(self)
    }
}

impl<T: Element> From<ArrayD<T>> for DynArray {
    fn from(array: ArrayD<T>) -> Self {
        T::into_dyn_array(array)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::IxDyn;

    #[test]
    fn typed_access_round_trips() {
        let a = ArrayD::<i16>::zeros(IxDyn(&[2, 3, 4]));
        let d = DynArray::from(a.clone());
        assert_eq!(d.data_type(), DataType::I16);
        assert_eq!(d.shape(), &[2, 3, 4]);
        assert_eq!(d.len(), 24);
        assert_eq!(d.as_typed::<i16>(), Some(&a));
        assert!(d.as_typed::<f32>().is_none());
        let d = d.into_typed::<f32>().unwrap_err();
        assert_eq!(d.into_typed::<i16>().unwrap(), a);
    }
}
