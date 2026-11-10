# SPDX-License-Identifier: Apache-2.0
"""larmorprepx's field-map nodes of a BOLD run (``larmorx.pipelines.larmorprepx.fieldmaps``):
fMRIPrep's small helpers and PIPE-20 at the pipeline level. The nodes themselves are checked
against fMRIPrep's files by ``validation/investigations/sdcflows-fieldmap-b/check_bold_stage.py``
and on every platform by the golden case ``py.larmorprepx.fieldmaps``."""

from pathlib import Path

import pytest

from larmorx.pipelines.larmorprepx import fieldmaps as fm


def test_select_ref_prefers_the_first_single_band_reference() -> None:
    assert fm.select_ref(["sb1.nii.gz", "sb2.nii.gz"], "boldref.nii.gz") == "sb1.nii.gz"
    assert fm.select_ref([], "boldref.nii.gz") == "boldref.nii.gz"
    assert fm.select_ref(None, ["a.nii.gz", "b.nii.gz"]) == "a.nii.gz"
    assert fm.select_ref("sb.nii.gz", "boldref.nii.gz") == "sb.nii.gz"
    assert fm.select_ref([], Path("ref.nii.gz")) == "ref.nii.gz"
    with pytest.raises(IndexError):
        fm.select_ref([], [])


def test_gen_inverses() -> None:
    assert fm.gen_inverses(None) == [True]
    assert fm.gen_inverses([]) == [True]
    assert fm.gen_inverses("boldref2anat.txt") == [True, False]
    assert fm.gen_inverses(["boldref2anat.txt", "anat2std.h5"]) == [True, False, False]


def test_fieldmap_jacobian_follows_fmripreps_rule() -> None:
    assert fm.fieldmap_jacobian("PEPOLAR")
    for method in ("PHASEDIFF", "MAPPED", "ANAT"):
        assert not fm.fieldmap_jacobian(method)
        assert fm.fieldmap_jacobian(method, force=["fmap-jacobian"])
    assert not fm.fieldmap_jacobian("PEPOLAR", ignore=["fmap-jacobian"])
    assert not fm.fieldmap_jacobian(None, force=["fmap-jacobian"])
    with pytest.raises(ValueError, match="EstimatorType"):
        fm.fieldmap_jacobian("pepolar")


def test_a_missing_readout_time_stops_the_pipeline() -> None:
    """PIPE-20 (rule B at the pipeline level): the error comes before any file is read."""
    estimates = fm.FieldmapEstimates(
        fmap_id=["auto_00000"], fmap_ref=["ref.nii.gz"], fmap_coeff=[["coeff.nii.gz"]]
    )
    with pytest.raises(fm.FieldmapMetadataError, match="Missing readout time information"):
        fm.resampling_fieldmap(
            "auto_00000", estimates, "target.nii.gz", "xfm.txt", {"PhaseEncodingDirection": "j"}
        )
    with pytest.raises(fm.FieldmapMetadataError, match="phase-encoding direction"):
        fm.resampling_fieldmap(
            "auto_00000", estimates, "target.nii.gz", "xfm.txt", {"TotalReadoutTime": 0.05}
        )
    with pytest.raises(ValueError, match="not found in the index"):
        fm.resampling_fieldmap(
            "auto_00001", estimates, "target.nii.gz", "xfm.txt", {"TotalReadoutTime": 0.05}
        )
