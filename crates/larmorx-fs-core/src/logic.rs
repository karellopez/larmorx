// SPDX-License-Identifier: Apache-2.0
//! Logic-problem reports (spec §17): counters per place, thinned to the first 32 calls and the
//! powers of two after them.

use std::collections::HashMap;

use crate::process::{BUILD_ROOT, Process, Stop};

/// The counters of the checks and of the reports, and whether reports are on (read once).
#[derive(Clone, Debug, Default)]
pub(crate) struct LogicState {
    checks: HashMap<(Vec<u8>, i32), u64>,
    reports: HashMap<(Vec<u8>, i32), u64>,
    report: Option<bool>,
}

/// A response to a logic problem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicResponse {
    /// The old behaviour (0).
    Old = 0,
    /// The fix (1).
    Fix = 1,
}

/// Whether the `count`-th call (from 1) is looked at: the first 32, then the powers of two.
fn thinned(count: u64) -> bool {
    count <= 32 || count.is_power_of_two()
}

impl Process {
    /// `spendTimeCheckingForLogicProblem(file, line)`. A line of one million or more is a failed
    /// assertion.
    pub fn spend_time_checking_for_logic_problem(
        &mut self,
        file: &[u8],
        line: i32,
    ) -> Result<bool, Stop> {
        if line >= 1_000_000 {
            let f = format!("{BUILD_ROOT}/utils/utils.cpp");
            return Err(self.assert_failed(f.as_bytes(), 1760, b"line < 1000000"));
        }
        let c = self.logic.checks.entry((file.to_vec(), line)).or_insert(0);
        *c += 1;
        Ok(thinned(*c))
    }

    /// `copeWithLogicProblem2(fixer, message, file, line, function)`: "fix" when the fixer
    /// variable is set (even empty), else "old"; reported on standard output only when
    /// `FREESURFER_reportLogicErrors` is set, with the same thinning. Returns the response and
    /// whether it reported.
    pub fn cope_with_logic_problem(
        &mut self,
        fixer: Option<&[u8]>,
        message: &[u8],
        file: &[u8],
        line: i32,
        function: &[u8],
    ) -> (LogicResponse, bool) {
        let response = match fixer {
            Some(v) if self.env.get_bytes(v).is_some() => LogicResponse::Fix,
            _ => LogicResponse::Old,
        };
        let on = match self.logic.report {
            Some(on) => on,
            None => {
                let on = self.env.is_set("FREESURFER_reportLogicErrors");
                self.logic.report = Some(on);
                on
            }
        };
        if !on {
            return (response, false);
        }
        let c = self.logic.reports.entry((file.to_vec(), line)).or_insert(0);
        *c += 1;
        if !thinned(*c) {
            return (response, false);
        }
        let mut b = file.to_vec();
        b.extend_from_slice(format!(":{line} ").as_bytes());
        b.extend_from_slice(function);
        b.extend_from_slice(b" - probable logic error: ");
        b.extend_from_slice(message);
        b.extend_from_slice(if response == LogicResponse::Fix {
            b", fixing it\n"
        } else {
            b", not fixing it\n"
        });
        if let Some(v) = fixer {
            b.extend_from_slice(b".  Set ");
            b.extend_from_slice(v);
            b.extend_from_slice(b" to fix it\n");
        }
        self.put_out(&b);
        (response, true)
    }
}
