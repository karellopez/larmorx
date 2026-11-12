# SPDX-License-Identifier: Apache-2.0
"""Black-box probe of the reading and fit edges relay 3c needed (2026-11-11): tedana 26.0.3
(through the oracle-output cache), the original in both modes and the replica's default
(``LARMORX_GPL_BIN``), each one's exit status and standard error as the contract keeps it
(``replay.py``'s form: a traceback's first and last lines, no warning location or source line).

The cases: the order of the first reading's two errors for an echo that is both 3D and of another
grid; the second reading's checks across echoes (a 3D first echo before a second echo of another
grid); 4D masks of one volume holding other values than 0 and 1 (nilearn's mask values check);
masks of other grids with such values; an empty mask beside a 3D echo; every voxel removed by the
adaptive mask (no fit voxel) with each fit type and mode::

    python -P mask_probe.py <work dir> [--only <case>,...]   (boxed)
"""

from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def _phantom(shape=(6, 7, 3), vols=4, tes=(12.0, 28.0, 44.0)):
    import numpy as np

    i, j, k = np.indices(shape)
    s0 = 800.0 + 10 * i + 3 * j + k
    t2 = 30.0 + i + j
    out = []
    for e, te in enumerate(tes):
        decay = np.ones(shape)
        for _ in range(round(te)):
            decay = decay * (1.0 - 1.0 / t2)
        series = (s0 * decay)[..., None] * (1.0 + 0.01 * ((np.arange(vols) + e) % 3 - 1))
        out.append(series.astype(np.float32))
    return out


def make_inputs(d: Path) -> None:
    import nibabel as nib
    import numpy as np

    d.mkdir(parents=True, exist_ok=True)
    aff = np.diag([-3.0, 3.0, 3.0, 1.0])
    echoes = _phantom()
    for e, x in enumerate(echoes, 1):
        nib.save(nib.Nifti1Image(x, aff), d / f"e{e}.nii.gz")
        nib.save(nib.Nifti1Image(x[..., 0], aff), d / f"e{e}-3d.nii.gz")  # a 3D echo
        nib.save(nib.Nifti1Image(x[:-1], aff), d / f"e{e}-grid.nii.gz")  # another grid, 4D
        nib.save(nib.Nifti1Image(x[:-1, ..., 0], aff), d / f"e{e}-grid3d.nii.gz")  # both
    shape = echoes[0].shape[:3]
    i, _, _ = np.indices(shape)
    masks = {
        "ones": np.ones(shape),
        "vals012": (i % 3).astype(float),  # 0, 1, 2
        "vals12": 1.0 + (i % 2),  # 1, 2: no background
        "half": np.where(i % 2 == 0, 0.5, 0.0),  # 0, 0.5
        "neg": np.where(i % 2 == 0, -1.0, 1.0),  # -1, 1
        "empty": np.zeros(shape),
    }
    for name, m in masks.items():
        nib.save(nib.Nifti1Image(m.astype(np.float32), aff), d / f"mask-{name}.nii.gz")
        nib.save(nib.Nifti1Image(m[..., None].astype(np.float32), aff), d / f"mask4d-{name}.nii.gz")
        nib.save(nib.Nifti1Image(m[:-1].astype(np.float32), aff), d / f"maskgrid-{name}.nii.gz")
    # every voxel without good signal: a negative first echo (§6.1's base count 0)
    dead = [x.copy() for x in echoes]
    dead[0] = -dead[0]
    for e, x in enumerate(dead, 1):
        nib.save(nib.Nifti1Image(x, aff), d / f"dead-e{e}.nii.gz")


E = ["-d", "<I>/e1.nii.gz", "<I>/e2.nii.gz", "<I>/e3.nii.gz"]
TES = ["-e", "12", "28", "44"]
DEAD = ["-d", "<I>/dead-e1.nii.gz", "<I>/dead-e2.nii.gz", "<I>/dead-e3.nii.gz"]


def _echoes(first: str, second: str = "e2", third: str = "e3") -> list[str]:
    return ["-d", f"<I>/{first}.nii.gz", f"<I>/{second}.nii.gz", f"<I>/{third}.nii.gz"]


