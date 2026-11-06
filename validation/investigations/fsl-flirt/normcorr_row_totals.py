# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""specs/flirt.md §6.4 and specs/mcflirt.md §6.4 (spec writer, 2026-11-06): the cause of the
clean-room record's `real/weights-normcorr`. Builds a probe of the FLIRT replica whose normalised
correlation adds each row's total weight to the count's running total at the row's end instead of
each point's weight as it is visited (FSL's), into a separate target directory, then restores the
source. Run the probe (`LARMORX_UPSTREAM_DEFECTS=reproduce <target>/release/larmorx-nc fsl flirt
-in epi.nii -ref t1.nii -refweight refweight.nii -cost normcorr -dof 6 -omat out.mat -verbose 7`
on the case's inputs) and compare its trace with FSL's and the original's
(`validation/investigations/cleanroom-flirt/probe.py <dir> --args "<the same> -verbose 7"`):
with row totals the replica gives the original's evaluations (the 4 mm stage's 47th: 0.050056,
FSL 0.050021) and its final matrix to every printed digit.

Usage (from the repository root, cargo on the PATH): normcorr_row_totals.py <target dir>
"""

import os
import subprocess
import sys
from pathlib import Path

SRC = Path("crates-nc/larmorx-nc-core/src/costfns.rs")


def main() -> None:
    target = sys.argv[1]
    s = SRC.read_text()
    a = s.index("pub fn p_normcorr_weighted(")
    b = s.index("normcorr_from_sums(num_b", a)
    seg = s[a:b]
    seg2 = seg.replace(
        "let (mut num, mut num_a, mut num_b) = (0f32, 0f32, 0f32);",
        "let (mut num, mut num_a, mut num_b) = (0f32, 0f32, 0f32);\n    let mut row_num = 0f32;",
    )
    seg2 = seg2.replace(
        "                        num += weight;\n", "                        row_num += weight;\n"
    )
    seg2 = seg2.replace(
        "            num_a += num;\n",
        "            num += row_num;\n            row_num = 0.0;\n            num_a += num;\n",
    )
    assert seg2.count("row_num") == 4, "the replica's source changed: adapt the probe"
    try:
        SRC.write_text(s[:a] + seg2 + s[b:])
        env = dict(os.environ, CARGO_TARGET_DIR=target)
        subprocess.run(
            ["cargo", "build", "--release", "-p", "larmorx-nc-cli"],
            cwd="crates-nc",
            env=env,
            check=True,
        )
    finally:
        SRC.write_text(s)
    print(f"probe: {target}/release/larmorx-nc (source restored)")


main()
