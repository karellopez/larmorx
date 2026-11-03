# SPDX-License-Identifier: Apache-2.0
"""Black-box runs of FSL 6.0.7.23's ``fast`` behind FAST's spec answers (relay 2, 2026-11-03),
each through the oracle cache (``use`` mode, the ``fsl-fast-replica`` suite's runner), and the
model of each answer written from ``specs/fast.md``'s new text alone, which predicts FSL's
result before it is compared:

- ``nan-noneg``: NaN voxels in an image without negative values: ``_restore`` keeps NaN there
  (§4.1's negative-value rule is not applied, so nothing replaces the NaN; the field is 1);
- ``nan-neg``: the same NaN voxels with one negative value (the replica suite's ``t1_nan``):
  the rule sets every value outside ``[0, max]``, NaN included, to 0: ``_restore`` 0 there;
- ``nan-background``: every background voxel NaN, so the first voxel is NaN: the extrema are
  NaN, the mask is empty, every initial mean 0 and the k-means exception (exit 255);
- ``nan-first``: only the first voxel (in FSL's radiological frame) NaN: the same;
- ``n0-verbose``: ``-v -n 0``: FAST aborts in the partial volumes after writing `` Starting
  Partial Volume Estimation `` without flushing it: standard output (a pipe) ends at the last
  flushed line (§7);
- ``nan-stats`` (``--finding`` random testing's case 1840 of 2026-11-03: two channels, four
  classes, ``-v``): a class left without weight in the initialisation makes every class's
  statistics NaN (§5.3, §5.6): its NaN likelihood zeroes every posterior, so every class then
  prints ``MEAN 1 STDDEV -nan`` (means 0, covariances NaN); with the bias field the weights
  ``0 / NaN`` make the field and the corrected images NaN, and every later ``MEAN`` is ``-nan``;
  with ``-N`` the means stay 0 (``MEAN 1``) to the end.

    PYTHONPATH=python:validation/src boxed.sh 8 python -P \\
        validation/investigations/spec-answers-3/fast_probes.py <out-dir> [probe ...] \\
        [--finding <cleanroom-fast-01840-original.json>]

The derived inputs are written to ``<workspace>/oracles/inputs/spec-answers-3/fast/`` from the
replica suite's ``t1_half`` (``oracles/inputs/fsl-fast-replica/``).
"""

from __future__ import annotations

import json
import re
import shutil
import sys
from pathlib import Path

import numpy as np
from larmorx_validation.parity import fsl_fast_replica as rep


def _inputs() -> Path:
    from larmorx_validation.parity.fsl_mcflirt_replica import workspace

    ws = workspace()
    assert ws is not None
    d = ws / "oracles" / "inputs" / "spec-answers-3" / "fast"
    d.mkdir(parents=True, exist_ok=True)
    return d


