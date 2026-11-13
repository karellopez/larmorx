# SPDX-License-Identifier: Apache-2.0
"""larmorprepx's confounds workflow of a BOLD run (``larmorx.pipelines.larmorprepx.confounds``):
fMRIPrep's small helpers and the workflow's argument checks. The workflow itself is checked
against fMRIPrep's graph and the 36 oracle runs' files by
``validation/investigations/confounds/check_workflow.py``."""

from pathlib import Path

import pytest

from larmorx.pipelines.larmorprepx import confounds as pc


def test_select_cols_follows_pandas_header(tmp_path: Path) -> None:
    table = tmp_path / "confounds.tsv"
    table.write_text(
        "global_signal\ta_comp_cor_00\tt_comp_cor_00\tstd_dvars\tdvars\tcsf\tcsf\t\n"
        "1\t2\t3\t4\t5\t6\t7\t8\n",
        encoding="utf-8",
    )
    # pandas' C engine renames a repeated name and an empty one
    assert pc.select_cols(table) == ["global_signal", "dvars", "csf", "csf.1", "Unnamed: 7"]


def test_description_names_the_thresholds_and_the_grey_matter() -> None:
    text = pc.bold_confounds_description(0.5, 1.5)
    assert "a threshold of 0.5 mm FD or\n1.5 standardized DVARS" in text
    assert "thresholding the corresponding partial volume map at 0.05" in text
    fs = pc.bold_confounds_description(1, 2, freesurfer=True)
    assert "a threshold of 1 mm FD or\n2 standardized DVARS" in fs
    assert "FreeSurfer's *aseg* segmentation" in fs
    assert text.endswith("[@patriat_improved_2017].\n")


def test_constants() -> None:
    assert pc.SIGNALS_CLASS_LABELS == (
        "global_signal",
        "csf",
        "white_matter",
        "csf_wm",
        "tcompcor",
    )
    assert pc.MODEL_FORMULA == "(dd1(rps + wm + csf + gsr))^^2 + others"
    assert list(pc.MOTION_METADATA) == [
        "trans_x",
        "trans_y",
        "trans_z",
        "rot_x",
        "rot_y",
        "rot_z",
        "framewise_displacement",
    ]


def test_merge_rois_ravels_its_inputs() -> None:
    assert pc._ravel("a", ["b", "c"], ("d",)) == ["a", "b", "c", "d"]


def test_the_sink_needs_the_source_file(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="source_file is required"):
        pc.bold_confounds(
            "bold.nii.gz", "mask.nii.gz", "ref.nii.gz", "hmc.txt", 0, "t1w_mask.nii.gz",
            ["gm.nii.gz", "wm.nii.gz", "csf.nii.gz"], "coreg.txt", metadata={},
            out_dir=tmp_path, fmriprep_dir=tmp_path / "out",
        )  # fmt: skip
    with pytest.raises(ValueError, match="'fix' or 'reproduce'"):
        pc.bold_confounds(
            "bold.nii.gz", "mask.nii.gz", "ref.nii.gz", "hmc.txt", 0, "t1w_mask.nii.gz",
            ["gm.nii.gz", "wm.nii.gz", "csf.nii.gz"], "coreg.txt", metadata={},
            out_dir=tmp_path, upstream_defects="faithful",
        )  # fmt: skip


def test_ds_confounds_names_and_dismisses_the_echo(tmp_path: Path) -> None:
    table = tmp_path / "confounds_expansion_desc-motion_outliers.tsv"
    table.write_text("a\tb\n1\t2\n", encoding="utf-8")
    src = "/data/sub-01/ses-1/func/sub-01_ses-1_task-rest_echo-1_bold.nii.gz"
    out = pc.ds_confounds(table, {"b": {"Units": "mm"}, "a": {}}, src, tmp_path / "fmriprep")
    tsv = Path(out.out_file[0])
    assert tsv.relative_to(tmp_path / "fmriprep").as_posix() == (
        "sub-01/ses-1/func/sub-01_ses-1_task-rest_desc-confounds_timeseries.tsv"
    )
    assert tsv.read_bytes() == table.read_bytes()
    meta = Path(out.out_meta).read_text(encoding="utf-8")
    assert meta.index('"a"') < meta.index('"b"')  # keys sorted
    one = pc.ds_confounds(table, {}, src, tmp_path / "echo", echo_idx=[1])
    assert Path(one.out_file[0]).name == (
        "sub-01_ses-1_task-rest_echo-1_desc-confounds_timeseries.tsv"
    )
