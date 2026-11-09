# SPDX-License-Identifier: Apache-2.0
"""sdcflows' init_unwarp_wf as lx.sdcflows.unwarp_workflow: the thread setting, the errors and
the outputs on a small generated EPI (sdcflows' own graph against it on real data:
validation/investigations/sdcflows-fieldmap-b/check_unwarp.py)."""

from __future__ import annotations

import warnings
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx
from larmorx._defects import UpstreamDefectWarning

AFF = np.diag([-3.0, 3.0, 3.5, 1.0])  # LAS, as most EPIs (a positive-axes series: SDC-08)
SHAPE = (24, 22, 16)
META = {"PhaseEncodingDirection": "j-", "TotalReadoutTime": 0.05}


def _inputs(tmp: Path) -> tuple[str, str, str]:
    i, j, k = np.indices(SHAPE)
    head = ((i - 11.5) / 9) ** 2 + ((j - 10.5) / 8) ** 2 + ((k - 7.5) / 6) ** 2 <= 1
    vol = np.where(head, 300 + (i * 7 + j * 5 + k * 3) % 50, (i + j + k) % 7).astype(np.int16)
    epi = str(tmp / "sub-01_task-x_bold.nii.gz")
    lx.save((np.stack([vol, vol + 4], -1), AFF), epi)
    grid = lx.sdcflows.bspline_grid(lx.Image(vol, AFF), (16.0, 16.0, 10.0))
    coeffs = (np.indices(tuple(grid.shape)).sum(0) % 5 - 2).astype(np.float32) * 3
    coeff = str(tmp / "coeff.nii.gz")
    lx.save((coeffs, np.asarray(grid.affine)), coeff)
    xfm = str(tmp / "xfm.txt")
    Path(xfm).write_text(
        "#Insight Transform File V1.0\n#Transform 0\nTransform: AffineTransform_float_3_3\n"
        "Parameters: 1 0 0 0 1 0 0 0 1 0.5 -1 0.25\nFixedParameters: 0 0 0\n",
        encoding="utf-8",
    )
    return epi, coeff, xfm


@pytest.mark.parametrize(
    ("omp", "free", "debug", "want"),
    [(8, None, False, 8), (8, None, True, 1), (8, 12.5, False, 2), (8, 0.5, False, 1),
     (2, 100.0, False, 2), (4, 5.6, False, 1)],
)  # fmt: skip
def test_num_threads(omp: int, free: float | None, debug: bool, want: int) -> None:
    assert lx.sdcflows.unwarp_num_threads(omp, free, debug) == want


def test_errors(tmp_path: Path) -> None:
    epi, coeff, xfm = _inputs(tmp_path)
    with pytest.raises(TypeError, match="in_data"):
        lx.sdcflows.unwarp_workflow([epi], META, coeff)  # type: ignore[arg-type]
    with pytest.raises(ValueError, match="SDC-26"):
        lx.sdcflows.unwarp_workflow(epi, META, coeff, fmap2data_xfm=xfm, data2fmap_xfm=xfm)
    with pytest.raises(KeyError, match="PhaseEncodingDirection"):
        lx.sdcflows.unwarp_workflow(epi, {"TotalReadoutTime": 0.05}, coeff)
    with pytest.raises(ValueError, match="Unknown total-readout time"):
        lx.sdcflows.unwarp_workflow(epi, {"PhaseEncodingDirection": "j"}, coeff)


def test_outputs(tmp_path: Path) -> None:
    epi, coeff, xfm = _inputs(tmp_path)
    out = tmp_path / "unwarp_wf"
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        res = lx.sdcflows.unwarp_workflow(
            epi, META, [coeff], data2fmap_xfm=xfm, omp_nthreads=4, free_mem=12.0, out_dir=out
        )
    assert res.fieldwarp is None and res.num_threads == 2
    assert (
        Path(res.fieldmap).relative_to(out).as_posix() == "resample/sub-01_task-x_bold_field.nii.gz"
    )  # type: ignore[arg-type]
    assert (
        Path(res.corrected).relative_to(out).as_posix()
        == "merge/sub-01_task-x_bold_sdc_merged.nii.gz"
    )  # type: ignore[arg-type]
    assert Path(res.corrected_ref).parent == out / "brainextraction_wf" / "clipper_post"  # type: ignore[arg-type]
    assert Path(res.corrected_mask).parent == out / "brainextraction_wf" / "masker"  # type: ignore[arg-type]
    assert lx.io.load(res.corrected).shape == (*SHAPE, 2)  # type: ignore[arg-type]
    assert set(res.nodes) == {"rotime", "resample", "merge", "average", "brainextraction_wf"}
    assert res.nodes["rotime"].readout_time == 0.05
    # both transforms with reproduce: data2fmap_xfm used, with the warning (SDC-26)
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        both = lx.sdcflows.unwarp_workflow(
            epi, META, coeff, fmap2data_xfm=coeff, data2fmap_xfm=xfm,
            out_dir=tmp_path / "both", upstream_defects="reproduce",
        )  # fmt: skip
    assert any(
        isinstance(w.message, UpstreamDefectWarning) and "SDC-26" in str(w.message) for w in caught
    )
    a = np.asarray(lx.io.load(res.fieldmap).data)  # type: ignore[arg-type]
    b = np.asarray(lx.io.load(both.fieldmap).data)  # type: ignore[arg-type]
    assert a.tobytes() == b.tobytes()
    # without out_dir: a temporary folder, the paths None
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        tmp = lx.sdcflows.unwarp_workflow(epi, META, coeff, jacobian=False)
    assert tmp.corrected is None and tmp.fieldmap is None
