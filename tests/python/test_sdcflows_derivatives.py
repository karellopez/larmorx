# SPDX-License-Identifier: Apache-2.0
"""niworkflows' DerivativesDataSink, pybids' build_path, sdcflows' derivatives, estimator
workflows and init_fmap_preproc_wf: behaviours and errors on small generated files (the values
are niworkflows', pybids' and sdcflows': validation/investigations/sdcflows-fieldmap-b/
check_derivatives.py compares them on the oracle runs and on edge cases)."""

from __future__ import annotations

import json
import warnings
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx
from larmorx._defects import UpstreamDefectWarning
from larmorx.io.nibabel_writer import HeaderDataError

SRC = "/bids/sub-01/fmap/sub-01_acq-a_run-01_fieldmap.nii.gz"
FM = {"suffix": "fieldmap", "datatype": "fmap", "desc": "preproc"}


def _image(path: Path, data: np.ndarray) -> str:
    lx.save((data, np.diag([2.0, 2.0, 2.5, 1.0])), str(path))
    return str(path)


def test_build_path_examples() -> None:
    pats = [
        "sub-{subject}[/ses-{session}]/anat/sub-{subject}[_ses-{session}]_"
        "{suffix<T[12]w|inplaneT[12]>}{extension<.nii|.nii.gz|.json>|.nii.gz}",
        "sub-{subject}[/ses-{session}]/anat/sub-{subject}[_space-{space}]_"
        "{suffix<T1w|inplaneT[12]>}{extension<.nii|.nii.gz|.json>|.nii.gz}",
    ]
    ents = {"extension": ".nii", "space": "MNI", "subject": "001", "suffix": "inplaneT2"}
    assert lx.bids.build_path(ents, pats) == "sub-001/anat/sub-001_inplaneT2.nii"
    assert (
        lx.bids.build_path(ents, pats, strict=True)
        == "sub-001/anat/sub-001_space-MNI_inplaneT2.nii"
    )
    assert lx.bids.build_path({**ents, "suffix": "bold"}, pats) is None
    many = lx.bids.build_path({"subject": ["01", "02"], "suffix": "T1w"}, pats)
    assert many == ["sub-01/anat/sub-01_T1w.nii.gz", "sub-02/anat/sub-02_T1w.nii.gz"]


def test_sink_names_sidecar_and_copy(tmp_path: Path) -> None:
    f = _image(tmp_path / "in.nii", np.arange(24, dtype=np.float32).reshape(2, 3, 4))
    res = lx.niworkflows.derivatives_data_sink(
        f, SRC, base_directory=tmp_path / "out", compress=True, entities=FM,
        meta_dict={"A": 1, "B": 1}, init_metadata={"B": 2}, metadata={"Units": "Hz"},
    )  # fmt: skip
    assert res.out == str(
        tmp_path / "out/niworkflows/sub-01/fmap/sub-01_acq-a_run-01_desc-preproc_fieldmap.nii.gz"
    )
    assert res.compression == (True,)
    meta = json.loads(Path(res.out_meta).read_text())  # type: ignore[arg-type]
    assert meta == {"A": 1, "B": 2, "Units": "Hz"}
    # the data are kept bit for bit
    a, b = lx.io.load(f), lx.io.load(res.out)
    assert np.array_equal(np.asarray(a.data), np.asarray(b.data))


def test_sink_mask_coerced_and_clamped(tmp_path: Path) -> None:
    f = _image(tmp_path / "m.nii.gz", np.array([[[1.0, 0.0], [300.0, -4.0]]], dtype=np.float32))
    ents = {"suffix": "mask", "datatype": "fmap", "desc": "brain"}
    with pytest.warns(UpstreamDefectWarning, match="PIPE-29"):
        res = lx.niworkflows.derivatives_data_sink(f, SRC, base_directory=tmp_path, entities=ents)
    out = lx.io.load(res.out)
    assert np.asarray(out.data).dtype == np.uint8
    assert np.asarray(out.data).ravel(order="F").tolist() == [1, 255, 0, 0]


