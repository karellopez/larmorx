# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""specs/topup.md §4.6 (spec writer, 2026-11-03): the regulariser's matrix `R` (`E = cᵀ R c`,
its gradient `2 λ' R c`, the Levenberg-Marquardt Hessian's part `2 λ' R`), from the spec's text,
against FSL's Hessians of random testing's reproducer 10753 (quadratic B-splines at knot spacings
4, 4, 3 on a level of 12 x 8 x 7 voxels of 5 x 5 x 8 mm).

From the text: `R` is translation-invariant. For two coefficients whose basis functions are
`(Δx, Δy, Δz)` knots apart, `R = Σ_{d1 ≤ d2} w_{d1 d2} Π_a S_a(Δ_a; m_a)` with `w = 1` on the
three squared second derivatives and `2` on the three mixed ones, `m_a` the derivative order
along axis `a` of that term (0, 1 or 2), and `S_a(Δ; m) = Σ_{x ∈ ℤ} b^{(m)}(x) b^{(m)}(x − Δ k_a)
/ v_a^{2m}`: the 1-D basis function's `m`-th derivative per voxel, sampled at every integer
voxel offset `x` from its centre (over the whole support, not only the image), `v_a` the level's
voxel size (mm). For quadratic B-splines the second derivative at a knot (`|x| = k/2` or `3k/2`,
integers when `k` is even) is the mean of its one-sided values: `−1/(2k²)` and `+1/(2k²)`.

FSL's regulariser Hessian is measured, not assumed: the first Levenberg-Marquardt Hessian
(`--debug=3`) of the case at `--lambda=1e-4` (its own) and at `--lambda=1e-3`, at the zero
starting field (the data part is the same in both), so `H(1e-3) − H(1e-4) = 2 (Δλ · SSD) R` with
`SSD` the first cost's (`-v`).

Usage:
  bending_model.py run <out dir>      FSL's two runs (oracle cache, use mode)
  bending_model.py check <out dir> [--variant inner | outer | integral]
     inner / outer: the second derivative at a knot taken from the inner or the outer piece;
     integral: the exact integrals of the products instead of the sums at integer points.
"""

import json
import sys
from pathlib import Path

import numpy as np

TRIAL = Path("oracles/difftest/2026-11-03-topup-original")
FINDING = 10753
HESS = "TopupDebugHesslevel01_iter01_attempt00.txt"


def run(out: Path) -> None:
    import shutil

    from larmorx_validation.difftest.tools import fsl_topup_original as D
    from larmorx_validation.parity import fsl_topup_replica as rep
    from larmorx_validation.parity.fsl_mcflirt_replica import workspace

    trial = workspace() / TRIAL
    j = json.loads(
        (trial / f"findings/fsl-topup-original/topup-trial-{FINDING:05d}-original.json").read_text()
    )
    r, _ = D.TOOL.case(j["recipe"], "original").payload[1]
    for lam in ("1e-4", "1e-3"):
        d = out / f"lambda-{lam}"
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
        args = rep._prepare(r, d, None)
        keep = {"--debug", "-v", "--verbose", "--miter", "--lambda"}
        args = [a for a in args if a.split("=")[0] not in keep]
        args += ["--miter=1", "--debug=3", "-v", f"--lambda={lam}"]
        p = rep.run_oracle(args, cwd=d, settings=dict(r.settings))
        (d / "ARGS.txt").write_text(" ".join(args) + "\n")
        (d / "STDOUT.txt").write_text(p.stdout)
        print(d.name, "exit", p.code, "Hessian written:", (d / HESS).exists())


def b2(t, m, k, knot):
    """The quadratic B-spline (knot units t) or its derivatives per voxel."""
    a = abs(t)
    if m == 0:
        return 0.75 - a * a if a <= 0.5 else (0.5 * (1.5 - a) ** 2 if a < 1.5 else 0.0)
    if m == 1:
        s = np.sign(t)
        return s * (-2.0 * a) / k if a <= 0.5 else (s * (a - 1.5) / k if a < 1.5 else 0.0)
    if abs(a - 1.5) < 1e-6:
        return {"mean": 0.5, "inner": 1.0, "outer": 0.0, "sampled": 0.0}[knot] / k**2
    if abs(a - 0.5) < 1e-6:
        return {"mean": -0.5, "inner": -2.0, "outer": 1.0, "sampled": -0.5}[knot] / k**2
    return (-2.0 if a < 0.5 else (1.0 if a < 1.5 else 0.0)) / k**2


def s1d(delta, m, k, v, variant):
    if variant == "integral":
        x = np.linspace(-2.0 * k, 2.0 * k, 400 * k + 1)
        f = np.array([b2(t / k, m, k, "mean") for t in x])
        g = np.array([b2((t - delta * k) / k, m, k, "mean") for t in x])
        return np.trapezoid(f * g, x) / v ** (2 * m)
    knot = {"inner": "inner", "outer": "outer", "mean": "mean"}.get(variant, "sampled")
    tot = 0.0
    for x in range(-3 * k, 3 * k + 1):
        tot += b2(x / k, m, k, knot) * b2((x - delta * k) / k, m, k, knot)
    return tot / v ** (2 * m)


def read_hess(path: Path, n: int) -> np.ndarray:
    t = np.loadtxt(path)
    h = np.zeros((n, n))
    # sparse triplets (row, column, value), one-based, by columns; FSL ends the file with the
    # last diagonal entry once more (as 0 when it was written already): summed
    np.add.at(h, (t[:, 0].astype(int) - 1, t[:, 1].astype(int) - 1), t[:, 2])
    return h


def ssd(stdout: str) -> float:
    ln = next(ln for ln in stdout.splitlines() if ln.startswith("SSD = "))
    return float(ln.split("SSD = ")[1].split()[0])


def check(out: Path, variant: str) -> None:
    k = (4, 4, 3)
    v = (5.0, 5.0, 8.0)
    nc = (5, 4, 5)
    n = int(np.prod(nc))
    d1, d2 = out / "lambda-1e-4", out / "lambda-1e-3"
    h1 = read_hess(d1 / HESS, n)
    h2 = read_hess(d2 / HESS, n)
    s = ssd((d1 / "STDOUT.txt").read_text())
    s2 = ssd((d2 / "STDOUT.txt").read_text())
    fsl = (h2 - h1) / (2.0 * (1e-3 - 1e-4) * s)
    terms = []
    for a in range(3):
        for b in range(a, 3):
            m = [0, 0, 0]
            m[a] += 1
            m[b] += 1
            terms.append((1.0 if a == b else 2.0, m))
    cache = {}

    def S(ax, delta, m):
        key = (ax, delta, m)
        if key not in cache:
            cache[key] = s1d(delta, m, k[ax], v[ax], variant)
        return cache[key]

    idx = [(i, j, l) for l in range(nc[2]) for j in range(nc[1]) for i in range(nc[0])]
    model = np.zeros((n, n))
    for p, (pi, pj, pl) in enumerate(idx):
        for q, (qi, qj, ql) in enumerate(idx):
            dlt = (qi - pi, qj - pj, ql - pl)
            if max(abs(x) for x in dlt) > 2:
                continue
            model[p, q] = sum(
                w * S(0, dlt[0], m[0]) * S(1, dlt[1], m[1]) * S(2, dlt[2], m[2]) for w, m in terms
            )
    nz = np.abs(fsl) > 1e-12 * np.abs(fsl).max()
    rel = np.abs(model - fsl)[nz] / np.abs(fsl)[nz]
    scale = float((fsl * model).sum() / (model * model).sum())
    print(
        f"{variant}: SSD {s:.6g} (both runs {s2:.6g}); FSL's R from the two Hessians: "
        f"{int(nz.sum())} entries, max |R| {np.abs(fsl).max():.4g}; the model's relative "
        f"difference p50 {np.percentile(rel, 50):.3g}, max {rel.max():.3g}; "
        f"least-squares scale FSL/model {scale:.7f}; entries the model has and FSL not "
        f"{int(((np.abs(model) > 1e-12 * np.abs(fsl).max()) & ~nz).sum())}"
    )


def main():
    out = Path(sys.argv[2])
    if sys.argv[1] == "run":
        run(out)
    else:
        variant = sys.argv[sys.argv.index("--variant") + 1] if "--variant" in sys.argv else "spec"
        check(out, variant)


main()
