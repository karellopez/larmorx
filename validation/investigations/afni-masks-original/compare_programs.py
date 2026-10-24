# SPDX-License-Identifier: Apache-2.0
"""The replica's and the original's transcripts (exit status, standard output and error) on the
command lines where larmorx speaks for itself: the rule-B refusals (AFNI-41, AFNI-48), the
notices of the corrected defects, a missing input; in both modes, in fresh directories. Prints
a unified diff of the two transcripts (working directories and elapsed times normalized), or
"IDENTICAL transcripts". How the clean-room original came to word every refusal and notice as
the replica does (2026-10-24), without reading the replica's source.

    python -P compare_programs.py <work> [replica binary]

<work> holds `blackbox.py phantoms <work>`'s inputs; the replica is $LARMORX_GPL_BIN or the one
blackbox.py finds, the original $LX_BIN or this checkout's `target/release/larmorx`.
"""

import difflib
import importlib.util
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("blackbox", HERE / "blackbox.py")
bb = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bb)

CASES = [
    ("nb40", "3dAutomask -nbhrs 40 -prefix m.nii @/data/head-f32.nii"),
    ("nbabc", "3dAutomask -nbhrs abc -prefix m.nii @/data/head-f32.nii"),
    ("nb5x", "3dAutomask -nbhrs 5x -prefix m.nii @/data/head-f32.nii"),
    ("nb40NN1", "3dAutomask -nbhrs 40 -NN1 -prefix m.nii @/data/head-f32.nii"),
    ("nb12then40", "3dAutomask -nbhrs 12 -nbhrs 40 -prefix m.nii @/data/head-f32.nii"),
    ("nb40then12", "3dAutomask -nbhrs 40 -nbhrs 12 -prefix m.nii @/data/head-f32.nii"),
    ("nb40missing", "3dAutomask -dilate 1 -erode 1 -nbhrs 40 -prefix m.nii @/data/nothere.nii"),
    ("nb40q", "3dAutomask -q -nbhrs 40 -prefix m.nii @/data/head-f32.nii"),
    ("p0nb40", "3dAutomask -peels 0 -nbhrs 40 -prefix m.nii @/data/head-f32.nii"),
    ("nb20", "3dAutomask -nbhrs 20 -prefix m.nii @/data/head-f32.nii"),
    ("nb40neg", "3dAutomask -nbhrs 40 -prefix m.nii @/data/series-negative.nii"),
    ("p0", "3dAutomask -peels 0 -prefix m.nii @/data/head-f32.nii"),
    ("amissing", "3dAutomask -prefix m.nii @/data/nothere.nii"),
    ("t2x3", "3dUnifize -T2 -T2 -T2 -prefix u.nii @/data/small-t2.nii"),
    ("t2x4input", "3dUnifize -input @/data/nothere.nii -T2 -T2 -T2 -T2 -prefix u.nii"),
    ("t2x3epi", "3dUnifize -T2 -T2 -T2 -EPI -prefix u.nii @/data/small-series.nii"),
    ("t2x3q", "3dUnifize -quiet -T2 -T2 -T2 -prefix u.nii @/data/small-t2.nii"),
    ("t2x3up", "3dUnifize -T2 -T2 -T2 -T2up 100 -prefix u.nii @/data/small-t2.nii"),
    ("t2x3ssave", "3dUnifize -T2 -T2 -T2 -ssave s.nii -prefix u.nii @/data/small-t2.nii"),
    ("t2up", "3dUnifize -T2 -T2up 100 -prefix u.nii @/data/small-t2.nii"),
    ("t2up9999", "3dUnifize -T2 -T2up 99.99 -prefix u.nii @/data/small-t2.nii"),
    ("epineg", "3dUnifize -EPI -prefix u.nii @/data/series-negative.nii"),
    ("uinmissing", "3dUnifize -input @/data/nothere.nii -prefix u.nii"),
    ("umissing", "3dUnifize -prefix u.nii @/data/nothere.nii"),
]


def transcript(work: Path, who: str) -> list[str]:
    lines = []
    for mode in ("fix", "reproduce"):
        for label, command in CASES:
            prog, *args = command.split()
            cwd = work / "compare" / f"{who}-{label}-{mode}"
            shutil.rmtree(cwd, ignore_errors=True)
            cwd.mkdir(parents=True)
            cmd, extra = bb._command(f"{who}:{prog}")
            env = {**bb._base_env(work, "1"), **extra, "LARMORX_UPSTREAM_DEFECTS": mode,
                   "AFNI_MESSAGE_COLORIZE": "NO"}  # fmt: skip
            p = subprocess.run([*cmd, *bb._expand(args, work)], cwd=cwd, env=env,
                               capture_output=True, text=True)  # fmt: skip
            text = p.stdout + p.stderr
            text = re.sub(r"Elapsed = [0-9.]+ sec", "Elapsed = N sec", text)
            lines += [f"===== {label} [{mode}]", *text.splitlines(), f"[exit {p.returncode}]"]
    return lines


def main() -> int:
    work = Path(sys.argv[1]).resolve()
    if len(sys.argv) > 2:
        bb.GPL = sys.argv[2]
    a, b = transcript(work, "gpl"), transcript(work, "lx")
    diff = list(difflib.unified_diff(a, b, "replica", "original", lineterm="", n=1))
    print("\n".join(diff) if diff else "IDENTICAL transcripts")
    return 0


if __name__ == "__main__":
    os.environ.setdefault("AFNI_MESSAGE_COLORIZE", "NO")
    sys.exit(main())
