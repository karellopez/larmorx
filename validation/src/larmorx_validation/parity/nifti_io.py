"""Parity of NIfTI reading and writing with nibabel.

For every NIfTI file of the test-data catalog, larmorx and nibabel read the file and must
agree bit for bit on the data (as ``img.dataobj``, as ``get_fdata()`` in float64 and float32,
and unscaled), on the header fields after nibabel's load-time fixes, on the extensions, and
on the affine and qform/sform. Then larmorx writes the image back, and nibabel must read the
same data and affine; the header larmorx writes must match the one nibabel writes for the same
image.
"""

from __future__ import annotations

import contextlib
import gzip
import logging
import tempfile
import warnings
from pathlib import Path
from typing import Any

import larmorx_testdata as td
import numpy as np

import larmorx as lx
from larmorx_validation.parity import compare
from larmorx_validation.parity.harness import BOTH_ERROR, EXPECTED, Case, CheckList, Outcome, Suite

AFFINE_ATOL = 1e-9  # mm

NIFTI_SUFFIXES = (".nii", ".nii.gz", ".hdr", ".hdr.gz", ".HDR")

#: Deliberate differences from nibabel, with the reason reported for each.
EXPECTED_DIVERGENCES = {
    "synthetic/nifti/layouts/gzip-content-named-nii.nii": "larmorx detects gzip from the file content; nibabel from the file name (and fails)",
    "synthetic/nifti/layouts/plain-content-named-gz.nii.gz": "larmorx detects gzip from the file content; nibabel from the file name (and fails)",
    "synthetic/nifti/malformed/vox-offset-zero.nii": "larmorx rejects a single file with vox_offset 0; nibabel reads the header bytes as voxel data",
    "synthetic/nifti/dtypes/rgb24.nii": "RGB24 data are not supported by larmorx yet",
    "synthetic/nifti/dtypes/rgba32.nii": "RGBA32 data are not supported by larmorx yet",
}

#: Header fields compared after loading (vox_offset is excluded: nibabel resets it in memory).
FIELDS = (
    "magic",
    "dim_info",
    "dim",
    "intent_p1",
    "intent_p2",
    "intent_p3",
    "intent_code",
    "datatype",
    "bitpix",
    "slice_start",
    "pixdim",
    "scl_slope",
    "scl_inter",
    "slice_end",
    "slice_code",
    "xyzt_units",
    "cal_max",
    "cal_min",
    "slice_duration",
    "toffset",
    "descrip",
    "aux_file",
    "qform_code",
    "sform_code",
    "quatern_b",
    "quatern_c",
    "quatern_d",
    "qoffset_x",
    "qoffset_y",
    "qoffset_z",
    "srow_x",
    "srow_y",
    "srow_z",
    "intent_name",
    "data_type",
    "db_name",
    "extents",
    "session_error",
    "regular",
    "glmax",
    "glmin",
)


def _nib():
    import nibabel

    return nibabel


def _category(entry: td.FileEntry) -> str:
    parts = entry.path.split("/")
    if parts[0] == "synthetic":
        return f"synthetic/{parts[2]}"
    return entry.collection.id


def cases(tier: str) -> list[Case]:
    out = []
    for entry in td.select(tier=tier, tags={"nifti"}):
        if "pair-data" in entry.tags or not entry.name.endswith(NIFTI_SUFFIXES):
            continue
        out.append(Case(entry.path, _category(entry), entry.description, entry))
    return sorted(out, key=lambda c: (c.category, c.id))


def _quiet(fn, *args, **kwargs):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return fn(*args, **kwargs)


def _try(fn, *args, **kwargs) -> tuple[Any, Exception | None]:
    try:
        return _quiet(fn, *args, **kwargs), None
    except Exception as exc:  # the reference or larmorx rejecting the input
        return None, exc


def _nib_read(path: Path):
    nib = _nib()
    img = nib.load(path)
    data = np.asanyarray(img.dataobj)
    return img, data, img.affine


def _ours(value: Any) -> Any:
    if isinstance(value, bytes):
        return value.rstrip(b"\x00")
    return value


