// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The debug output of `mri_robust_register --debug` (queue 3.2-13), the pieces the library's
//! sites share (translated from FreeSurfer 8.2.0's `mri_robust_register/Registration.cpp`,
//! `RegRobust.h`, `RegRobust.cpp`, `RegPowell.cpp`; the sites themselves are in
//! [`crate::registration`] and [`crate::powell`]):
//!
//! - [`Cout`]: `std::cout`'s state for numbers, which `RegPowell`'s debug report changes for the
//!   rest of the program (`cout.setf(ios::fixed, ios::floatfield)` and `setprecision(14)`: every
//!   later `double` on standard output prints with 14 decimals);
//! - [`press_a_key`]: `std::cin >> ch` (a `char`), the key the debug output waits for between
//!   iterations: white space skipped, one character read; at the end of the input (standard input
//!   from `/dev/null`, a closed pipe) the stream fails, and every later read returns at once;
//! - [`debug_write`]: `MRIwrite` of an intermediate image, its status not looked at;
//! - [`print_fixed4`], [`print_fixed3`]: `std::cout << M` of a `vnl_matrix_fixed` (VXL's `print`:
//!   each row's numbers separated by a blank, one row per line, at the stream's state);
//! - [`save_gaussian_pyramid`]: `Registration::saveGaussianPyramid(p, prefix)`.

use std::io::{BufRead, Write};

use larmorx_core::cfmt;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, Mri};

use crate::mymatrix::Mat4;
use crate::registration::Io;

/// `std::cout`'s state for floating-point numbers: its precision (6 at the start) and whether its
/// float field is `fixed` (`%.<precision>f`) rather than the default (`%.<precision>g`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cout {
    pub precision: usize,
    pub fixed: bool,
}

impl Default for Cout {
    fn default() -> Self {
        Cout {
            precision: 6,
            fixed: false,
        }
    }
}

impl Cout {
    /// `std::cout << v` in this state.
    pub fn os(&self, v: f64) -> String {
        if self.fixed {
            cfmt::f(v, self.precision)
        } else {
            cfmt::g(v, self.precision)
        }
    }
}

/// `std::cin >> ch` for a `char` (the standard input in `input`; `None`: an empty one): the
/// standard output flushed first (`cin` is tied to `cout`); then, unless the stream has failed
/// already (`failed`), white space skipped (the C locale's: blank, tab, newline, vertical tab,
/// form feed, carriage return) and one character read. The end of the input (or a read error)
/// before a character makes the stream fail, which every later read keeps.
pub fn press_a_key<R: BufRead + ?Sized>(
    out: &mut dyn Write,
    input: Option<&mut R>,
    failed: &mut bool,
) {
    let _ = out.flush();
    if *failed {
        return;
    }
    let Some(input) = input else {
        *failed = true;
        return;
    };
    loop {
        let buf = match input.fill_buf() {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                *failed = true;
                return;
            }
        };
        if buf.is_empty() {
            *failed = true;
            return;
        }
        let blanks = buf
            .iter()
            .take_while(|&&c| matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'))
            .count();
        if blanks < buf.len() {
            input.consume(blanks + 1);
            return;
        }
        input.consume(blanks);
    }
}

/// `MRIwrite(m, name)` of a debug image (FreeSurfer does not look at its status: a write that
/// fails prints the writer's messages, and the program goes on).
pub fn debug_write(g: &mut Globals, io: &mut Io, m: &mut Mri, name: &str) -> Result<(), Fatal> {
    mri_write(
        g,
        io.out,
        io.err,
        io.env,
        io.dir,
        m,
        name,
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    Ok(())
}

/// `os << M` of a `vnl_matrix_fixed<double, 4, 4>` in the stream's state `cout`.
pub fn print_fixed4(out: &mut dyn Write, m: &Mat4, cout: Cout) {
    for row in m {
        let line: Vec<String> = row.iter().map(|&v| cout.os(v)).collect();
        let _ = writeln!(out, "{}", line.join(" "));
    }
}

/// `os << M` of a `vnl_matrix_fixed<double, 3, 3>`.
pub fn print_fixed3(out: &mut dyn Write, m: &[[f64; 3]; 3], cout: Cout) {
    for row in m {
        let line: Vec<String> = row.iter().map(|&v| cout.os(v)).collect();
        let _ = writeln!(out, "{}", line.join(" "));
    }
}

/// `Registration::saveGaussianPyramid(p, prefix)`: `  Saving Pyramid <prefix>`, then each level
/// `i` written as `<prefix>-r<i>.mgz` (in the working folder; the registration's name is not
/// part of it).
pub fn save_gaussian_pyramid(
    g: &mut Globals,
    io: &mut Io,
    p: &mut [Mri],
    prefix: &str,
) -> Result<(), Fatal> {
    let _ = writeln!(io.out, "  Saving Pyramid {prefix}");
    for (i, m) in p.iter_mut().enumerate() {
        debug_write(g, io, m, &format!("{prefix}-r{i}.mgz"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(input: &[u8], presses: usize) -> (Vec<bool>, usize) {
        let mut r = std::io::Cursor::new(input.to_vec());
        let mut failed = false;
        let mut out = Vec::new();
        let mut states = Vec::new();
        for _ in 0..presses {
            press_a_key(&mut out, Some(&mut r), &mut failed);
            states.push(failed);
        }
        (states, r.position() as usize)
    }

    #[test]
    fn a_key_press_skips_blanks_reads_one_character_and_fails_at_the_end() {
        // two characters among blanks, then the end: the third press fails, the fourth too
        let (states, pos) = keys(b" \n a\t\r\x0b\x0cb  \n", 4);
        assert_eq!(states, [false, false, true, true]);
        assert_eq!(pos, 12);
        // an empty input fails at once; no input at all likewise
        assert_eq!(keys(b"", 2).0, [true, true]);
        let mut failed = false;
        press_a_key::<std::io::Empty>(&mut Vec::new(), None, &mut failed);
        assert!(failed);
        // a failed stream reads nothing more
        let mut r = std::io::Cursor::new(b"x".to_vec());
        let mut failed = true;
        press_a_key(&mut Vec::new(), Some(&mut r), &mut failed);
        assert_eq!(r.position(), 0);
    }

    #[test]
    fn the_stream_state_prints_fixed_or_general() {
        let c = Cout::default();
        assert_eq!(c.os(0.1 + 0.2), "0.3");
        let f = Cout {
            precision: 14,
            fixed: true,
        };
        assert_eq!(f.os(0.1 + 0.2), "0.30000000000000");
        assert_eq!(f.os(-1.5e-20), "-0.00000000000000");
        assert_eq!(f.os(123456.0), "123456.00000000000000");
        let g = Cout {
            precision: 14,
            fixed: false,
        };
        assert_eq!(g.os(0.1 + 0.2), "0.3");
        assert_eq!(g.os(1.0 / 3.0), "0.33333333333333");
    }
}
