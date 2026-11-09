# SPDX-License-Identifier: Apache-2.0
"""sdcflows' derivatives of a field-map estimator as a plain function.

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers; the same in 2.15.1 and
2.15.2, fMRIPrep 25.2.x's): ``workflows/outputs.py`` (``init_fmap_derivatives_wf``,
``DerivativesDataSink`` with ``out_path_base = 'sdcflows'``, ``_gendesc``, ``_getname``,
``_getsourcetype``, ``_selectintent``), each node through larmorx's port (niworkflows'
``MergeSeries`` and ``DerivativesDataSink``: :func:`larmorx.niworkflows.merge_series`,
:func:`larmorx.niworkflows.derivatives_data_sink`).

``init_fmap_reports_wf`` (the ``FieldmapReportlet`` figure, ``figures/..._fieldmap.svg``) belongs
to larmorprepx's reports part (decision D7 of the field maps' part B): no figure is written.
"""

from __future__ import annotations

import os
import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName

__all__ = [
    "SDCFLOWS_OUT_PATH_BASE",
    "FmapDerivativesResult",
    "fmap_derivatives_workflow",
    "gendesc",
    "getname",
    "getsourcetype",
    "selectintent",
]

#: sdcflows' ``DerivativesDataSink.out_path_base`` (fMRIPrep sets ``''`` on every ``ds_`` node).
SDCFLOWS_OUT_PATH_BASE = "sdcflows"


def gendesc(infiles: Any) -> str | list[str]:
    """sdcflows' ``_gendesc``: ``'coeff'`` for one coefficients file, ``coeff0``, ``coeff1``, ...
    for several."""
    if isinstance(infiles, (str, bytes)):
        infiles = [infiles]
    if len(infiles) == 1:
        return "coeff"
    return [f"coeff{i}" for i, _ in enumerate(infiles)]


def getname(infile: Any) -> str | list[str]:
    """sdcflows' ``_getname``: the file name(s) without the folders."""
    if isinstance(infile, (list, tuple)):
        return [Path(f).name for f in infile]
    return Path(infile).name


def getsourcetype(infiles: Sequence[Any]) -> str:
    """sdcflows' ``_getsourcetype``: ``'epi'`` when the first source is an ``_epi`` image, else
    ``'magnitude'``."""
    fname = Path(infiles[0]).name
    return "epi" if fname.endswith(("_epi.nii.gz", "_epi.nii")) else "magnitude"


def selectintent(metadata: Any) -> list[str]:
    """sdcflows' ``_selectintent``: every ``IntendedFor`` of the sources' metadata, unique and
    sorted."""
    from larmorx.bids import listify

    return sorted({el for m in listify(metadata) for el in listify(m.get("IntendedFor", []))})


@dataclass(frozen=True)
class FmapDerivativesResult:
    """``init_fmap_derivatives_wf``'s outputs (the derivative files: ``fieldmap``,
    ``fmap_ref``, ``fmap_coeff`` with ``write_coeff``, ``fmap_mask`` with ``write_mask``) and its
    nodes: ``merge_fmap`` (the merged field), the sinks' results (``sinks``: ``ds_reference``,
    ``ds_fieldmap``, ``ds_mask``, and ``ds_coeff`` as a tuple, one per coefficients file)."""

    fieldmap: str
    fmap_ref: str
    fmap_coeff: tuple[str, ...] | None
    fmap_mask: str | None
    merge_fmap: str | None
    sinks: Mapping[str, Any]


