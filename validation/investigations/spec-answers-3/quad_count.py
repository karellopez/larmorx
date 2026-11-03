# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""specs/topup.md §4.2 (spec writer, 2026-11-03): the number of quadratic B-spline coefficients
along an axis of `n` voxels at knot spacing `k > 1`, from the spec's text: start from
`floor(n / k) + 1`; when `(floor(n / k)) · k` is below half the basis function's sampled width
(`floor(K / 2)`, `K = 3k − 1` for even `k`, `3k` for odd `k`) the count stays there (FSL-145:
2 coefficients when `k ≤ n < 2k`, **1 when `n < k`**); otherwise it grows while `(nc − 1) k −
floor(K / 2) < n − 1`, which gives `ceil((2n + k − 4) / (2k)) + 2`.

Checked on FSL's `_fieldcoef` of random testing's reproducer 10753 (24 x 16 x 14 voxels of
2.5 x 2.5 x 4 mm, quadratic splines) run with `--warpres=50` and `100` (knot spacings 20, 20, 13
and 40, 40, 25 at full resolution), at `--subsamp=2` and `1`, through the oracle cache, and on a
table of sizes against the closed form. An axis with one coefficient cannot be refitted (the
refit's two wrap-around rows need two): with `--subsamp=2` FSL stops while preparing.

A knot spacing of 2 voxels (spec writer, 2026-11-03, sub-task 4): the count is not grown only
when `floor(n / k) k < floor(K / 2)`; at `k = 2` (`floor(K / 2) = 2`) that holds for `n = 1`
alone, so axes of 2 and 3 voxels get the general count, 3. Checked on three cut pairs of the
topup replica suite's half-resolution pair (32 x 32 x 3 and 32 x 32 x 2 voxels of 6 x 6 x 6.6
mm at `--warpres=13.2`: spacings 2, 2, 2; 32 x 32 x 3 at `--warpres=19.8`: spacings 3, 3, 3),
whose `_fieldcoef` FSL writes at full resolution.

Usage: quad_count.py <out dir>
"""

import json
import math
import shutil
import sys
from pathlib import Path

import nibabel as nib


def count(n: int, k: int) -> int:
    if k == 1:
        return n
    half = (3 * k - 1) // 2 if k % 2 == 0 else (3 * k) // 2
    nc = n // k + 1
    if (nc - 1) * k < half:
        return nc  # FSL-145: the count is not grown
    while (nc - 1) * k - half < n - 1:
        nc += 1
    return nc


def closed(n: int, k: int) -> int:
    return math.ceil((2 * n + k - 4) / (2 * k)) + 2


def main():
    from larmorx_validation.difftest.tools import fsl_topup_original as D
    from larmorx_validation.parity import fsl_topup_replica as rep
    from larmorx_validation.parity.fsl_mcflirt_replica import workspace

    bad = [(n, k) for k in range(2, 30) for n in range(2 * k, 300) if count(n, k) != closed(n, k)]
    print(f"closed form against the rule for n >= 2k (k 2..29, n < 300): {len(bad)} differ")
    out = Path(sys.argv[1])
    trial = workspace() / "oracles/difftest/2026-11-03-topup-original"
    j = json.loads(
        (trial / "findings/fsl-topup-original/topup-trial-10753-original.json").read_text()
    )
    r, _ = D.TOOL.case(j["recipe"], "original").payload[1]
    for wr, ss in (("50", "2"), ("100", "2"), ("50", "1"), ("100", "1")):
        d = out / f"warpres-{wr}-subsamp-{ss}"
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
        drop = ("--warpres", "--miter", "--subsamp")
        args = [a for a in rep._prepare(r, d, None) if not a.startswith(drop)]
        args += [f"--warpres={wr}", "--miter=1", f"--subsamp={ss}"]
        p = rep.run_oracle(args, cwd=d, settings=dict(r.settings))
        if p.code != 0:
            print(
                f"--warpres={wr} --subsamp={ss}: exit {p.code}: {' '.join(p.stderr.split())[-160:]}"
            )
            continue
        c = nib.load(str(next(d.glob("my_topup_fieldcoef*"))))
        n = [round(float(v)) for v in c.get_qform()[:3, 3]]
        k = [round(float(v)) for v in c.header.get_zooms()[:3]]
        want = [count(a, b) for a, b in zip(n, k)]
        print(
            f"--warpres={wr} --subsamp={ss}: exit {p.code}; axes {n} at knot spacings {k}: FSL lays "
            f"{list(c.shape)}, the rule {want}"
        )


def k2_probes(out: Path) -> None:
    """The spacing-2 probes (above): FSL's `_fieldcoef` sizes against the rule."""
    import numpy as np

    from larmorx_validation.parity import fsl_topup_replica as rep
    from larmorx_validation.parity.fsl_mcflirt_replica import workspace

    src = nib.load(str(rep.inputs() / "half.nii.gz"))
    half = np.asarray(src.dataobj, np.float32)
    base = workspace() / "oracles/inputs/spec-answers-3/topup-k2"
    base.mkdir(parents=True, exist_ok=True)
    for nz in (3, 2):
        p = base / f"cut{nz}.nii.gz"
        if not p.exists():
            rep._save(p, half[:, :, 8 : 8 + nz], src.affine)
    (base / "acq.txt").write_text(rep.TEXTS["acq.txt"], encoding="utf-8")
    for nz, wr in ((3, "13.2"), (2, "13.2"), (3, "19.8")):
        d = out / f"k2-cut{nz}-warpres-{wr}"
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
        shutil.copy(base / f"cut{nz}.nii.gz", d)
        shutil.copy(base / "acq.txt", d)
        args = [
            f"--imain=cut{nz}.nii.gz",
            "--datain=acq.txt",
            "--out=my_topup",
            "--splineorder=2",
            f"--warpres={wr}",
            "--subsamp=1",
            "--miter=1",
            "--fwhm=0",
            "--estmov=0",
        ]
        p = rep.run_oracle(args, cwd=d, settings={})
        if p.code != 0:
            print(f"cut {nz} --warpres={wr}: exit {p.code}: {' '.join(p.stderr.split())[-160:]}")
            continue
        c = nib.load(str(next(d.glob("my_topup_fieldcoef*"))))
        n = [round(float(v)) for v in c.get_qform()[:3, 3]]
        k = [round(float(v)) for v in c.header.get_zooms()[:3]]
        want = [count(a, b) for a, b in zip(n, k)]
        print(
            f"cut {nz} --warpres={wr}: axes {n} at knot spacings {k}: FSL lays "
            f"{list(c.shape)}, the rule {want}, the general count "
            f"{[closed(a, b) if b > 1 else a for a, b in zip(n, k)]}"
        )


main()
k2_probes(Path(sys.argv[1]))
