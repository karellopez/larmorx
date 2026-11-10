# SPDX-License-Identifier: Apache-2.0
"""nipype's command lines on Windows (``larmorx._nipype``): the words of a line keep a path's
backslashes, and BET's relative names keep nipype's quotes and a path on another drive (CI,
2026-11-10: on Windows antsRegistration, topup and BET were given ``C:Users...``)."""

from __future__ import annotations

import shlex

from larmorx import _nipype


def test_the_words_of_a_line_on_windows_keep_backslashes():
    line = (
        r"--output [ C:\Users\RUNNER~1\AppData\Local\Temp\x\ants_susceptibility, "
        r"C:\Users\RUNNER~1\x\warped.nii.gz ] -f 'C:\Program Files\a b\ref.nii.gz' "
        r'--config "D:\a\larmorx\b02b0_quick.cnf" ' + shlex.quote(r"C:\it's\file.nii.gz")
    )
    assert _nipype._split_line(line, windows=True) == [
        "--output",
        "[",
        r"C:\Users\RUNNER~1\AppData\Local\Temp\x\ants_susceptibility,",
        r"C:\Users\RUNNER~1\x\warped.nii.gz",
        "]",
        "-f",
        r"C:\Program Files\a b\ref.nii.gz",
        "--config",
        r"D:\a\larmorx\b02b0_quick.cnf",
        r"C:\it's\file.nii.gz",
    ]


def test_the_words_of_a_line_elsewhere_are_the_shells():
    line = r"-i /tmp/a\ b/x.nii.gz '/tmp/c d/y.nii.gz' # not a comment"
    assert _nipype._split_line(line, windows=False) == shlex.split(line)
    assert _nipype._split_line(line, windows=False)[1] == "/tmp/a b/x.nii.gz"


def test_bets_relative_names_on_windows():
    start = r"C:\Users\runneradmin\Temp\work\ss\skullstrip_first_pass"
    quoted = shlex.quote(r"C:\Users\runneradmin\Temp\work\ss\head.nii.gz")
    assert quoted.startswith("'")
    assert _nipype._bet_relpath(quoted, start, windows=True) == r"'..\head.nii.gz'"
    assert _nipype._bet_relpath(r"C:\Users\x\head.nii.gz", r"C:\Users\y", windows=True) == (
        r"..\x\head.nii.gz"
    )
    # another drive: no relative path joins two drives, the path stays as it is
    other = shlex.quote(r"D:\a\head.nii.gz")
    assert _nipype._bet_relpath(other, start, windows=True) == other
    words = _nipype._split_line("-i " + _nipype._bet_relpath(quoted, start, windows=True), True)
    assert words == ["-i", r"..\head.nii.gz"]