def _theirs(hdr, name: str) -> Any:
    value = hdr[name]
    if value.shape:
        return tuple(v.item() for v in value)
    value = value.item()
    return value.rstrip(b"\x00") if isinstance(value, bytes) else value


def _header_fields_check(checks: CheckList, ours, theirs, *, label: str = "header fields") -> None:
    mismatches = []
    names = [f for f in FIELDS if f in theirs]
    if label == "header as stored":
        names.append("vox_offset")
    for name in names:
        a = getattr(ours, name)
        if name == "regular":
            a = bytes([a])
        a, b = _ours(a), _theirs(theirs, name)
        if compare.equal(name, a, b).passed is False:
            mismatches.append(f"{name}: {a!r} != {b!r}")
    checks(f"{label} ({len(names)})", not mismatches, detail="; ".join(mismatches[:6]))


def _extensions(hdr) -> list[tuple[int, bytes]]:
    return [(int(e.get_code()), bytes(e.content).rstrip(b"\x00")) for e in hdr.extensions]


def _head(path: Path, n: int) -> bytes:
    """The first ``n`` bytes of the content, decompressing gzip (works on truncated streams)."""
    with path.open("rb") as f:
        gz = f.read(2) == b"\x1f\x8b"
    with gzip.open(path, "rb") if gz else path.open("rb") as f:
        return f.read(n)


def _decompressed(path: Path) -> bytes:
    raw = path.read_bytes()
    return gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw


def _compare_stored_headers(path: Path, checks: CheckList) -> bool:
    """Compare the header as stored (no fixes) with nibabel's parse of the same bytes. Returns
    whether they agree; adds no check if larmorx cannot parse a header at all."""
    nib = _nib()
    stored, err = _try(lx.io.read_header, path)
    if err is not None:
        return True
    size = 348 if stored.version == 1 else 540
    klass = nib.Nifti2Header if stored.version == 2 else nib.Nifti1Header
    theirs, err = _try(klass, _head(path, size), check=False)
    if err is not None:
        checks("header as stored", False, detail=f"larmorx parses it, nibabel does not: {err!r}")
        return False
    before = len(checks.checks)
    _header_fields_check(checks, stored, theirs, label="header as stored")
    return all(c.passed for c in checks.checks[before:])


def _compare_reads(
    path: Path, checks: CheckList, img: lx.Image, ref, ref_data: np.ndarray, ref_affine: np.ndarray
) -> None:
    checks.add(compare.identical("data, default (nibabel img.dataobj)", img.data, ref_data))

    for label, dtype in (
        ("float64 (get_fdata)", np.float64),
        ("float32 (get_fdata(float32))", np.float32),
    ):
        theirs, their_err = _try(ref.get_fdata, dtype=dtype)
        ours, our_err = _try(lx.load, path, dtype=dtype)
        if np.issubdtype(ref.get_data_dtype(), np.complexfloating):
            # Deliberate: nibabel drops the imaginary part (numpy's ComplexWarning); larmorx refuses.
            checks(
                f"data as {label}: complex data refused (nibabel drops the imaginary part)",
                our_err is not None,
            )
        elif their_err or our_err:
            checks(
                f"data as {label}",
                bool(their_err) == bool(our_err),
                detail=f"nibabel: {their_err!r}; larmorx: {our_err!r}",
            )
        else:
            checks.add(compare.identical(f"data as {label}", ours.data, theirs))

    mapped = lx.load(path, mmap=True)
    checks.add(compare.identical("data, memory-mapped (mmap=True)", mapped.data, ref_data))
    del mapped

    unscaled, err = _try(lambda: ref.dataobj.get_unscaled())
    if err is None:
        checks.add(
            compare.identical(
                "stored values (unscaled)", lx.load(path, scaled=False).data, unscaled
            )
        )

    checks.add(compare.close("affine", img.affine, ref_affine, AFFINE_ATOL))
    # nibabel resets the scale factors and vox_offset in a loaded image's header; its reading of
    # the header as stored is the header class applied to the raw block (with the same fixes).
    h = img.header
    size = 348 if h.version == 1 else 540
    rh = _quiet(type(ref.header), _head(path, size))
    rh.extensions = ref.header.extensions
    _header_fields_check(checks, h, rh)
    checks.add(
        compare.equal(
            "extensions", [(e.code, e.trimmed_content) for e in h.extensions], _extensions(rh)
        )
    )
    checks.add(compare.equal("shape", img.shape, ref.shape))
    checks.add(
        compare.equal(
            "zooms", tuple(float(z) for z in h.zooms), tuple(float(z) for z in rh.get_zooms())
        )
    )
    checks.add(compare.equal("on-disk data type", h.data_dtype, rh.get_data_dtype()))
    their_si, si_err = _try(rh.get_slope_inter)
    checks.add(
        compare.equal(
            "scale factors",
            h.slope_inter if si_err is None else "error",
            (None if their_si == (None, None) else their_si) if si_err is None else "error",
        )
    )
    their_q, q_err = _try(rh.get_qform)
    if q_err is None and h.qform is not None:
        checks.add(compare.close("qform matrix", h.qform, their_q, AFFINE_ATOL))
    else:
        checks(
            "qform matrix", (q_err is None) == (h.qform is not None), detail=f"nibabel: {q_err!r}"
        )
    checks.add(compare.close("sform matrix", h.sform, rh.get_sform(), AFFINE_ATOL))

    stored = lx.io.read_header(path)
    checks("header bytes re-serialised exactly", stored.to_bytes() == _head(path, size))


