// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/MRIio_old.cpp`'s allocation helper (queue item 3.1-25): `lcalloc` and the message
//! `MGHprint_error` ends the program with.

use std::io::Write;

use larmorx_core::defects::FS_174;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;

/// `MGHprint_error(str)`: `str` on standard output, then the end of the program with exit status
/// 0 (FS-174: status 1 by default, 0 with `reproduce`).
fn mgh_print_error(g: &mut Globals, out: &mut dyn Write, s: &str) -> Fatal {
    let _ = out.write_all(s.as_bytes());
    if g.notices.reproduce() {
        g.notices.add(
            &FS_174,
            "a failed allocation ends the program with exit status 0",
        );
        Fatal::Exit(0)
    } else {
        g.notices.add(
            &FS_174,
            "a failed allocation ends the program with exit status 1 (FreeSurfer's: 0)",
        );
        Fatal::Exit(1)
    }
}

/// `lcalloc(nmemb, size)`: `nmemb` zeroed elements; when `nmemb * size` bytes cannot be had (more
/// than an address can count, beyond the largest object, or refused by the system),
/// `Cannot calloc()` and the end of the program (FS-174).
pub fn lcalloc<T: Clone + Default>(
    g: &mut Globals,
    out: &mut dyn Write,
    nmemb: usize,
    size: usize,
) -> Result<Vec<T>, Fatal> {
    let fits = nmemb
        .checked_mul(size)
        .is_some_and(|b| b <= isize::MAX as usize);
    let mut v: Vec<T> = Vec::new();
    if !fits || v.try_reserve_exact(nmemb).is_err() {
        return Err(mgh_print_error(g, out, "Cannot calloc()\n"));
    }
    v.resize(nmemb, T::default());
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn failures_end_the_program() {
        for (mode, status) in [(UpstreamDefects::Fix, 1), (UpstreamDefects::Reproduce, 0)] {
            let mut g = Globals::new(mode);
            let mut out = Vec::new();
            let v: Vec<u8> = lcalloc(&mut g, &mut out, 3, 1).unwrap();
            assert_eq!(v, vec![0, 0, 0]);
            let r: Result<Vec<u8>, Fatal> = lcalloc(&mut g, &mut out, usize::MAX, 2);
            assert_eq!(r.err().map(Fatal::status), Some(status));
            assert_eq!(out, b"Cannot calloc()\n");
            assert!(g.notices.contains(&FS_174));
        }
    }
}
