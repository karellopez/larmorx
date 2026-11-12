# SPDX-License-Identifier: Apache-2.0
"""One porting card per program: ``docs/freesurfer/cards/<program>.md``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: the cards name FreeSurfer's files, functions and the
options its parsers compare, read from FreeSurfer's source.

    python -P validation/investigations/freesurfer-inventory/cards.py \\
        --tag <workspace>/tags/freesurfer-8.2.0 --docs docs/freesurfer \\
        --trace <scratch>/r4/trace --runs <workspace>/oracles/freesurfer-8.2.0/runs \\
        --out docs/freesurfer/cards

Each card joins what the inventory measured for one program: programs.toml (kind, source,
streams, callers, the calls recon-all and fMRIPrep make), waves.toml (wave, crates, queue items,
reach per module, what waits, inputs, checks), code.toml (sources, flags, libraries, the files it
reaches), third-party.toml and placement.toml (the borrowed pieces it reaches), data-files.toml
(the package's files it reads), the strace jobs of data_trace.py (``--trace``: the subject files
it read and wrote), the recordings (``--runs``: what its recorded steps wrote), variation.md (a
table in this file), and the options its parser compares (extracted from its own source files:
candidates, which its replica item confirms one by one).
"""

from __future__ import annotations

import argparse
import json
import re
import tomllib
from collections import defaultdict
from pathlib import Path

# variation.md, per program (sections 1-3): what can make results depend on threads, seeds, the
# build or the machine.
VARIATION: dict[str, list[str]] = {
    "mri_coreg": [
        "MRIgaussianSmoothNI's kernel sums are long double, partitioned by thread (variation.md §2;"
        " measured: the same transform at 1, 2 and 4 threads with its defaults)",
        "dithering on by default with seed 53: GSL-derived RANLUX389 (P-08)",
    ],
    "mri_convert": ["MRIgaussianSmoothNI (smoothing options): long double, thread-partitioned"],
    "mri_normalize": [
        "MRIgaussianSmoothNI (smoothing options): long double, thread-partitioned",
        "-seed 1234 from recon-all",
    ],
    "mri_segreg": [
        "MRIgaussianSmoothNI: long double, thread-partitioned",
        "drand48 seeded from the time of day (PDFtodSeed) unless a seed is given (variation.md §3)",
    ],
    "mri_vol2surf": [
        "MRIgaussianSmoothNI (smoothing options)",
        "a time-of-day seed only when it synthesizes its source",
    ],
    "mri_vol2vol": ["drand48 seeded from the time of day unless a seed is given (noise options)"],
    "mri_surf2surf": ["a time-of-day seed only when it synthesizes data"],
    "mri_robust_template": [
        "initialXforms' centroid summed in an OpenMP critical block: run-to-run nondeterministic"
        " with four or more inputs and more than one thread, only for an unbiased template (with"
        " --fixtp, recon-all's and sMRIPrep's default, never used; measured)",
        "glibc rand() for a random first time point (only without --inittp)",
    ],
    "mri_robust_register": ["measured: the same transform at 1, 2 and 4 threads (RegRobust path)"],
    "mri_brainvol_stats": [
        "ComputeBrainVolumeStats2 sums VoxelVol in parallel: exact for 1 mm voxels, thread-count"
        " dependent for non-integer voxel volumes"
    ],
    "mris_fix_topology": [
        "recon-all always passes -threads 1 (nondeterministic with more)",
        "-seed 1234",
    ],
    "mris_topo_fixer": ["-seed 1234"],
    "mris_smooth": ["-seed 1234"],
    "mris_sphere": ["-seed 1234; threaded in the second call"],
    "mris_curvature": ["-seed 1234"],
    "mris_ca_label": ["-seed 1234"],
    "mri_segstats": ["--seed 1234"],
    "mris_register": ["threaded (-threads N)"],
    "mris_volmask": [
        "--parallel: both hemispheres at once (a race fixed upstream, recon-all 4860)"
    ],
    "mris_place_surface": [
        "x87 long double in the surface-gradient code (surfgrad.cpp)",
        "MRIScomputeBorderValues_new's parallel means: printed only",
    ],
    "mris_remesh": ["x87 long double in the surface-gradient code (surfgrad.cpp)"],
    "mris_remove_intersection": ["x87 long double in the surface-gradient code (surfgrad.cpp)"],
    "mris_convert": [
        "x87 long double in surfgrad.cpp and in gifticlib's line writer (GIFTI output)",
    ],
    "AntsN4BiasFieldCorrectionFs": [
        "one ITK thread (--threads-nondetermistic, default 1); GCC 8.4.0 keeps the B-spline fit's"
        " float roundings (GCC bug 110182 measured: variation.md §1)",
        "srand48(53) for --replace-zeros (FS_ANTS_N4_REPLACE_ZEROS)",
    ],
    "AntsDenoiseImageFs": ["ITK at one thread, set unconditionally"],
    "mri_mask": ["drand48 for its noise option"],
    "mri_synthstrip": ["PyTorch's threads (--threads): to measure (variation.md §5)"],
    "mri_synthseg": ["TensorFlow's threads (--threads): to measure (variation.md §5)"],
    "mri_synthmorph": ["TensorFlow's threads (-j): to measure; about 18 GiB at inference"],
    "mri_sclimbic_seg": ["TensorFlow's threads: to measure (variation.md §5)"],
}

