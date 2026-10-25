// SPDX-License-Identifier: Apache-2.0
//! ITK's output window for the warnings its file IO prints while a program runs
//! (`itkWarningMacro` in `NiftiImageIO`), kept per thread until the program's command line
//! writes them to its standard error ([`take`]).
//!
//! The readers and writers here are called from many places that have no error stream of their
//! own; ITK prints such warnings from wherever it meets them. A command line drains the buffer
//! when its program ends (larmorx-ants' `cli::run`), so the warnings follow the program's other
//! messages: ITK prints them while the file is written, which for the programs here is at their
//! end.

use std::cell::RefCell;

thread_local! {
    static WARNINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// At most this many warnings are kept (a library caller that never drains the buffer, e.g.
/// Python's `lx.save`, keeps only the first ones).
const KEPT: usize = 64;

/// `itkWarningMacro(message)` of an object of class `class` at `file:line` (ITK's own path
/// from `Modules/`; the object's address, which is not reproducible, as `0x0`).
pub fn warning(file: &str, line: u32, class: &str, message: &str) {
    let text = format!("WARNING: In {file}, line {line}\n{class} (0x0): {message}\n\n");
    WARNINGS.with(|w| {
        let mut w = w.borrow_mut();
        if w.len() < KEPT {
            w.push(text);
        }
    });
}

/// The warnings of this thread so far, oldest first; the buffer is left empty.
pub fn take() -> Vec<String> {
    WARNINGS.with(|w| std::mem::take(&mut *w.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_are_kept_until_taken() {
        let _ = take();
        warning(
            "Modules/IO/NIFTI/src/itkNiftiImageIO.cxx",
            2314,
            "NiftiImageIO",
            "x",
        );
        let w = take();
        assert_eq!(
            w,
            [
                "WARNING: In Modules/IO/NIFTI/src/itkNiftiImageIO.cxx, line 2314\nNiftiImageIO (0x0): x\n\n"
            ]
        );
        assert!(take().is_empty());
    }
}