CASES = [
    # the first reading: an echo both 3D and of another grid, with a mask of the echoes' grid
    ("first-3d-and-grid", [*_echoes("e1-grid3d"), *TES, "--mask", "<I>/mask-ones.nii.gz"]),
    ("second-3d-and-grid", [*_echoes("e1", "e2-grid3d"), *TES, "--mask", "<I>/mask-ones.nii.gz"]),
    # the second reading: a 3D first echo, a second echo of another grid
    ("3d-then-grid", [*_echoes("e1-3d", "e2-grid"), *TES, "--mask", "<I>/mask-ones.nii.gz"]),
    ("grid-then-3d", [*_echoes("e1-grid", "e2-3d"), *TES, "--mask", "<I>/mask-ones.nii.gz"]),
    ("3d-empty-mask", [*_echoes("e1-3d"), *TES, "--mask", "<I>/mask-empty.nii.gz"]),
    ("3d-mask-vals012", [*_echoes("e1-3d"), *TES, "--mask", "<I>/mask-vals012.nii.gz"]),
    # 4D masks of one volume (nilearn's second reading) with other values
    *[
        (f"mask4d-{m}", [*E, *TES, "--mask", f"<I>/mask4d-{m}.nii.gz"])
        for m in ("ones", "vals012", "vals12", "half", "neg", "empty")
    ],
    # masks of another grid with other values
    *[
        (f"maskgrid-{m}", [*E, *TES, "--mask", f"<I>/maskgrid-{m}.nii.gz"])
        for m in ("vals012", "vals12", "empty")
    ],
    # no fit voxel: every voxel's base count 0
    ("no-fit-loglin", [*DEAD, *TES, "--mask", "<I>/mask-ones.nii.gz"]),
    ("no-fit-none", [*DEAD, *TES, "--mask", "<I>/mask-ones.nii.gz", "--masktype", "none"]),
    ("no-fit-loglin-ts", [*DEAD, *TES, "--mask", "<I>/mask-ones.nii.gz", "--fitmode", "ts"]),
    ("no-fit-curvefit", [*DEAD, *TES, "--mask", "<I>/mask-ones.nii.gz", "--fittype", "curvefit"]),
    (
        "no-fit-curvefit-ts",
        [*DEAD, *TES, "--mask", "<I>/mask-ones.nii.gz", "--fittype", "curvefit", "--fitmode", "ts"],
    ),
    ("no-fit-te-count", [*DEAD, "-e", "12", "28", "--mask", "<I>/mask-ones.nii.gz"]),
    ("no-fit-indep", [*DEAD, *TES, "--mask", "<I>/mask-ones.nii.gz", "--n-independent-echos", "3"]),
]


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("work", type=Path)
    p.add_argument("--only", default="")
    p.add_argument("--runs", default="tedana,reproduce,default,replica")
    a = p.parse_args(argv)
    sys.path.insert(0, str(HERE))
    import replay
    from larmorx_validation.parity import tedana_t2smap_original as so

    inputs = a.work / "inputs"
    if not (inputs / "dead-e3.nii.gz").exists():
        make_inputs(inputs)
    blackbox, _, _, _ = so.tools()
    env = blackbox.environment()
    only = set(a.only.split(",")) - {""}
    for name, words in CASES:
        if only and name not in only:
            continue
        args = [w.replace("<I>", str(inputs)) for w in words] + ["--out-dir", "out"]
        work = a.work / name
        shutil.rmtree(work, ignore_errors=True)
        print(f"########## {name}: {' '.join(words)}")
        runs = {
            "tedana": lambda args=args, work=work: so.run_tedana(
                args, work / "tedana", env, "a probe of relay 3c"
            ),
            "reproduce": lambda args=args, work=work: so.run_original(
                args, work / "reproduce", {**env, "LARMORX_UPSTREAM_DEFECTS": "reproduce"}
            ),
            "default": lambda args=args, work=work: so.run_original(
                args, work / "default", {**env, "LARMORX_UPSTREAM_DEFECTS": "fix"}
            ),
            "replica": lambda args=args, work=work: so.run_replica(args, work / "replica", env),
        }
        for run in a.runs.split(","):
            if run == "replica" and so.replica_program() is None:
                continue
            code, _, err = runs[run]()
            text = replay._contract(err).replace(str(inputs), "<I>").replace(str(work), "<W>")
            print(f"===== {run}: exit {code}")
            print(text, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
