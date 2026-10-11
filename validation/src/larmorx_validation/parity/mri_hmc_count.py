# SPDX-License-Identifier: Apache-2.0
"""FSL-02, measured before it is decided: mcflirt's correlation count against the corrected one.

mcflirt's weighted normalised correlation divides by a count ``N`` that is a running sum of
running sums of the weights, many orders of magnitude larger than the number of points, which
makes the correlation effectively uncentred (``specs/mcflirt.md`` §6.4; register entry
FSL-02). larmorx can use the sum of the weights instead (``lx.mri.hmc(...,
normcorr_count="weights")``), the weighted Pearson correlation. Whether that becomes the
default is decided with the user (D13 item 4); this module measures it:

- against mcflirt, on every run of mcflirt's variability band (``oracles/fsl-6.0.7/mcflirt/
  band/band.json``): the matrix RMS deviation (radius 80 mm) and the parameter differences of
  larmorx's matrices from mcflirt's, for larmorx reproducing mcflirt, larmorx's default (the
  other defects corrected, mcflirt's count) and the corrected count, next to mcflirt's own
  deviation under small perturbations of its input (the band);
- against the synthetic ground truth (``larmorx-testdata`` ``synthetic/hmc``), where the runs
  register to the phantom at rest or to their own middle volume: the mean RMS deviation from
  the true motion.

``python -m larmorx_validation.parity.mri_hmc_count --out docs/validation`` writes
``mri-hmc-normcorr-count.md`` and its JSON.
"""

from __future__ import annotations

import argparse
import json
import time
import warnings
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.parity.mri_hmc import (
    _fov_centre,
    _read_mats,
    _resolve,
    _truth,
    decompose,
    framewise_displacement,
    oracle_dir,
    rms_deviation,
)
from larmorx_validation.parity.mri_hmc_band import band_for
from larmorx_validation.report import table

#: The configurations measured: name -> keyword arguments of ``lx.mri.hmc``.
CONFIGS = {
    "reproduce": {"upstream_defects": "reproduce"},
    "default": {},
    "weights": {"normcorr_count": "weights"},
}


def _kwargs(argv: list[str]) -> dict[str, Any] | None:
    """``lx.mri.hmc``'s arguments for a recorded mcflirt command line (estimation only), or
    ``None`` for options the measurement does not cover."""
    kw: dict[str, Any] = {"resample": False}
    i = 0
    while i < len(argv):
        a = argv[i]
        v = argv[i + 1] if i + 1 < len(argv) else ""
        if a == "-in":
            kw["bold"] = _resolve(v)
        elif a in ("-reffile", "-r"):
            kw["reference"] = _resolve(v)
        elif a == "-refvol":
            kw["ref_index"] = int(v)
        elif a in ("-dof", "-stages", "-bins"):
            kw[a[1:]] = int(v)
        elif a in ("-smooth", "-rotation"):
            kw[a[1:]] = float(v)
        elif a == "-cost":
            kw["cost"] = v
        elif a in ("-out", "-o", "-init", "-verbose", "-scaling"):
            pass
        elif a in ("-mats", "-plots", "-rmsrel", "-rmsabs", "-stats", "-report"):
            i += 1
            continue
        elif a == "-meanvol":
            kw["mean"] = True
            i += 1
            continue
        elif a == "-fudge":
            kw["fudge"] = True
            i += 1
            continue
        else:
            return None
        i += 2
    return kw


def _compare(ours: np.ndarray, theirs: list[np.ndarray], centre: np.ndarray) -> dict[str, float]:
    idx = [
        i for i in range(len(theirs)) if np.isfinite(theirs[i]).all() and np.isfinite(ours[i]).all()
    ]
    devs = np.array([rms_deviation(theirs[i], ours[i], centre) for i in idx])
    pa = np.array([decompose(theirs[i], centre) for i in idx])
    pb = np.array([decompose(ours[i], centre) for i in idx])
    fa, fb = framewise_displacement(pa), framewise_displacement(pb)
    r = float(np.corrcoef(fa, fb)[0, 1]) if fa.std() > 0 and fb.std() > 0 else 1.0
    return {
        "rms_median": float(np.median(devs)),
        "rms_p95": float(np.percentile(devs, 95)),
        "param_mm_median": float(np.median(np.abs(pa[:, 3:] - pb[:, 3:]))),
        "param_deg_median": float(np.median(np.degrees(np.abs(pa[:, :3] - pb[:, :3])))),
        "fd_r": r,
        "fd_max": float(fa.max()),
    }


