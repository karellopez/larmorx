# SPDX-License-Identifier: Apache-2.0
"""niworkflows' ``DerivativesDataSink``: a derivative file named and stored as BIDS-Derivatives
say, with its JSON sidecar.

Ported from niworkflows 1.15.4 (Apache-2.0, the NiPreps developers; the same in 1.15.2,
fMRIPrep 25.2.6's image, and 1.15.3): ``interfaces/bids.py`` (``DerivativesDataSink``,
``DEFAULT_DTYPES``, ``_get_tf_resolution``), ``utils/misc.py`` (``_copy_any``),
``utils/images.py`` (``unsafe_write_nifti_header_and_data``, ``set_consumables``),
``utils/bids.py`` (``relative_to_root``, :func:`larmorx.sdcflows.relative_to_root`) and
``data/nipreps.json`` (the derivatives' entities and path patterns,
:data:`larmorx.bids.NIPREPS_CONFIG`); with pybids 0.22.0's ``parse_file_entities`` and
``build_path`` (:mod:`larmorx.bids`), nipype 1.12.0's ``copyfile`` (a hard link, else a copy,
with the related ``.mat``/``.img``/``.HEAD`` files) and nibabel 5.4.2's header setters and
writer (``set_qform``, ``set_sform``, ``set_xyzt_units``, ``set_zooms``, ``set_data_dtype``,
``set_slope_inter``, ``write_to``, ``array_to_file``).

The files are the bytes niworkflows writes: the same header block, extensions, padding and data,
gzip-compressed by Python's ``gzip`` at level 9 with no name and a zero time stamp (the
compressed bytes are the platform's zlib's; the contents are the same everywhere). TemplateFlow
is not a dependency: :data:`STANDARD_SPACES` is TemplateFlow 25.1.2's list of templates (both
kept environments'), and a ``resolution`` entity of a standard space needs the template's
metadata (``template_metadata``; the templates part of larmorx brings TemplateFlow's).

Corrected by default (``upstream_defects="reproduce"`` gives niworkflows' behaviour):

- PIPE-26: with two or more list-valued entity inputs the files are matched to the names in the
  order of a Python ``set`` of the entity names (it changes between processes); here the names
  are in the sorted order of those entities (never reproduced: nondeterminism).
- PIPE-27: the warning about a changed data type names the original type twice; here the new
  type is named.
- PIPE-28: ``compress`` with fewer values than ``in_file`` (but more than one) fails with
  Python's ``IndexError``; here a ``ValueError`` says so.
- PIPE-29: the data type coercion casts out-of-range values and NaN to an integer type by
  numpy's cast (undefined in C; on x86-64 a wrapped value); here they are clamped to the type's
  range (NaN to 0) with a warning, the rule for undefined conversions.
- PIPE-30: a ``RepetitionTime`` input for an image that is not 4-D makes the header fix set four
  voxel sizes on it, and nibabel's ``HeaderDataError`` stops the sink; here the fourth voxel
  size is set only on an image that has a fourth axis (the sidecar keeps the value).
"""

from __future__ import annotations

import gzip
import json
import logging
import os
import re
import shutil
from collections import defaultdict
from collections.abc import Collection, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName
from larmorx.io.nibabel_writer import HeaderDataError

__all__ = [
    "BIDS_DERIV_ENTITIES",
    "BIDS_DERIV_PATTERNS",
    "DEFAULT_DTYPES",
    "STANDARD_SPACES",
    "STATIC_INPUTS",
    "DerivativesDataSinkResult",
    "copy_any",
    "derivatives_data_sink",
    "get_tf_resolution",
]

_LOGGER = logging.getLogger("larmorx.niworkflows")

regz = re.compile(r"\.gz$")


def _nipreps_spec() -> dict[str, Any]:
    from larmorx.bids import NIPREPS_CONFIG

    with open(NIPREPS_CONFIG, encoding="utf-8") as f:
        return json.load(f)  # type: ignore[no-any-return]


