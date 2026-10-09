"""Benchmarks of NIfTI reading and writing: larmorx vs nibabel and SimpleITK.

Reads include materialising the voxel array: nibabel loads lazily and memory-maps uncompressed
files, so its read is ``np.asanyarray(nib.load(path, mmap=False).dataobj)``. Files are read from the OS page cache after a warm-up, so the
numbers measure parsing, decompression and conversion rather than disk speed. Writes use each
tool's default gzip level unless stated; output sizes are reported because the levels differ.
"""

from __future__ import annotations

import argparse
import datetime as dt
import shutil
import sys
import tempfile
from collections.abc import Callable
from pathlib import Path
from typing import Any

import larmorx_testdata as td
import numpy as np

import larmorx as lx
from larmorx_validation import environment
from larmorx_validation.bench.harness import Measurement, measure
from larmorx_validation.report import environment_section, table, write_json


def _optional(name: str):
    try:
        return __import__(name)
    except ImportError:
        return None


def select_inputs(tier: str, quick: bool) -> list[td.FileEntry]:
    """Benchmark inputs: files tagged ``benchmark`` in ``tier``, else the largest real NIfTI files."""
    real = [
        e
        for e in td.select(tier=tier, tags={"nifti"})
        if not e.path.startswith("synthetic/") and "pair-data" not in e.tags
    ]
    chosen = [e for e in real if "benchmark" in e.tags]
    # Also a representative spread of typical inputs: anatomical, functional, template.
    for tag in ("T1w", "bold", "template"):
        typical = sorted(
            (e for e in real if tag in e.tags and e not in chosen), key=lambda e: -e.size
        )
        if typical:
            chosen.append(typical[0])
    if not chosen:
        chosen = sorted(real, key=lambda e: -e.size)[:3]
    chosen = sorted({e.path: e for e in chosen}.values(), key=lambda e: e.size)
    return chosen[:2] if quick else chosen


def _label(entry: td.FileEntry, img: lx.Image, compressed: bool) -> str:
    kind = "nii.gz" if compressed else "nii"
    shape = "×".join(str(s) for s in img.shape)
    return f"{entry.name.split('.')[0]} ({shape} {img.dtype}, .{kind})"


def bench_file(
    entry: td.FileEntry, workdir: Path, repeats: int, threads: list[int]
) -> list[Measurement]:
    nib = _optional("nibabel")
    sitk = _optional("SimpleITK")
    src = td.get(entry)
    img = lx.load(src)
    data_bytes = img.data.nbytes
    out: list[Measurement] = []

    def add(label: str, op: str, tool: str, fn: Callable[[], Any], **extra: Any) -> None:
        out.append(Measurement(label, op, tool, measure(fn, repeats=repeats), data_bytes, extra))

    # The same image, compressed (as distributed) and uncompressed.
    # Separate directories: nifti_clib-based readers (ITK, so SimpleITK) strip the extension and
    # prefer x.nii over x.nii.gz when both exist, which would time the wrong file.
    (workdir / "gz").mkdir(exist_ok=True)
    (workdir / "raw").mkdir(exist_ok=True)
    gz_path = workdir / "gz" / "input.nii.gz"
    raw_path = workdir / "raw" / "input.nii"
    if src.name.endswith(".gz"):
        shutil.copyfile(src, gz_path)
    else:
        lx.save(img, gz_path)
    lx.save(img, raw_path)

    def run_layout(path: Path, compressed: bool) -> None:
        label = _label(entry, img, compressed)
        for n in threads:
            add(
                label,
                "read",
                f"larmorx (threads={n or 'all'})",
                lambda p=path, n=n: lx.load(p, n_threads=n),
            )
        if nib is not None:
            add(
                label,
                "read",
                "nibabel",
                lambda p=path: np.asanyarray(nib.load(p, mmap=False).dataobj),
            )
        add(
            label,
            "read float32",
            "larmorx",
            lambda p=path: lx.load(p, dtype=np.float32, n_threads=0),
        )
        if nib is not None:
            add(
                label,
                "read float32",
                "nibabel",
                lambda p=path: nib.load(p, mmap=False).get_fdata(dtype=np.float32),
            )
        if sitk is not None:
            add(
                label,
                "read",
                "SimpleITK",
                lambda p=path: sitk.GetArrayViewFromImage(sitk.ReadImage(str(p))).copy(),
            )

        suffix = ".nii.gz" if compressed else ".nii"
        target = path.parent / f"out{suffix}"
        for n in threads:
            add(
                label,
                "write",
                f"larmorx (threads={n or 'all'})",
                lambda n=n: lx.save(img, target, n_threads=n),
            )
            out[-1].extra["output_bytes"] = target.stat().st_size
        if nib is not None:
            nimg = nib.Nifti1Image(np.asarray(img.data), img.affine, dtype=img.dtype)
            add(label, "write", "nibabel", lambda: nib.save(nimg, target))
            out[-1].extra["output_bytes"] = target.stat().st_size
        if sitk is not None and img.ndim <= 4 and img.dtype.kind in "iuf":
            simg = sitk.GetImageFromArray(
                np.ascontiguousarray(np.asarray(img.data).T), isVector=False
            )
            add(
                label,
                "write",
                "SimpleITK",
                lambda: sitk.WriteImage(simg, str(target), useCompression=compressed),
            )
            out[-1].extra["output_bytes"] = target.stat().st_size
        if compressed:
            add(
                label,
                "write (gzip level 6)",
                "larmorx (threads=all)",
                lambda: lx.save(img, target, compression_level=6, n_threads=0),
            )
            out[-1].extra["output_bytes"] = target.stat().st_size
            if nib is not None:
                import nibabel.openers

                def nib_level6() -> None:
                    old = nibabel.openers.Opener.default_compresslevel
                    nibabel.openers.Opener.default_compresslevel = 6
                    try:
                        nib.save(nimg, target)
                    finally:
                        nibabel.openers.Opener.default_compresslevel = old

                add(label, "write (gzip level 6)", "nibabel", nib_level6)
                out[-1].extra["output_bytes"] = target.stat().st_size

    for path, compressed in ((gz_path, True), (raw_path, False)):
        run_layout(path, compressed)
    return out


