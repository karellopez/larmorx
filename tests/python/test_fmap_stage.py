# SPDX-License-Identifier: Apache-2.0
"""larmorprepx's field-map estimation stage (fMRIPrep's caller of sdcflows'
init_fmap_preproc_wf) and the Python steps of sdcflows' 3dQwarp estimation: behaviours and
errors on small generated datasets (fMRIPrep's and sdcflows' own code against these:
validation/investigations/sdcflows-fieldmap-b/check_fmap_stage.py and check_qwarp_steps.py)."""

from __future__ import annotations

import json
import logging
import warnings
from pathlib import Path
from types import SimpleNamespace
from typing import Any

import numpy as np
import pytest

import larmorx as lx
from larmorx._defects import UpstreamDefectWarning
from larmorx.pipelines.larmorprepx import estimators as le

AFF = np.diag([2.0, 2.0, 2.5, 1.0])


def _put(path: Path, shape: tuple[int, ...], meta: dict[str, Any] | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lx.save((np.zeros(shape, np.int16), AFF), str(path))
    if meta is not None:
        stem = str(path).removesuffix(".nii.gz")
        Path(stem + ".json").write_text(json.dumps(meta), encoding="utf-8")


def _dataset(root: Path) -> Path:
    """Three PEPOLAR estimators: one with a readout time, one with only an estimate, and a
    "sophisticated" one (two EPI files and an SBRef)."""
    (root / "dataset_description.json").parent.mkdir(parents=True, exist_ok=True)
    (root / "dataset_description.json").write_text(
        '{"Name": "t", "BIDSVersion": "1.8.0"}', encoding="utf-8"
    )
    for acq in ("good", "est", "soph"):
        for d, pe in (("AP", "j-"), ("PA", "j")):
            meta: dict[str, Any] = {"PhaseEncodingDirection": pe, "B0FieldIdentifier": f"pe_{acq}"}
            meta["EstimatedTotalReadoutTime" if acq == "est" else "TotalReadoutTime"] = 0.05
            _put(root / "sub-01" / "fmap" / f"sub-01_acq-{acq}_dir-{d}_epi.nii.gz", (4, 4, 4, 2), meta)  # fmt: skip
    _put(
        root / "sub-01" / "func" / "sub-01_task-a_acq-soph_dir-AP_sbref.nii.gz",
        (4, 4, 4),
        {"PhaseEncodingDirection": "j-", "B0FieldIdentifier": "pe_soph", "TotalReadoutTime": 0.05},
    )
    return root


def _estimators(root: Path, *acqs: str) -> list[Any]:
    reg = lx.sdcflows.FieldmapRegistry()
    out = []
    for acq in acqs:
        files = sorted((root / "sub-01").rglob(f"*_acq-{acq}_*.nii.gz"))
        out.append(lx.sdcflows.FieldmapEstimation([lx.sdcflows.FieldmapFile(f) for f in files], registry=reg))  # fmt: skip
    return out


class _Lines(logging.Handler):
    def __init__(self) -> None:
        super().__init__(logging.DEBUG)
        self.records: list[logging.LogRecord] = []

    def emit(self, record: logging.LogRecord) -> None:
        self.records.append(record)


def _logger(name: str) -> tuple[logging.Logger, _Lines]:
    log = logging.getLogger(name)
    log.handlers.clear()
    log.setLevel(logging.DEBUG)
    log.propagate = False
    h = _Lines()
    log.addHandler(h)
    return log, h


def test_plan_pepolar_inputs_messages_and_options(tmp_path: Path) -> None:
    root = _dataset(tmp_path / "ds ~1")
    ests = _estimators(root, "good")
    log, h = _logger("t.plan")
    plan = le.plan_fieldmap_estimation(
        ests, output_dir=tmp_path / "out", subject="01", omp_nthreads=3, logger=log
    )
    assert [e.bids_id for e in plan.estimators] == ["pe_good"]
    assert plan.inputs["pe_good"]["in_data"] == [str(s.path) for s in ests[0].sources]
    assert plan.inputs["pe_good"]["metadata"] == [s.metadata for s in ests[0].sources]
    assert plan.preproc_options["out_path_base"] == ""
    assert plan.preproc_options["omp_nthreads"] == 3
    assert plan.preproc_options["use_metadata_estimates"] is False
    assert not plan.needs_anat and plan.precomputed_buffers == {}
    assert "A total of 1 fieldmaps" in plan.desc
    msgs = [r.getMessage() for r in h.records]
    assert msgs[0].startswith("B0 field inhomogeneity map will be estimated with the following 1")
    assert msgs[1].startswith('Setting up fieldmap "pe_good" (EstimatorType.PEPOLAR) with <')
    # 'estimated' reads EstimatedTotalReadoutTime; a number skips the check
    est = _estimators(root, "est")
    plan = le.plan_fieldmap_estimation(
        est, output_dir="/o", subject="01", fallback_total_readout_time="estimated"
    )
    assert plan.preproc_options["use_metadata_estimates"] is True
    plan = le.plan_fieldmap_estimation(
        est, output_dir="/o", subject="01", fallback_total_readout_time=1
    )
    assert plan.preproc_options["fallback_total_readout_time"] == 1.0
    with pytest.raises(ValueError, match="'estimated'"):
        le.plan_fieldmap_estimation(
            est, output_dir="/o", subject="01", fallback_total_readout_time="soon"
        )


def test_plan_missing_readout_exits_65(tmp_path: Path) -> None:
    root = _dataset(tmp_path / "ds")
    ests = _estimators(root, "good", "est")
    log, h = _logger("t.exit")
    with pytest.raises(SystemExit) as info:
        le.plan_fieldmap_estimation(ests, output_dir="/o", subject="01", logger=log)
    assert info.value.code == le.EX_DATAERR == 65
    crit = [r for r in h.records if r.levelno == logging.CRITICAL]
    assert crit[0].getMessage() == (
        "Missing readout time information. See documentation for `--fallback-total-readout-time`."
    )
    assert "Missing readout timing information for <" in str(crit[0].exc_info[1])  # type: ignore[index]
    with pytest.raises(RuntimeError, match="Missing readout timing information"):
        le.plan_fieldmap_estimation(
            _estimators(root, "est"), output_dir="/o", subject="01", exit_on_missing_readout=False
        )


def test_plan_sophisticated_pepolar(tmp_path: Path) -> None:
    root = _dataset(tmp_path / "ds")
    ests = _estimators(root, "soph")
    assert sorted(s.suffix for s in ests[0].sources) == ["epi", "epi", "sbref"]
    with pytest.raises(NotImplementedError, match="Sophisticated PEPOLAR schemes"):
        le.plan_fieldmap_estimation(ests, output_dir="/o", subject="01")


def test_precomputed_and_pipe31(tmp_path: Path) -> None:
    root = _dataset(tmp_path / "ds")
    ests = _estimators(root, "good", "est")
    deriv = tmp_path / "deriv"
    (deriv / "sub-01" / "fmap").mkdir(parents=True)
    (deriv / "dataset_description.json").write_text(
        '{"Name": "d", "BIDSVersion": "1.8.0", "DatasetType": "derivative"}', encoding="utf-8"
    )
    for desc in ("preproc", "coeff"):
        _put(deriv / "sub-01" / "fmap" / f"sub-01_fmapid-pegood_desc-{desc}_fieldmap.nii.gz", (3, 3, 3))  # fmt: skip
    log, h = _logger("t.pre")
    cache = le.precomputed_fieldmaps(deriv, "01", logger=log)
    assert list(cache) == ["pegood"] and sorted(cache["pegood"]) == ["coeffs", "fieldmap"]
    assert h.records[0].getMessage().startswith(f"Detected precomputed fieldmaps in {deriv}")
    with pytest.raises(ValueError, match="PIPE-31"):
        le.plan_fieldmap_estimation(
            ests, cache, output_dir="/o", subject="01", fallback_total_readout_time=0.05
        )
    with pytest.raises(KeyError, match="magnitude"):
        le.plan_fieldmap_estimation(
            ests, cache, output_dir="/o", subject="01", fallback_total_readout_time=0.05,
            upstream_defects="reproduce",
        )  # fmt: skip
    _put(deriv / "sub-01" / "fmap" / "sub-01_fmapid-pegood_desc-epi_fieldmap.nii.gz", (3, 3, 3))
    cache = le.precomputed_fieldmaps({"a": deriv}, "01")
    plan = le.plan_fieldmap_estimation(
        ests, cache, output_dir="/o", subject="01", fallback_total_readout_time=0.05
    )
    assert [e.bids_id for e in plan.estimators] == ["pe_est"]
    assert plan.precomputed_buffers["fmap_id"] == ["pe_good"]
    assert plan.precomputed_buffers["fmap_mask"] == ["MISSING"]
    assert plan.precomputed_buffers["sdc_method"] == ["precomputed"]
    # every field map precomputed: nothing is estimated, the buffers hold the derivatives
    only = le.plan_fieldmap_estimation(ests[:1], cache, output_dir="/o", subject="01")
    assert only.estimators == () and only.desc == ""
    res = only.run(work_dir=tmp_path / "work")
    assert res.preproc is None and list(res.estimates.fmap_id) == ["pe_good"]
    assert list(res.estimates.fmap_coeff) == [cache["pegood"]["coeffs"]]


def test_merge_buffers_semantics() -> None:
    pre = {
        "fmap": ["p/f.nii.gz"], "fmap_ref": ["p/r.nii.gz"], "fmap_coeff": [["p/c0", "p/c1"]],
        "fmap_mask": ["MISSING"], "fmap_id": ["a"], "sdc_method": ["precomputed"],
    }  # fmt: skip
    run = SimpleNamespace(
        fmap=("f2",), fmap_ref=("r2",), fmap_coeff=(("c2",),), fmap_mask=("m2",),
        fmap_id=("b",), method=("PEB",),
    )  # fmt: skip
    est = le.merge_buffers(pre, run)
    assert list(est.fmap_coeff) == [["p/c0", "p/c1"], ["c2"]]
    assert list(est.fmap_id) == ["a", "b"] and list(est.sdc_method) == ["precomputed", "PEB"]
    empty = le.merge_buffers({}, None)
    assert list(empty.fmap_id) == [] and list(empty.fmap) == []


def test_run_needs_the_anatomical_inputs_for_syn() -> None:
    call = le.SynPreprocessingCall("auto_00000", "syn_preprocessing_auto_00000", ("b.nii",), ({},), {})  # fmt: skip
    plan = le.FieldmapEstimationPlan((), {}, {}, {}, (), {}, {}, (call,))
    assert plan.needs_anat
    with pytest.raises(ValueError, match="anatomical inputs"):
        plan.run(work_dir="/nowhere", t1w_preproc="t1.nii.gz")


def test_qwarp_steps(tmp_path: Path) -> None:
    f4 = tmp_path / "ser ~ies_epi.nii.gz"
    lx.save((np.arange(4 * 4 * 3 * 5, dtype=np.int16).reshape(4, 4, 3, 5), AFF), str(f4))
    res = lx.sdcflows.flatten(f4, {"PhaseEncodingDirection": "j"}, 2, out_dir=tmp_path / "o")
    assert [Path(p).name for p in res.out_data] == [
        "ser ~ies_epi_idx-000.nii.gz",
        "ser ~ies_epi_idx-001.nii.gz",
    ]
    np.testing.assert_array_equal(np.asarray(lx.io.load(res.out_data[1]).data), np.arange(240).reshape(4, 4, 3, 5)[..., 1])  # fmt: skip
    with pytest.raises(FileNotFoundError):
        lx.sdcflows.flatten([tmp_path / "none.nii.gz"], [{}])
    with pytest.raises(ValueError, match="not enough values"):
        lx.sdcflows.flatten([], [])
    files, args = lx.sdcflows.sorted_pe(
        [("a", {"PhaseEncodingDirection": "i"}), ("b", {"PhaseEncodingDirection": "i-"})]
    )
    assert files == [["a"], ["b"]] and args == "-noYdis -noZdis"
    with pytest.raises(ValueError, match="orthogonal"):
        lx.sdcflows.sorted_pe(
            [("a", {"PhaseEncodingDirection": "j"}), ("b", {"PhaseEncodingDirection": "k"})]
        )
    warp = tmp_path / "w_WARP.nii.gz"
    lx.save((np.ones((5, 4, 1, 1, 3), np.float32), AFF), str(warp))
    with pytest.warns(UpstreamDefectWarning, match="SDC-24"):
        fixed = lx.sdcflows.qwarp2ants(warp, out_dir=tmp_path / "q")
    assert Path(fixed).name == "_warpfieldw_WARP.nii.gz"
    hdr = lx.io.load(fixed).header
    assert lx.io.load(fixed).shape == (5, 4, 1, 1, 3) and int(hdr.intent_code) == 1007
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        same = lx.sdcflows.qwarp2ants(warp, out_dir=tmp_path / "r", upstream_defects="reproduce")
    assert lx.io.load(same).shape == (5, 4, 1, 3)
    with pytest.raises(NotImplementedError, match="3dQwarp"):
        lx.sdcflows.qwarp_workflow([str(f4)], [{}])
