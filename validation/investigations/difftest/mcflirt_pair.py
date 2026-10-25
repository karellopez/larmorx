# SPDX-License-Identifier: Apache-2.0
"""An mcflirt finding of the differential tests by hand: its shrunk inputs rebuilt, FSL 6.0.7's
mcflirt and the replica (``larmorx-nc fsl mcflirt``, ``reproduce``) run on its arguments with
optional edits, and where they part: the first differing line of standard error, the ``.par``
rows that differ (with every digit), and the output images' differing voxels.

    python -P validation/investigations/difftest/mcflirt_pair.py <finding.json> <dir> \
        [--threads N] [--preload <libcrmath.so>] ["<old>=><new>" ...]

``--preload`` runs mcflirt with a CORE-MATH preload (``build_crmath_double_preload.sh``: the
double functions correctly rounded, glibc's float ones kept, as the replica computes).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.difftest import core, inputs
from larmorx_validation.parity import fsl_mcflirt_replica as fr


def main() -> int:
    argv = sys.argv[1:]
    threads = 1
    if "--threads" in argv:
        i = argv.index("--threads")
        threads = int(argv[i + 1])
        del argv[i : i + 2]
    oracle_settings: dict[str, str | None] = {}
    if "--preload" in argv:
        i = argv.index("--preload")
        oracle_settings["LD_PRELOAD"] = argv[i + 1]
        del argv[i : i + 2]
    finding = json.loads(Path(argv[0]).read_text(encoding="utf-8"))
    work = Path(argv[1]).resolve()
    edits = [e.split("=>", 1) for e in argv[2:]]
    recipe = finding.get("shrunk") or finding["recipe"]
    work.mkdir(parents=True, exist_ok=True)
    paths = {}
    for name, spec in recipe["inputs"].items():
        p = work / (name + inputs.suffix_of(spec))
        inputs.materialize(spec, p)
        paths[name] = str(p)
    line = " ".join(core.substitute(core.arguments(recipe), lambda n: paths[n]))
    for old, new in edits:
        assert old in line, old
        line = line.replace(old, new)
    print("args:", line)
    runs = {}
    for who in ("mcflirt", "replica"):
        d = work / who
        d.mkdir(exist_ok=True)
        for f in d.iterdir():
            if f.is_file():
                f.unlink()
        args = line.replace("{run}", str(d)).split()
        if who == "mcflirt":
            runs[who] = fr.run_oracle(args, cwd=d, settings=oracle_settings)
        else:
            runs[who] = fr.run_replica(args, cwd=d, settings={}, mode="reproduce", threads=threads)
        (work / f"{who}.stderr.txt").write_text(runs[who].stderr, encoding="utf-8")
        print(f"{who}: exit {runs[who].code}")
    ea, er = runs["mcflirt"].stderr.splitlines(), runs["replica"].stderr.splitlines()
    first = next((i for i, (x, y) in enumerate(zip(ea, er, strict=False)) if x != y), None)
    if first is not None:
        print(
            f"stderr line {first + 1} of {len(ea)}:\n  mcflirt {ea[first]}\n  replica {er[first]}"
        )
    a, r = work / "mcflirt", work / "replica"
    for f in sorted(a.glob("*.par")):
        pa, pr = f.read_text().splitlines(), (r / f.name).read_text().splitlines()
        for i, (x, y) in enumerate(zip(pa, pr, strict=False)):
            if x != y:
                print(f"{f.name} volume {i}:\n  mcflirt {x}\n  replica {y}")
    for f in sorted(a.glob("*.nii.gz")):
        g = r / f.name
        if not g.is_file():
            continue
        x, y = np.asarray(nib.load(f).dataobj), np.asarray(nib.load(g).dataobj)
        if x.shape != y.shape:
            print(f"{f.name}: shapes {x.shape} {y.shape}")
            continue
        d = np.argwhere(x != y)
        if len(d):
            vols = sorted({int(v[3]) for v in d}) if x.ndim == 4 else []
            print(
                f"{f.name}: {len(d)} voxels differ, volumes {vols}; e.g. {tuple(int(i) for i in d[0])}: {x[tuple(d[0])]!r} {y[tuple(d[0])]!r}"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main())
