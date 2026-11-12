# SPDX-License-Identifier: Apache-2.0
"""tedana's ``t2smap``, the clean-room original (``crates/larmorx-tedana``): through its own
binding (``larmorx._core.tedana_t2smap_cleanroom``) the version, a parser error, a whole
log-linear run on a small phantom (the files, the call file, the registry), the same files at
one and four threads, and a default's correction with its notice; and since it is wired in
(2026-11-12) through ``lx.tedana.t2smap(..., implementation="clean-room")`` (in this process:
paths and in-memory images, the notices as warnings, a failure) and the command lines (the
console script and ``larmorx._core.cli_main``, with larmorx's options before the family), each
giving the binding's files."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
import pytest

import larmorx as lx
from larmorx import _core, _replica

run = _core.tedana_t2smap_cleanroom
ENV = [("COLUMNS", "80")]
TES = ["-e", "0.012", "0.028", "0.044"]


def _phantom(d: Path) -> list[str]:
    """Three echoes (8 × 6 × 4 × 5) of S0 · (1 − 1/T2*)^TE with seeded noise, and a mask."""
    rng = np.random.default_rng(20261111)
    x, y, z = np.meshgrid(np.arange(8), np.arange(6), np.arange(4), indexing="ij")
    s0 = 1000.0 + 25.0 * x - 10.0 * y
    factor = 1.0 - 1.0 / (18.0 + 3.0 * y + 2.0 * z)
    affine = np.diag([-3.0, 3.0, 3.5, 1.0])
    names = []
    for i, te in enumerate((12, 28, 44), 1):
        decay = np.ones_like(factor)
        for _ in range(te):
            decay = decay * factor
        e = (s0 * decay)[..., None] * (1.0 + 0.02 * rng.standard_normal((8, 6, 4, 5)))
        nib.Nifti1Image(e.astype(np.float32), affine).to_filename(d / f"e{i}.nii.gz")
        names.append(f"e{i}.nii.gz")
    mask = np.zeros((8, 6, 4), dtype=np.uint8)
    mask[1:7] = 1
    nib.Nifti1Image(mask, affine).to_filename(d / "mask.nii.gz")
    return ["-d", *names, *TES, "--mask", "mask.nii.gz"]


def test_version_and_parser_error(tmp_path: Path) -> None:
    assert run(["-v"], ENV, str(tmp_path)) == (0, "t2smap v26.0.3\n", "")
    code, out, err = run([], ENV, str(tmp_path))
    assert (code, out) == (2, "")
    assert err.endswith("t2smap: error: the following arguments are required: -d, -e\n")


def test_loglin_run_and_threads(tmp_path: Path) -> None:
    args = _phantom(tmp_path)
    data = {}
    for threads in ("1", "4"):
        code, _, err = run(
            [*args, "--n-threads", threads, "--out-dir", f"o{threads}"], ENV, str(tmp_path)
        )
        assert code == 0, err
        assert "Workflow completed" in err
        out = tmp_path / f"o{threads}"
        data[threads] = {
            p.name: np.asanyarray(nib.load(p).dataobj).tobytes()
            for p in sorted(out.glob("*.nii.gz"))
        }
        data[threads]["table"] = (out / "desc-confounds_timeseries.tsv").read_bytes()
    assert data["1"] == data["4"]
    out = tmp_path / "o1"
    registry = json.loads((out / "desc-tedana_registry.json").read_text(encoding="utf-8"))
    assert registry["combined img"] == "desc-optcom_bold.nii.gz"
    call = (out / "t2smap_call.sh").read_text(encoding="utf-8")
    assert call == "t2smap " + " ".join([*args, "--n-threads", "1", "--out-dir", "o1"])
    t2s = nib.load(out / "T2starmap.nii.gz")
    assert t2s.header["sform_code"] == 2 and t2s.header["qform_code"] == 0
    assert nib.load(out / "desc-optcom_bold.nii.gz").shape == (8, 6, 4, 5)


@pytest.mark.parametrize("mode", ["fix", "reproduce"])
def test_exclude_index_equal_to_the_volumes(tmp_path: Path, mode: str) -> None:
    args = [*_phantom(tmp_path), "--exclude", "5", "--out-dir", "o"]
    code, _, err = run(args, [*ENV, ("LARMORX_UPSTREAM_DEFECTS", mode)], str(tmp_path))
    assert code == 1
    if mode == "fix":
        assert "is not below the number of timepoints in the data (5)." in err
        assert "larmorx: upstream defect TED-02 (tedana) corrected" in err
    else:
        assert "IndexError: index 5 is out of bounds for axis 0 with size 5" in err
        assert "larmorx: upstream defect TED-02 (tedana) reproduced" in err


@pytest.mark.parametrize("fitmode", ["all", "ts"])
def test_curvefit_run(tmp_path: Path, fitmode: str) -> None:
    """The curve fit (§7.3): the failures image (uint8) and, with ``--verbose``, the variance
    maps, beside the maps the fit replaces; finite fits close to the log-linear ones."""
    args = [*_phantom(tmp_path), "--fitmode", fitmode]
    code, _, err = run([*args, "--out-dir", "ll"], ENV, str(tmp_path))
    assert code == 0, err
    code, _, err = run(
        [*args, "--fittype", "curvefit", "--verbose", "--out-dir", "cf"], ENV, str(tmp_path)
    )
    assert code == 0, err
    out = tmp_path / "cf"
    fail = nib.load(out / "desc-decayFitFailures_mask.nii.gz")
    assert fail.get_data_dtype() == np.uint8
    assert np.asanyarray(fail.dataobj).max() <= 1
    names = (
        "stat-variance_desc-t2star_statmap.nii.gz",
        "stat-variance_desc-s0_statmap.nii.gz",
        "stat-covariance_desc-t2star+s0_statmap.nii.gz",
    )
    for name in names:
        assert nib.load(out / name).shape[:3] == (8, 6, 4)
    t2_ll = np.asanyarray(nib.load(tmp_path / "ll" / "T2starmap.nii.gz").dataobj)
    t2_cf = np.asanyarray(nib.load(out / "T2starmap.nii.gz").dataobj)
    inside = t2_ll > 0
    assert np.all(np.isfinite(t2_cf[inside]))
    assert np.median(np.abs(t2_cf[inside] / t2_ll[inside] - 1)) < 0.2


def _files(out: Path) -> dict[str, bytes]:
    """The images' data and the table and registry of an output directory, by name."""
    files = {
        p.name: np.asanyarray(nib.load(p).dataobj).tobytes() for p in sorted(out.glob("*.nii.gz"))
    }
    for name in ("desc-confounds_timeseries.tsv", "desc-tedana_registry.json"):
        files[name] = (out / name).read_bytes()
    return files