def test_sink_defects_and_errors(tmp_path: Path) -> None:
    files = [_image(tmp_path / f"f{i}.nii.gz", np.zeros((2, 2, 2), np.float32)) for i in range(3)]
    kw = {"base_directory": tmp_path, "allowed_entities": ("custom",)}
    ents = {**FM, "custom": ["a", "b", "c"]}
    with pytest.raises(ValueError, match="PIPE-28"):
        lx.niworkflows.derivatives_data_sink(
            files, SRC, compress=[True, False], entities=ents, **kw
        )
    with pytest.raises(IndexError):
        lx.niworkflows.derivatives_data_sink(
            files, SRC, compress=[True, False], entities=ents, upstream_defects="reproduce", **kw
        )
    # PIPE-30: RepetitionTime for a 3-D image
    bold = "/bids/sub-01/func/sub-01_task-x_bold.nii.gz"
    with pytest.warns(UpstreamDefectWarning, match="PIPE-30"):
        res = lx.niworkflows.derivatives_data_sink(
            files[0], bold, base_directory=tmp_path, entities={"suffix": "boldref"},
            metadata={"RepetitionTime": 2.0},
        )  # fmt: skip
    assert json.loads(Path(res.out_meta).read_text()) == {"RepetitionTime": 2.0}  # type: ignore[arg-type]
    with pytest.raises(HeaderDataError, match="Expecting 3 zoom values"):
        lx.niworkflows.derivatives_data_sink(
            files[0], bold, base_directory=tmp_path, entities={"suffix": "boldref"},
            metadata={"RepetitionTime": 2.0}, upstream_defects="reproduce",
        )  # fmt: skip
    with pytest.raises(ValueError, match="not an entity"):
        lx.niworkflows.derivatives_data_sink(
            files[0], SRC, base_directory=tmp_path, entities={"x": 1}
        )
    with pytest.raises(ValueError, match="not metadata"):
        lx.niworkflows.derivatives_data_sink(
            files[0], SRC, base_directory=tmp_path, metadata={"desc": 1}
        )
    with pytest.raises(FileNotFoundError):
        lx.niworkflows.derivatives_data_sink(
            str(tmp_path / "none.nii"), SRC, base_directory=tmp_path
        )
    with pytest.raises(ValueError, match="Could not build path"):
        lx.niworkflows.derivatives_data_sink(
            files[0], SRC, base_directory=tmp_path, entities={"suffix": "nothing"}
        )
    with pytest.raises(NotImplementedError, match="templates part"):
        lx.niworkflows.derivatives_data_sink(
            files[0], bold, base_directory=tmp_path,
            entities={"suffix": "bold", "space": "MNI152NLin6Asym", "resolution": "2"},
        )  # fmt: skip
    with pytest.warns(UpstreamDefectWarning, match="PIPE-26"):
        lx.niworkflows.derivatives_data_sink(
            files[:1] * 4, SRC, base_directory=tmp_path, allowed_entities=("a", "b"),
            entities={**FM, "a": [1, 2], "b": ["x", "y"]},
        )  # fmt: skip


def _bids(tmp_path: Path) -> Path:
    root = tmp_path / "bids"
    (root / "sub-01" / "fmap").mkdir(parents=True)
    (root / "dataset_description.json").write_text('{"Name": "x", "BIDSVersion": "1.8.0"}')
    for d, pe in (("AP", "j-"), ("PA", "j")):
        _image(
            root / "sub-01" / "fmap" / f"sub-01_dir-{d}_epi.nii.gz", np.ones((4, 4, 4), np.float32)
        )
        (root / "sub-01" / "fmap" / f"sub-01_dir-{d}_epi.json").write_text(
            json.dumps({"PhaseEncodingDirection": pe, "IntendedFor": "func/a_bold.nii.gz"})
        )
    return root


def test_estimator_workflow_and_preproc_errors(tmp_path: Path) -> None:
    root = _bids(tmp_path)
    reg = lx.sdcflows.FieldmapRegistry()
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        ests = lx.sdcflows.find_estimators(
            layout=lx.bids.BIDSLayout(root, validate=False),
            subject="01",
            fmapless=False,
            registry=reg,
        )
    (est,) = ests
    # no readout time: init_fmap_preproc_wf stops before any work
    with pytest.raises(RuntimeError, match="Missing readout timing information"):
        lx.sdcflows.fmap_preproc_workflow(ests, omp_nthreads=1, output_dir=tmp_path / "o")
    with pytest.raises(TypeError, match="SDC-22"):
        lx.sdcflows.estimator_workflow(est, slopy=True)
    wf = lx.sdcflows.estimator_workflow(est, slopy=True, upstream_defects="reproduce", sloppy=True)
    assert wf.name == "wf_auto_00000" and wf.function == "topup_workflow"
    assert wf.options["sloppy"] is True and "slopy" not in wf.options
    assert [Path(p).name for p in wf.inputs["in_data"]] == [
        "sub-01_dir-AP_epi.nii.gz",
        "sub-01_dir-PA_epi.nii.gz",
    ]
    assert est.get_workflow(debug=True) is wf  # sdcflows keeps the first workflow
    with pytest.raises(ValueError, match="unknown field"):
        lx.sdcflows.fmap_preproc_workflow(
            ests, omp_nthreads=1, output_dir=tmp_path / "o", fallback_total_readout_time=0.05,
            inputs={"auto_00000": {"epi_ref": "x"}},
        )  # fmt: skip
    with pytest.raises(TypeError, match="has no input"):
        wf.run(tmp_path / "w", epi_ref="x")