def _compare_writes(path: Path, checks: CheckList, img: lx.Image, ref) -> None:
    nib = _nib()
    suffix = "".join(path.suffixes[-2:]) if path.name.endswith(".gz") else path.suffix
    with tempfile.TemporaryDirectory() as tmp:
        ours_path = Path(tmp) / f"ours{suffix}"
        lx.save(img, ours_path, n_threads=2)
        back, err = _try(_nib_read, ours_path)
        if err is not None:
            checks("nibabel reads larmorx's file", False, detail=repr(err))
            return
        _, back_data, back_affine = back
        checks.add(
            compare.identical("round trip: nibabel reads the same data", back_data, img.data)
        )
        checks.add(
            compare.close(
                "round trip: nibabel reads the same affine", back_affine, img.affine, AFFINE_ATOL
            )
        )
        checks.add(
            compare.identical(
                "round trip: larmorx reads the same data", lx.load(ours_path).data, img.data
            )
        )

        # Re-saving an unscaled image: the header must be the one nibabel writes.
        if img.header is not None and img.header.slope_inter in (None, (1.0, 0.0)):
            theirs_path = Path(tmp) / f"theirs{suffix}"
            _quiet(nib.save, ref, theirs_path)
            ours_h, theirs_h = lx.io.read_header(ours_path), lx.io.read_header(theirs_path)
            diffs = [
                f"{k}: {a!r} != {b!r}"
                for k, a in ours_h.to_dict().items()
                if k not in ("scl_slope", "scl_inter")
                and not compare.equal(k, a, b := theirs_h.to_dict()[k]).passed
            ]
            checks(
                "re-save: header as nibabel writes it (scale factors aside)",
                not diffs,
                detail="; ".join(diffs[:6]),
            )

        # A fresh image (no header): the header nibabel writes for the same data and affine.
        data = np.asarray(img.data)
        if data.dtype.kind in "iufc" and img.header is not None and img.header.version == 1:
            fresh_ours, fresh_theirs = Path(tmp) / "fresh_ours.nii", Path(tmp) / "fresh_theirs.nii"
            lx.save(lx.Image(data, img.affine), fresh_ours)
            _, err = _try(
                nib.save, nib.Nifti1Image(data, img.affine, dtype=data.dtype), fresh_theirs
            )
            if err is None:
                a, b = (
                    lx.io.read_header(fresh_ours).to_dict(),
                    lx.io.read_header(fresh_theirs).to_dict(),
                )
                quat = ("quatern_b", "quatern_c", "quatern_d")
                diffs = [
                    f"{k}: {a[k]!r} != {b[k]!r}"
                    for k in a
                    if k not in quat and not compare.equal(k, a[k], b[k]).passed
                ]
                checks(
                    "fresh image: header as nibabel writes it",
                    not diffs,
                    detail="; ".join(diffs[:6]),
                )
                # q and -q are the same rotation; for half turns (w = 0) nibabel's sign comes from
                # LAPACK's eigenvectors, so compare the decoded qform matrices, not the parameters.
                qa, qb = lx.io.read_header(fresh_ours).qform, lx.io.read_header(fresh_theirs).qform
                checks.add(compare.close("fresh image: same qform as nibabel's", qa, qb, 1e-5))
                checks(
                    "fresh image: data bytes as nibabel writes them",
                    _decompressed(fresh_ours)[352:] == _decompressed(fresh_theirs)[352:],
                )


