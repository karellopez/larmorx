// SPDX-License-Identifier: Apache-2.0
//! The debugging stack (spec §19): the current title and note of the functions entered, and a
//! stack of at most 49 (title, note) pairs, for crash reports.

use crate::process::Process;

/// The stack's capacity.
pub const STACK_LIMIT: usize = 49;
/// The bytes a title or note keeps.
pub const TEXT_LIMIT: usize = 511;

fn kept(s: &[u8]) -> Vec<u8> {
    s[..s.len().min(TEXT_LIMIT)].to_vec()
}

/// The current title and note and the stack.
#[derive(Clone, Debug, Default)]
pub(crate) struct DebugStack {
    title: Vec<u8>,
    note: Vec<u8>,
    stack: Vec<(Vec<u8>, Vec<u8>)>,
}

impl Process {
    /// `xDbg_PushStack(title, note)`: pushes the pair (a full stack changes nothing).
    pub fn xdbg_push(&mut self, title: &[u8], note: &[u8]) {
        let s = &mut self.xdbg;
        if s.stack.len() < STACK_LIMIT {
            s.stack.push((kept(title), kept(note)));
        }
    }

    /// `xDbg_PopStack`: the top pair back into the current title and note (an empty stack
    /// changes nothing).
    pub fn xdbg_pop(&mut self) {
        let s = &mut self.xdbg;
        if let Some((t, n)) = s.stack.pop() {
            s.title = t;
            s.note = n;
        }
    }

    /// Entering a function (`DebugEnterFunction`): pushes the current title and note, then the
    /// given title is current and the note empty.
    pub fn debug_enter_function(&mut self, title: &[u8]) {
        let (t, n) = (self.xdbg.title.clone(), self.xdbg.note.clone());
        self.xdbg_push(&t, &n);
        self.xdbg.title = kept(title);
        self.xdbg.note.clear();
    }

    /// Leaving a function (`DebugExitFunction`).
    pub fn debug_exit_function(&mut self) {
        self.xdbg_pop();
    }

    /// `xDbg_SetStackDesc`: the current title.
    pub fn xdbg_set_stack_desc(&mut self, text: &[u8]) {
        self.xdbg.title = kept(text);
    }

    /// `xDbg_SetCurrentNote`.
    pub fn xdbg_set_current_note(&mut self, text: &[u8]) {
        self.xdbg.note = kept(text);
    }

    /// `xDbg_GetCurrentFunction`: the title on top of the stack, `(No current function)` when
    /// it is empty.
    pub fn xdbg_current_function(&self) -> Vec<u8> {
        self.xdbg
            .stack
            .last()
            .map_or_else(|| b"(No current function)".to_vec(), |(t, _)| t.clone())
    }

    /// `xDbg_GetCurrentNote`.
    pub fn xdbg_current_note(&self) -> &[u8] {
        &self.xdbg.note
    }

    /// The current title (`xDbg_sStackDesc`).
    pub fn xdbg_stack_desc(&self) -> &[u8] {
        &self.xdbg.title
    }

    /// `xDbg_Printf`: writes nothing, but flushes every output stream.
    pub fn xdbg_printf(&mut self) {
        self.flush();
    }
}
