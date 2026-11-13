// SPDX-License-Identifier: Apache-2.0
//! The interpolation and precision names (spec §12) and the volume's shape (§15).

/// `MRIinterpCode(name)` (§12): the first three characters, ASCII case folded, against `nea`
/// (0), `tri` and `tli` (1), `sin` (2), `cub` (5); a shorter name matches nothing; else -1.
pub fn interp_code(name: &[u8]) -> i32 {
    if name.len() < 3 {
        return -1;
    }
    let p: Vec<u8> = name[..3].iter().map(u8::to_ascii_lowercase).collect();
    match &p[..] {
        b"nea" => 0,
        b"tri" | b"tli" => 1,
        b"sin" => 2,
        b"cub" => 5,
        _ => -1,
    }
}

/// `MRIinterpString(code)` (§12): 0 `nearest`, 1 `trilinear`, 2 `sinc`, 5 `cubic`, else none.
pub fn interp_string(code: i32) -> Option<&'static str> {
    match code {
        0 => Some("nearest"),
        1 => Some("trilinear"),
        2 => Some("sinc"),
        5 => Some("cubic"),
        _ => None,
    }
}

const PRECISIONS: [(&str, i32); 6] = [
    ("uchar", 0),
    ("short", 4),
    ("ushrt", 10),
    ("int", 1),
    ("long", 2),
    ("float", 3),
];

/// `MRIprecisionCode(name)` (§12): the whole name, ASCII case folded; else -1.
pub fn precision_code(name: &[u8]) -> i32 {
    let n: Vec<u8> = name.iter().map(u8::to_ascii_lowercase).collect();
    PRECISIONS
        .iter()
        .find(|(p, _)| p.as_bytes() == &n[..])
        .map_or(-1, |&(_, c)| c)
}

/// `MRIprecisionString(code)` (§12): the name, or none.
pub fn precision_string(code: i32) -> Option<&'static str> {
    PRECISIONS.iter().find(|&&(_, c)| c == code).map(|&(p, _)| p)
}

/// A volume's shape (`MRI::Shape`, §15): width, height, depth and frames; equal when those
/// four are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    pub nframes: i32,
}

impl crate::process::Process {
    /// `MRI::Shape(w, h, d, nframes)` (§4 step 1, §15): the shape, with the message for each
    /// extent that is not positive.
    pub fn mri_shape(&mut self, width: i32, height: i32, depth: i32, nframes: i32) -> Shape {
        self.shape_messages(&[width, height, depth, nframes]);
        Shape {
            width,
            height,
            depth,
            nframes,
        }
    }
}
