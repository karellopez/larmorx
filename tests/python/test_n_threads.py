# SPDX-License-Identifier: Apache-2.0
"""Every call of a program takes ``n_threads`` (CLAUDE.md rule 5: "explicit ``n_threads`` on
every call"): the families that wrap programs (ANTs, AFNI, FSL, the head-motion correction,
tedana) have it on every public function but their few helpers, which run no program. The
wrappers that gained it in the documentation revision (FLIRT's other programs and scripts,
``fslroi``, ``standard_space_roi``, ImageMath's ``D``, ``antsApplyTransformsToPoints``) give
the same result at every count and refuse a negative one."""

from __future__ import annotations

import inspect

import numpy as np
import pytest

import larmorx as lx
from larmorx.transforms import ItkTransform

pytestmark = pytest.mark.usefixtures("no_replica")

#: Public functions of these families that run no program: readers of a file, option sets and
#: lists of operations.
HELPERS = {
    "fsl": {"read_vtk"},
    "ants": {"image_math_operations"},
    "afni": {"fmriprep_slice_timing", "fmriprep_unifize_options", "fmriprep_volreg_options"},
    "mri": {"read_mats"},
    "tedana": set(),
}

#: The wrappers that gained ``n_threads`` in the documentation revision (2026-11-13).
ADDED = (
    "fsl.convert_xfm",
    "fsl.avscale",
    "fsl.img2imgcoord",
    "fsl.img2stdcoord",
    "fsl.std2imgcoord",
    "fsl.applyxfm4d",
    "fsl.makerot",
    "fsl.midtrans",
    "fsl.rmsdiff",
    "fsl.pointflirt",
    "fsl.aff2rigid",
    "fsl.extracttxt",
    "fsl.pairreg",
    "fsl.fslroi",
    "fsl.standard_space_roi",
    "ants.apply_transforms_to_points",
    "ants.distance_map",
)


def _function(name: str):
    family, function = name.split(".")
    return getattr(getattr(lx, family), function)


@pytest.mark.parametrize("family", sorted(HELPERS))
def test_every_program_call_takes_n_threads(family):
    module = getattr(lx, family)
    missing = sorted(
        name
        for name in module.__all__
        if inspect.isfunction(getattr(module, name))
        and "n_threads" not in inspect.signature(getattr(module, name)).parameters
    )
    assert missing == sorted(HELPERS[family])


@pytest.mark.parametrize("name", ADDED)
def test_n_threads_is_a_keyword_with_default_one(name):
    parameter = inspect.signature(_function(name)).parameters["n_threads"]
    assert parameter.kind is inspect.Parameter.KEYWORD_ONLY
    assert parameter.default == 1
    assert "n_threads" in (_function(name).__doc__ or "")


def _affine() -> np.ndarray:
    m = np.eye(4)
    m[:3, :3] = [[1.1, 0.05, 0.0], [-0.04, 0.95, 0.02], [0.0, 0.03, 1.02]]
    m[:3, 3] = [2.0, -3.0, 1.5]
    return m


def _itk_affine() -> ItkTransform:
    m = _affine()
    params = np.concatenate([m[:3, :3].ravel(), m[:3, 3]])
    return ItkTransform("AffineTransform_double_3_3", params, np.zeros(3))


def _image() -> lx.Image:
    data = (np.arange(8 * 7 * 6) % 13).astype(np.float32).reshape((8, 7, 6), order="F")
    return lx.Image(data, np.diag([2.0, 2.0, 2.0, 1.0]))


@pytest.mark.parametrize(
    "call",
    [
        lambda n: lx.fsl.convert_xfm(_affine(), inverse=True, n_threads=n),
        lambda n: lx.fsl.makerot(30.0, axis=(0.0, 1.0, 1.0), n_threads=n),
        lambda n: lx.fsl.avscale(_affine(), n_threads=n).scales,
        lambda n: lx.fsl.rmsdiff(_affine(), np.eye(4), _image(), n_threads=n),
        lambda n: lx.fsl.img2imgcoord([[1.0, 2.0, 3.0]], _image(), _image(), n_threads=n),
        lambda n: np.asarray(lx.fsl.fslroi(_image(), x=(1, 4), n_threads=n).data),
        lambda n: np.asarray(lx.ants.distance_map(_image(), n_threads=n).data),
        lambda n: lx.ants.apply_transforms_to_points(
            [[3.0, 0.0, 1.0]], [_itk_affine()], n_threads=n
        ),
    ],
    ids=[
        "convert_xfm",
        "makerot",
        "avscale",
        "rmsdiff",
        "img2imgcoord",
        "fslroi",
        "distance_map",
        "apply_transforms_to_points",
    ],
)
def test_the_result_does_not_depend_on_n_threads(call):
    one = call(1)
    for n in (0, 3):
        np.testing.assert_array_equal(call(n), one)


@pytest.mark.parametrize(
    "call",
    [
        lambda: lx.fsl.convert_xfm(np.eye(4), n_threads=-1),
        lambda: lx.fsl.extracttxt("x", "missing.txt", n_threads=-1),
        lambda: lx.fsl.fslroi(_image(), x=(0, 2), n_threads=-1),
        lambda: lx.ants.apply_transforms_to_points([[0.0, 0.0, 0.0]], n_threads=-1),
    ],
    ids=["convert_xfm", "extracttxt", "fslroi", "apply_transforms_to_points"],
)
def test_a_negative_count_is_refused(call):
    with pytest.raises(ValueError, match="n_threads must be 0"):
        call()
