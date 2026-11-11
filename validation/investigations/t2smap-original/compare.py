# SPDX-License-Identifier: Apache-2.0
"""Black-box reports of ``t2smap`` programs on the spec's probe cases, for the clean-room original.

Written from ``specs/t2smap.md`` and runs of the programs only (the clean-room part of
2026-11-11): it reuses the spec's probe (``validation/investigations/t2smap-spec/blackbox.py``:
its inputs, cases, environment and report form) and adds cases for questions the spec leaves to
black-box runs, then writes one report per program::

    python -P compare.py <work dir> <name> <program word>...

``<name>`` names the report (``<work>/<name>.txt``) and the program's own work folder
(``<work>/<name>/``). tedana's program (the launcher's path ends in ``t2smap``) runs through the
oracle-output cache (``oracle_cache.run``, the family ``python``); every other program runs
directly. With ``--check <reference name>`` each case that both programs completed is judged by
the spec's ``check.py`` (§17.3) against that reference's output folder, and the verdicts are
appended to the report. ``--only a,b`` runs those cases only; ``--env NAME=VALUE`` adds a
variable (``LARMORX_UPSTREAM_DEFECTS=reproduce``). Then ``diff <work>/<a>.txt <work>/<b>.txt``.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

HERE = Path(__file__).resolve().parent
SPEC = HERE.parent / "t2smap-spec"
sys.path.insert(0, str(SPEC))
import blackbox  # noqa: E402  (the spec's probe: inputs, cases, report form)

E3 = blackbox.E3
TES = blackbox.TES
MASK = blackbox.MASK


def extra_inputs(d: Path) -> None:
    """Inputs of the added cases (seeded numpy, plain arithmetic, as the probe's)."""
    ech = blackbox.series()
    nan2 = ech[1].copy()
    nan2[7, 6, 3, 2] = np.nan  # a count-1 voxel whose echo-2 sample is NaN (group 2's fit)
    blackbox.save(nan2, d / "nan-e2.nii.gz")
    inf1 = ech[0].copy()
    inf1[7, 6, 3, 2] = np.inf  # an infinite first-echo mean (the dropout percentile)
    blackbox.save(inf1, d / "inf-e1.nii.gz")
    blackbox.save(-ech[1], d / "neg-e2.nii.gz")  # echo 2 bad everywhere: every count 1
    for i, e in enumerate(ech, 1):
        blackbox.save(e[..., :1], d / f"one-e{i}.nii.gz")
        c = e.copy()
        c[7, 6, 3, :] = c[7, 6, 3, 0]  # a voxel constant over time (PAID's zero weights)
        blackbox.save(c, d / f"const-e{i}.nii.gz")
    _, _, head = blackbox.phantom()
    one = np.zeros(head.shape, dtype=np.uint8)
    one[7, 6, 3] = 1
    blackbox.save(one, d / "mask-one.nii.gz", dtype=np.uint8)
    mask = np.asanyarray(nib.load(str(d / "mask.nii.gz")).dataobj)
    blackbox.save(mask[..., None], d / "mask-4d.nii.gz", dtype=np.uint8)
    blackbox.save(mask[:, :, :-1], d / "mask-grid.nii.gz", dtype=np.uint8)
    # voxels with one good echo among the others: half a slab with echo 2 rising (the decay
    # test fails: count 1, the two-echo fit's start has T2* < 0) and six voxels whose echoes 2
    # and 3 fall to a twentieth and a hundredth of echo 1 (count 1 by the dropout test, too few
    # to move its thresholds: the two-echo fit converges)
    mixed = ech[1].copy()
    mixed[0::2, :, 3, :] = ech[0][0::2, :, 3, :] * 1.2
    mixed[6:9, 6:8, 4, :] = ech[0][6:9, 6:8, 4, :] * 0.05
    blackbox.save(mixed, d / "mixed4-e2.nii.gz")
    mixed3 = ech[2].copy()
    mixed3[6:9, 6:8, 4, :] = ech[0][6:9, 6:8, 4, :] * 0.01
    blackbox.save(mixed3, d / "mixed4-e3.nii.gz")


def _analyze(arr, dtype, *, header=None, endian="<", scale=None):
    """An ANALYZE 7.5 ``.hdr``/``.img`` pair written field by field (no ``.mat``): ``header`` a
    nibabel Analyze-family header class, ``scale`` (slope, inter) the stored values' scaling."""
    hdr = (header or nib.AnalyzeHeader)(endianness=endian)
    hdr.set_data_shape(arr.shape)
    hdr.set_data_dtype(dtype)
    hdr.set_zooms((3.0, 3.0, 3.5, 2.0)[: arr.ndim])
    values = np.asarray(arr, dtype=np.float64)
    if scale is not None:
        values = np.round((values - scale[1]) / scale[0])
    data = values.astype(np.dtype(dtype).newbyteorder(endian))
    return hdr, data


def _write_analyze(hdr, data, stem: Path) -> None:
    with open(f"{stem}.hdr", "wb") as f:
        hdr.write_to(f)
    Path(f"{stem}.img").write_bytes(data.tobytes(order="F"))


def format_inputs(d: Path) -> None:
    """Inputs of the readers' cases (relay 2c): NIfTI-1 pairs with a scaling float32 and float64
    arithmetic round apart, ANALYZE 7.5 (plain, SPM's origin, slope and intercept, the gl/cal
    scaling, a .mat file, big-endian), MGH/MGZ (float32, int16, oblique, as a mask of NIfTI
    echoes); and the inputs of the message cases (a computed mask that erodes away, a one-voxel
    image, a cap of 0 with one positive limited value)."""
    ech = blackbox.series()
    mask = np.asanyarray(nib.load(str(d / "mask.nii.gz")).dataobj)
    for i, e in enumerate(ech, 1):
        img = nib.Nifti1Pair(np.round((e - 1.3) / 0.37).astype(np.int16), blackbox.AFFINE)
        img.header.set_xyzt_units("mm", "sec")
        img.header.set_zooms((3.0, 3.0, 3.5, 2.0))
        img.header.set_slope_inter(0.37, 1.3)
        img.to_filename(str(d / f"p16-e{i}.hdr"))
        hdr, data = _analyze(e, np.float32)
        _write_analyze(hdr, data, d / f"a32-e{i}")
        hdr, data = _analyze(e, np.float32, endian=">")
        _write_analyze(hdr, data, d / f"b32-e{i}")
        hdr, data = _analyze(e, np.int16, header=nib.Spm2AnalyzeHeader, scale=(0.37, 1.3))
        hdr["scl_slope"] = 0.37
        hdr["scl_inter"] = 1.3
        hdr["origin"][:3] = (9, 8, 5)
        _write_analyze(hdr, data, d / f"s16-e{i}")
        hdr, data = _analyze(e, np.int16, scale=(0.37, 320.0))
        hdr["glmin"], hdr["glmax"] = -1000, 1000
        hdr["cal_min"], hdr["cal_max"] = -50.0, 690.0
        _write_analyze(hdr, data, d / f"c16-e{i}")
        img = nib.Spm2AnalyzeImage(e.astype(np.float32), blackbox.AFFINE)
        img.header.set_zooms((3.0, 3.0, 3.5, 2.0))
        img.to_filename(str(d / f"mat-e{i}.img"))
        img = nib.MGHImage(e.astype(np.float32), blackbox.AFFINE)
        img.header.set_zooms((3.0, 3.0, 3.5, 2000.0))
        img.to_filename(str(d / f"m-e{i}.mgz"))
    hdr, data = _analyze(mask, np.uint8)
    _write_analyze(hdr, data, d / "a32-mask")
    hdr, data = _analyze(mask[..., None], np.uint8)
    _write_analyze(hdr, data, d / "a32-mask4d")
    hdr, data = _analyze(mask, np.uint8, header=nib.Spm2AnalyzeHeader)
    hdr["origin"][:3] = (9, 8, 5)
    _write_analyze(hdr, data, d / "s16-mask")
    nib.MGHImage(mask, blackbox.AFFINE).to_filename(str(d / "m-mask.mgz"))
    nib.MGHImage(mask, blackbox.AFFINE).to_filename(str(d / "mn-mask.mgz"))
    # an oblique grid (10 degrees about z, 5 about x): int16 echoes in uncompressed .mgh files
    cz, sz = np.cos(np.radians(10.0)), np.sin(np.radians(10.0))
    cx, sx = np.cos(np.radians(5.0)), np.sin(np.radians(5.0))
    rot = np.array([[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]]) @ np.array(
        [[1, 0, 0], [0, cx, -sx], [0, sx, cx]]
    )
    oblique = np.eye(4)
    oblique[:3, :3] = rot @ np.diag([-3.0, 3.0, 3.5])
    oblique[:3, 3] = [21.7, -18.3, -11.9]
    for i, e in enumerate(ech, 1):
        img = nib.MGHImage(np.round(e).astype(np.int16), oblique)
        img.header.set_zooms((3.0, 3.0, 3.5, 2000.0))
        img.to_filename(str(d / f"mo-e{i}.mgh"))
    nib.MGHImage(mask, oblique).to_filename(str(d / "mo-mask.mgz"))
    # the message cases: a checkerboard that two erosions remove (an empty computed mask), a
    # one-voxel series (too few values for compute_epi_mask's threshold), echo 2 negative
    # everywhere but one voxel (a cap of 0 with one positive limited T2*)
    x, y, z = np.meshgrid(*(np.arange(n) for n in mask.shape), indexing="ij")
    board = ((x + y + z) % 2 == 0)[..., None]
    for i, e in enumerate(ech, 1):
        blackbox.save(np.where(board, e, e * 0.01), d / f"board-e{i}.nii.gz")
        blackbox.save(e[7:8, 6:7, 3:4, :], d / f"tiny-e{i}.nii.gz")
    neg = -ech[1]
    neg[7, 6, 3, :] = ech[1][7, 6, 3, :]
    blackbox.save(neg, d / "negmost-e2.nii.gz")
    (d / "formats.done").write_text("relay 2c\n", encoding="utf-8")


def more_inputs(d: Path) -> None:
    """Inputs added after the first runs (relay 2c): bright planes one voxel thick (every other
    z) that the computed mask's two erosions remove (an empty mask, TED-03), an infinite sample
    in echo 3 where the decay method leaves two good echoes, and both infinities in one echo
    (PAID's warnings)."""
    ech = blackbox.series()
    shape = ech[0].shape
    z = np.arange(shape[2])[None, None, :, None]
    rng = np.random.default_rng(20261111)
    for i, f in enumerate((0.7, 0.5, 0.35), 1):
        level = np.where(z % 2 == 0, 1000.0 * f, 10.0 * f)
        noise = rng.standard_normal(shape)
        blackbox.save(level * (1.0 + 0.02 * noise), d / f"slab-e{i}.nii.gz")
    inf3 = ech[2].copy()
    inf3[7, 6, 3, 2] = np.inf
    blackbox.save(inf3, d / "inf-e3.nii.gz")
    # both infinities in echo 2 of one voxel: echo 2 is bad there (count 1), but group 2's PAID
    # weights read it
    both = ech[1].copy()
    both[7, 6, 3, 2] = np.inf
    both[7, 6, 3, 3] = -np.inf
    blackbox.save(both, d / "both-e2.nii.gz")
    (d / "more.done").write_text("relay 2c\n", encoding="utf-8")


def files(names: list[str]) -> list[str]:
    return ["-d", *(f"<I>/{n}" for n in names)]


EXTRA: list[tuple[str, list[str], int]] = [
    ("x-nan-echo2", [*files(["e1.nii.gz", "nan-e2.nii.gz", "e3.nii.gz"]), *TES, *MASK], 1),
    ("x-inf-echo1", [*files(["inf-e1.nii.gz", "e2.nii.gz", "e3.nii.gz"]), *TES, *MASK], 1),
    ("x-cap-zero", [*files(["e1.nii.gz", "neg-e2.nii.gz", "e3.nii.gz"]), *TES, *MASK], 1),
    ("x-one-volume", [*files([f"one-e{i}.nii.gz" for i in (1, 2, 3)]), *TES, *MASK], 1),
    (
        "x-paid-one-volume",
        [*files([f"one-e{i}.nii.gz" for i in (1, 2, 3)]), *TES, *MASK, "--combmode", "paid"],
        1,
    ),
    (
        "x-paid-constant",
        [*files([f"const-e{i}.nii.gz" for i in (1, 2, 3)]), *TES, *MASK, "--combmode", "paid"],
        1,
    ),
    ("x-single-voxel", [*E3, *TES, "--mask", "<I>/mask-one.nii.gz"], 1),
    ("x-mask-4d", [*E3, *TES, "--mask", "<I>/mask-4d.nii.gz"], 1),
    ("x-mask-grid", [*E3, *TES, "--mask", "<I>/mask-grid.nii.gz"], 1),
    ("x-n-independent-negative", [*E3, *TES, *MASK, "--n-independent-echos", "-1"], 1),
    (
        "x-dummy-negative",
        [*E3, *TES, *MASK, "--dummy-scans", "-1", "--exclude", "0"],
        1,
    ),
    ("x-te-single-inf", ["-d", "<I>/e1.nii.gz", "-e", "inf", *MASK], 1),
    ("x-debug-ts", [*E3, *TES, *MASK, "--fitmode", "ts", "--debug"], 1),
    ("x-help-40", ["-h"], 1),
    # the curve fit (relay 2b): failures of voxels with one good echo (TED-08), the failure
    # counts' messages, the variances, the per-volume fits, one volume, an infinite sample, two
    # echoes per volume (m = 2: SciPy's warning), the used volumes, no mask, PAID per volume
    (
        "x-curvefit-decay",
        [*E3, *TES, *MASK, "--masktype", "decay", "--fittype", "curvefit", "--debug", "--verbose"],
        1,
    ),
    (
        "x-ts-curvefit-decay",
        [
            *E3,
            *TES,
            *MASK,
            "--masktype",
            "decay",
            "--fittype",
            "curvefit",
            "--fitmode",
            "ts",
            "--debug",
            "--verbose",
        ],
        1,
    ),
    (
        "x-one-volume-curvefit",
        [
            *files([f"one-e{i}.nii.gz" for i in (1, 2, 3)]),
            *TES,
            *MASK,
            "--fittype",
            "curvefit",
            "--verbose",
            "--debug",
        ],
        1,
    ),
    (
        "x-inf-sample-curvefit",
        [
            *files(["e1.nii.gz", "inf-e2.nii.gz", "e3.nii.gz"]),
            *TES,
            *MASK,
            "--fittype",
            "curvefit",
            "--debug",
            "--verbose",
        ],
        1,
    ),
    (
        "x-two-echoes-ts-curvefit",
        [
            *files(["e1.nii.gz", "e2.nii.gz"]),
            "-e",
            "0.012",
            "0.028",
            *MASK,
            "--fittype",
            "curvefit",
            "--fitmode",
            "ts",
            "--verbose",
        ],
        1,
    ),
    (
        "x-curvefit-used-volumes",
        [
            *E3,
            *TES,
            *MASK,
            "--fittype",
            "curvefit",
            "--dummy-scans",
            "1",
            "--exclude",
            "2",
            "--debug",
        ],
        1,
    ),
    ("x-curvefit-no-mask", [*E3, *TES, "--fittype", "curvefit", "--debug"], 1),
    (
        "x-curvefit-one-echo-voxels",
        [
            *files(["e1.nii.gz", "mixed4-e2.nii.gz", "mixed4-e3.nii.gz"]),
            *TES,
            *MASK,
            "--masktype",
            "dropout",
            "decay",
            "--fittype",
            "curvefit",
            "--verbose",
            "--debug",
        ],
        1,
    ),
    (
        "x-ts-curvefit-one-echo-voxels",
        [
            *files(["e1.nii.gz", "mixed4-e2.nii.gz", "mixed4-e3.nii.gz"]),
            *TES,
            *MASK,
            "--masktype",
            "dropout",
            "decay",
            "--fittype",
            "curvefit",
            "--fitmode",
            "ts",
            "--verbose",
            "--debug",
        ],
        1,
    ),
    (
        "x-cap-zero-curvefit",
        [
            *files(["e1.nii.gz", "neg-e2.nii.gz", "e3.nii.gz"]),
            *TES,
            *MASK,
            "--fittype",
            "curvefit",
            "--verbose",
            "--debug",
        ],
        1,
    ),
    (
        "x-ts-curvefit-paid",
        [*E3, *TES, *MASK, "--fittype", "curvefit", "--fitmode", "ts", "--combmode", "paid"],
        1,
    ),
]
# relay 2c: the readers (§4.1: NIfTI-1 pairs, ANALYZE 7.5, MGH/MGZ), TED-16 and TED-13 without
# the dropout method (§15.1), and messages not yet observed (TED-03's two masks, TED-18 with a
# positive value, TED-09 per volume, PAID with an infinite sample, a -d/-e count mismatch with
# --n-independent-echos)
NONE = ["--masktype", "none"]
EXTRA += [
    ("x-exclude-all-none", [*E3, *TES, *MASK, *NONE, "--exclude", "0:6"], 1),
    (
        "x-exclude-all-none-curvefit",
        [*E3, *TES, *MASK, *NONE, "--exclude", "0:6", "--fittype", "curvefit"],
        1,
    ),
    ("x-exclude-all-decay", [*E3, *TES, *MASK, "--masktype", "decay", "--exclude", "0:6"], 1),
    ("x-dummy-all-none", [*E3, *TES, *MASK, *NONE, "--dummy-scans", "6"], 1),
    (
        "x-dummy-all-none-curvefit",
        [*E3, *TES, *MASK, *NONE, "--dummy-scans", "6", "--fittype", "curvefit"],
        1,
    ),
    ("x-dummy-all-none-ts", [*E3, *TES, *MASK, *NONE, "--dummy-scans", "6", "--fitmode", "ts"], 1),
    ("x-epi-board", [*files([f"board-e{i}.nii.gz" for i in (1, 2, 3)]), *TES], 1),
    ("x-epi-empty", [*files([f"slab-e{i}.nii.gz" for i in (1, 2, 3)]), *TES], 1),
    (
        "x-dummy-all-none-2e",
        [
            *files(["e1.nii.gz", "e2.nii.gz"]),
            "-e",
            "0.012",
            "0.028",
            *MASK,
            *NONE,
            "--dummy-scans",
            "6",
        ],
        1,
    ),
    (
        "x-inf-both-paid",
        [*files(["e1.nii.gz", "both-e2.nii.gz", "e3.nii.gz"]), *TES, *MASK, "--combmode", "paid"],
        1,
    ),
    (
        "x-inf-echo3-decay-paid",
        [
            *files(["e1.nii.gz", "e2.nii.gz", "inf-e3.nii.gz"]),
            *TES,
            *MASK,
            "--masktype",
            "decay",
            "--combmode",
            "paid",
        ],
        1,
    ),
    ("x-epi-tiny", [*files([f"tiny-e{i}.nii.gz" for i in (1, 2, 3)]), *TES], 1),
    ("x-cap-zero-one", [*files(["e1.nii.gz", "negmost-e2.nii.gz", "e3.nii.gz"]), *TES, *MASK], 1),
    (
        "x-inf-sample-ts",
        [*files(["e1.nii.gz", "inf-e2.nii.gz", "e3.nii.gz"]), *TES, *MASK, "--fitmode", "ts"],
        1,
    ),
    (
        "x-inf-sample-paid",
        [*files(["e1.nii.gz", "inf-e2.nii.gz", "e3.nii.gz"]), *TES, *MASK, "--combmode", "paid"],
        1,
    ),
    (
        "x-te-count-independent",
        [*E3, "-e", "0.012", "0.028", *MASK, "--n-independent-echos", "3"],
        1,
    ),
    ("x-pair-i16", [*files([f"p16-e{i}.hdr" for i in (1, 2, 3)]), *TES, *MASK], 1),
    ("x-pair-i16-img", [*files([f"p16-e{i}.img" for i in (1, 2, 3)]), *TES, *MASK], 1),
    (
        "x-analyze",
        [*files([f"a32-e{i}.hdr" for i in (1, 2, 3)]), *TES, "--mask", "<I>/a32-mask.hdr"],
        1,
    ),
    ("x-analyze-nomask", [*files([f"a32-e{i}.hdr" for i in (1, 2, 3)]), *TES], 1),
    (
        "x-analyze-mask-4d",
        [*files([f"a32-e{i}.img" for i in (1, 2, 3)]), *TES, "--mask", "<I>/a32-mask4d.hdr"],
        1,
    ),
    (
        "x-analyze-be",
        [*files([f"b32-e{i}.hdr" for i in (1, 2, 3)]), *TES, "--mask", "<I>/a32-mask.hdr"],
        1,
    ),
    (
        "x-analyze-spm",
        [*files([f"s16-e{i}.hdr" for i in (1, 2, 3)]), *TES, "--mask", "<I>/s16-mask.hdr"],
        1,
    ),
    (
        "x-analyze-cal",
        [*files([f"c16-e{i}.hdr" for i in (1, 2, 3)]), *TES, "--mask", "<I>/a32-mask.hdr"],
        1,
    ),
    ("x-analyze-mat", [*files([f"mat-e{i}.img" for i in (1, 2, 3)]), *TES, *MASK], 1),
    ("x-mgh", [*files([f"m-e{i}.mgz" for i in (1, 2, 3)]), *TES, "--mask", "<I>/m-mask.mgz"], 1),
    ("x-mgh-nomask", [*files([f"m-e{i}.mgz" for i in (1, 2, 3)]), *TES], 1),
    (
        "x-mgh-oblique-i16",
        [*files([f"mo-e{i}.mgh" for i in (1, 2, 3)]), *TES, "--mask", "<I>/mo-mask.mgz"],
        1,
    ),
    ("x-mgh-oblique-nomask", [*files([f"mo-e{i}.mgh" for i in (1, 2, 3)]), *TES], 1),
    ("x-mgh-mask-on-nifti", [*E3, *TES, "--mask", "<I>/mn-mask.mgz"], 1),
]
NO_OUT_DIR = blackbox.NO_OUT_DIR | {"x-help-40"}
CASE_ENV = {"x-help-40": {"COLUMNS": "40"}}


def run_case(program: list[str], full: list[str], case: Path, env: dict, never: str):
    if program[0].endswith("t2smap"):
        from larmorx_validation import oracle_cache

        policy = oracle_cache.Policy(threads=1, family="python", never=never)
        res = oracle_cache.run(
            [*program, *full],
            env=env,
            cwd=case,
            outputs=[case / "out"],
            policy=policy,
            timeout=900,
            runner="t2smap-original-compare",
        )
        return (
            res.returncode,
            res.stdout_bytes.decode("utf-8", "replace"),
            res.stderr_bytes.decode("utf-8", "replace"),
        )
    proc = subprocess.run(
        [*program, *full], cwd=case, env=env, capture_output=True, text=True, timeout=900
    )
    return proc.returncode, proc.stdout, proc.stderr


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("work", type=Path)
    p.add_argument("name")
    p.add_argument("program", nargs="+")
    p.add_argument("--check", default=None)
    p.add_argument("--only", default=None)
    p.add_argument("--env", action="append", default=[])
    a = p.parse_args(argv)
    root = a.work.resolve() / a.name
    if root.exists():
        import shutil

        shutil.rmtree(root)
    root.mkdir(parents=True)
    # the inputs are shared by every report of the work folder (the same paths in every call
    # file); made once, seeded, so every program reads the same bytes
    inputs = a.work.resolve() / "inputs"
    if not (inputs / "mixed4-e3.nii.gz").exists():
        if inputs.exists():
            import shutil

            shutil.rmtree(inputs)
        blackbox.make_inputs(inputs)
        extra_inputs(inputs)
    if not (inputs / "formats.done").exists():
        format_inputs(inputs)
    if not (inputs / "more.done").exists():
        more_inputs(inputs)
    env = blackbox.environment()
    for kv in a.env:
        k, _, v = kv.partition("=")
        env[k] = v
    only = set(a.only.split(",")) if a.only else None
    lines: list[str] = []
    done: list[str] = []
    for name, args, runs in [*blackbox.CASES, *EXTRA]:
        if only and name not in only:
            continue
        case = root / name
        case.mkdir()
        full = [w.replace("<I>", str(inputs)) for w in args]
        if name not in NO_OUT_DIR:
            full = [*full, "--out-dir", "out"]
        lines.append(f"== {name}: {' '.join(args)}")
        cenv = {**env, **CASE_ENV.get(name, {})}
        for run in range(runs):
            never = "" if run == 0 else "a second run depends on the first run's outputs"
            code, out, err = run_case(a.program, full, case, cenv, never)
            lines.append(f"-- run {run + 1}: exit {code}")
            lines += [
                f"  out| {s}" for s in blackbox.normalise(out.replace(str(inputs), "<I>"), root)
            ]
            lines += [
                f"  err| {s}" for s in blackbox.normalise(err.replace(str(inputs), "<I>"), root)
            ]
        lines += [s.replace(str(inputs), "<I>") for s in blackbox.describe_dir(case / "out", root)]
        if (case / "out" / "t2smap_call.sh").exists():
            done.append(name)
    if a.check:
        ref_root = a.work.resolve() / a.check
        for name in done:
            ref = ref_root / name / "out"
            if (
                not (ref / "desc-optcom_bold.nii.gz").exists()
                and not (ref / "ts_OC.nii.gz").exists()
            ):
                continue
            proc = subprocess.run(
                [
                    sys.executable,
                    "-I",
                    str(SPEC / "check.py"),
                    str(ref),
                    str(root / name / "out"),
                    "--cwd",
                    str(ref_root / name),
                    "--details",
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            lines.append(f"== check {name}: exit {proc.returncode}")
            lines += [f"  {s}" for s in proc.stdout.splitlines()]
            lines += [f"  err| {s}" for s in proc.stderr.splitlines()[-5:]]
    (a.work.resolve() / f"{a.name}.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"COMPARE-END {a.name}: {len(done)} cases with outputs")
    return 0


if __name__ == "__main__":
    os.environ.setdefault("PYTHONHASHSEED", "0")
    sys.exit(main(sys.argv[1:]))