_SPEC = _nipreps_spec()
#: niworkflows' ``nipreps.json`` entities (the derivatives' entities, in its order).
BIDS_DERIV_ENTITIES: tuple[dict[str, Any], ...] = tuple(_SPEC["entities"])
#: niworkflows' ``nipreps.json`` path patterns.
BIDS_DERIV_PATTERNS: tuple[str, ...] = tuple(_SPEC["default_path_patterns"])
_CONFIG_ENTITIES = frozenset(e["name"] for e in BIDS_DERIV_ENTITIES)

#: TemplateFlow 25.1.2's templates (``templateflow.api.templates()``), niworkflows'
#: ``STANDARD_SPACES``: a ``space`` entity among them gets the xform codes 4 (else 2).
STANDARD_SPACES: tuple[str, ...] = (
    "Fischer344", "MNI152Lin", "MNI152NLin2009aAsym", "MNI152NLin2009aSym",
    "MNI152NLin2009bAsym", "MNI152NLin2009bSym", "MNI152NLin2009cAsym", "MNI152NLin2009cSym",
    "MNI152NLin6Asym", "MNI152NLin6Sym", "MNI305", "MNIColin27", "MNIInfant",
    "MNIPediatricAsym", "MouseIn", "NKI", "NMT31Sym", "OASIS30ANTs", "PNC", "RESILIENT", "SUIT",
    "UNCInfant", "VALiDATe29", "WHS", "dhcpAsym", "dhcpSym", "dhcpVol", "fsLR", "fsaverage",
    "onavg",
)  # fmt: skip

#: The data types the sink coerces a suffix's files to (niworkflows' ``DEFAULT_DTYPES``).
DEFAULT_DTYPES: defaultdict[str | None, str | None] = defaultdict(
    lambda: None,
    (("mask", "uint8"), ("dseg", "int16"), ("probseg", "float32"), ("boldref", "float32")),
)

#: The interface's own inputs (not entities, not metadata).
STATIC_INPUTS = frozenset(
    {
        "base_directory",
        "check_hdr",
        "compress",
        "data_dtype",
        "dismiss_entities",
        "in_file",
        "meta_dict",
        "source_file",
    }
)

# nibabel's ``unit_codes``
_UNIT_CODES = {
    "unknown": 0, "meter": 1, "mm": 2, "micron": 3, "sec": 8, "msec": 16, "usec": 24,
    "hz": 32, "ppm": 40, "rads": 48,
}  # fmt: skip
_UNIT_LABELS = {v: k for k, v in _UNIT_CODES.items()}
# nibabel's NIfTI data type codes
_DTYPE_CODES = {
    "uint8": 2, "int16": 4, "int32": 8, "float32": 16, "complex64": 32, "float64": 64,
    "int8": 256, "uint16": 512, "uint32": 768, "int64": 1024, "uint64": 1280,
    "complex128": 1792,
}  # fmt: skip
_SINGLE_MAGIC = {1: b"n+1\x00", 2: b"n+2\x00"}


@dataclass(frozen=True)
class DerivativesDataSinkResult:
    """``DerivativesDataSink``'s outputs: the files written (``out_file``), the JSON sidecar
    (``out_meta``, written for a single file with metadata), whether each file is compressed
    (``compression``) and whether its header was fixed (``fixed_hdr``)."""

    out_file: tuple[str, ...]
    out_meta: str | None
    compression: tuple[bool, ...]
    fixed_hdr: tuple[bool, ...]

    @property
    def out(self) -> str | list[str]:
        """``out_file`` as nipype passes it on: the path for one file, else the list."""
        return self.out_file[0] if len(self.out_file) == 1 else list(self.out_file)


def _listify(obj: Any) -> list[Any]:
    from larmorx.bids import listify

    out = listify(obj)
    return [] if out is None else list(out)