@contextlib.contextmanager
def _quiet_logs():
    """Silence the header-fix messages both libraries log (each file is read several times)."""
    loggers = [logging.getLogger("larmorx"), logging.getLogger("nibabel")]
    levels = [lg.level for lg in loggers]
    for lg in loggers:
        lg.setLevel(logging.CRITICAL)
    try:
        yield
    finally:
        for lg, level in zip(loggers, levels, strict=True):
            lg.setLevel(level)


def run_case(case: Case, checks: CheckList) -> None:
    with _quiet_logs():
        _run_case(case, checks)


def _run_case(case: Case, checks: CheckList) -> None:
    entry: td.FileEntry = case.payload
    path = td.get(entry)
    ref_result, ref_err = _try(_nib_read, path)
    img, lx_err = _try(lx.load, path)

    if case.id in EXPECTED_DIVERGENCES:
        sides = f"larmorx: {'reads it' if lx_err is None else repr(lx_err)}; nibabel: {'reads it' if ref_err is None else repr(ref_err)}"
        raise Outcome(EXPECTED, f"{EXPECTED_DIVERGENCES[case.id]} ({sides})")
    if ref_err is not None and lx_err is not None:
        # Agreeing to reject is not enough: where the header can be parsed, both must parse it
        # the same way (e.g. header-only files, or files whose data are truncated).
        if _compare_stored_headers(path, checks):
            raise Outcome(BOTH_ERROR, f"nibabel: {ref_err!r}; larmorx: {lx_err!r}")
        return
    if ref_err is not None:
        checks("larmorx rejects what nibabel rejects", False, detail=f"nibabel: {ref_err!r}")
        return
    if lx_err is not None:
        checks("larmorx reads what nibabel reads", False, detail=f"larmorx: {lx_err!r}")
        return
    ref, ref_data, ref_affine = ref_result
    _compare_reads(path, checks, img, ref, ref_data, ref_affine)
    _compare_writes(path, checks, img, ref)


def suite() -> Suite:
    nib = _nib()
    return Suite(
        name="nifti-io",
        title="NIfTI reading and writing",
        tool="larmorx.io.load / larmorx.io.save (crate larmorx-io)",
        reference=f"nibabel {nib.__version__}",
        thresholds=[
            (
                "Voxel data (all five read modes, including memory-mapped)",
                "bit-identical: same dtype, same values, NaN = NaN, -0.0 ≠ +0.0",
            ),
            ("Header fields, extensions, shape, zooms, scale factors", "identical"),
            ("Affine, qform and sform matrices", f"max |diff| ≤ {AFFINE_ATOL:g} mm"),
            ("Written files read back by nibabel", "bit-identical data; affine ≤ 1e-9 mm"),
            (
                "Header written for a fresh image",
                "identical to nibabel's; qform matrix ≤ 1e-5 (float32 storage)",
            ),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        notes=(
            "- nibabel computes the qform in `numpy.longdouble`, which is 80-bit on Linux x86-64 and "
            "64-bit elsewhere; larmorx uses f64 everywhere. The affine threshold covers that difference.\n"
            "- larmorx returns arrays in native byte order; nibabel keeps the file's byte order. Values "
            "are compared exactly; the byte order of the dtype is not.\n"
            "- For complex data, `get_fdata` in nibabel silently drops the imaginary part; larmorx "
            "raises instead (checked as such).\n"
            "- Expected divergences are deliberate and listed with their reason."
        ),
    )