def _binding_files(tmp_path: Path, extra: list[str] | None = None) -> dict[str, bytes]:
    """The binding's run of the phantom (``extra`` added) in a directory of its own."""
    d = tmp_path / "binding"
    d.mkdir()
    args = [*_phantom(d), *(extra or []), "--n-threads", "1", "--out-dir", "out"]
    code, _, err = run(args, ENV, str(d))
    assert code == 0, err
    return _files(d / "out")


def test_the_wrapper_runs_the_original_in_this_process(tmp_path: Path) -> None:
    d = tmp_path / "wrapper"
    d.mkdir()
    _phantom(d)
    echoes = [d / f"e{i}.nii.gz" for i in (1, 2, 3)]
    tes = [0.012, 0.028, 0.044]
    res = lx.tedana.t2smap(
        echoes, tes, mask=d / "mask.nii.gz", out_dir=d / "out", implementation="clean-room",
        n_threads=2,
    )  # fmt: skip
    assert res.implementation == _replica.ORIGINAL
    assert res.implementation.kind == "clean-room" and res.implementation.package == "larmorx"
    assert res.out_dir == str(d / "out") and res.fit_failures is None
    assert _files(d / "out") == _binding_files(tmp_path)
    t2s = nib.load(d / "out" / "T2starmap.nii.gz")
    assert np.array_equal(res.t2star.data, np.asanyarray(t2s.dataobj))
    assert res.t2star.implementation == res.implementation
    assert res.optcom.data.shape == (8, 6, 4, 5) and res.adaptive_mask.data.dtype == np.int32
    assert res.confounds["rmse_mean"].shape == (5,)
    assert res.files["combined img"] == "desc-optcom_bold.nii.gz"
    assert res.dataset_description["GeneratedBy"][0]["Version"] == "26.0.3"
    assert "Workflow completed" in res.log and "upstream defect" not in res.log
    # in-memory echoes (written as NIfTI-1 by the wrapper) give the same maps
    memory = [nib.load(p) for p in echoes]
    again = lx.tedana.t2smap(
        [(np.asanyarray(m.dataobj), m.affine) for m in memory], tes,
        mask=nib.load(d / "mask.nii.gz"), implementation="clean-room", n_threads=0,
    )  # fmt: skip
    assert np.array_equal(again.t2star.data, res.t2star.data)
    assert np.array_equal(again.optcom.data, res.optcom.data)
    assert again.out_dir is None