def get_tf_resolution(
    space: str,
    resolution: str,
    template_metadata: Mapping[str, Any],
    templateflow_version: str,
) -> str:
    """niworkflows' ``_get_tf_resolution``: the sidecar's ``Resolution`` text for a template's
    resolution (``'01'`` and ``'1'`` alike), from the template's metadata (its
    ``template_description.json``: ``res`` -> ``zooms``); ``'Unknown'`` for a resolution it does
    not list."""
    resolutions = template_metadata.get("res", {})
    res_meta = None
    padded_res = f"{resolution:0>2}"
    for r in (resolution, padded_res):
        if r in resolutions:
            res_meta = resolutions[r]
    if res_meta is None:
        return "Unknown"
    xyz = "x".join([str(c) for c in res_meta["zooms"]])
    return f"Template {space} ({xyz} mm^3), curated by TemplateFlow {templateflow_version}"


def _gzip_writer(fobj: Any) -> gzip.GzipFile:
    """niworkflows' gzip stream: level 9, no file name, time stamp 0."""
    return gzip.GzipFile("", "wb", 9, fobj, 0.0)


def copy_any(src: str | os.PathLike[str], dst: str | os.PathLike[str]) -> bool:
    """niworkflows' ``_copy_any``: ``src`` copied to ``dst``, compressed or decompressed when
    one name ends with ``.gz`` (gzip level 9, no name, time stamp 0), else by nipype's
    ``copyfile`` (a hard link, else a copy, with the related files). ``True`` when the bytes were
    streamed."""
    src, dst = os.fspath(src), os.fspath(dst)
    src_isgz = src.endswith(".gz")
    dst_isgz = dst.endswith(".gz")
    if not src_isgz and not dst_isgz:
        from larmorx._nipype import copyfile

        copyfile(src, dst)
        return False
    if os.path.exists(dst):
        os.unlink(dst)
    src_open = gzip.open if src_isgz else open
    with src_open(src, "rb") as f_in, open(dst, "wb") as f_out:  # type: ignore[operator]
        if dst_isgz:
            gz_out = _gzip_writer(f_out)
            shutil.copyfileobj(f_in, gz_out)
            gz_out.close()
        else:
            shutil.copyfileobj(f_in, f_out)
    return True


def _nifti_version(path: str) -> int | None:
    """The NIfTI version of the image nibabel's ``load`` makes of ``path`` (1 or 2), or ``None``
    when it is not a ``Nifti1Image``: another extension, a header nibabel does not recognise
    (its ``ImageFileError``, which the sink ignores) or a CIFTI-2 file (``Cifti2Image``: a NIfTI-2
    header with an intent code from 3000 to 3099). NIfTI-1 is recognised by its magic (``ni1`` or
    ``n+1``), NIfTI-2 by its header size."""
    lower = path.lower()
    if lower.endswith((".nii.bz2", ".nii.zst")):
        raise NotImplementedError(
            f"{path}: nibabel reads .nii.bz2 and .nii.zst files; larmorx's reader does not"
        )
    if not lower.endswith((".nii", ".nii.gz")):
        return None
    opener = gzip.open if lower.endswith(".gz") else open
    with opener(path, "rb") as f:  # type: ignore[operator]
        block = f.read(540)
    if len(block) >= 348 and block[344:348].rstrip(b"\x00") in (b"ni1", b"n+1"):
        return 1
    if len(block) >= 540 and 540 in (
        int.from_bytes(block[:4], "little"),
        int.from_bytes(block[:4], "big"),
    ):
        intent = int(_core.nifti_parse_header(bytes(block))["intent_code"])
        return None if 3000 <= intent < 3100 else 2
    return None


def _image_header(path: str) -> dict[str, Any]:
    """The header ``nb.load(path).header`` holds: the stored fields with nibabel's load-time fixes
    and the single-file magic (``update_header``)."""
    fields, _fixes = _core.nifti_read_header(path, fix=True)
    fields["magic"] = _SINGLE_MAGIC[int(fields["version"])]
    return fields  # type: ignore[no-any-return]


def _header_dtype(fields: dict[str, Any]) -> np.dtype:
    info = _core.nifti_header_info(fields)
    order = "<" if fields["byte_order"] == "<" else ">"
    return np.dtype(info["dtype"]).newbyteorder(order)


