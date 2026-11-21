// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The C library's `calloc` as the replica models it (larmorx's own code, the C library's
//! behaviour on x86-64 Linux): sizes FreeSurfer computes in `size_t` wrap around for negative
//! counts and unknown types, and an allocation past the address space fails ([`CALLOC_LIMIT`]);
//! a zeroed allocation is not touched until it is written, as glibc's `calloc` of fresh pages is
//! not, so a header that asks for a large buffer costs the port no more memory than it costs
//! FreeSurfer until the buffer is filled ([`zeroed`]). Where whether an allocation succeeds
//! depends on the machine's memory, the port answers as a machine of [`MACHINE_BYTES`] does
//! (FS-756, FS-760): the same answer on every platform.

/// The size above which `calloc` is taken to fail (128 TiB, the x86-64 user address space).
pub const CALLOC_LIMIT: u64 = 1 << 47;

/// The size beyond which an allocation that FreeSurfer then writes whole succeeds or not
/// depending on the machine's memory (16 GiB): the port answers as a machine that refuses it.
pub const MACHINE_BYTES: u64 = 1 << 34;

/// `ENOMEM`: what a failed `calloc` leaves in `errno` (FreeSurfer's next error message prints it).
pub const ENOMEM: i32 = 12;

/// `n` default values (zeros for the numbers, `None` for options of boxes) in an allocation the
/// machine gives (`None` where it refuses `n` values), not touched until written: the memory is
/// asked for first without being filled, then made as zeros, which the system's allocator gives
/// as fresh pages for a large size (glibc's `calloc`), so a buffer sized from a header costs only
/// what is written into it. For a type whose default value is not all zero bits each value is
/// written (as the port's own `Vec` of such values always was).
///
/// The first request asks for at least [`PROBE_BYTES`]: glibc's `malloc` raises the size from
/// which it maps fresh pages to that of any mapped block freed below 32 MiB (its dynamic mmap
/// threshold), so a freed probe of the buffer's own size would put the buffer itself, and every
/// later one up to that size, on the heap, where freed memory is not given back (the conformed
/// conversion of `mri_convert` kept its 11 MiB input after freeing it: 39 MiB against
/// FreeSurfer's 29, relay 48). A probe above 32 MiB leaves the threshold as it was.
pub fn zeroed<T: Clone + Default>(n: usize) -> Option<Vec<T>> {
    let mut probe: Vec<T> = Vec::new();
    let size = std::mem::size_of::<T>().max(1);
    let at_least = if n == 0 { 0 } else { n.max(PROBE_BYTES.div_ceil(size)) };
    probe.try_reserve_exact(at_least).ok()?;
    drop(probe);
    Some(vec![T::default(); n])
}

/// The smallest probe [`zeroed`] asks for: above glibc's largest dynamic mmap threshold (32 MiB on
/// 64-bit systems), whose freeing leaves the threshold as it was.
pub const PROBE_BYTES: usize = (32 << 20) + 4096;

/// `calloc(n, 1)` of bytes: `None` (and `errno` [`ENOMEM`]) past [`CALLOC_LIMIT`] or where the
/// machine refuses them; otherwise `n` zero bytes not touched until written ([`zeroed`]).
pub fn calloc_bytes(errno: &mut i32, n: u64) -> Option<Vec<u8>> {
    let v = if n > CALLOC_LIMIT {
        None
    } else {
        usize::try_from(n).ok().and_then(zeroed::<u8>)
    };
    if v.is_none() {
        *errno = ENOMEM;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeros_and_refusals() {
        let mut e = 0;
        assert_eq!(calloc_bytes(&mut e, 5), Some(vec![0u8; 5]));
        assert_eq!(e, 0);
        assert_eq!(calloc_bytes(&mut e, CALLOC_LIMIT + 1), None);
        assert_eq!(e, ENOMEM);
        assert_eq!(zeroed::<f32>(3), Some(vec![0.0; 3]));
        assert_eq!(zeroed::<Option<Box<u8>>>(2), Some(vec![None, None]));
        assert!(zeroed::<u64>(usize::MAX).is_none());
    }
}
