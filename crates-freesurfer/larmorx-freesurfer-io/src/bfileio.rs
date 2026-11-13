// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/bfileio.cpp`'s byte reversal of a buffer of numbers (queue item 3.1-25):
//! `byteswapbufshort`, `byteswapbuffloat`, `byteswapbufdouble`.

use larmorx_core::defects::FS_175;
use larmorx_freesurfer_core::globals::Globals;

/// Reverses the bytes of each `size`-byte group that starts below `nbufbytes` (a group may end past
/// the count, inside the buffer: FreeSurfer's `freadFloat` passes a count of 1 for a 4-byte
/// buffer). FS-175: a group that would end past the buffer is left as it is (both modes;
/// FreeSurfer reverses memory past its buffer). 0.
fn swap_groups(g: &mut Globals, buf: &mut [u8], nbufbytes: i64, size: usize, what: &str) -> i32 {
    let mut n: i64 = 0;
    while n < nbufbytes {
        let at = n as usize;
        if at + size > buf.len() {
            g.notices.add(
                &FS_175,
                format!(
                    "{what}: a count of {nbufbytes} bytes for a buffer of {}: the last number reaches past the buffer and is left as it is",
                    buf.len()
                ),
            );
            break;
        }
        buf[at..at + size].reverse();
        n += size as i64;
    }
    0
}

/// `byteswapbufshort(buf, nbufbytes)`.
pub fn byteswapbufshort(g: &mut Globals, buf: &mut [u8], nbufbytes: i64) -> i32 {
    swap_groups(g, buf, nbufbytes, 2, "byteswapbufshort")
}

/// `byteswapbuffloat(buf, nbufbytes)`.
pub fn byteswapbuffloat(g: &mut Globals, buf: &mut [u8], nbufbytes: i64) -> i32 {
    swap_groups(g, buf, nbufbytes, 4, "byteswapbuffloat")
}

/// `byteswapbufdouble(buf, nbufbytes)`.
pub fn byteswapbufdouble(g: &mut Globals, buf: &mut [u8], nbufbytes: i64) -> i32 {
    swap_groups(g, buf, nbufbytes, 8, "byteswapbufdouble")
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn groups() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut b = [1, 2, 3, 4, 5, 6];
        byteswapbufshort(&mut g, &mut b, 3);
        assert_eq!(b, [2, 1, 4, 3, 5, 6]);
        let mut b = [1, 2, 3, 4];
        byteswapbuffloat(&mut g, &mut b, 1);
        assert_eq!(b, [4, 3, 2, 1]);
        assert!(!g.notices.contains(&FS_175));
        let mut b = [1, 2, 3, 4, 5, 6];
        byteswapbuffloat(&mut g, &mut b, 6);
        assert_eq!(b, [4, 3, 2, 1, 5, 6]);
        assert!(g.notices.contains(&FS_175));
        let mut b = [1, 2, 3, 4, 5, 6, 7, 8];
        byteswapbufdouble(&mut g, &mut b, -3);
        assert_eq!(b, [1, 2, 3, 4, 5, 6, 7, 8]);
        byteswapbufdouble(&mut g, &mut b, 8);
        assert_eq!(b, [8, 7, 6, 5, 4, 3, 2, 1]);
    }
}
