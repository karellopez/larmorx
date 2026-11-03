# SPDX-License-Identifier: Apache-2.0
"""The partial volumes' fraction search and its near ties (``specs/fast.md`` §4.8.8, §5.8, §8.7;
spec writer's answer of 2026-11-03), modelled from the spec text alone.

A mixed voxel's fraction is the grid value ``δ`` (101 values, steps of 0.01 accumulated in
single precision) of least energy, the first strictly smaller one winning; the energy is a smooth
function of ``δ`` with one minimum on realistic statistics, so two neighbouring grid values tie
when the minimum lies near their midpoint. The model draws mixed voxels of two classes (two
channels: realistic log-domain means and covariances of a T1w and a T2w, as several channels'
``-v`` lines print them; one channel: a T1w's grey and white matter) and finds each one's
fraction (a) with the spec's arithmetic, (b) with the class statistics changed in their last bits
(a relative change ``ε`` of every mean and covariance entry, as FSL against FSL with correctly
rounded maths: its statistics differ by about 1e-7 to 1e-5 after the iterations), and (c) for
several channels, with another evaluation of the same energy (the quadratic form through the
explicit 2 x 2 inverse instead of a solve: the matrix arithmetic's last bits). It counts the
voxels whose fraction moves and by how many steps.

    python -I validation/investigations/spec-answers-3/pv_ties_model.py
"""

from __future__ import annotations

import numpy as np

F = np.float32


def fractions() -> np.ndarray:
    """§8.7, several channels: δ_{k+1} = δ_k + 1/100, single precision, while δ <= 1."""
    out, d, s = [], F(0), F(1) / F(100)
    while d <= 1:
        out.append(d)
        d = F(d + s)
    return np.array(out, dtype=F)


def energy_multi(y, mu_a, mu_b, c_a, c_b, delta, explicit_inverse=False):
    """§5.8: E = dᵀ C⁻¹ d + logf(det C) at one fraction, every voxel at once (two channels)."""
    dl = np.float64(delta)
    om = np.float64(F(1) - delta)
    mu = dl * mu_a + om * mu_b
    c = dl * dl * c_a + om * om * c_b
    det = c[0, 0] * c[1, 1] - c[0, 1] * c[1, 0]
    d = y - mu
    if explicit_inverse:
        inv = np.array([[c[1, 1], -c[0, 1]], [-c[1, 0], c[0, 0]]]) / det
        q = np.einsum("ni,ij,nj->n", d, inv, d)
    else:
        z = np.linalg.solve(c, d.T).T
        q = np.einsum("ni,ni->n", d, z)
    return (q + np.log(np.float64(F(det))).astype(F)).astype(F)


def energy_single(x, mu_a, mu_b, s_a, s_b, delta):
    """§4.8.8: E = ((x - μ_δ)² / σ²_δ + logf σ²_δ) / 2, single precision."""
    dl, om = F(delta), F(F(1) - F(delta))
    mu = F(dl * F(mu_a)) + F(om * F(mu_b))
    s2 = F(F(dl * dl) * F(s_a)) + F(F(om * om) * F(s_b))
    t = (x - F(mu)).astype(F)
    return ((F(t * t) / F(s2) + np.log(F(s2)).astype(F)) / F(2)).astype(F)


def argmin(energies: np.ndarray) -> np.ndarray:
    """The first strictly smaller energy wins, starting from 1e13 (§4.8.8)."""
    best = np.full(energies.shape[1], F(1e13))
    idx = np.zeros(energies.shape[1], dtype=int)
    for k, e in enumerate(energies):
        better = best > e
        best = np.where(better, e, best)
        idx = np.where(better, k, idx)
    return idx


def report(label: str, ref: np.ndarray, new: np.ndarray) -> str:
    moved = ref != new
    steps = np.abs(ref - new)[moved]
    return (
        f"{label}: {int(moved.sum())} of {ref.size} voxels move "
        f"({moved.mean():.2e}); steps {sorted(set(steps.tolist()))}"
    )


def main() -> None:
    rng = np.random.default_rng(20261103)
    n = 200_000
    deltas = fractions()
    # two channels (log domain: a T1w and a T2w, grey and white matter as several channels'
    # -v lines print them: means, standard deviations; a correlation within each class)
    mu_a, mu_b = np.array([6.50, 6.10]), np.array([6.84, 5.85])
    sd_a, sd_b, rho = np.array([0.060, 0.070]), np.array([0.045, 0.050]), -0.3
    c_a = np.array(
        [[sd_a[0] ** 2, rho * sd_a[0] * sd_a[1]], [rho * sd_a[0] * sd_a[1], sd_a[1] ** 2]]
    )
    c_b = np.array(
        [[sd_b[0] ** 2, rho * sd_b[0] * sd_b[1]], [rho * sd_b[0] * sd_b[1], sd_b[1] ** 2]]
    )
    true = rng.random(n)
    mu_t = true[:, None] * mu_a + (1 - true[:, None]) * mu_b
    c_t = (true**2)[:, None, None] * c_a + ((1 - true) ** 2)[:, None, None] * c_b
    y = mu_t + np.einsum("nij,nj->ni", np.linalg.cholesky(c_t), rng.standard_normal((n, 2)))
    y = y.astype(F).astype(np.float64)  # the log images are single precision

    def multi(ma, mb, ca, cb, explicit=False):
        return argmin(np.array([energy_multi(y, ma, mb, ca, cb, d, explicit) for d in deltas]))

    ref = multi(mu_a, mu_b, c_a, c_b)
    print("several channels (two):")
    print(
        "  "
        + report(
            "the quadratic form by the explicit inverse", ref, multi(mu_a, mu_b, c_a, c_b, True)
        )
    )
    for eps in (1e-7, 1e-6, 1e-5):
        r = lambda a, eps=eps: a * (1 + eps * rng.choice([-1.0, 1.0], size=a.shape))  # noqa: E731
        print(
            "  "
            + report(f"statistics changed by {eps:g}", ref, multi(r(mu_a), r(mu_b), r(c_a), r(c_b)))
        )
    # one channel: grey and white matter of a T1w (log domain)
    ga, gb, sa, sb = 6.50, 6.84, 0.060**2, 0.045**2
    x = (
        true * ga
        + (1 - true) * gb
        + np.sqrt(true**2 * sa + (1 - true) ** 2 * sb) * rng.standard_normal(n)
    ).astype(F)
    one = [F(0)]
    while True:
        nxt = F(np.float64(one[-1]) + 0.01)
        if nxt > 1:
            break
        one.append(nxt)

    def single(a, b, s1, s2):
        return argmin(np.array([energy_single(x, a, b, s1, s2, d) for d in one]))

    ref1 = single(ga, gb, sa, sb)
    print("one channel:")
    for eps in (1e-7, 1e-6, 1e-5):
        k = lambda v, eps=eps: v * (1 + eps * rng.choice([-1.0, 1.0]))  # noqa: E731
        print(
            "  "
            + report(f"statistics changed by {eps:g}", ref1, single(k(ga), k(gb), k(sa), k(sb)))
        )


if __name__ == "__main__":
    main()