def _fmt_time(s: float) -> str:
    return f"{s * 1e3:.1f} ms" if s < 1 else f"{s:.2f} s"


def render(
    results: list[Measurement], env: dict[str, Any], tier: str, repeats: int, command: str
) -> str:
    lines = [
        "# Benchmarks: NIfTI reading and writing",
        "",
        "larmorx (`larmorx.load` / `larmorx.save`, crate `larmorx-io`) against nibabel and SimpleITK.",
        "",
        f"- **Generated:** {dt.date.today().isoformat()} on {env['platform']} ({env['cpu']}, {env['logical_cpus']} logical CPUs), with `{command}`",
        f"- **Method:** median of {repeats} runs after one warm-up, files in the OS page cache (parsing, "
        "decompression and conversion, not disk speed). Reads materialise the voxel array (nibabel "
        "with `mmap=False`; memory-mapping uncompressed files is a nibabel feature larmorx does not "
        "have yet). "
        "*Speed-up* is the nibabel median divided by the tool's median for the same operation.",
        f"- **Test data:** larmorx-testdata `{env['larmorx_testdata_commit']}`, tier `{tier}`",
        "- **Pitfall avoided:** ITK (and so SimpleITK and ANTs) reads NIfTI through nifti_clib, which "
        "prefers `x.nii` over `x.nii.gz` when both exist; compressed and uncompressed inputs are kept "
        "in separate directories.",
        "- **Defaults differ:** nibabel writes gzip level 1, larmorx level 2 (zlib-rs; the same file "
        "size as zlib's level 1), SimpleITK zlib's default (6). The *level 6* rows compare larmorx and "
        "nibabel at the same level.",
        "",
    ]
    for label in dict.fromkeys(m.input for m in results):
        rows_for = [m for m in results if m.input == label]
        lines += [f"## {label}", ""]
        rows = []
        for op in dict.fromkeys(m.operation for m in rows_for):
            ms = [m for m in rows_for if m.operation == op]
            ref = next((m for m in ms if m.tool == "nibabel"), None)
            for m in ms:
                speedup = f"{ref.median / m.median:.1f}×" if ref is not None else "–"
                size = m.extra.get("output_bytes")
                rows.append(
                    (
                        op,
                        m.tool,
                        _fmt_time(m.median),
                        _fmt_time(m.best),
                        f"{m.throughput_mb_s:,.0f}",
                        speedup,
                        f"{size / 1e6:.1f} MB" if size else "",
                    )
                )
        lines += [
            table(
                ("Operation", "Tool", "Median", "Best", "MB/s", "Speed-up vs nibabel", "Output"),
                rows,
            ),
            "",
        ]
    lines += ["## Environment", "", environment_section(env), ""]
    return "\n".join(lines)


def main(args: argparse.Namespace) -> int:
    tier = "smoke" if args.quick else args.tier
    repeats = 2 if args.quick else args.repeats
    inputs = select_inputs(tier, args.quick)
    if not inputs:
        print("no benchmark inputs in this tier", file=sys.stderr)
        return 1
    results: list[Measurement] = []
    with tempfile.TemporaryDirectory() as tmp:
        for entry in inputs:
            print(
                f"benchmarking {entry.path} ({entry.size / 1e6:.1f} MB)",
                file=sys.stderr,
                flush=True,
            )
            results += bench_file(entry, Path(tmp), repeats, args.threads)
    env = environment.describe(("nibabel", "SimpleITK"))
    command = "python -m larmorx_validation bench nifti-io" + (
        " --quick" if args.quick else f" --tier {tier} --repeats {repeats}"
    )
    report = render(results, env, tier, repeats, command)
    if args.out:
        out = Path(args.out)
        out.mkdir(parents=True, exist_ok=True)
        (out / "nifti-io.md").write_text(report, encoding="utf-8")
        write_json(
            out / "nifti-io.json",
            {
                "environment": env,
                "tier": tier,
                "repeats": repeats,
                "results": [m.to_dict() for m in results],
            },
        )
        print(f"report: {out / 'nifti-io.md'}", file=sys.stderr)
    else:
        print(report)
    return 0
