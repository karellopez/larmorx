# SPDX-License-Identifier: Apache-2.0
"""nipype's command lines with file names that hold a blank (PIPE-80): nipype writes the files
inside ANTs' bracketed arguments (and a few others) unquoted, so the shell splits such a name;
larmorx quotes them by default, ``upstream_defects="reproduce"`` keeps nipype's line and its
failure. The node directory itself may hold a blank in both modes (larmorx's absolute output
names are quoted: nipype writes them relative to the node directory, one word)."""

from __future__ import annotations

import os
import warnings
from pathlib import Path

import nibabel as nib
import numpy as np
import pytest

from larmorx import UpstreamDefectWarning, _nipype

AFFINE = np.diag([2.0, 2.0, 2.0, 1.0])
#: An identity affine in ITK's text format (antsApplyTransforms reads it as a file).
IDENTITY = (
    "#Insight Transform File V1.0\n#Transform 0\nTransform: AffineTransform_double_3_3\n"
    "Parameters: 1 0 0 0 1 0 0 0 1 0 0 0\nFixedParameters: 0 0 0\n"
)


def _inputs(folder: Path) -> tuple[str, str]:
    folder.mkdir(parents=True, exist_ok=True)
    data = (np.arange(6 * 5 * 4, dtype=np.float32).reshape(6, 5, 4) % 7) * 10.0
    image = folder / "in.nii.gz"
    nib.Nifti1Image(data, AFFINE).to_filename(image)
    xfm = folder / "xfm.txt"
    xfm.write_text(IDENTITY, encoding="utf-8")
    return str(image), str(xfm)


def _pipe80(caught: list[warnings.WarningMessage]) -> list[UpstreamDefectWarning]:
    return [
        w.message
        for w in caught
        if isinstance(w.message, UpstreamDefectWarning) and w.message.entry == "PIPE-80"
    ]


def _apply(tmp: Path, inputs: Path, node: Path) -> _nipype.Command:
    image, xfm = _inputs(inputs)
    return _nipype.apply_transforms(
        cwd=node,
        input_image=image,
        reference_image=image,
        transforms=[xfm],
        invert_transform_flags=[True],
        interpolation="Linear",
    )


def test_the_line_nipype_writes_and_the_quoted_one(tmp_path: Path) -> None:
    folder = tmp_path / "a b"
    cmd = _apply(tmp_path, folder, tmp_path / "node")
    xfm = str(folder / "xfm.txt")
    # nipype's line: the bracketed transform unquoted, the input image quoted (a File trait)
    assert f"--transform [ {xfm}, 1 ]" in cmd.cmdline
    assert f"--input '{folder / 'in.nii.gz'}'" in cmd.cmdline
    assert f"--transform [ '{xfm}', 1 ]" in cmd.quoted_cmdline
    assert xfm + "," not in cmd.argv()
    assert xfm + "," in cmd.argv(quoted=True)
    # the default's words keep the name whole, with the notice
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        words = _nipype.words(cmd, "fix", run=True)
    assert xfm + "," in words
    (notice,) = _pipe80(caught)
    assert notice.mode == "fix" and xfm in notice.detail
    # only the names nipype's line splits: the input image is quoted by nipype itself
    assert str(folder / "in.nii.gz") not in notice.detail
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        words = _nipype.words(cmd, "reproduce", run=True)
    assert words == cmd.run_argv() and xfm + "," not in words
    (notice,) = _pipe80(caught)
    assert notice.mode == "reproduce"


def test_a_line_without_blanks_is_nipype_s(tmp_path: Path) -> None:
    cmd = _apply(tmp_path, tmp_path / "in", tmp_path / "node")
    assert cmd.quoted_args is None and cmd.quoted_run_args is None
    assert cmd.quoted_cmdline == cmd.cmdline
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        for mode in ("fix", "reproduce"):
            assert _nipype.words(cmd, mode, run=True) == cmd.run_argv()
    assert not _pipe80(caught)


def test_apply_transforms_below_a_folder_with_a_blank(tmp_path: Path) -> None:
    folder = tmp_path / "a b"
    cmd = _apply(tmp_path, folder, folder / "node")
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        _nipype.run(cmd, n_threads=1)
    out = Path(cmd.outputs["output_image"])
    assert out.parent == folder / "node" and out.is_file()
    np.testing.assert_array_equal(
        np.asarray(nib.load(out).dataobj), np.asarray(nib.load(folder / "in.nii.gz").dataobj)
    )
    assert [n.mode for n in _pipe80(caught)] == ["fix"]
    # nipype's line: antsApplyTransforms gets the transform's name without its blank
    out.unlink()
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        with pytest.raises(RuntimeError, match="antsApplyTransforms failed"):
            _nipype.run(cmd, n_threads=1, upstream_defects="reproduce")
    assert [n.mode for n in _pipe80(caught)] == ["reproduce"]
    assert not out.exists()


def test_only_the_node_directory_holds_a_blank(tmp_path: Path) -> None:
    """nipype writes the output name relative to the node directory; larmorx runs the program
    in this process with the name absolute, quoted: both modes run, no notice."""
    node = tmp_path / "work dir" / "node"
    cmd = _apply(tmp_path, tmp_path / "in", node)
    assert cmd.quoted_args is None and cmd.quoted_run_args is None
    assert str(node / "in_trans.nii.gz") in cmd.run_argv()
    for mode in ("fix", "reproduce"):
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            _nipype.run(cmd, n_threads=1, upstream_defects=mode)
        assert (node / "in_trans.nii.gz").is_file() and not _pipe80(caught)
        (node / "in_trans.nii.gz").unlink()


