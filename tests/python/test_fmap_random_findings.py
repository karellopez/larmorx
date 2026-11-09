# SPDX-License-Identifier: Apache-2.0
"""What random testing of the field maps' part B found (relay 27: the difftest tools
``ndimage-skimage``, ``niworkflows-helpers`` and ``sdcflows-estimation``), each a permanent
check here: SciPy's iterated binary morphology writes into the caller's ``int8`` mask (SCI-31),
sdcflows' ``brain_masker`` with ``padding=0`` writes empty images (SDC-46), and larmorx's
mistakes fixed (``GetReadoutTime``'s and ``DistortionParameters``' output checks, the random
walker's mode warnings, ``UniformGrid`` of a 4-D image, ``brain_masker``'s ``IndexError``,
``match_histogram``'s numpy message, ``DenoiseImage``'s listed outputs). Every expected text
and value is the original's (SciPy 1.18.1 and 1.15.3, scikit-image 0.26.0, sdcflows 2.17.1,
nipype 1.12.0, numpy 2.5.4 and 2.3.5)."""

from __future__ import annotations

import warnings

import numpy as np
import pytest

import larmorx as lx
from larmorx import ndimage as nd


def entries(fn, *args, **kwargs):
    with warnings.catch_warnings(record=True) as record:
        warnings.simplefilter("always")
        out = fn(*args, **kwargs)
    ids = [w.message.entry for w in record if isinstance(w.message, lx.UpstreamDefectWarning)]
    return out, ids, [str(w.message) for w in record]


def _save(path, data, affine=None):
    lx.save((np.asfortranarray(data), np.eye(4) if affine is None else affine), path)
    return str(path)


def test_sci31_the_callers_int8_mask():
    a = np.array([1, 1], bool)
    # SciPy: binary_erosion(a, iterations=2, mask=m) leaves m = [1, -1]
    m = np.array([0, 1], np.int8)
    out, ids, _ = entries(nd.binary_erosion, a, iterations=2, mask=m)
    assert ids == ["SCI-31"] and out.tolist() == [True, False] and m.tolist() == [0, 1]
    out, ids, _ = entries(nd.binary_erosion, a, iterations=2, mask=m, upstream_defects="reproduce")
    assert ids == ["SCI-31"] and out.tolist() == [True, False] and m.tolist() == [1, -1]
    # binary_closing's erosion reads the mask its dilation overwrote: SciPy gives [0, 0]
    b = np.array([0, 1], bool)
    m = np.array([0, 0], np.int8)
    out, ids, _ = entries(nd.binary_closing, b, iterations=2, mask=m)
    assert set(ids) == {"SCI-31"} and out.tolist() == [False, True] and m.tolist() == [0, 0]
    out, ids, _ = entries(nd.binary_closing, b, iterations=2, mask=m, upstream_defects="reproduce")
    assert "SCI-31" in ids and out.tolist() == [False, False] and m.tolist() == [0, -1]
    # another mask type is SciPy's copy (no notice); one pass reads the mask only
    m8 = np.array([0, 1], np.uint8)
    _, ids, _ = entries(nd.binary_erosion, a, iterations=2, mask=m8, upstream_defects="reproduce")
    assert ids == [] and m8.tolist() == [0, 1]
    m = np.array([0, 1], np.int8)
    _, ids, _ = entries(nd.binary_erosion, a, iterations=1, mask=m, upstream_defects="reproduce")
    assert ids == [] and m.tolist() == [0, 1]


def _head(shape=(24, 24, 24)) -> np.ndarray:
    z, y, x = np.indices(shape, dtype=np.float64)
    c = [(s - 1) / 2 for s in shape]
    r2 = (x - c[2]) ** 2 + (y - c[1]) ** 2 + ((z - c[0]) * 1.1) ** 2
    rng = np.random.default_rng(46)
    head = np.where(r2 < 49, 120.0, np.where(r2 < 81, 60.0, 4.0))
    return (head + rng.integers(0, 6, shape)).astype(np.float32)


def test_sdc46_brain_masker_without_padding(tmp_path):
    import nibabel as nb

    f = _save(tmp_path / "clipped.nii.gz", _head())
    r, ids, _ = entries(lx.sdcflows.brain_masker, f, str(tmp_path / "a_mask.nii.gz"), 0)
    assert "SDC-46" in ids
    assert np.asarray(r.probseg.data).shape == (24, 24, 24)
    assert nb.load(r.out_mask).shape == (24, 24, 24)
    r, ids, _ = entries(
        lx.sdcflows.brain_masker,
        f,
        str(tmp_path / "b_mask.nii.gz"),
        0,
        upstream_defects="reproduce",
    )
    assert "SDC-46" in ids
    assert nb.load(r.out_mask).shape == (0, 0, 0) and nb.load(r.out_probseg).shape == (0, 0, 0)
    # the brain is written unmasked
    assert np.asarray(nb.load(r.out_brain).dataobj).tobytes() == _head().tobytes()
    # padding 5: no notice
    _, ids, _ = entries(lx.sdcflows.brain_masker, f, str(tmp_path / "c_mask.nii.gz"))
    assert "SDC-46" not in ids