def fmap_derivatives_workflow(
    source_files: Sequence[str | os.PathLike[str]],
    fieldmap: str | os.PathLike[str] | Sequence[str | os.PathLike[str]],
    fmap_ref: str | os.PathLike[str],
    fmap_coeff: str | os.PathLike[str] | Sequence[str | os.PathLike[str]] | None = None,
    fmap_mask: str | os.PathLike[str] | None = None,
    fmap_meta: Any = (),
    *,
    output_dir: str | os.PathLike[str],
    bids_fmap_id: str | None = None,
    custom_entities: Mapping[str, Any] | None = None,
    write_coeff: bool = False,
    write_mask: bool = False,
    out_path_base: str = SDCFLOWS_OUT_PATH_BASE,
    out_dir: str | os.PathLike[str] | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> FmapDerivativesResult:
    """sdcflows' ``init_fmap_derivatives_wf``: an estimator's field map, reference, mask and
    coefficients written as BIDS-Derivatives under ``output_dir/out_path_base``.

    The inputs are the workflow's ``inputnode``: ``source_files`` (the estimator's BIDS files,
    which name the derivatives), ``fieldmap`` (merged into a series by ``MergeSeries``),
    ``fmap_ref``, ``fmap_coeff``, ``fmap_mask``, ``fmap_meta`` (the sources' metadata: their
    ``IntendedFor``). ``bids_fmap_id`` is the estimator's ``B0FieldIdentifier`` (the ``fmapid``
    entity is it without non-alphanumeric characters); ``custom_entities`` are more entities of
    the names. The files:

    - ``ds_reference``: ``fmap_ref`` as ``desc-epi_fieldmap`` (EPI sources) or
      ``desc-magnitude_fieldmap``;
    - ``ds_fieldmap``: the merged field as ``desc-preproc_fieldmap``, with a JSON sidecar
      (``Units`` Hz, ``B0FieldIdentifier``, ``RawSources``, ``AnatomicalReference``,
      ``IntendedFor``, and ``AssociatedCoefficients`` with ``write_coeff``);
    - with ``write_mask``, ``ds_mask``: ``fmap_mask`` as ``desc-brain_mask`` (``uint8``);
    - with ``write_coeff``, ``ds_coeff``: each coefficients file as ``desc-coeff[N]_fieldmap``.

    ``out_dir`` holds ``merge_fmap``'s file (nipype's node folder; a temporary folder without
    it, the path then ``None``). ``out_path_base`` is sdcflows' (fMRIPrep: ``''``)."""
    from larmorx.niworkflows import derivatives_data_sink, merge_series
    from larmorx.sdcflows.workflows import _Workdir

    _defects.check(upstream_defects)
    custom = dict(custom_entities or {})
    if bids_fmap_id:
        custom["fmapid"] = re.sub(r"[^a-zA-Z0-9]", "", bids_fmap_id)
    sources = [os.fspath(s) for s in source_files]
    allowed = tuple(custom)
    common = {
        "base_directory": os.fspath(output_dir),
        "out_path_base": out_path_base,
        "allowed_entities": allowed,
        "upstream_defects": upstream_defects,
        "n_threads": n_threads,
    }
    wd = _Workdir(out_dir)
    try:
        merged = merge_series(fieldmap, out_dir=wd.node("merge_fmap"), n_threads=n_threads)
        assert merged is not None
        sinks: dict[str, Any] = {}
        ds_reference = derivatives_data_sink(
            fmap_ref,
            sources,
            compress=True,
            dismiss_entities=("fmap",),
            entities={
                "suffix": "fieldmap",
                "datatype": "fmap",
                "desc": getsourcetype(sources),
                **custom,
            },
            **common,  # type: ignore[arg-type]
        )
        sinks["ds_reference"] = ds_reference
        mask_out = None
        if write_mask:
            if fmap_mask is None:
                raise ValueError("write_mask needs fmap_mask")
            ds_mask = derivatives_data_sink(
                fmap_mask,
                sources,
                compress=True,
                dismiss_entities=("fmap",),
                entities={"desc": "brain", "suffix": "mask", "datatype": "fmap", **custom},
                **common,  # type: ignore[arg-type]
            )
            sinks["ds_mask"] = ds_mask
            mask_out = ds_mask.out_file[0]
        coeff_out = None
        meta: dict[str, Any] = {"Units": "Hz"}
        if bids_fmap_id:
            meta["B0FieldIdentifier"] = bids_fmap_id
        meta["RawSources"] = sources
        meta["AnatomicalReference"] = getname(ds_reference.out)
        meta["IntendedFor"] = selectintent(fmap_meta)
        if write_coeff:
            if fmap_coeff is None:
                raise ValueError("write_coeff needs fmap_coeff")
            coeffs = (
                [os.fspath(fmap_coeff)]
                if isinstance(fmap_coeff, (str, os.PathLike))
                else [os.fspath(c) for c in fmap_coeff]
            )
            descs = gendesc(coeffs)
            descs = [descs] if isinstance(descs, str) else descs
            results = tuple(
                derivatives_data_sink(
                    c,
                    sources,
                    compress=True,
                    entities={"suffix": "fieldmap", "datatype": "fmap", "desc": d, **custom},
                    **common,  # type: ignore[arg-type]
                )
                for c, d in zip(coeffs, descs, strict=True)
            )
            sinks["ds_coeff"] = results
            coeff_out = tuple(r.out for r in results)
            meta["AssociatedCoefficients"] = getname(list(coeff_out))
        ds_fieldmap = derivatives_data_sink(
            merged,
            sources,
            compress=True,
            entities={"desc": "preproc", "suffix": "fieldmap", "datatype": "fmap", **custom},
            metadata=meta,
            **common,  # type: ignore[arg-type]
        )
        sinks["ds_fieldmap"] = ds_fieldmap
        res = FmapDerivativesResult(
            ds_fieldmap.out,  # type: ignore[arg-type]
            ds_reference.out,  # type: ignore[arg-type]
            coeff_out,  # type: ignore[arg-type]
            mask_out,
            merged,
            sinks,
        )
    except BaseException:
        wd.fail()
        raise
    return wd.finish(res)  # type: ignore[no-any-return]
