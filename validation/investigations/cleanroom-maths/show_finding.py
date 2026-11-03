# SPDX-License-Identifier: Apache-2.0
"""A random-testing finding of fslmaths' or fslstats' clean-room original
(``difftest --tools fsl-maths-original,fsl-stats-original``) made concrete for its triage: the
shrunk recipe's inputs written into ``<outdir>/fsl`` and ``<outdir>/orig``, FSL 6.0.7.23 run in
the first (through the oracle cache, ``use`` mode) and the original (``LARMORX_MATHS_BIN``, in
``reproduce`` mode unless ``--fix``) in the second; both runs' exit status, standard output and
error printed, and the input and output images' sizes, header codes and first values, so the
files can be compared by hand (findings 24 to 30 of ``docs/findings/fsl-maths-original.md``).

Run (in a memory box): ``python -P show_finding.py <finding.json> <outdir> [--fix] [--full]``.
"""

import json
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.difftest.tools import fsl_avwutils
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_maths as fo
from larmorx_validation.parity import fsl_stats as so


def _show(d: Path, full: bool) -> None:
    for p in sorted(d.iterdir()):
        if ".nii" in p.name or p.name.endswith((".hdr", ".hdr.gz")):
            try:
                im = nib.load(str(p))
            except Exception as e:  # noqa: BLE001
                print(f"  {p.name}: {e}")
                continue
            a = np.asarray(im.dataobj)
            h = im.header
            vals = a.ravel()[: (a.size if full else 8)]
            print(f"  {p.name}: {a.dtype}{a.shape} q{int(h['qform_code'])} s{int(h['sform_code'])}"
                  f" pixdim {h['pixdim'][:5]} values {vals}")  # fmt: skip
        else:
            text = p.read_text(encoding="utf-8", errors="replace")
            print(f"  {p.name}: {text if full else text[:400]!r}")


def main(argv: list[str]) -> int:
    finding = json.loads(Path(argv[0]).read_text(encoding="utf-8"))
    out = Path(argv[1])
    fix = "--fix" in argv
    full = "--full" in argv
    recipe = finding.get("shrunk") or finding["recipe"]
    stats = finding["tool"] == "fsl-stats-original"
    mod = so if stats else fo
    case = (fsl_avwutils.case_stats if stats else fsl_avwutils.case_maths)(recipe, "original")
    run = case.payload[1]
    a, b = out / "fsl", out / "orig"
    for d in (a, b):
        d.mkdir(parents=True, exist_ok=True)
    args = mod._prepare(run, a)
    mod._prepare(run, b)
    settings = dict(run.settings)
    print("args:", " ".join(args), "| settings:", settings)
    print("inputs:")
    _show(a, full)
    fsl = mod.rep.run_oracle(args, cwd=a, settings=settings)
    ours = mod.run_original(
        args, cwd=b, settings=settings, mode=defects.FIX if fix else defects.REPRODUCE
    )
    for name, r, d in (("FSL", fsl, a), ("original", ours, b)):
        print(f"--- {name}: exit {r.code}")
        print(f"stdout: {r.stdout[:1500]!r}")
        print(f"stderr: {r.stderr[:1500]!r}")
        _show(d, full)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
