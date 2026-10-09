"""Parity of larmorx's reading of NIfTI geometry *as ITK does it* with ANTsPy.

ANTs works in ITK physical space, and ITK chooses between a header's qform and sform with
rules that differ from nibabel's. For every NIfTI/Analyze file of the catalog, larmorx's
``NiftiHeader.itk_geometry`` must give the number of dimensions, size, spacing, origin and
direction cosines that ITK 5.4.5 reads (``ants.image_header_info``: ITK's ImageIO, without
ANTsPy's restrictions on pixel types and dimensions).
"""

from __future__ import annotations

import contextlib
import io
import os

import larmorx_testdata as td
import numpy as np

import larmorx as lx
from larmorx_validation.parity import compare
from larmorx_validation.parity.harness import BOTH_ERROR, EXPECTED, Case, CheckList, Outcome, Suite
from larmorx_validation.parity.nifti_io import NIFTI_SUFFIXES, _category, _try

ATOL = 1e-9


def cases(tier: str) -> list[Case]:
    out = []
    for entry in td.select(tier=tier, tags={"nifti"}):
        if "pair-data" in entry.tags or not entry.name.endswith(NIFTI_SUFFIXES):
            continue
        out.append(Case(entry.path, _category(entry), entry.description, entry))
    return sorted(out, key=lambda c: (c.category, c.id))


@contextlib.contextmanager
def _quiet_stderr():
    """ITK prints warnings straight to stderr (file descriptor 2); silence them."""
    devnull = os.open(os.devnull, os.O_WRONLY)
    saved = os.dup(2)
    try:
        os.dup2(devnull, 2)
        with contextlib.redirect_stderr(io.StringIO()):
            yield
    finally:
        os.dup2(saved, 2)
        os.close(saved)
        os.close(devnull)


def _from_ants_image(path) -> dict:
    import ants

    with _quiet_stderr():
        img = ants.image_read(str(path))
    return {
        "ndim": img.dimension,
        "size": tuple(int(n) for n in img.shape),
        "spacing": img.spacing,
        "origin": img.origin,
        "direction": np.asarray(img.direction),
    }


def _from_simpleitk(path) -> dict:
    import SimpleITK as sitk

    reader = sitk.ImageFileReader()
    reader.SetFileName(str(path))
    with _quiet_stderr():
        reader.ReadImageInformation()
    n = reader.GetDimension()
    spacing = np.array(reader.GetSpacing())
    direction = np.array(reader.GetDirection()).reshape(n, n)
    # ReadImageInformation reports the ImageIO's values; ITK's ImageFileReader (what ANTs uses)
    # then makes negative spacings positive by flipping the direction column.
    negative = spacing < 0
    spacing[negative] *= -1
    direction[:, negative] *= -1
    return {
        "ndim": n,
        "size": tuple(reader.GetSize()),
        "spacing": tuple(spacing),
        "origin": reader.GetOrigin(),
        "direction": direction,
    }


def _from_ants_header_info(path) -> dict:
    import ants

    with _quiet_stderr():
        h = ants.image_header_info(str(path))
    return {
        "ndim": int(h["nDimensions"]),
        "size": tuple(int(n) for n in h["dimensions"]),
        "spacing": h["spacing"],
        "origin": h["origin"],
        # image_header_info returns the direction transposed and rounded to 4 decimals.
        "direction": np.asarray(h["direction"]).T,
    }


#: Oracles in order of preference: (name, reader, tolerance). ANTsPy's image_read is ITK 5.4.5
#: exactly but refuses some pixel types and dimensions; SimpleITK reads the header with ITK 5.4
#: for any pixel type in 2 to 5 dimensions; image_header_info covers the rest, rounded.
ORACLES = (
    ("ANTsPy image_read", _from_ants_image, ATOL),
    ("SimpleITK ReadImageInformation", _from_simpleitk, ATOL),
    ("ANTsPy image_header_info (4 decimals)", _from_ants_header_info, 1e-4),
)


def _oracle(path):
    errors = []
    for name, read, tol in ORACLES:
        geometry, err = _try(read, path)
        if err is None:
            return name, geometry, tol, errors
        errors.append(f"{name}: {err!r}")
    return None, None, None, errors


#: Deliberate differences, with the reason reported for each.
EXPECTED_DIVERGENCES = {
    "synthetic/nifti/layouts/gzip-content-named-nii.nii": "larmorx detects gzip from the file content; ITK from the file name (and fails)",
    "synthetic/nifti/malformed/extension-bad-size.nii": "larmorx (like nibabel) rejects an extension smaller than its 8-byte header; nifti_clib skips it",
}


def run_case(case: Case, checks: CheckList) -> None:
    path = td.get(case.payload)
    if case.id in EXPECTED_DIVERGENCES:
        raise Outcome(EXPECTED, EXPECTED_DIVERGENCES[case.id])
    name, theirs, tol, errors = _oracle(path)
    ours, our_err = _try(lambda: lx.io.read_header(path).itk_geometry)
    if theirs is None and our_err is not None:
        raise Outcome(BOTH_ERROR, f"ITK: {'; '.join(errors)}; larmorx: {our_err!r}")
    if theirs is None:
        checks(
            "larmorx rejects what ITK rejects",
            False,
            detail=f"ITK: {'; '.join(errors)}; larmorx: source {ours.source}",
        )
        return
    if our_err is not None:
        checks(f"larmorx reads what ITK reads [{name}]", False, detail=f"larmorx: {our_err!r}")
        return
    tag = f" [{name}]"
    checks.add(compare.equal("dimensions" + tag, ours.ndim, theirs["ndim"]))
    checks.add(compare.equal("size" + tag, tuple(ours.size), theirs["size"]))
    checks.add(
        compare.close("spacing" + tag, np.array(ours.spacing), np.array(theirs["spacing"]), tol)
    )
    checks.add(
        compare.close("origin (LPS)" + tag, np.array(ours.origin), np.array(theirs["origin"]), tol)
    )
    checks.add(compare.close("direction cosines" + tag, ours.direction, theirs["direction"], tol))


def suite() -> Suite:
    import ants

    return Suite(
        name="itk-geometry",
        title="NIfTI geometry as ITK reads it",
        tool="larmorx.io.NiftiHeader.itk_geometry (crate larmorx-io, module nifti::itk)",
        reference=f"ITK 5.4.5 through ANTsPy {ants.__version__} (falling back to SimpleITK's ITK 5.4 for pixel types and dimensions ANTsPy refuses)",
        thresholds=[
            ("Number of dimensions, size", "identical"),
            (
                "Spacing, origin (LPS), direction cosines",
                f"max |diff| ≤ {ATOL:g} (1e-4 where only the rounded ANTsPy header summary is available)",
            ),
        ],
        cases=cases,
        run_case=run_case,
        packages=("antspyx",),
        notes=(
            "ITK picks the sform only when it is orthonormal and either the qform is unset, the "
            "sform code is SCANNER_ANAT, or the two transforms agree to 1e-4; otherwise it uses the "
            "qform, and with neither it uses origin 0 and identity directions. ITK 5.4 does not "
            "read NIfTI-2, so ANTs rejects those files."
        ),
    )
