# SPDX-License-Identifier: Apache-2.0
"""Which inputs reach AFNI-89 (``specs/3dvolreg.md`` §7.3): AFNI's 3dvolreg as a black box.

Usage (inside a memory box):

    scripts/orchestration/boxed.sh 4 python -P validation/investigations/3dvolreg-spec/afni89.py <work folder>

Each case writes a small float32 series of plain arithmetic (3 x 3 x 3.5 mm voxels, 4 volumes),
runs AFNI 26.3.05's 3dvolreg through the oracle cache and prints its exit status, the least
squares' error lines (``lsqfit :: <k> vectors are all 0``, the Cholesky failure, the crash) or,
when the run ends, the largest parameter it reports. Written from the spec and black-box runs.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import afni_run as A  # noqa: E402


def save(a: np.ndarray, path: Path) -> Path:
    aff = np.diag([-3.0, -3.0, 3.5, 1.0])
    img = nib.Nifti1Image(a.astype(np.float32), aff)
    img.set_qform(aff, 1)
    img.set_sform(aff, 1)
    img.to_filename(path)
    return path


def series(shape, kind: str, value: float = 100.0, nt: int = 4) -> np.ndarray:
    nx, ny, nz = shape
    _, j, k = np.meshgrid(np.arange(nx), np.arange(ny), np.arange(nz), indexing="ij")
    if kind == "const":
        v = np.full(shape, value)
    elif kind == "halfz":  # the lower half of the slices constant, the upper half 0
        v = np.where(k < nz // 2, value, 0.0)
    elif kind == "const-along-x":  # constant along x only
        v = value + 5.0 * ((j * 7 + k * 3) % 11)
    else:
        raise ValueError(kind)
    return np.repeat(v[..., None], nt, axis=3)


def cases(work: Path):
    """(name, series, options)."""
    out = []
    for shape in [
        (20, 22, 12),
        (29, 29, 29),
        (30, 29, 29),
        (29, 29, 30),
        (30, 30, 30),
        (40, 44, 24),
    ]:
        out.append((f"const100-{'x'.join(map(str, shape))}", series(shape, "const"), ["-twopass"]))
    for value in (1.0, 37.3, 1000.0, 123.456, 7.0e5):
        out.append((f"const{value:g}-40x44x24", series((40, 44, 24), "const", value), ["-twopass"]))
    g = (40, 44, 24)
    out += [
        ("const100-40x44x24-no-twopass", series(g, "const"), []),
        ("const100-40x44x24-cubic", series(g, "const"), ["-twopass", "-cubic"]),
        ("const100-40x44x24-edging-1", series(g, "const"), ["-twopass", "-edging", "1"]),
        ("const100-40x44x24-twoblur-3", series(g, "const"), ["-twopass", "-twoblur", "3"]),
        ("const100-40x44x24-delta-0.3", series(g, "const"), ["-twopass", "-delta", "0.3"]),
        ("halfz100-40x44x24", series(g, "halfz"), ["-twopass"]),
        ("const-along-x-40x44x24", series(g, "const-along-x"), ["-twopass"]),
    ]
    zw = save(np.zeros((*g, 1)), work / "zero_weight.nii.gz")
    out.append(("zero-weight", series(g, "const-along-x"), ["-weight", f"{zw}[0]"]))
    out.append(
        ("zero-weight-twopass", series(g, "const-along-x"), ["-twopass", "-weight", f"{zw}[0]"])
    )
    return out


PATTERN = r"lsqfit|Choleski|Fatal Signal|weights are all|skipping"


def main() -> int:
    work = Path(sys.argv[1]).resolve()
    work.mkdir(parents=True, exist_ok=True)
    for name, arr, opts in cases(work):
        p = save(arr, work / f"{name}.nii.gz")
        r = A.run([*opts, "-prefix", "o.nii", "-1Dfile", "p.1D", str(p)], work / name)
        msgs = [
            re.sub(r"\x1b\[[0-9;]*m", "", ln)
            for ln in r.stderr.splitlines()
            if re.search(PATTERN, ln)
        ]
        extra = ""
        if r.status == 0 and (work / name / "p.1D").exists():
            extra = f", largest |parameter| {np.abs(np.array(A.params(work / name / 'p.1D'))).max():.3g}"
        print(
            f"{name:32s} {' '.join(o for o in opts if not o.endswith('[0]')):24s} exit {r.status}{extra}"
        )
        for m in msgs[:4]:
            print("      ", m.replace("\a", "<BEL>"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
