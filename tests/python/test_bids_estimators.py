# SPDX-License-Identifier: Apache-2.0
"""lx.bids, sdcflows' estimators and fMRIPrep's selection on small generated datasets (the values
are pybids', sdcflows' and fMRIPrep's: validation/investigations/sdcflows-fieldmap-b/
check_estimators.py compares them on the oracle datasets)."""

from __future__ import annotations

import json
import re
import warnings
from pathlib import Path

import pytest

import larmorx as lx
from larmorx.pipelines.larmorprepx import estimators as le


def _write(path: Path, meta: dict | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"")
    if meta is not None:
        path.with_name(path.name.split(".")[0] + ".json").write_text(json.dumps(meta))


@pytest.fixture
def dataset(tmp_path: Path) -> Path:
    root = tmp_path / "bids"
    root.mkdir()
    (root / "dataset_description.json").write_text('{"Name": "x", "BIDSVersion": "1.8.0"}')
    (root / "task-rest_bold.json").write_text('{"RepetitionTime": 2, "Zeta": 9}')
    s = root / "sub-01"
    _write(s / "anat" / "sub-01_T1w.nii.gz")
    bold = {"PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05, "Zeta": 1}
    _write(s / "func" / "sub-01_task-rest_run-01_bold.nii.gz", bold)
    _write(s / "func" / "sub-01_task-rest_run-2_bold.nii.gz", bold)
    for d, pe in (("AP", "j-"), ("PA", "j")):
        _write(
            s / "fmap" / f"sub-01_dir-{d}_epi.nii.gz",
            {"PhaseEncodingDirection": pe, "TotalReadoutTime": 0.05, "IntendedFor": "func/sub-01_task-rest_run-01_bold.nii.gz"},
        )  # fmt: skip
    _write(
        s / "fmap" / "sub-01_phasediff.nii.gz",
        {
            "EchoTime1": 0.004,
            "EchoTime2": 0.006,
            "IntendedFor": ["func/sub-01_task-rest_run-2_bold.nii.gz"],
        },
    )
    _write(s / "fmap" / "sub-01_magnitude1.nii.gz")
    _write(root / "code" / "sub-01_T1w.nii.gz")
    _write(s / ".hidden_T1w.nii.gz")
    return root


def test_index_entities_metadata_and_queries(dataset: Path) -> None:
    lay = lx.bids.BIDSLayout(dataset, validate=False)
    files = lay.get(return_type="filename")
    assert not any("/code/" in f or ".hidden" in f for f in files)
    bold = lay.get(suffix="bold", run=1, extension=".nii.gz")[0]
    assert bold.filename == "sub-01_task-rest_run-01_bold.nii.gz"
    assert type(bold).__name__ == "BIDSImageFile"
    assert list(bold.get_entities()) == [
        "subject",
        "task",
        "run",
        "suffix",
        "extension",
        "datatype",
    ]
    md = bold.get_metadata()
    assert list(md) == ["PhaseEncodingDirection", "RepetitionTime", "TotalReadoutTime", "Zeta"]
    assert md["Zeta"] == 1 and type(md["RepetitionTime"]).__name__ == "PaddedInt"
    assert str(lay.get_runs()[0]) == "01"
    assert len(lay.get(run="1", extension=".nii.gz")) == 1  # run is an integer entity: 01 = 1
    assert lay.get(PhaseEncodingDirection="J") == []  # metadata compare as text
    assert len(lay.get(suffix="epi", IntendedFor=lx.bids.Query.REQUIRED)) == 2
    assert len(lay.get(suffix="bold", extension=".nii.gz", session=lx.bids.Query.NONE)) == 2
    assert lay.get_file(bold.relpath) is bold
    with pytest.raises(ValueError, match="'subjekt' is not a recognized entity"):
        lay.get(subjekt="01")
    with pytest.raises(lx.bids.BIDSEntityError):
        lay.get_B0FieldIdentifiers()
    with pytest.raises(lx.bids.TargetError):
        lay.get(return_type="id")


def test_estimators_and_registry(dataset: Path) -> None:
    lay = lx.bids.BIDSLayout(dataset, validate=False)
    reg = lx.sdcflows.FieldmapRegistry()
    found = lx.sdcflows.find_estimators(layout=lay, subject="01", registry=reg)
    assert [(e.method.name, e.bids_id, len(e.sources)) for e in found] == [
        ("PHASEDIFF", "auto_00000", 2),
        ("PEPOLAR", "auto_00001", 2),
    ]
    assert repr(found[0]) == (
        "FieldmapEstimation(sources=<2 files>, method=<EstimatorType.PHASEDIFF: 3>, bids_id='auto_00000')"
    )
    assert lx.sdcflows.get_identifier("func/sub-01_task-rest_run-2_bold.nii.gz", registry=reg) == (
        "auto_00000",
    )
    assert le.get_estimator(
        lay, str(dataset / "sub-01/func/sub-01_task-rest_run-01_bold.nii.gz"), registry=reg
    ) == ("auto_00001",)
    with pytest.raises(KeyError, match="'bogus'"):
        lx.sdcflows.get_identifier("x", "bogus", registry=reg)
    with pytest.raises(KeyError, match=re.escape("'{by}'")):
        lx.sdcflows.get_identifier("x", "bogus", registry=reg, upstream_defects="reproduce")
    with pytest.raises(ValueError, match="already"):
        lx.sdcflows.FieldmapEstimation(list(found[0].sources), registry=reg)
    lx.sdcflows.clear_registry(reg)
    assert not reg.estimators and not reg.intents


def test_bidict_add_takes_the_last_free_index() -> None:
    d = lx.sdcflows.bidict()
    d["auto_00001"] = "a"
    assert d.add("b") == "auto_00000"  # range(2): 0 free, 1 used
    d["auto_00005"] = "c"
    assert d.add("e") == "auto_00003"  # SDC-18: range(4) -> 2 and 3 free, the last wins
    assert d["e"] == "auto_00003"


def test_fieldmapfile_checks(dataset: Path) -> None:
    epi = dataset / "sub-01/fmap/sub-01_dir-AP_epi.nii.gz"
    f = lx.sdcflows.FieldmapFile(epi)
    assert f.suffix == "epi" and f.metadata["TotalReadoutTime"] == 0.05
    assert f.bids_root == dataset
    with pytest.raises(lx.sdcflows.MetadataError, match="PhaseEncodingDirection"):
        lx.sdcflows.FieldmapFile(epi, find_meta=False)
    with pytest.raises(FileNotFoundError):
        lx.sdcflows.FieldmapFile(dataset / "sub-01/fmap/sub-01_dir-XX_epi.nii.gz")
    t1 = lx.sdcflows.FieldmapFile(dataset / "sub-01/anat/sub-01_T1w.nii.gz")
    assert t1.metadata == {"TotalReadoutTime": 0.0}


def test_fmriprep_selection(dataset: Path) -> None:
    lay = le.fmriprep_layout(dataset, ["01"])
    bold = [[f.path] for f in lay.get(suffix="bold", extension=".nii.gz")]
    reg = lx.sdcflows.FieldmapRegistry()
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        ests, emap = le.map_fieldmap_estimation(
            lay, "01", None, bold, False, False, False, None, registry=reg
        )
    assert [e.bids_id for e in ests] == ["auto_00000", "auto_00001"]
    assert sorted(Path(k).name for k in emap) == [
        "sub-01_task-rest_run-01_bold.nii.gz",
        "sub-01_task-rest_run-2_bold.nii.gz",
    ]
    for e in ests:
        le.check_pepolar_scheme(e)
    assert le.setting_up_message(ests[0]) == (
        'Setting up fieldmap "auto_00000" (EstimatorType.PHASEDIFF) with '
        "<sub-01_phasediff.nii.gz, sub-01_magnitude1.nii.gz>"
    )
    assert le.map_fieldmap_estimation(lay, "01", None, bold, True, False, False, None) == ([], {})