def _nan_voxels(t: np.ndarray) -> tuple:
    """The five brain voxels the replica suite's ``t1_nan`` sets to NaN."""
    brain = np.argwhere(t > 0)
    return tuple(brain[:: max(1, len(brain) // 5)][:5].T)


def make_inputs() -> dict[str, Path]:
    import nibabel as nib

    src = rep.inputs() / "t1_half.nii.gz"
    img = nib.load(str(src))
    t = np.asarray(img.dataobj, dtype=np.float32)
    d = _inputs()
    out: dict[str, Path] = {}

    def save(name: str, a: np.ndarray) -> None:
        p = d / f"{name}.nii.gz"
        if not p.exists():
            nib.save(nib.Nifti1Image(a.astype(np.float32), img.affine, img.header), str(p))
        out[name] = p

    noneg = np.maximum(t, 0)
    a = noneg.copy()
    a[_nan_voxels(t)] = np.nan
    save("nan_noneg", a)
    a = noneg.copy()
    a[a <= 0] = np.nan
    save("nan_background", a)
    # FSL's first voxel: stored (0, 0, 0), or (nx - 1, 0, 0) when FSL mirrors x on reading (a
    # positive-determinant sform, else qform)
    a = noneg.copy()
    first = (a.shape[0] - 1, 0, 0) if np.linalg.det(img.affine[:3, :3]) > 0 else (0, 0, 0)
    a[first] = np.nan
    save("nan_first", a)
    return out


# ------------------------------------------------------------------------------------------------
# The models: what the new spec text predicts


def model_extrema(v: np.ndarray) -> tuple[float, float]:
    """§4.1: from the first voxel (radiological storage order), each strictly larger value a new
    maximum, else each strictly smaller one a new minimum: a NaN first voxel makes both NaN."""
    flat = v.ravel(order="F")
    mx = mn = flat[0]
    for x in flat[1:]:  # numpy comparisons with NaN are False, as the rule says
        if x > mx:
            mx = x
        elif x < mn:
            mn = x
    return float(mx), float(mn)


def model_restore(v: np.ndarray) -> np.ndarray:
    """§4.1 and §3.3: ``_restore`` outside the mask is the input after the negative-value rule
    times ``exp(-b)`` with ``b`` 0 there (§4.5): NaN stays NaN unless the rule replaced it."""
    mx, mn = model_extrema(v)
    i = v.astype(np.float32).copy()
    if mn < 0:
        q = np.sort(i.ravel())  # NaN values last (§8.3)
        p02 = q[min(int(np.float32(q.size) * np.float32(0.02)), q.size - 1)]
        if p02 < 0:
            i = i - np.float32(mn)
        else:
            i[~((i >= 0) & (i <= np.float32(mx)))] = 0  # NaN fails both comparisons
    return i


def model_mask_empty(v: np.ndarray) -> bool:
    mx, _ = model_extrema(v)
    hi = np.float32(mx) + np.float32(1)
    return not bool(np.any((v > 0) & (v < hi)))


# ------------------------------------------------------------------------------------------------


def run(name: str, args: list[str], files: list[Path], out: Path) -> dict:
    d = out / name
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    for f in files:
        shutil.copy(f, d / f.name)
    p = rep.run_oracle(args, cwd=d, settings={}, never="")
    return {"exit": p.code, "stdout": p.stdout, "stderr": p.stderr, "dir": d}


def main() -> None:
    import nibabel as nib

    argv = sys.argv[1:]
    finding = None
    if "--finding" in argv:
        i = argv.index("--finding")
        finding = Path(argv[i + 1])
        del argv[i : i + 2]
    out = Path(argv[0])
    want = set(argv[1:])
    ins = make_inputs()
    half = rep.inputs() / "t1_half.nii.gz"
    t1nan = rep.inputs() / "t1_nan.nii.gz"
    res: dict = {}

    def load(p: Path) -> np.ndarray:
        return np.asarray(nib.load(str(p)).dataobj)

    for name, src in (("nan-noneg", ins["nan_noneg"]), ("nan-neg", t1nan)):
        if want and name not in want:
            continue
        r = run(name, ["-B", "-b", "-o", "seg", src.name], [src], out)
        v = load(src)
        nanv = np.isnan(v)
        fsl_r, fsl_b = load(r["dir"] / "seg_restore.nii.gz"), load(r["dir"] / "seg_bias.nii.gz")
        pred = model_restore(v)
        res[name] = {
            "exit": r["exit"],
            "nan_voxels": int(nanv.sum()),
            "fsl_restore_at_nan": [float(x) for x in fsl_r[nanv]],
            "fsl_bias_at_nan": [float(x) for x in fsl_b[nanv]],
            "model_restore_at_nan": [float(x) for x in pred[nanv]],
            "model_agrees": bool(
                np.array_equal(np.isnan(fsl_r[nanv]), np.isnan(pred[nanv]))
                and np.array_equal(np.nan_to_num(fsl_r[nanv]), np.nan_to_num(pred[nanv]))
            ),
        }
    for name, key in (("nan-background", "nan_background"), ("nan-first", "nan_first")):
        if want and name not in want:
            continue
        src = ins[key]
        r = run(name, ["-v", "-o", "seg", src.name], [src], out)
        v = load(src)
        # FSL's radiological frame: mirrored along x when the matrix's determinant is positive
        img = nib.load(str(src))
        rad = v[::-1] if np.linalg.det(img.affine[:3, :3]) > 0 else v
        res[name] = {
            "exit": r["exit"],
            "stdout": r["stdout"].splitlines(),
            "stderr_tail": r["stderr"].splitlines()[-3:],
            "model_extrema": model_extrema(rad),
            "model_mask_empty": model_mask_empty(rad),
            "model_predicts": "exit 255, every initial mean 0, the k-means exception",
        }
    if not want or "n0-verbose" in want:
        r = run("n0-verbose", ["-v", "-n", "0", "-o", "seg", half.name], [half], out)
        lines = r["stdout"].splitlines()
        res["n0-verbose"] = {
            "exit": r["exit"],
            "stdout_lines": len(lines),
            "stdout_tail": lines[-4:],
            "has_pve_line": any("Starting Partial Volume Estimation" in ln for ln in lines),
            "stderr_tail": r["stderr"].splitlines()[-2:],
            "model_predicts": "the last line a flushed one; no ' Starting Partial Volume Estimation '",
        }
    if finding is not None and (not want or "nan-stats" in want):
        from larmorx_validation.difftest import tools

        doc = json.loads(finding.read_text(encoding="utf-8"))
        case = tools.get(doc["tool"]).case(doc.get("shrunk") or doc["recipe"], "original")
        runspec = case.payload[1]
        for name, extra in (("nan-stats", []), ("nan-stats-N", ["-N"])):
            d = out / name
            shutil.rmtree(d, ignore_errors=True)
            d.mkdir(parents=True)
            args = [*extra, *rep._prepare(runspec, d)]
            p = rep.run_oracle(args, cwd=d, settings=dict(runspec.settings), never=runspec.never)
            lines = p.stdout.splitlines()
            tanaka = [i for i, ln in enumerate(lines) if ln.startswith("Tanaka Iteration")]
            first_class = [ln for ln in lines[tanaka[0] + 1 :] if ln.startswith(" CHANNEL")][:1]
            later = [ln for ln in lines[tanaka[1] + 1 :] if ln.startswith(" CHANNEL")]
            means = lambda ln: re.findall(r"MEAN (\S+)", ln)  # noqa: E731
            res[name] = {
                "args": " ".join(args),
                "exit": p.code,
                "first_iteration_means": means(first_class[0]) if first_class else None,
                "later_means": sorted({m for ln in later for m in means(ln)}),
                "model_predicts": (
                    "first iteration every MEAN 1; later every MEAN -nan"
                    if not extra
                    else "every MEAN 1 to the end"
                ),
            }
    for k, v in res.items():
        print(f"== {k}: {json.dumps(v, default=str)[:900]}")
    (out / "probes.json").write_text(json.dumps(res, indent=1, default=str), encoding="utf-8")


if __name__ == "__main__":
    main()
