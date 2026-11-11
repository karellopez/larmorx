# SPDX-License-Identifier: Apache-2.0
"""tedana's ``t2smap``, the clean-room original (``crates/larmorx-tedana``) through its own
binding (``larmorx._core.tedana_t2smap_cleanroom``), before it is wired into
``lx.tedana.t2smap``: the version, a parser error, a whole log-linear run on a small phantom
(the files, the call file, the registry), the same files at one and four threads, and a
default's correction with its notice."""

from __future__ import annotations

import json
from pathlib import Path

import nibabel as nib
import numpy as np
import pytest

from larmorx import _core

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
