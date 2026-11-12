// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's debugging stack (`fs.core`, queue item 3.1-22), translated from FreeSurfer
//! 8.2.0's `utils/xDebug.cpp` and `include/xDebug.h` (the `DebugEnterFunction`,
//! `DebugExitFunction`, `DebugNote` and `DebugPrint` macros, with `kDebugging` on as the official
//! build compiles them): a stack of up to 49 function titles with their notes, kept for a crash
//! report. Its output starts only with `xDbg_Init`, which none of the programs the replica covers
//! calls, so its prints write nothing; each one still flushes every output stream
//! (`fflush(NULL)`, as `xDbg_Printf` does with its stream unset), which orders a program's
//! standard output before what it writes next on standard error.

use std::io::Write;

use crate::globals::Globals;

/// `xDbg_knMaxStackDepth`, `xDbg_knMaxDescLength`.
pub const MAX_STACK_DEPTH: usize = 50;
pub const MAX_DESC_LENGTH: usize = 512;

/// The stack (`masStackTitle`, `masStackNote`, `mCurrentStackDepth`), the current function's
/// title and note (`xDbg_sStackDesc`, `xDbg_sCurNoteDesc`) and whether output is on
/// (`xDbg_gbOutput`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct XDebug {
    titles: Vec<Vec<u8>>,
    notes: Vec<Vec<u8>>,
    pub stack_desc: Vec<u8>,
    pub cur_note: Vec<u8>,
    pub output: bool,
}

fn prefix(s: &[u8], n: usize) -> Vec<u8> {
    let s = match s.iter().position(|&b| b == 0) {
        Some(k) => &s[..k],
        None => s,
    };
    s[..s.len().min(n)].to_vec()
}

/// `xDbg_Printf(fmt, ...)` with the message formatted: written on standard error when output is
/// on; the streams flushed either way.
pub fn printf(g: &Globals, out: &mut dyn Write, err: &mut dyn Write, message: &str) {
    if g.xdebug.output {
        let _ = err.write_all(message.as_bytes());
    }
    let _ = out.flush();
    let _ = err.flush();
}

/// `xDbg_PushStack(title, note)`: kept (up to 512 bytes each) while fewer than 49 are kept, else
/// `Stack limit reached, can't store name.` through [`printf`].
pub fn push_stack(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    title: &[u8],
    note: &[u8],
) {
    if g.xdebug.titles.len() + 1 < MAX_STACK_DEPTH {
        g.xdebug.titles.push(prefix(title, MAX_DESC_LENGTH));
        g.xdebug.notes.push(prefix(note, MAX_DESC_LENGTH));
    } else {
        printf(g, out, err, "Stack limit reached, can't store name.\n");
    }
}

/// `xDbg_PopStack()`: the last title and note (511 bytes at most) become the current ones; with
/// none kept, `ERROR: xDbg_PopStack call when stack is empty.` through [`printf`].
pub fn pop_stack(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write) {
    match (g.xdebug.titles.pop(), g.xdebug.notes.pop()) {
        (Some(t), Some(n)) => {
            g.xdebug.stack_desc = prefix(&t, MAX_DESC_LENGTH - 1);
            g.xdebug.cur_note = prefix(&n, MAX_DESC_LENGTH - 1);
        }
        _ => printf(
            g,
            out,
            err,
            "ERROR: xDbg_PopStack call when stack is empty.\n",
        ),
    }
}

/// `xDbg_SetStackDesc(fmt, ...)`: the current title (511 bytes at most, `vsnprintf`'s limit).
pub fn set_stack_desc(g: &mut Globals, desc: &str) {
    g.xdebug.stack_desc = prefix(desc.as_bytes(), MAX_DESC_LENGTH - 1);
}

/// `xDbg_SetCurrentNote(fmt, ...)` (`DebugNote`).
pub fn set_current_note(g: &mut Globals, note: &str) {
    g.xdebug.cur_note = prefix(note.as_bytes(), MAX_DESC_LENGTH - 1);
}

/// `DebugEnterFunction((fmt, ...))`: pushes the current title and note, then the new title and
/// an empty note.
pub fn enter_function(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, title: &str) {
    let (t, n) = (g.xdebug.stack_desc.clone(), g.xdebug.cur_note.clone());
    push_stack(g, out, err, &t, &n);
    set_stack_desc(g, title);
    set_current_note(g, "");
}

/// `DebugExitFunction`.
pub fn exit_function(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write) {
    pop_stack(g, out, err);
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn stack() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        enter_function(&mut g, &mut out, &mut err, "MRISfoo");
        set_current_note(&mut g, "step 1");
        enter_function(&mut g, &mut out, &mut err, &"x".repeat(600));
        assert_eq!(g.xdebug.stack_desc.len(), 511);
        exit_function(&mut g, &mut out, &mut err);
        assert_eq!(
            (g.xdebug.stack_desc.as_slice(), g.xdebug.cur_note.as_slice()),
            (&b"MRISfoo"[..], &b"step 1"[..])
        );
        exit_function(&mut g, &mut out, &mut err);
        assert!(g.xdebug.stack_desc.is_empty());
        exit_function(&mut g, &mut out, &mut err);
        for _ in 0..60 {
            push_stack(&mut g, &mut out, &mut err, b"t", b"n");
        }
        assert_eq!(g.xdebug.titles.len(), 49);
        // Output is off: nothing is written.
        assert!(out.is_empty() && err.is_empty());
        g.xdebug.output = true;
        pop_stack(&mut g, &mut out, &mut err);
        push_stack(&mut g, &mut out, &mut err, b"t", b"n");
        push_stack(&mut g, &mut out, &mut err, b"t", b"n");
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "Stack limit reached, can't store name.\n"
        );
    }
}
