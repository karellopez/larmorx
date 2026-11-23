// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's licence-key check (`chklc`, `utils/chklc.cpp` of FreeSurfer 8.2.0): **not a
//! check in the replica**. FreeSurfer's licence key is a runtime check, not a term of its
//! licence (the decision log, 2026-10-19), so `larmorx-freesurfer` needs no key and its programs
//! go on as FreeSurfer does with a valid one. Its messages, for reference only, are described in
//! `specs/freesurfer-core.md` (§14); with `DIAG_NO` above 0 FreeSurfer's check also prints the
//! key file's name and contents on standard output, which the replica never does.

/// `chklc(msg)` where the caller has no environment at hand (the surface readers and writers): 1,
/// as FreeSurfer returns after a valid key; the check counts as made (`errno` untouched: the only
/// official runs that reach these callers first, the surface probe's harness, set FreeSurfer's side
/// door, `SURFER_SIDEDOOR`, which returns before the licence file is read).
pub fn chklc(g: &mut crate::globals::Globals) -> i32 {
    g.chklc_done = true;
    1
}

/// `chklc(msg)` with the program's environment: 1. Its first call in a run leaves `errno` at 0, as
/// FreeSurfer's check leaves it after reading a valid licence file to its end (C's `fscanf`;
/// measured on the official `mri_vol2vol`, track A's relay 53: an `EEXIST` left by an earlier
/// `mkdir` no longer shows in a later error's `strerror` line), unless `SURFER_SIDEDOOR` is set
/// (FreeSurfer then returns before reading the file); later calls return at once and leave it.
pub fn chklc_env(g: &mut crate::globals::Globals, env: &crate::run::Environment) -> i32 {
    if !g.chklc_done {
        g.chklc_done = true;
        if env.get("SURFER_SIDEDOOR").is_none() {
            g.errno = 0;
        }
    }
    1
}

#[cfg(test)]
mod tests {
    #[test]
    fn no_key_is_needed() {
        let env = crate::run::Environment::default();
        let mut g = crate::globals::Globals::new(larmorx_core::defects::UpstreamDefects::Fix);
        g.errno = 17;
        assert_eq!(super::chklc_env(&mut g, &env), 1);
        assert_eq!(g.errno, 0);
        g.errno = 2;
        assert_eq!(super::chklc_env(&mut g, &env), 1);
        assert_eq!(g.errno, 2);
        // the side door, or a first call without the environment: errno kept
        let door = crate::run::Environment::from_pairs([("SURFER_SIDEDOOR", "1")]);
        let mut g = crate::globals::Globals::new(larmorx_core::defects::UpstreamDefects::Fix);
        g.errno = 17;
        assert_eq!(super::chklc_env(&mut g, &door), 1);
        assert_eq!(g.errno, 17);
        let mut g = crate::globals::Globals::new(larmorx_core::defects::UpstreamDefects::Fix);
        g.errno = 17;
        assert_eq!(super::chklc(&mut g), 1);
        assert_eq!(super::chklc_env(&mut g, &env), 1);
        assert_eq!(g.errno, 17);
    }
}
