// SPDX-License-Identifier: Apache-2.0
//! Messages (spec §2) and the end of a program (§3): errors, warnings, debug lines, fatal
//! errors, FreeSurfer's older C-style error calls, the error log, the older programs' option
//! errors and usage exit, and failed assertions.

use std::io::Write;

use larmorx_core::defects::{FS_01, FS_03, FS_263};

use crate::clib::strerror;
use crate::process::{BUILD_ROOT, DIAG_VERBOSE, Process, Stop};

/// The exit status of a fatal error with code `code` (its low 8 bits, §3).
pub fn exit_status(code: i32) -> i32 {
    code & 0xff
}

impl Process {
    /// Sets the error log (`setErrorLog`): every later error line is appended to this file.
    pub fn set_error_log(&mut self, path: &[u8]) {
        self.error_log = Some(path.to_vec());
    }

    /// Switches fatal errors into exceptions (`throwExceptions`).
    pub fn set_throw_exceptions(&mut self, on: bool) {
        self.throw_exceptions = on;
    }

    /// Appends `line` and a newline to the error log, when one is set; a log that cannot be
    /// opened is skipped without a message but leaves `errno` set (§2).
    fn log_line(&mut self, line: &[u8]) {
        let Some(path) = self.error_log.clone() else {
            return;
        };
        let opened = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(crate::files::path_of(&path));
        match opened {
            Ok(mut f) => {
                let mut b = line.to_vec();
                b.push(b'\n');
                let _ = f.write_all(&b);
            }
            Err(e) => self.errno = e.raw_os_error().unwrap_or(5),
        }
    }

    /// An error message (`fs::error`): `error: <message>\n` on standard error, and the message
    /// in the error log.
    pub fn error(&mut self, message: &[u8]) {
        let mut b = b"error: ".to_vec();
        b.extend_from_slice(message);
        b.push(b'\n');
        self.put_err(&b);
        self.log_line(message);
    }

    /// A warning (`fs::warning`): `warning: <message>\n` on standard error (not in the log).
    pub fn warning(&mut self, message: &[u8]) {
        let mut b = b"warning: ".to_vec();
        b.extend_from_slice(message);
        b.push(b'\n');
        self.put_err(&b);
    }

    /// A debug line (`fs::debug`): `debug: <message>\n`, only with the verbose diagnostic bit.
    pub fn debug(&mut self, message: &[u8]) {
        if self.gdiag & DIAG_VERBOSE != 0 {
            let mut b = b"debug: ".to_vec();
            b.extend_from_slice(message);
            b.push(b'\n');
            self.put_err(&b);
        }
    }

    /// A fatal error (`fs::fatal(code)`): the message as an error, then the end of the program
    /// with status `code mod 256`. With exceptions on, code 0 returns `Ok` (the program goes
    /// on) and another code is FS-263 ([`Process::throw_or_abort`]).
    pub fn fatal(&mut self, code: i32, message: &[u8]) -> Result<(), Stop> {
        self.error(message);
        self.end_with(code)
    }

    /// What a fatal error with `code` does after its message (§3).
    fn end_with(&mut self, code: i32) -> Result<(), Stop> {
        if !self.throw_exceptions {
            return Err(Stop::Exit(exit_status(code)));
        }
        if code == 0 {
            return Ok(());
        }
        self.throw_or_abort(code)
    }

    /// FS-263: with exceptions on, a fatal error with a code other than 0. `fix`: the code
    /// reaches the caller ([`Stop::Thrown`]); `reproduce`: the C++ library's terminate line and
    /// an abort (status 134).
    fn throw_or_abort(&mut self, code: i32) -> Result<(), Stop> {
        if self.mode.reproduce() {
            self.note(
                &FS_263,
                "the fatal error aborted the program (status 134) instead of reaching its handler",
            );
            self.put_err(b"terminate called after throwing an instance of 'int'\n");
            self.flush();
            return Err(Stop::Exit(134));
        }
        self.note(
            &FS_263,
            format!("the fatal error's code {code} reached the program's handler instead of an abort (status 134)"),
        );
        Err(Stop::Thrown(code))
    }