OPTION_PATTERNS = [
    re.compile(
        r"(?:stricmp|strcasecmp|strcmp|strncmp|strncasecmp)\s*\(\s*"
        r"(?:option|argv\s*\[[^\]]*\]|flag|opt|arg|str|pargv\s*\[[^\]]*\])\s*,\s*\"([^\"]+)\""
    ),
    re.compile(r"(?:CMDargNErr|CMDnoArgs)?\s*\(\s*\"(-[-A-Za-z0-9_.]+)\"\s*,\s*(?:nargc|nargs)"),
]
ADD_ARGUMENT = re.compile(r"addArgument\(\s*((?:\"[^\"]*\"\s*,\s*)+)")
PY_ARGUMENT = re.compile(r"add_argument\(\s*((?:['\"][^'\"]*['\"]\s*,\s*)+)")
TCSH_CASE = re.compile(r"^\s*case\s+\"?(-[^\":\s]+)\"?\s*:", re.M)
LETTER_CASE = re.compile(r"case\s+'([A-Za-z])'\s*:")


def load(path: Path) -> dict:
    with path.open("rb") as fh:
        return tomllib.load(fh)


def options(tag: Path, sources: list[str], kind: str) -> tuple[list[str], list[str]]:
    """Option names its parser compares (in source order, unique) and single letters of an
    old-style switch on toupper(*option)."""
    found: list[str] = []
    letters: list[str] = []
    for src in sources:
        try:
            text = (tag / src).read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        if kind in ("tcsh", "csh"):
            found += TCSH_CASE.findall(text)
            continue
        if kind.startswith("python"):
            for m in PY_ARGUMENT.finditer(text):
                found += re.findall(r"['\"](-[^'\"]+)['\"]", m.group(1))
            continue
        for pat in OPTION_PATTERNS:
            found += pat.findall(text)
        for m in ADD_ARGUMENT.finditer(text):
            found += [x for x in re.findall(r"\"([^\"]*)\"", m.group(1)) if x.startswith("-")]
        if re.search(r"switch\s*\(\s*toupper\s*\(\s*\*\s*option", text):
            letters += LETTER_CASE.findall(text)
    seen: set[str] = set()
    out = []
    for x in found:
        x = x.strip()
        if not x or x in seen or " " in x or len(x) > 40:
            continue
        seen.add(x)
        out.append(x)
    return out, sorted(set(letters), key=str.upper)


def subj(path: str) -> str:
    return re.sub(r"^[^/]*sub-[^/]+/", "<subj>/", path)


