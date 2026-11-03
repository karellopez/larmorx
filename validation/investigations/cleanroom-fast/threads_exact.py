# SPDX-License-Identifier: Apache-2.0
"""FAST's clean-room original: the same bits from two builds and on any thread count.

Runs ``larmorx fsl fast`` of a reference build (``--ref``) on 1 thread and of the build under
test (``--new``) on each of ``--threads``, on one command per code path the speed work touched
(fMRIPrep's call, the bias field, -v's sweep totals, 2, 4 and 5 classes, -t 2 and the MRF
options, a 2D image, several channels with their bias fields and -n 4, the priors with -P), and
compares every output file (decompressed), the standard output and the standard error byte for
byte. Prints one line per command and the differences; exit status 1 when any differ.

The inputs are the replica suite's derived inputs (``<workspace>/oracles/inputs/
fsl-fast-replica``); no FSL program runs. Run it inside a memory box::

    scripts/orchestration/boxed.sh 8 python -P validation/investigations/cleanroom-fast/threads_exact.py \\
        --ref <old larmorx> --new target/release/larmorx --threads 1 14
"""

from __future__ import annotations

import argparse
import gzip
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

CASES = {
    "fmriprep": "-I 0 -N -p -g -S 1 @t1_half.nii.gz",
    "default-verbose": "-v -o seg @t1_half.nii.gz",
    "n2-bias": "-n 2 -b -B -o seg @t1_half.nii.gz",
    "n4-probs": "-n 4 -p -o seg @t1_half.nii.gz",
    "n5": "-n 5 -I 1 -O 1 -o seg @t1_half.nii.gz",
    "t2-options": "-t 2 -H 0.3 -R 0.5 -f 0.05 -l 30 -o seg @t1_half.nii.gz",
    "2d": "-p -o seg @t1_2d.nii.gz",
    "s2-outputs": "-S 2 -B -b -g -p -v -o seg @mni_t1.nii.gz @mni_t2.nii.gz",
    "s2-n4": "-S 2 -n 4 -g -o seg @mni_t2.nii.gz @mni_t1.nii.gz",
    "s3": "-S 3 -B -b -o seg @mni_bold.nii.gz @mni_t2.nii.gz @mni_t1.nii.gz",
    "s5-n5": "-S 5 -n 5 -P -v -o seg @mni_sum.nii.gz @mni_ratio.nii.gz @mni_bold.nii.gz @mni_t2.nii.gz @mni_t1.nii.gz",
    "s2-priors": "-S 2 -A @tpl_csf.nii.gz @tpl_gm.nii.gz @tpl_wm.nii.gz -a @ident.mat -P -o seg @mni_t2.nii.gz @mni_t1.nii.gz",
    "priors-P": "-A @tpl_csf.nii.gz @tpl_gm.nii.gz @tpl_wm.nii.gz -a @ident.mat -P -o seg @mni_t1.nii.gz",
}


def run(exe: str, args: str, inputs: Path, work: Path, threads: int) -> dict[str, bytes]:
    work.mkdir(parents=True)
    argv = []
    for a in args.split():
        if a.startswith("@"):
            shutil.copy(inputs / a[1:], work / a[1:])
            a = a[1:]
        argv.append(a)
    before = {p.name for p in work.iterdir()}
    env = {
        "PATH": "/usr/bin:/bin",
        "HOME": os.environ.get("HOME", "/tmp"),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        "FSLDIR": str(work),
        "OMP_NUM_THREADS": str(threads),
        "LARMORX_IMPLEMENTATION": "original",
    }
    p = subprocess.run(
        [exe, "fsl", "fast", *argv], cwd=work, env=env, capture_output=True, check=False
    )
    out = {"<exit>": str(p.returncode).encode(), "<stdout>": p.stdout, "<stderr>": p.stderr}
    for f in sorted(work.iterdir()):
        if f.name not in before and f.is_file():
            raw = f.read_bytes()
            out[f.name] = gzip.decompress(raw) if f.name.endswith(".gz") else raw
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--ref", required=True)
    ap.add_argument("--new", required=True)
    ap.add_argument("--threads", type=int, nargs="+", default=[1, 14])
    ap.add_argument("--inputs", type=Path)
    ap.add_argument("--case", nargs="*")
    a = ap.parse_args()
    a.ref, a.new = str(Path(a.ref).resolve()), str(Path(a.new).resolve())
    ws = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[5]))
    inputs = a.inputs or ws / "oracles" / "inputs" / "fsl-fast-replica"
    bad = 0
    with tempfile.TemporaryDirectory(prefix="lx-fast-exact-") as t:
        for name, args in CASES.items():
            if a.case and name not in a.case:
                continue
            ref = run(a.ref, args, inputs, Path(t) / name / "ref", 1)
            diffs = []
            for n in a.threads:
                new = run(a.new, args, inputs, Path(t) / name / f"new{n}", n)
                for k in sorted(set(ref) | set(new)):
                    if ref.get(k) != new.get(k):
                        diffs.append(f"{n} threads: {k}")
            files = len(ref) - 3
            print(
                f"{name:16s} exit {ref['<exit>'].decode()} {files:2d} files: "
                + ("identical" if not diffs else f"{len(diffs)} DIFFER")
            )
            for d in diffs:
                print("    " + d)
            bad += bool(diffs)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
