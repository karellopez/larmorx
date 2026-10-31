# SPDX-License-Identifier: Apache-2.0
"""betsurf, FSL 6.0.7.23 against the clean-room original, on a batch of calls (black box).

``python -P betsurf_cases.py <phantoms> <dir> [names...]`` runs :mod:`compare` on each named
case (all by default) in ``<dir>/<name>``: the parity suite's phantoms (``<phantoms>``, the
directory ``fsl_bet._phantoms()`` writes) and the real images of the suite with the inputs FSL
derived from them (``oracles/inputs/fsl-bet/<digest>/``: bet2's mesh, FLIRT's matrix to MNI152
and the T2 registered to the T1). One line per file: images by values (masks by Dice and the
boundary distance), meshes by the per-vertex distance. ``LARMORX_BIN`` names the build.
"""

from __future__ import annotations

import hashlib
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import compare

CACHE = Path(os.environ.get("LARMORX_TESTDATA_CACHE", Path.home() / ".cache/larmorx-testdata/data"))
#: The workspace: ``LARMORX_WORKSPACE``, else the first parent holding ``oracles/`` (a worktree
#: lies inside the main checkout).
WORKSPACE = Path(
    os.environ.get("LARMORX_WORKSPACE")
    or next(
        (p for p in Path(__file__).resolve().parents if (p / "oracles" / "inputs").is_dir()),
        Path.cwd(),
    )
)
DERIVED = WORKSPACE / "oracles" / "inputs" / "fsl-bet"

REAL = {
    "adult": "openneuro/ds000005/sub-01/anat/sub-01_T1w.nii.gz",
    "older-82y": "openneuro/ds003592/sub-280/ses-1/anat/sub-280_ses-1_T1w.nii.gz",
    "child-7y": "openneuro/ds000256/sub-CTS203/anat/sub-CTS203_T1w.nii.gz",
    "tumour": "openneuro/ds005003/sub-RS001/ses-1/anat/sub-RS001_ses-1_T1w.nii.gz",
    "philips": "openneuro/ds002603/sub-01/anat/sub-01_acq-SaggitalNsig1Mm1_T1w.nii.gz",
}


def derived(name: str) -> tuple[Path, Path, Path | None]:
    t1 = CACHE / REAL[name]
    d = DERIVED / hashlib.sha256(t1.read_bytes()).hexdigest()[:20]
    t2 = d / "t2_to_t1.nii.gz"
    return d / "t1_mesh.vtk", d / "t1_to_std.mat", t2 if t2.is_file() else None


def cases(ph: Path) -> dict[str, list[str]]:
    h, t2 = str(ph / "head.nii.gz"), str(ph / "head-t2.nii.gz")
    mesh, mat = str(ph / "mesh" / "head_mesh.vtk"), str(ph / "head_to_std.mat")
    out = {
        "ph-t1only": ["-v", "-1", "-m", "-o", "-s", h, mesh, mat, "out"],
        "ph-t1t2": ["-v", "-m", "-o", "-s", h, t2, mesh, mat, "out"],
        "ph-t1only-p1": ["-v", "-1", "-m", "-o", "-s", "-p", "1", h, mesh, mat, "out"],
        "ph-t1t2-p1": ["-v", "-m", "-o", "-s", "-p", "1", h, t2, mesh, mat, "out"],
    }
    for name in REAL:
        t1 = CACHE / REAL[name]
        if not t1.is_file():
            continue
        m, x, t2r = derived(name)
        if not m.is_file():
            continue
        out[f"{name}-t1only"] = ["-v", "-1", "-m", "-o", "-s", str(t1), str(m), str(x), "out"]
        if t2r is not None:
            out[f"{name}-t1t2"] = ["-v", "-m", "-o", "-s", str(t1), str(t2r), str(m), str(x), "out"]
    return out


def main(argv: list[str]) -> int:
    ph, d, *names = argv
    all_cases = cases(Path(ph))
    for name in names or list(all_cases):
        r = compare.compare(Path(d) / name, "betsurf", all_cases[name])
        print(f"=== {name}: exit {r.pop('exit')}, stdout same {r.pop('stdout same')}, "
              f"stderr same {r.pop('stderr same')}")
        for k in ("stdout", "stderr"):
            if k in r:
                print(f"  {k}: {r.pop(k)!r}")
        files = r.pop("files")
        if isinstance(files, tuple):
            print(f"  files differ: {files}")
        for k, v in r.items():
            if "values differ" in str(v) and str(v).split("; ")[1].startswith("0 of"):
                continue
            if "p95 0.0000 max 0.0000" in str(v):
                continue
            print(f"  {k}: {v}")
        sys.stdout.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
