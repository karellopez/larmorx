// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's licence-key check (`chklc`, `utils/chklc.cpp` of FreeSurfer 8.2.0): **not a
//! check in the replica**. FreeSurfer's licence key is a runtime check, not a term of its
//! licence (the decision log, 2026-10-19), so `larmorx-freesurfer` needs no key and its programs
//! go on as FreeSurfer does with a valid one. Its messages, for reference only, are described in
//! `specs/freesurfer-core.md` (§14); with `DIAG_NO` above 0 FreeSurfer's check also prints the
//! key file's name and contents on standard output, which the replica never does.

/// `chklc(msg)`: 1, as FreeSurfer returns after a valid key.
pub fn chklc() -> i32 {
    1
}

#[cfg(test)]
mod tests {
    #[test]
    fn no_key_is_needed() {
        assert_eq!(super::chklc(), 1);
    }
}