    /// The C-style error (`ErrorPrintf(code, ...)`, the message already formatted): the code
    /// becomes the last error; the C library's text of `errno` when it is not 0, then the
    /// message, as errors (FS-01: `reproduce` writes a NUL byte after the message).
    pub fn error_printf(&mut self, code: i32, message: &[u8]) {
        self.gerror = code;
        if self.errno != 0 {
            let text = strerror(self.errno);
            let mut b = b"error: ".to_vec();
            b.extend_from_slice(text.as_bytes());
            b.push(b'\n');
            self.put_err(&b);
            self.log_line(text.as_bytes());
        }
        let mut m = message.to_vec();
        if self.mode.reproduce() {
            m.push(0);
            self.note(&FS_01, "a NUL byte written after a C-style error's message");
        } else {
            self.note(
                &FS_01,
                "no NUL byte written after a C-style error's message",
            );
        }
        let mut b = b"error: ".to_vec();
        b.extend_from_slice(&m);
        b.push(b'\n');
        self.put_err(&b);
        self.log_line(&m);
    }

    /// `ErrorReturn(value, (code, ...))`: the C-style error, then `value` for the caller.
    pub fn error_return<T>(&mut self, value: T, code: i32, message: &[u8]) -> T {
        self.error_printf(code, message);
        value
    }

    /// The C-style "print and exit" (`ErrorExit(code, ...)`): the C-style error, then the end
    /// of the program with status `code mod 256` (with exceptions on, as [`Process::fatal`]).
    pub fn error_exit(&mut self, code: i32, message: &[u8]) -> Result<(), Stop> {
        self.error_printf(code, message);
        self.end_with(code)
    }

    /// The older programs' "flag needs arguments" (`CMDargNErr`): its line on standard error,
    /// exit status 255.
    pub fn cmd_arg_n_err(&mut self, flag: &[u8], n: i32) -> Stop {
        let mut b = b"ERROR: ".to_vec();
        b.extend_from_slice(flag);
        b.extend_from_slice(
            format!(
                " flag needs {n} argument{}\n",
                if n == 1 { "" } else { "s" }
            )
            .as_bytes(),
        );
        self.put_err(&b);
        Stop::Exit(255)
    }

    /// The usage exit (`CMDusageExit`): nothing printed, exit status 1.
    pub fn cmd_usage_exit(&mut self) -> Stop {
        Stop::Exit(1)
    }

    /// A failed internal assertion (`assertFailed(file, line, test)`): its line on standard
    /// output and standard error, the break line on standard output, then the end of the
    /// program: FS-03, exit status 1 (`fix`) or 139, the segmentation fault's status
    /// (`reproduce`). FreeSurfer's endless wait under `FREESURFER_HANG_ON_ASSERT` is never
    /// reproduced.
    pub fn assert_failed(&mut self, file: &[u8], line: i32, test: &[u8]) -> Stop {
        let mut b = b"ASSERTION FAILED: ".to_vec();
        b.extend_from_slice(file);
        b.extend_from_slice(format!(":{line} ").as_bytes());
        b.extend_from_slice(test);
        b.push(b'\n');
        self.put_out(&b);
        self.put_err(&b);
        self.put_out(format!(" break {BUILD_ROOT}/utils/diag.cpp:85\n").as_bytes());
        self.flush();
        if self.mode.reproduce() {
            self.note(
                &FS_03,
                "the failed assertion ended the program with a segmentation fault's status 139",
            );
            Stop::Exit(139)
        } else {
            self.note(&FS_03, "the failed assertion ended the program with status 1 instead of a segmentation fault");
            Stop::Exit(1)
        }
    }

    /// HIPS (no longer supported, §19): every HIPS function writes this error and returns 0.
    pub fn hips_unsupported(&mut self) -> i32 {
        self.put_err(b"error: hips is not supported\n");
        0
    }
}