def _xyzt_units(fields: dict[str, Any]) -> tuple[str, str]:
    code = int(fields["xyzt_units"])
    xyz = code % 8
    return _UNIT_LABELS[xyz], _UNIT_LABELS[code - xyz]


def _zooms(fields: dict[str, Any]) -> tuple[Any, ...]:
    dim = list(fields["dim"])
    ndim = int(dim[0])
    if ndim == 0:
        return (1.0,)
    pixdim = np.asarray(fields["pixdim"], dtype=np.float32)
    return tuple(pixdim[1 : ndim + 1])


def _set_zooms(fields: dict[str, Any], zooms: Sequence[Any]) -> None:
    ndim = int(next(iter(fields["dim"])))
    z = np.asarray(zooms)
    if len(z) != ndim:
        raise HeaderDataError(f"Expecting {ndim} zoom values for ndim {ndim}")
    if np.any(z < 0):
        raise HeaderDataError("zooms must be positive")
    pixdim = np.asarray(fields["pixdim"], dtype=np.float32).copy()
    pixdim[1 : ndim + 1] = z[:]
    fields["pixdim"] = [float(v) for v in pixdim]


def _set_slope_inter(fields: dict[str, Any], slope: float, inter: float) -> None:
    if slope in (0, np.inf, -np.inf):
        raise HeaderDataError("Slope cannot be 0 or infinite")
    if inter in (np.inf, -np.inf):
        raise HeaderDataError("Intercept cannot be infinite")
    if np.isnan(slope) ^ np.isnan(inter):
        raise HeaderDataError("None or both of slope, inter should be nan")
    fields["scl_slope"] = float(np.float32(slope))
    fields["scl_inter"] = float(np.float32(inter))


def _slope_inter(fields: dict[str, Any]) -> tuple[float, float]:
    """nibabel's ``ArrayProxy`` scaling of a header: ``get_slope_inter`` with ``None`` as 1 and
    0."""
    slope = float(fields["scl_slope"])
    inter = float(fields["scl_inter"])
    if slope == 0 or not np.isfinite(slope):
        return 1.0, 0.0
    if not np.isfinite(inter):
        raise HeaderDataError(f"Valid slope but invalid intercept {inter}")
    return slope, inter


def _set_data_dtype(fields: dict[str, Any], dtype: np.dtype) -> None:
    name = dtype.newbyteorder("=").name
    if name not in _DTYPE_CODES:
        raise ValueError(f"data dtype {dtype} not supported by NIfTI")
    fields["datatype"] = _DTYPE_CODES[name]
    fields["bitpix"] = dtype.itemsize * 8


