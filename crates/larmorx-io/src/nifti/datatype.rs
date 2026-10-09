//! NIfTI `datatype` codes.

use larmorx_core::element::DataType;

/// The NIfTI code of each supported type (`NIFTI_TYPE_*`).
const CODES: [(i16, DataType); 12] = [
    (2, DataType::U8),
    (4, DataType::I16),
    (8, DataType::I32),
    (16, DataType::F32),
    (32, DataType::C64),
    (64, DataType::F64),
    (256, DataType::I8),
    (512, DataType::U16),
    (768, DataType::U32),
    (1024, DataType::I64),
    (1280, DataType::U64),
    (1792, DataType::C128),
];

/// The element type of `code`, or `None` for unknown codes and for the codes larmorx does not
/// support yet (binary, RGB24, RGBA32, float128, complex256).
pub fn from_code(code: i16) -> Option<DataType> {
    CODES.iter().find(|(c, _)| *c == code).map(|(_, t)| *t)
}

/// The NIfTI code of `data_type`.
pub fn to_code(data_type: DataType) -> i16 {
    CODES
        .iter()
        .find(|(_, t)| *t == data_type)
        .map(|(c, _)| *c)
        .expect("every DataType has a NIfTI code")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip() {
        for t in DataType::ALL {
            assert_eq!(from_code(to_code(t)), Some(t));
        }
        for unsupported in [0, 1, 128, 255, 1536, 2048, 2304] {
            assert_eq!(from_code(unsupported), None);
        }
    }
}