def short_list(items: list[str], limit: int = 30) -> str:
    items = sorted(set(items))
    head = ", ".join(f"`{x}`" for x in items[:limit])
    return head + (f" and {len(items) - limit} more" if len(items) > limit else "")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--tag", type=Path, required=True)
    ap.add_argument("--docs", type=Path, required=True)
    ap.add_argument("--trace", type=Path)
    ap.add_argument("--runs", type=Path)
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()

    progs = load(args.docs / "programs.toml")["program"]
    code = {p["name"]: p for p in load(args.docs / "code.toml")["program"]}
    waves = load(args.docs / "waves.toml")
    wprog = {p["name"]: p for p in waves["program"]}
    items = {i["id"]: i for i in waves["item"]}
    third = load(args.docs / "third-party.toml")["finding"]
    pieces = load(args.docs / "placement.toml")["piece"]
    data = load(args.docs / "data-files.toml")["file"]

    reads: dict[str, set[str]] = defaultdict(set)
    writes: dict[str, set[str]] = defaultdict(set)
    if args.trace and args.trace.is_dir():
        for f in sorted(args.trace.glob("*.json")):
            d = json.loads(f.read_text(encoding="utf-8"))
            for p, v in d.get("programs", {}).items():
                reads[p] |= {subj(x) for x in v.get("subject-read", [])}
                writes[p] |= {subj(x) for x in v.get("subject:write", [])}
    if args.runs and args.runs.is_dir():
        for st in args.runs.glob("*/record/steps/*.json"):
            try:
                d = json.loads(st.read_text(encoding="utf-8"))
            except (OSError, ValueError):
                continue
            argv = d.get("argv") or []
            if argv:
                writes[Path(argv[0]).name] |= set(d.get("outputs", {}))

    args.out.mkdir(parents=True, exist_ok=True)
    index = []
    for p in progs:
        n = p["name"]
        w = wprog[n]
        c = code.get(n, {})
        sources = c.get("sources") or ([p["source"]] if p["source"] else [])
        opts, letters = options(args.tag, sources, p["kind"])
        tps = [
            t
            for t in third
            if n in str(t.get("programs_core", "")) or n in str(t.get("programs_full_only", ""))
        ]
        tp_ids = {t["id"] for t in tps}
        pcs = [x for x in pieces if tp_ids & set(x.get("findings", []))]
        dfs = [f for f in data if n in f["programs"]]
        lines = [
            f"# `{n}`: porting card",
            "",
            "**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source;"
            " `docs/freesurfer/README.md`). Generated by"
            " `validation/investigations/freesurfer-inventory/cards.py`; do not edit by hand.",
            "",
            "| | |",
            "|---|---|",
            f"| Kind | {p['kind']} |",
            f"| Source | `{p['source']}` |" if p["source"] else "| Source | not in the tree |",
            f"| Streams | {', '.join(p['streams']) or 'none (conditional)'} ({w['group']}) |",
            f"| Wave | {w['wave']} |",
            f"| Replica's crate | `{w['crate']}` |"
            if w["crate"]
            else "| Replica's crate | (later) |",
            f"| Clean room's crate | `{w['clean_room_crate']}` |"
            if w["clean_room_crate"]
            else "| Clean room's crate | (later) |",
            f"| Queue items | {', '.join(w['items']) or '-'} |",
            f"| Called by | {', '.join(p['called_by'])} |",
            f"| Runs | {', '.join(f'`{x}`' for x in w['runs']) or '-'} |",
            f"| How it runs in the reference runs | {p['run']} |",
            f"| Stages (programs.md §3) | {', '.join(p['stages']) or '-'} |",
        ]
        if w["note"]:
            lines.append(f"| Why this wave | {w['note']} |")
        lines += ["", "## The calls recon-all and fMRIPrep make", ""]
        lines += [f"- `{u}`" for u in p["uses"]] or ["- (none recorded)"]
        lines += ["", "## Inputs and outputs (measured)", ""]
        lines.append(
            "- Subject files read (strace of the recorded steps run again, relay 4): "
            + (short_list(sorted(reads.get(n, []))) or "none seen")
        )
        lines.append(
            "- Files written (the recordings' outputs and the strace jobs): "
            + (short_list(sorted(writes.get(n, []))) or "none seen")
        )
        if dfs:
            total = sum(f["bytes"] for f in dfs)
            lines.append(
                f"- Data files of the package ({len(dfs)}, {total / 1e6:,.1f} MB; data-files.toml): "
                + short_list([f["path"] for f in dfs], 20)
            )
        lines += ["", "## Options", ""]
        if opts or letters:
            lines.append(
                "Every option its parser compares, as written in its own source (older parsers"
                " compare without the leading dash and without case); candidates: the replica"
                " item confirms each and adds its messages and failure cases."
            )
            lines.append("")
            if opts:
                lines.append(", ".join(f"`{x}`" for x in opts))
            if letters:
                lines.append("")
                lines.append(
                    "Single-letter options of its `switch (toupper(*option))`: "
                    + ", ".join(f"`{x}`" for x in letters)
                )
        else:
            lines.append("No option comparisons found in its own files (read its parser).")
        lines += ["", "## Code", ""]
        if c:
            lines.append(
                f"- Its own files ({c.get('own_files', 0)}, {c.get('own_lines', 0):,} lines): "
                + ", ".join(f"`{s}`" for s in sources)
            )
            lines.append(
                f"- Libraries: {', '.join(c.get('internal_libraries', []))}; external: "
                f"{', '.join(c.get('external_libraries', [])) or '-'}"
            )
            lines.append(
                f"- Built with `{c.get('cxx_flags', '')}` (C++), `{c.get('c_flags', '')}` (C)"
            )
            fr = c.get("files_reached", {})
            big = sorted(fr.items(), key=lambda kv: -kv[1])[:12]
            if big:
                lines.append(
                    "- Library files it reaches most (lines of their functions, core view): "
                    + ", ".join(f"`{f}` ({k:,})" for f, k in big)
                )
        reach = w.get("reach", {})
        if reach:
            lines.append(
                "- Reach per module (waves.md §3): "
                + ", ".join(f"{m} {k:,}" for m, k in sorted(reach.items(), key=lambda kv: -kv[1]))
            )
        if w.get("waits"):
            lines.append(
                "- Option paths that wait for a later module: "
                + ", ".join(f"{m} ({k:,} lines)" for m, k in w["waits"].items())
            )
        lines.append(
            f"- Its queue item brings {w['item_lines']:,} lines (the functions only it reaches)"
        )
        for iid in w["items"]:
            it = items.get(iid)
            if it:
                lines.append(f"  - {iid}: {it['what']} ({it['lines']:,} lines)")
        lines += ["", "## Borrowed code", ""]
        if tps:
            for t in tps:
                lines.append(f"- {t['id']}: {t['title']} ({t['terms']}; {t['porting']})")
            for x in pcs:
                lines.append(
                    f"- {x['id']} ({x['piece']}): its replica in {x['replica_package']};"
                    f" larmorx-freesurfer uses {x['freesurfer_replica_uses']}"
                )
        else:
            lines.append("- none found in its reach (third-party.md)")
        lines += ["", "## Variation", ""]
        lines += [f"- {v}" for v in VARIATION.get(n, [])] or [
            "- nothing found beyond the common facts (variation.md: GCC 8.4.0, SSE2, glibc 2.35's"
            " libm, drand64 left to right)"
        ]
        lines += ["", "## Checks", "", f"- {w['checks']}", ""]
        if p["notes"]:
            lines += ["## Notes", "", p["notes"], ""]
        (args.out / f"{n}.md").write_text("\n".join(lines), encoding="utf-8")
        index.append((w["wave"], n, p["kind"], w["group"]))

    order = {
        x: i for i, x in enumerate(("3.1", "3.2", "3.3", "3.4", "3.5", "3.6", "N", "3.7", "later"))
    }
    idx = [
        "# FreeSurfer 8.2.0: the porting cards",
        "",
        "**Closed to FreeSurfer clean-room implementers** (docs/freesurfer/README.md). One card per"
        " program of programs.toml, generated by"
        " `validation/investigations/freesurfer-inventory/cards.py` (its README says how).",
        "",
        "| Wave | Program | Kind | Group |",
        "|---|---|---|---|",
    ]
    for wave, n, kind, group in sorted(index, key=lambda x: (order[x[0]], x[1].lower())):
        idx.append(f"| {wave} | [`{n}`]({n}.md) | {kind} | {group} |")
    (args.out / "README.md").write_text("\n".join(idx) + "\n", encoding="utf-8")
    print(f"{len(index)} cards in {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