def _header_stream(fields: dict[str, Any]) -> bytes:
    """nibabel's ``Nifti1Header.write_to`` of a single file: the header block, the extension flag
    and the extensions (each padded to 16 bytes)."""
    version = int(fields["version"])
    single_offset = 352 if version == 1 else 544
    order = "little" if fields["byte_order"] == "<" else "big"
    exts = b""
    for code, content in fields["extensions"]:
        size = -(-(8 + len(content)) // 16) * 16
        exts += size.to_bytes(4, order, signed=True) + int(code).to_bytes(4, order, signed=True)
        exts += bytes(content) + b"\x00" * (size - 8 - len(content))
    vox_offset = float(fields["vox_offset"])
    min_vox_offset = single_offset + len(exts)
    if vox_offset == 0:
        fields["vox_offset"] = float(min_vox_offset)
    elif vox_offset < min_vox_offset:
        raise HeaderDataError(f"vox offset set to {vox_offset}, but need at least {min_vox_offset}")
    block = _core.nifti_header_info(fields)["header_bytes"]
    flag = b"\x01\x00\x00\x00" if fields["extensions"] else b"\x00\x00\x00\x00"
    return bytes(block) + flag + exts


def _write_nifti(path: str, fields: dict[str, Any], data_bytes: bytes) -> None:
    """niworkflows' ``unsafe_write_nifti_header_and_data``: the header, zeros up to the data
    offset (``array_to_file``'s seek), the data; gzip-compressed for a ``.gz`` name."""
    head = _header_stream(fields)
    offset = int(float(fields["vox_offset"]))
    stream = head + b"\x00" * max(0, offset - len(head))
    if len(head) > offset:
        stream = head  # nibabel's seek backwards then fails: never reached (checked above)
    with open(path, "wb") as fobj:
        if path.endswith(".gz"):
            gz = _gzip_writer(fobj)
            gz.write(stream)
            gz.write(data_bytes)
            gz.close()
        else:
            fobj.write(stream)
            fobj.write(data_bytes)


def _raw_data(path: str, fields: dict[str, Any]) -> bytes:
    """The stored voxel bytes (``dataobj.get_unscaled()`` as ``array_to_file`` writes it back):
    the data block at the header's offset, in the file's own byte order."""
    info = _core.nifti_header_info(fields)
    nbytes = int(np.prod(info["shape"], dtype=np.int64)) * np.dtype(info["dtype"]).itemsize
    offset = int(float(fields["vox_offset"]))
    opener = gzip.open if path.lower().endswith(".gz") else open
    with opener(path, "rb") as f:  # type: ignore[operator]
        f.seek(offset)
        data = f.read(nbytes)
    if len(data) < nbytes:
        raise OSError(f"{path}: expected {nbytes} bytes of data, the file has {len(data)}")
    return data  # type: ignore[no-any-return]


def _coerced(
    path: str,
    fields: dict[str, Any],
    data_dtype: np.dtype,
    upstream_defects: str,
    n_threads: int,
) -> np.ndarray:
    """``np.rint(nii.dataobj).astype(dtype)`` (integer types) or ``np.asanyarray(nii.dataobj,
    dtype=dtype)``: the scaled data in the new type; PIPE-29 for undefined integer casts."""
    from larmorx.io.nibabel_header import load_nibabel

    data = np.asanyarray(load_nibabel(path, n_threads=n_threads).data)
    if not np.issubdtype(data_dtype, np.integer):
        return np.asanyarray(data, dtype=data_dtype)
    rounded = np.rint(data)
    if rounded.dtype.kind in "iu" or upstream_defects == "reproduce":
        return rounded.astype(data_dtype)
    info = np.iinfo(data_dtype)
    bad = np.isnan(rounded) | (rounded < info.min) | (rounded > info.max)
    if bad.any():
        _defects.warn(
            [
                (
                    "PIPE-29",
                    "niworkflows",
                    "fix",
                    f"DerivativesDataSink's cast of {path} to {data_dtype} has {int(bad.sum())} "
                    "values out of the type's range or NaN, whose integer conversion is "
                    "undefined; they were clamped to the range (NaN to 0)",
                )
            ],
            stacklevel=4,
        )
        rounded = np.clip(np.nan_to_num(rounded, nan=0.0), info.min, info.max)
    return rounded.astype(data_dtype)


def derivatives_data_sink(
    in_file: str | os.PathLike[str] | Sequence[str | os.PathLike[str]],
    source_file: str | os.PathLike[str] | Sequence[str | os.PathLike[str]],
    *,
    base_directory: str | os.PathLike[str] | None = None,
    out_path_base: str = "niworkflows",
    check_hdr: bool = True,
    compress: bool | Sequence[bool | None] | None = (),
    data_dtype: str | None = None,
    dismiss_entities: str | Sequence[str] | None = (),
    meta_dict: Mapping[str, Any] | None = None,
    allowed_entities: Collection[str] = (),
    entities: Mapping[str, Any] | None = None,
    metadata: Mapping[str, Any] | None = None,
    init_metadata: Mapping[str, Any] | None = None,
    standard_spaces: Collection[str] = STANDARD_SPACES,
    template_metadata: Mapping[str, Mapping[str, Any]] | None = None,
    templateflow_version: str | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> DerivativesDataSinkResult:
    """niworkflows' ``DerivativesDataSink``: ``in_file`` (one or several files) stored under
    ``base_directory/out_path_base`` with a BIDS-Derivatives name made from the entities of
    ``source_file`` (those all sources share, less ``dismiss_entities``), overridden by
    ``entities`` (the interface's entity inputs: ``desc``, ``suffix``, ``datatype``, ``space``,
    ... and ``allowed_entities``' own) and the input file's extension (``compress``: ``True``
    gzip, ``False`` plain, ``None`` as it is), by the first of niworkflows' ``nipreps.json``
    patterns that matches.

    A NIfTI file's header is fixed (``check_hdr``) when its xform codes, units or voxel sizes are
    not a derivative's (codes 1, or 4/2 with a ``space``; ``mm``; ``sec`` for ``bold``; the
    ``RepetitionTime`` input as the fourth voxel size), and its data are converted to
    ``data_dtype`` (``'source'``: the first source's type; by default the suffix's
    :data:`DEFAULT_DTYPES`); otherwise the file is copied (:func:`copy_any`). For one file, its
    metadata go into a JSON sidecar (keys sorted, two-space indents): ``meta_dict``, then the
    constructor's metadata (``init_metadata``), then the metadata inputs (``metadata``), each
    winning over the one before; a ``Resolution`` text for a template's ``resolution``.

    ``base_directory`` defaults to the current directory (nipype's node directory);
    ``out_path_base`` is the class's (``'niworkflows'``; sdcflows' subclass ``'sdcflows'``,
    fMRIPrep ``''``). PIPE-26 to PIPE-30 (module docstring) follow ``upstream_defects``.
    ``n_threads`` never changes a result."""
    from larmorx.bids import Config, parse_file_entities
    from larmorx.bids.writing import build_path
    from larmorx.io.nibabel_header import nibabel_affine, set_xform
    from larmorx.io.nifti import NiftiHeader
    from larmorx.sdcflows.estimators import relative_to_root

    _defects.check(upstream_defects)
    entities = dict(entities or {})
    allowed = set(allowed_entities).union(_CONFIG_ENTITIES)
    for key in entities:
        if key not in allowed:
            raise ValueError(
                f"{key!r} is not an entity of niworkflows' nipreps.json nor of "
                "allowed_entities: pass it in metadata"
            )
    meta_inputs = dict(metadata or {})
    init_meta = dict(init_metadata or {})
    for key in (*meta_inputs, *init_meta):
        if key in STATIC_INPUTS or key in allowed:
            raise ValueError(f"{key!r} is an input or an entity of the sink, not metadata")

    in_files = [os.fspath(f) for f in _listify(in_file)]
    if not in_files:
        raise ValueError("in_file is mandatory")
    for f in in_files:
        if not os.path.exists(f):
            raise FileNotFoundError(f"in_file: {f} does not exist")
    sources = [os.fspath(f) for f in _listify(source_file)]
    if not sources:
        raise ValueError("source_file is mandatory")

    base = Path(os.getcwd() if base_directory is None else os.fspath(base_directory)).absolute()
    out_path = base / out_path_base
    out_path.mkdir(exist_ok=True, parents=True)

    meta: dict[str, Any] = {}
    if meta_dict is not None:
        meta = dict(meta_dict)
    meta.update(init_meta)

    custom_config = Config(
        name="custom",
        entities=BIDS_DERIV_ENTITIES,
        default_path_patterns=BIDS_DERIV_PATTERNS,
    )
    in_entities = [
        parse_file_entities(str(relative_to_root(s)), config=["bids", "derivatives", custom_config])
        for s in sources
    ]
    out_entities: dict[str, Any] = {
        k: v for k, v in in_entities[0].items() if all(ent.get(k) == v for ent in in_entities[1:])
    }
    for drop_entity in _listify(dismiss_entities or []):
        out_entities.pop(drop_entity, None)

    out_entities["extension"] = ["".join(Path(f).suffixes).lstrip(".") for f in in_files]
    comp = _listify(compress) or [None]
    if len(comp) == 1:
        comp = comp * len(in_files)
    if len(comp) < len(in_files):
        if upstream_defects == "reproduce":
            raise IndexError("list index out of range")
        raise ValueError(
            f"compress has {len(comp)} values for {len(in_files)} input files (PIPE-28: "
            "niworkflows fails with an IndexError)"
        )
    for i, ext in enumerate(out_entities["extension"]):
        if comp[i] is not None:
            ext = regz.sub("", ext)
            out_entities["extension"][i] = f"{ext}.gz" if comp[i] else ext

    # Entity inputs override (niworkflows iterates a set of the names: PIPE-26)
    for key in sorted(allowed):
        value = entities.get(key)
        if value is not None:
            out_entities[key] = value

    if out_entities.get("resolution") == "native" and out_entities.get("space"):
        out_entities.pop("resolution", None)

    resolution = out_entities.get("resolution")
    space = out_entities.get("space")
    if resolution:
        if space in standard_spaces:
            if template_metadata is None or space not in template_metadata:
                raise NotImplementedError(
                    f"the Resolution of template {space}'s resolution {resolution!r} needs the "
                    "template's metadata (template_metadata, templateflow_version): TemplateFlow "
                    "comes with larmorx's templates part"
                )
            res = get_tf_resolution(
                space, str(resolution), template_metadata[space], str(templateflow_version)
            )
        else:
            res = "Unknown"
        meta["Resolution"] = res

    if len(set(out_entities["extension"])) == 1:
        out_entities["extension"] = out_entities["extension"][0]

    custom_entities = set(out_entities) - set(_CONFIG_ENTITIES)
    patterns: Sequence[str] = BIDS_DERIV_PATTERNS
    if custom_entities:
        custom_pat = "_".join(f"{key}-{{{key}}}" for key in sorted(custom_entities))
        patterns = [pat.replace("_{suffix", f"_{custom_pat}_{{suffix") for pat in patterns]

    list_keys = [k for k, v in entities.items() if isinstance(v, (list, tuple))]
    if len(list_keys) > 1:
        _defects.warn(
            [
                (
                    "PIPE-26",
                    "niworkflows",
                    "fix",
                    f"the list-valued entities {sorted(list_keys)} are combined in their sorted "
                    "order (niworkflows: the order of a Python set, which changes between "
                    "processes)",
                )
            ],
            stacklevel=2,
        )

    dest_files = build_path(out_entities, path_patterns=patterns)
    if not dest_files:
        raise ValueError(f"Could not build path with entities {out_entities}.")
    dest_list = _listify(dest_files)
    if len(in_files) != len(dest_list):
        raise ValueError(
            f"Input files ({len(in_files)}) not matched "
            f"by interpolated patterns ({len(dest_list)})."
        )

    out_files: list[str] = []
    compression: list[bool] = []
    fixed_hdr = [False] * len(in_files)
    suffix_input = entities.get("suffix")
    out_file = out_path
    for i, (orig_file, dest_file) in enumerate(zip(in_files, dest_list, strict=True)):
        out_file = out_path / dest_file
        out_file.parent.mkdir(exist_ok=True, parents=True)
        out_files.append(str(out_file))
        compression.append(str(dest_file).endswith(".gz"))

        try:
            if os.path.samefile(orig_file, out_file):
                continue
        except FileNotFoundError:
            pass

        new_fields: dict[str, Any] | None = None
        new_data: np.ndarray | None = None
        version = _nifti_version(orig_file)
        is_nifti = version is not None
        dtype_name = data_dtype or DEFAULT_DTYPES[suffix_input]
        target_dtype: Any = dtype_name
        if is_nifti and (check_hdr or target_dtype):
            fields = _image_header(orig_file)
            if check_hdr:
                curr_units = tuple(None if u == "unknown" else u for u in _xyzt_units(fields))
                curr_codes = (int(fields["qform_code"]), int(fields["sform_code"]))
                units = (
                    curr_units[0] or "mm",
                    "sec" if out_entities["suffix"] == "bold" else None,
                )
                xcodes = (1, 1)
                if entities.get("space"):
                    xcodes = (4, 4) if entities["space"] in standard_spaces else (2, 2)
                curr_zooms = zooms = _zooms(fields)
                if "RepetitionTime" in meta_inputs:
                    rt = meta_inputs["RepetitionTime"]
                    if upstream_defects == "reproduce" or len(curr_zooms) == 4:
                        zooms = (*curr_zooms[:3], rt)
                    elif len(curr_zooms) > 4:
                        zooms = (*curr_zooms[:3], rt, *curr_zooms[4:])
                        _defects.warn(
                            [
                                (
                                    "PIPE-30",
                                    "niworkflows",
                                    "fix",
                                    f"RepetitionTime set as the fourth voxel size of {orig_file}"
                                    f" ({len(curr_zooms)}-D), its other sizes kept (niworkflows:"
                                    " nibabel's HeaderDataError)",
                                )
                            ],
                            stacklevel=2,
                        )
                    elif len(curr_zooms) < 4:
                        _defects.warn(
                            [
                                (
                                    "PIPE-30",
                                    "niworkflows",
                                    "fix",
                                    f"RepetitionTime not set in the header of {orig_file} (a "
                                    f"{len(curr_zooms)}-D image; niworkflows: nibabel's "
                                    "HeaderDataError)",
                                )
                            ],
                            stacklevel=2,
                        )
                if (curr_codes, curr_units, curr_zooms) != (xcodes, units, zooms):
                    fixed_hdr[i] = True
                    affine = nibabel_affine(NiftiHeader.from_dict(fields))
                    new_fields = dict(fields)
                    new_fields = set_xform(new_fields, "qform", affine, xcodes[0])
                    new_fields = set_xform(new_fields, "sform", affine, xcodes[1])
                    new_fields["xyzt_units"] = (
                        _UNIT_CODES[units[0] or "unknown"] + _UNIT_CODES[units[1] or "unknown"]
                    )
                    _set_zooms(new_fields, zooms)

            if target_dtype == "source":
                try:
                    target_dtype = _header_dtype(_image_header(sources[0]))
                except Exception:
                    _LOGGER.warning("Could not get data type of file %s", sources[0])
                    target_dtype = None

            if target_dtype:
                target_dtype = np.dtype(target_dtype)
                orig_dtype = _header_dtype(fields)
                if orig_dtype != target_dtype:
                    if upstream_defects == "reproduce":
                        _LOGGER.warning(
                            "Changing %s dtype from %s to %s", out_file, orig_dtype, orig_dtype
                        )
                    else:
                        _LOGGER.warning(
                            "Changing %s dtype from %s to %s", out_file, orig_dtype, target_dtype
                        )
                    new_data = _coerced(
                        orig_file, fields, target_dtype, upstream_defects, n_threads
                    )
                    if new_fields is None:
                        new_fields = dict(fields)
                    _set_data_dtype(new_fields, target_dtype)

        if out_file.exists() or out_file.is_symlink():
            out_file.unlink()
        if new_data is None and new_fields is None:
            copy_any(orig_file, str(out_file))
            continue
        assert new_fields is not None
        orig_fields = _image_header(orig_file)
        if new_data is None:
            slope, inter = _slope_inter(orig_fields)
            _set_slope_inter(new_fields, slope, inter)
            new_fields["vox_offset"] = float(int(float(orig_fields["vox_offset"])))
            data_bytes = _raw_data(orig_file, orig_fields)
        else:
            _set_slope_inter(new_fields, 1.0, 0.0)
            order = "<" if new_fields["byte_order"] == "<" else ">"
            stored = new_data.astype(new_data.dtype.newbyteorder(order), copy=False)
            data_bytes = np.asarray(stored).tobytes(order="F")
        _write_nifti(str(out_file), new_fields, data_bytes)

    out_meta = None
    if len(out_files) == 1:
        meta.update(meta_inputs)
        if meta:
            sidecar = out_file.parent / f"{out_file.name.split('.', 1)[0]}.json"
            if sidecar.exists() or sidecar.is_symlink():
                sidecar.unlink()
            sidecar.write_text(
                json.dumps(meta, sort_keys=True, indent=2), encoding="utf-8", newline="\n"
            )
            out_meta = str(sidecar)
    return DerivativesDataSinkResult(
        tuple(out_files), out_meta, tuple(compression), tuple(fixed_hdr)
    )
