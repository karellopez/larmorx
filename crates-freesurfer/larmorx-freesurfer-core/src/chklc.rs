// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's licence-key check (`chklc`, `utils/chklc.cpp` of FreeSurfer 8.2.0): **not a
//! check in the replica**. FreeSurfer's licence key is a runtime check, not a term of its
//! licence (the decision log, 2026-10-19), so `larmorx-freesurfer` needs no key and its programs
//! go on as FreeSurfer does with a valid one. Its messages, for reference only, are described in
//! `specs/freesurfer-core.md` (§14); with `DIAG_NO` above 0 FreeSurfer's check also prints the
//! key file's name and contents on standard output, which the replica never does.

/// `chklc(msg)`: 1, as FreeSurfer returns after a valid key. Its first call in a run leaves
/// `errno` at 0, as FreeSurfer's check leaves it after reading a valid licence file to its end
/// (C's `fscanf`; measured on the official `mri_vol2vol`, track A's relay 53: an `EEXIST` left by
/// an earlier `mkdir` no longer shows in a later error's `strerror` line); later calls return at
/// once and leave it.
pub fn chklc(g: &mut crate::globals::Globals) -> i32 {
    if !g.chklc_done {
        g.chklc_done = true;
        g.errno = 0;
    }
    1
}

#[cfg(test)]
mod tests {
    #[test]
    fn no_key_is_needed() {
        let mut g = crate::globals::Globals::new(larmorx_core::defects::UpstreamDefects::Fix);
        g.errno = 17;
        assert_eq!(super::chklc(&mut g), 1);
        assert_eq!(g.errno, 0);
        g.errno = 2;
        assert_eq!(super::chklc(&mut g), 1);
        assert_eq!(g.errno, 2);
    }
}