def test_brain_masker_without_positive_values_is_numpys_index_error(tmp_path):
    f = _save(tmp_path / "zeros.nii.gz", np.zeros((8, 8, 8), np.float32))
    with pytest.raises(IndexError, match=r"^index -1 is out of bounds for axis 0 with size 0$"):
        lx.sdcflows.brain_masker(f, str(tmp_path / "m.nii.gz"))


def test_get_readout_time_checks_its_outputs_as_nipype(tmp_path):
    from larmorx._nipype import TraitError

    f = _save(tmp_path / "epi.nii.gz", np.zeros((4, 4, 4), np.int16))
    with pytest.raises(TraitError) as e:
        lx.sdcflows.get_readout_time({"TotalReadoutTime": "0.05", "PhaseEncodingDirection": "j"}, f)
    assert str(e.value) == (
        "The 'readout_time' trait of a _GetReadoutTimeOutputSpec instance must be a float, but a "
        "value of '0.05' <class 'str'> was specified."
    )
    with pytest.raises(TraitError) as e:
        lx.sdcflows.get_readout_time({"TotalReadoutTime": 0.05, "PhaseEncodingDirection": "j+"}, f)
    assert str(e.value) == (
        "The 'pe_direction' trait of a _GetReadoutTimeOutputSpec instance must be 'i' or 'i-' or "
        "'j' or 'j-' or 'k' or 'k-', but a value of 'j+' <class 'str'> was specified."
    )
    r = lx.sdcflows.get_readout_time({"TotalReadoutTime": True, "PhaseEncodingDirection": "k-"}, f)
    assert (r.readout_time, r.pe_direction, r.pe_dir_fsl) == (1.0, "k-", "z-")
    # fMRIPrep's DistortionParameters: the same check of its own output spec
    with pytest.raises(TraitError, match="DistortionParametersOutputSpec instance must be 'i'"):
        lx.transforms.distortion_parameters(
            {"TotalReadoutTime": 0.05, "PhaseEncodingDirection": "x"}
        )


def test_random_walker_names_its_mode_only_with_a_system_to_solve():
    data = np.arange(30, dtype=np.float32).reshape(5, 6)
    labels = np.ones((5, 6), np.int32)
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        out = lx.skimage.random_walker(data, labels, mode="cg_mg")
    assert out.tolist() == labels.tolist()
    assert [str(x.message) for x in w] == [
        "Random walker only segments unlabeled areas, where labels == 0. No zero valued areas in "
        "labels were found. Returning provided labels."
    ]
    labels[0, 0], labels[4, 5] = 0, 2
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        lx.skimage.random_walker(data, labels, mode="cg_mg")
    assert '"cg_mg" not available' in str(w[0].message)


def test_uniform_grid_of_a_4d_image_off_the_grid_is_scipys_error(tmp_path):
    ref = _save(tmp_path / "ref.nii.gz", np.ones((5, 5, 5), np.float32))
    mov = _save(tmp_path / "mov.nii.gz", np.ones((5, 5, 5, 2), np.float32), np.diag([2.0, 2, 2, 1]))
    with pytest.raises(RuntimeError, match=r"^invalid shape for coordinate array$"):
        lx.sdcflows.uniform_grid([ref, mov], out_dir=tmp_path / "out")


def test_match_histogram_gives_numpys_message_with_nibabels_dtype(tmp_path):
    img = _save(tmp_path / "img.nii.gz", np.arange(27, dtype=np.uint8).reshape(3, 3, 3))
    ref = _save(tmp_path / "ref.nii.gz", np.full((3, 3, 3), 0.5, np.float32))
    with pytest.raises(TypeError) as e:
        lx.sdcflows.match_histogram(ref, img, out_dir=tmp_path / "o", upstream_defects="reproduce")
    assert str(e.value) == (
        "Cannot cast array data from dtype('<f4') to dtype('int64') according to the rule 'safe'"
    )


def test_pipe38_denoise_image_lists_its_noise_image_as_nipype(tmp_path):
    from larmorx import _nipype

    (tmp_path / "in.nii.gz").write_bytes(b"")
    cmd = _nipype.denoise_image(cwd=tmp_path, input_image=str(tmp_path / "in.nii.gz"))
    # nipype lists every name_source output (rule C, kept): the file the program does not write
    assert cmd.outputs["noise_image"] == str(tmp_path / "in_noise.nii.gz")
    assert "[" not in cmd.cmdline
