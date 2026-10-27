# SPDX-License-Identifier: Apache-2.0
"""How far antsAI's own result moves on fMRIPrep's calls where larmorx (ANTs 2.6.5 at one thread,
bit for bit) and fMRIPrep's own transform (ANTs 2.6.2 in fMRIPrep's image, at the node's
``NSLOTS``) are far apart: each call run again by the ANTs 2.6.5 oracle at 1 thread and three
times at 8 threads, and by fMRIPrep's image at 1 thread and twice at the node's thread count, every
transform compared with fMRIPrep's file and with ANTs 2.6.5 at one thread (the largest difference
of a parameter, and the largest displacement over a 100 mm cube centred on the transform's centre).

    PYTHONPATH=<checkout>/python:<checkout>/validation/src \\
        python -P ai_variability.py <out dir> <run>/<node below fmriprep_25_2_wf> ...

Separates the causes (``docs/porting-guide.md`` §6): a version or build difference shows between
the image's and the oracle's runs at the same thread count; the thread count's own effect between
1 and N threads of one build; a race between repeated runs at N threads.
"""

from __future__ import annotations

import itertools
import json
import os
import subprocess
import sys
from pathlib import Path

import numpy as np


def affine(path: Path) -> tuple[np.ndarray, np.ndarray]:
    """An ITK affine file's matrix (as a 4x4 point map) and its parameters."""
    import larmorx as lx

    t = lx.transforms.read(str(path), upstream_defects="reproduce")[-1]
    p = np.asarray(t.parameters, dtype=np.float64)
    c = np.asarray(t.fixed_parameters, dtype=np.float64)
    m = p[:9].reshape(3, 3)
    off = p[9:12] + c - m @ c
    a = np.eye(4)
    a[:3, :3], a[:3, 3] = m, off
    return a, p


def displacement(a: np.ndarray, b: np.ndarray, centre: np.ndarray) -> float:
    """The largest distance between where ``a`` and ``b`` map the corners of a 100 mm cube."""
    corners = np.array(list(itertools.product((-50.0, 50.0), repeat=3))) + centre
    pts = np.c_[corners, np.ones(8)]
    return float(np.linalg.norm((pts @ a.T - pts @ b.T)[:, :3], axis=1).max())


def main() -> int:
    from larmorx_validation.parity import ants_ai
    from larmorx_validation.parity.ants_programs import ants_bin
    from larmorx_validation.parity.fmriprep_nodes import oracle_run
    from larmorx_validation.stages.runners import Settings, container_command

    import larmorx as lx

    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    rows = []
    for spec in sys.argv[2:]:
        run = spec.split("/", 1)[0]
        work, args, seed = ants_ai._fmriprep_node(spec)
        theirs = work / "initialization.mat"
        r = oracle_run(run)
        nslots = 8
        report = (work / "_report" / "report.rst").read_text(errors="replace")
        for line in report.splitlines():
            if line.startswith("* NSLOTS : "):
                nslots = int(line.split(":")[1])
        case = out / spec.replace("/", "_")
        case.mkdir(exist_ok=True)
        results: dict[str, Path] = {}
        o_idx = args.index("-o")
        for label, threads in [("oracle-1", 1), *((f"oracle-8-{k}", 8) for k in (1, 2, 3))]:
            d = case / label
            d.mkdir(exist_ok=True)
            a = list(args)
            a[o_idx + 1] = str(d / "initialization.mat")
            env = {k: v for k, v in os.environ.items() if k not in ("LD_PRELOAD", "NSLOTS")}
            env.update(ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=str(threads), ANTS_RANDOM_SEED=seed)
            subprocess.run([str(ants_bin() / "antsAI"), *a], env=env, capture_output=True,
                           check=False)  # fmt: skip
            results[label] = d / "initialization.mat"
        words = (work / "command.txt").read_text(encoding="utf-8").split()
        s = Settings(run=r, compare=case)
        for label, threads in (("image-1", 1), (f"image-{nslots}-1", nslots),
                               (f"image-{nslots}-2", nslots)):  # fmt: skip
            d = case / label
            d.mkdir(exist_ok=True)
            env = {"NSLOTS": str(threads), "ANTS_RANDOM_SEED": seed, "OMP_NUM_THREADS": "1"}
            subprocess.run(container_command(s, d, words, env), capture_output=True, check=False)
            results[label] = d / "initialization.mat"
        ref, _ = affine(results["oracle-1"])
        mine_fm, _ = affine(theirs)
        centre = np.asarray(lx.transforms.read(str(theirs))[-1].fixed_parameters, float)[:3]
        row = {
            "case": spec,
            "nslots": nslots,
            "fmriprep_vs_oracle1": displacement(mine_fm, ref, centre),
        }
        for label, path in results.items():
            if not path.exists():
                row[label] = "missing"
                continue
            m, _ = affine(path)
            row[f"{label}_vs_oracle1"] = displacement(m, ref, centre)
            row[f"{label}_vs_fmriprep"] = displacement(m, mine_fm, centre)
        rows.append(row)
        print(json.dumps(row), flush=True)
    (out / "ai_variability.json").write_text(json.dumps(rows, indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