def _truth_in_series(kw: dict[str, Any]) -> list | None:
    """The ground truth of a synthetic series registered to one of its own volumes (mcflirt's
    default, the middle one, or ``-refvol``): ``inv(E_r) E_v``, from the sidecar's matrices to
    the phantom at rest."""
    bold = str(kw["bold"])
    if "reference" in kw or kw.get("mean") or "synthetic/hmc/" not in bold:
        return None
    import larmorx_testdata as td

    rel = "synthetic/hmc/" + bold.split("synthetic/hmc/", 1)[1].replace(".nii.gz", ".json")
    try:
        sidecar = json.loads(Path(td.get(rel)).read_text(encoding="utf-8"))
    except Exception:
        return None
    e = [np.array(m, dtype=np.float64) for m in sidecar.get("expected_fsl_mat") or []]
    if not e:
        return None
    to_ref = np.linalg.inv(e[kw.get("ref_index", len(e) // 2)])
    return [(to_ref @ m).tolist() for m in e]


def measure() -> dict[str, Any]:
    import larmorx as lx

    root = oracle_dir()
    if root is None:
        raise SystemExit("the recorded mcflirt runs were not found (LARMORX_MCFLIRT_ORACLE)")
    band = json.loads((root / "band" / "band.json").read_text(encoding="utf-8"))
    cases = sorted({b["case"] for b in band["runs"]})
    out: dict[str, Any] = {"cases": {}}
    for case in cases:
        run = json.loads((root / "runs" / case / "run.json").read_text(encoding="utf-8"))
        argv = run["steps"][0]["argv"][1:]
        kw = _kwargs(argv)
        if kw is None:
            continue
        out_arg = argv[argv.index("-out") + 1]
        mats_dir = root / "runs" / case / (Path(out_arg).name + ".mat")
        theirs, _ = _read_mats(mats_dir)
        reference = Path(kw.get("reference", kw["bold"]))
        centre = _fov_centre(reference)
        truth, _ = _truth(run)
        if truth is None:
            truth = _truth_in_series(kw)
        row: dict[str, Any] = {"band": band_for(case) or {}}
        mats = {}
        for name, extra in CONFIGS.items():
            t0 = time.perf_counter()
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                r = lx.mri.hmc(**kw, **extra, n_threads=0)
            mats[name] = r.matrices
            row[name] = {
                **_compare(r.matrices, theirs, centre),
                "seconds": time.perf_counter() - t0,
            }
            if truth is not None:
                row[name]["truth"] = float(
                    np.mean(
                        [
                            rms_deviation(np.array(t), m, centre)
                            for t, m in zip(truth, r.matrices, strict=True)
                        ]
                    )
                )
        if truth is not None:
            row["mcflirt_truth"] = float(
                np.mean(
                    [
                        rms_deviation(np.array(t), m, centre)
                        for t, m in zip(truth, theirs, strict=True)
                    ]
                )
            )
        # The count's own effect on larmorx: weights against mcflirt's count, both corrected.
        row["weights_vs_default"] = _compare(mats["weights"], list(mats["default"]), centre)
        out["cases"][case] = row
        print(
            f"{case}: p95 vs mcflirt reproduce {row['reproduce']['rms_p95']:.4f}, default "
            f"{row['default']['rms_p95']:.4f}, weights {row['weights']['rms_p95']:.4f}; band "
            f"{row['band'].get('rms_p95', float('nan')):.4f} mm",
            flush=True,
        )
    return out


def report(data: dict[str, Any]) -> str:
    rows, within = [], {"reproduce": 0, "default": 0, "weights": 0}
    n = 0
    for case, row in data["cases"].items():
        b = row["band"]
        p95 = b.get("rms_p95", float("nan"))
        n += 1
        for name in within:
            within[name] += row[name]["rms_p95"] <= max(p95, 0.1)
        rows.append(
            (
                f"`{case}`",
                f"{p95:.4f}",
                f"{row['reproduce']['rms_p95']:.4f}",
                f"{row['default']['rms_p95']:.4f}",
                f"**{row['weights']['rms_p95']:.4f}**",
                f"{row['weights']['param_mm_median']:.4f} / {row['weights']['param_deg_median']:.4f}",
                f"{row['weights']['fd_r']:.4f} ({b.get('fd_r', float('nan')):.4f})",
                f"{row['weights_vs_default']['rms_p95']:.4f}",
            )
        )
    truth_rows = [
        (
            f"`{case}`",
            f"{row['mcflirt_truth']:.4f}",
            f"{row['reproduce']['truth']:.4f}",
            f"{row['default']['truth']:.4f}",
            f"**{row['weights']['truth']:.4f}**",
        )
        for case, row in data["cases"].items()
        if "mcflirt_truth" in row
    ]
    lines = [
        "# FSL-02: mcflirt's correlation count against the corrected count",
        "",
        "Measured before deciding (D13 item 4, [upstream-defects.md](../api/upstream-defects.md)). "
        "mcflirt's normalised correlation divides by a count that is a running sum of running "
        "sums of the edge weights (`specs/mcflirt.md` §6.4), which makes it effectively "
        'uncentred. `lx.mri.hmc(..., normcorr_count="weights")` divides by the sum of the '
        "weights instead: a weighted Pearson correlation. Three larmorx configurations are "
        "compared with mcflirt 6.0.7's matrices on every run of mcflirt's variability band: "
        "`reproduce` (mcflirt's defects, mcflirt's count), `default` (the other defects corrected, "
        "mcflirt's count) and `weights` (the corrected count).",
        "",
        f"Generated with `python -m larmorx_validation.parity.mri_hmc_count` "
        f"({time.strftime('%Y-%m-%d')}).",
        "",
        "## Against mcflirt and its band",
        "",
        "95th percentile of the matrix RMS deviation from mcflirt's matrices (mm, radius 80 mm, "
        "about the reference's field-of-view centre). *Band*: mcflirt against itself under "
        "small perturbations of its input (the worst of three noise realisations and the reverse "
        "order). The last column is the count's own effect on larmorx (`weights` against "
        "`default`).",
        "",
        table(
            (
                "Run",
                "mcflirt band 95th pct",
                "reproduce",
                "default",
                "weights",
                "weights: median Δparam (mm / °)",
                "weights: FD r (band)",
                "weights vs default",
            ),
            rows,
        ),
        "",
        f"Within the band (or the spec's 0.1 mm where the band is smaller): reproduce "
        f"{within['reproduce']} of {n}, default {within['default']} of {n}, weights "
        f"{within['weights']} of {n}.",
        "",
        "`default` differs from `reproduce` only where another correction applies: the last "
        "volume (FSL-01) and in-plane runs (FSL-03). Where it leaves the band (the synthetic "
        "`background` and `tiny` series) the difference is the last volume, and the ground truth "
        "below shows which is closer: on `background` mcflirt's last volume, started from the "
        "identity, converges to a 180° rotation.",
        "",
    ]
    if truth_rows:
        lines += [
            "## Against the synthetic ground truth",
            "",
            "Mean RMS deviation from the true motion (mm), on the synthetic series registered to "
            "the phantom at rest (`-reffile`) or to their own middle volume (mcflirt's default).",
            "",
            table(("Run", "mcflirt", "reproduce", "default", "weights"), truth_rows),
            "",
        ]
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out", type=Path, default=Path("docs/validation"))
    args = p.parse_args(argv)
    data = measure()
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "mri-hmc-normcorr-count.json").write_text(
        json.dumps(data, indent=1), encoding="utf-8"
    )
    (args.out / "mri-hmc-normcorr-count.md").write_text(report(data), encoding="utf-8")


if __name__ == "__main__":
    main()