def test_registration_and_topup_lines(tmp_path: Path) -> None:
    folder = tmp_path / "a b"
    image, _ = _inputs(folder)
    reg = _nipype.registration(
        cwd=tmp_path / "node",
        fixed_image=image,
        moving_image=image,
        fixed_image_masks=[image],
        initial_moving_transform=[image],
        transforms=["Rigid"],
        transform_parameters=[(0.1,)],
        metric=["MI"],
        radius_or_number_of_bins=[32],
        number_of_iterations=[[10]],
        smoothing_sigmas=[[0.0]],
        shrink_factors=[[1]],
    )
    q = f"'{image}'"
    assert f"MI[ {image}, {image}, 1, 32 ]" in reg.cmdline
    assert f"MI[ {q}, {q}, 1, 32 ]" in reg.quoted_cmdline
    assert f"--masks [ {q}, NULL ]" in reg.quoted_cmdline
    assert f"--initial-moving-transform [ {q}, 0 ]" in reg.quoted_cmdline
    (folder / "topup").mkdir()  # nipype's TOPUP wants out_base's folder (absolute here)
    topup = _nipype.topup(
        cwd=folder / "topup",
        in_file=image,
        encoding_direction=["y", "y-"],
        readout_times=[0.05],
    )
    enc = str(folder / "topup" / "in_encfile.txt")
    assert f"--datain={enc}" in topup.cmdline
    assert f"--datain='{enc}'" in topup.quoted_cmdline


def test_the_notice_names_each_split_name_whole(tmp_path: Path) -> None:
    """A long folder (a deep working directory, Windows' and macOS' temporary folders): the
    notice lists the names nipype's line splits, each whole, not a cut text."""
    folder = tmp_path / ("x" * 40) / ("y" * 40) / "a b"
    cmd = _apply(tmp_path, folder, tmp_path / "node")
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        _nipype.words(cmd, "reproduce", run=True)
    (notice,) = _pipe80(caught)
    assert f"({folder / 'xfm.txt'}): nipype's line kept" in notice.detail


def test_windows_names_keep_their_backslashes() -> None:
    """On Windows a backslash separates a path's folders (no escape): a name needs quotes only
    for a blank, a quote or another character a shell reads, so a line whose names hold none
    is nipype's line; quoted names come back whole, drive letters and backslashes kept."""
    plain = r"C:\Users\runneradmin\AppData\Local\Temp\in\xfm.txt"
    for name in (plain, r"D:\data", r"\\server\share\sub-01\x.nii.gz", "in_trans.nii.gz"):
        assert _nipype._quote(name, windows=True) == name
        assert _nipype._split_line(f"[ {name}, 1 ]", windows=True) == ["[", name + ",", "1", "]"]
    for name in (r"C:\Users\John Smith\xfm.txt", r"C:\Users\O'Brien\x.nii", r"C:\a&b\x.nii"):
        quoted = _nipype._quote(name, windows=True)
        assert quoted != name
        assert _nipype._split_line(f"[ {quoted}, 1 ]", windows=True) == ["[", name + ",", "1", "]"]
    # a POSIX shell reads a backslash as an escape: quoted there
    assert _nipype._quote(plain, windows=False) == f"'{plain}'"


@pytest.mark.skipif(os.name == "nt", reason="a symbolic link needs a privilege on Windows")
def test_a_folder_reached_through_a_symbolic_link(tmp_path: Path) -> None:
    """macOS' temporary folders are reached through a link (/var is /private/var): the names
    as given, the run in both modes as for a real folder."""
    real = tmp_path / "private" / "var"
    real.mkdir(parents=True)
    link = tmp_path / "var"
    link.symlink_to(real, target_is_directory=True)
    folder = link / "a b"
    cmd = _apply(tmp_path, folder, folder / "node")
    xfm = str(folder / "xfm.txt")
    assert f"--transform [ '{xfm}', 1 ]" in cmd.quoted_cmdline
    assert cmd.outputs["output_image"] == str(folder / "node" / "in_trans.nii.gz")
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        _nipype.run(cmd, n_threads=1)
    (notice,) = _pipe80(caught)
    assert notice.mode == "fix" and xfm in notice.detail
    assert (real / "a b" / "node" / "in_trans.nii.gz").is_file()
    (real / "a b" / "node" / "in_trans.nii.gz").unlink()
    with warnings.catch_warnings(record=True):
        warnings.simplefilter("always")
        with pytest.raises(RuntimeError, match="antsApplyTransforms failed"):
            _nipype.run(cmd, n_threads=1, upstream_defects="reproduce")
    # a node folder alone with a blank, through the link: both modes run, no notice
    node = link / "work dir" / "node"
    cmd = _apply(tmp_path, link / "in", node)
    assert cmd.quoted_args is None and cmd.quoted_run_args is None
    for mode in ("fix", "reproduce"):
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            _nipype.run(cmd, n_threads=1, upstream_defects=mode)
        assert (real / "work dir" / "node" / "in_trans.nii.gz").is_file() and not _pipe80(caught)
        (node / "in_trans.nii.gz").unlink()