def test_the_wrappers_notices_and_failures(tmp_path: Path) -> None:
    """A voxel whose echoes are all equal (no decay): TED-46's correction named by a warning;
    a failing run raises ``RuntimeError`` with t2smap's messages."""
    _phantom(tmp_path)
    echoes = []
    for i in (1, 2, 3):
        img = nib.load(tmp_path / f"e{i}.nii.gz")
        data = np.asanyarray(img.dataobj).copy()
        data[3, 2, 1, :] = 1500.0
        echoes.append((data, img.affine))
    mask = tmp_path / "mask.nii.gz"
    with pytest.warns(lx.UpstreamDefectWarning) as caught:
        res = lx.tedana.t2smap(
            echoes, [0.012, 0.028, 0.044], mask=mask, implementation="clean-room"
        )
    named = [str(w.message).split(" ")[0] for w in caught]
    assert "TED-46" in named, named  # (TED-05's cap applies too)
    assert "upstream defect" not in res.log
    with pytest.raises(RuntimeError, match="the clean-room original") as e:
        lx.tedana.t2smap(echoes, [0.012, 0.028, 0.044], mask=mask, exclude=[5],
                         implementation="clean-room")  # fmt: skip
    assert "is not below the number of timepoints in the data (5)." in str(e.value)
    assert not isinstance(e.value, lx.ReplicaError)


def test_the_command_lines_run_the_original(tmp_path: Path) -> None:
    """``python -m larmorx`` (the console script) and ``_core.cli_main`` with larmorx's options
    before the family write the binding's files."""
    assert _core.cli_main(["larmorx", "tedana", "t2smap", "-v"], [], "clean-room") == (
        0,
        "t2smap v26.0.3\n",
        "",
    )
    d = tmp_path / "cli"
    d.mkdir()
    args = [*_phantom(d), "--out-dir", "out"]
    env = {k: v for k, v in os.environ.items() if k not in ("LARMORX_UPSTREAM_DEFECTS", "COLUMNS")}
    command = [sys.executable, "-m", "larmorx", "--implementation", "clean-room", "--n-threads",
               "2", "--upstream-defects", "fix", "tedana", "t2smap", *args]  # fmt: skip
    p = subprocess.run(command, cwd=d, env=env, capture_output=True, text=True, check=False)
    assert p.returncode == 0, p.stderr
    assert "Workflow completed" in p.stderr and p.stdout == ""
    expected = _binding_files(tmp_path)
    assert _files(d / "out") == expected
    e = tmp_path / "core"
    e.mkdir()
    args = [*_phantom(e), "--out-dir", str(e / "out")]
    args = [str(e / a) if a.endswith(".nii.gz") else a for a in args]
    code, out, err = _core.cli_main(
        ["larmorx", "--n-threads", "3", "tedana", "t2smap", *args], [], "clean-room"
    )
    assert (code, out) == (0, ""), err
    assert "Workflow completed" in err
    assert _files(e / "out") == expected
