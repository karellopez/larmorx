# SPDX-License-Identifier: Apache-2.0
"""The BET suite's messages (``larmorx_validation.parity.fsl_bet``): bash's report of a program a
script ran that ended by a signal names its process ID, right-aligned in five columns, so the
spaces before it change with the ID's number of digits (2026-11-18: the case
``defects/flirt-refuses`` failed on "line 229:  PID Aborted" against "line 229: PID Aborted" once
the machine's process IDs wrapped below 10,000)."""

from __future__ import annotations

import pytest
from larmorx_validation.parity import fsl_bet as suite

#: A 4-digit, a 5-digit and a 7-digit process ID.
PIDS = (4321, 12345, 1915072)


def _report(pid: int, line: int = 229, signal: str = "Aborted") -> str:
    """bash's report as it prints it: the ID in five columns (``%5d``), the status padded."""
    return (
        f"bet: line {line}: {pid:5d} {signal:<24}(core dumped) "
        "${FSLDIR}/bin/flirt -in in -ref ref -bogus\n"
    )


def test_the_report_keeps_the_padding_this_test_is_about() -> None:
    assert ": line 229:  4321 Aborted" in _report(4321)
    assert ": line 229: 12345 Aborted" in _report(12345)
    assert ": line 229: 1915072 Aborted" in _report(1915072)


@pytest.mark.parametrize("signal", ["Aborted", "Segmentation fault"])
def test_every_process_id_masks_to_one_text(signal: str) -> None:
    masked = {suite._mask_pids(_report(pid, signal=signal)) for pid in PIDS}
    assert masked == {
        f"bet: line 229: PID {signal:<24}${{FSLDIR}}/bin/flirt -in in -ref ref -bogus\n"
    }


@pytest.mark.parametrize("fsl_pid", PIDS)
@pytest.mark.parametrize("our_pid", PIDS)
def test_both_sides_of_the_comparison_agree(fsl_pid: int, our_pid: int) -> None:
    fsl = suite.fsl_text("Running FLIRT\n" + _report(fsl_pid))
    ours = suite.larmorx_text(
        "Running FLIRT\n" + _report(our_pid) + "larmorx: an upstream-defect notice\n"
    )
    assert fsl == ours


def test_the_masked_report_is_still_read_as_a_crash() -> None:
    for pid in PIDS:
        masked = suite._mask_pids("Running FLIRT\n" + _report(pid))
        assert suite._BASH_SIGNAL.search(masked), masked
        run = suite.Run(0, "", masked, directory=None)  # type: ignore[arg-type]
        assert suite.crashed(run)
        assert suite.before_crash(masked) == "Running FLIRT\n"


def test_what_is_not_a_process_id_still_differs() -> None:
    a, b = (suite._mask_pids(_report(4321, line=n)) for n in (229, 230))
    assert a != b
    plain = "bet: line 229: a message without a signal\n"
    assert suite._mask_pids(plain) == plain
