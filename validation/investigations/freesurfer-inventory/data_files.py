# SPDX-License-Identifier: Apache-2.0
"""Sub-task 4's list: every file of FreeSurfer 8.2.0's package a default-path program reads, with
its size, SHA-256, git-annex key, where FreeSurfer serves it, its licence and origin.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (built from FreeSurfer's source and package).

    python -P validation/investigations/freesurfer-inventory/data_files.py \\
        --trace <scratch>/trace --package-tsv <scratch>/package-files.tsv \\
        --annex-tsv <scratch>/annex-keys.tsv --annex-urls <scratch>/annex-urls-retry.tsv \\
        --tag <workspace>/tags/freesurfer-8.2.0 --runs <workspace>/oracles/freesurfer-8.2.0/runs \\
        --fs-home <package root> --toml docs/freesurfer/data-files.toml > <scratch>/data-files-tables.md

Inputs: ``data_trace.py``'s job results (which program opened which file, per job), the package's
files with sizes and SHA-256 (``data_refs.py package``), the tag's git-annex symlinks
(``annex-keys.tsv``) and the HEAD checks of their objects on FreeSurfer's annex remote, and the
recordings' command lines (files named on a command line count as read by that program even when
a job stopped it before it opened them). The tag's regular files are matched by SHA-256, so a file
that is in FreeSurfer's git repository without git-annex gets its GitHub address at the tag.
The licence and origin of each file come from ``RULES`` (each rule says where its claim comes
from); the proposed home applies the plan's data rules 1 to 3 (docs/freesurfer-plan.md).
"""

from __future__ import annotations

import argparse
import csv
import glob
import hashlib
import json
import os
import re
import sys
from pathlib import Path

GITHUB_RAW = "https://raw.githubusercontent.com/freesurfer/freesurfer/v8.2.0/"
SMALL = 2_000_000  # bytes: below this a FreeSurfer-licensed file could ship in larmorx-freesurfer

# (pattern on the package path, kind, licence, origin). The first match wins.
SLA = "FreeSurfer licence (Part B; docs/freesurfer/third-party.md section 2)"
RULES: list[tuple[str, str, str, str]] = [
    (
        r"^average/RB_all_.*\.gca$",
        "atlas (GCA)",
        SLA,
        "MGH: the subcortical atlas trained on manually labelled heads (Fischl et al. 2002, 2004); "
        "release 2020-01-02 (the with-skull one 2020_01_02)",
    ),
    (
        r"^average/[lr]h\.DKaparc\.atlas.*\.gcs$",
        "atlas (GCS)",
        SLA,
        "MGH: Desikan-Killiany cortical atlas (Desikan et al. 2006) trained on 40 heads (acfb40)",
    ),
    (
        r"^average/[lr]h\.CDaparc\.atlas.*\.gcs$",
        "atlas (GCS)",
        SLA,
        "MGH: Destrieux cortical atlas (Destrieux et al. 2010) trained on 40 heads (acfb40)",
    ),
    (
        r"^average/[lr]h\.DKTaparc\.atlas.*\.gcs$",
        "atlas (GCS)",
        SLA,
        "MGH: Desikan-Killiany-Tourville atlas (Klein and Tourville 2012) trained on 40 heads",
    ),
    (
        r"^average/[lr]h\.folding\.atlas.*\.tif$",
        "atlas (folding, .tif)",
        SLA,
        "MGH: the spherical folding atlas (Fischl et al. 1999), 40 heads (acfb40)",
    ),
    (
        r"^average/mni305\.cor(\.stripped)?\.mgz$",
        "template",
        "MNI's notice (the MINC package, license.minc.txt) for the data; " + SLA + " for MGH's "
        "conversion",
        "MNI305 average (Evans et al. 1993), converted by FreeSurfer from MINC's "
        "mni_autoreg average_305.mnc (average/mni305.cor.readme); the stripped one FreeSurfer's",
    ),
    (
        r"^average/mni_icbm152_nlin_asym_09c/",
        "template",
        "MNI's notice (average/mni_icbm152_nlin_asym_09c/COPYING) for the data; "
        + SLA
        + " for MGH's derivation",
        "ICBM 2009c nonlinear asymmetric (Fonov et al. 2011), made into registration targets by "
        "FreeSurfer's log/mkatlas (masked, symmetrised, cropped; LTAs between the views)",
    ),
    (
        r"^average/.*\.4dfp\.(img|ifh|hdr|mat|img\.rec)$|^average/.*\.lst$",
        "atlas (4dfp)",
        "Washington University's atlas: no licence statement with the files (the 4dfp code's "
        "notice forbids copying without permission, TP-09)",
        "Washington University (A. Snyder): 711-2B/711-2C atlases registered to MNI305 (2007); "
        "3T18yoSchwartzReactN32 a 3 T young-adult atlas (2009)",
    ),
    (
        r"^average/(mca-dura|vsinus).*",
        "prior (MNI152 space)",
        SLA,
        "MGH: priors for the v8 dura and venous-sinus segmentations, in MNI152 1 mm space",
    ),
    (r"^average/colortable_.*\.txt$", "colour table", SLA, "MGH (FreeSurfer)"),
    (r"^[A-Za-z]+LUT\.txt$", "colour table", SLA, "MGH (FreeSurfer)"),
    (
        r"^models/synthstrip\..*\.pt$",
        "network weights (PyTorch)",
        SLA + "; no separate terms stated with the weights",
        "MGH: SynthStrip (Hoopes et al. 2022)",
    ),
    (
        r"^models/synthseg_.*\.(h5|npy)$",
        "network weights (Keras HDF5) or label list (.npy)",
        SLA + "; no separate terms stated with the weights (SynthSeg's code is Apache-2.0)",
        "UCL/MGH: SynthSeg 2.0 (Billot et al. 2023)",
    ),
    (
        r"^models/synthmorph\..*\.h5$",
        "network weights (Keras HDF5)",
        SLA + "; no separate terms stated with the weights (voxelmorph's code is Apache-2.0)",
        "MGH: SynthMorph (Hoffmann et al. 2022, 2024)",
    ),
    (
        r"^models/(sclimbic|entowm|mca-dura|vsinus).*",
        "network weights (Keras HDF5) or table",
        SLA + "; no separate terms stated with the weights",
        "MGH: the sclimbic network family (Greve et al. 2021), models for v8's entorhinal white "
        "matter, MCA/dura and venous sinus segmentations",
    ),
    (
        r"^subjects/fsaverage[0-9]?/label/.*_exvivo.*\.label$",
        "label (fsaverage)",
        SLA,
        "MGH: Brodmann areas mapped from ex vivo histology (Fischl et al. 2008; Amunts, Zilles)",
    ),
    (
        r"^subjects/fsaverage[0-9]?/label/.*\.mpm\.vpnl\.label$",
        "label (fsaverage)",
        SLA,
        "Ventral visual stream cytoarchitectonic areas (Rosenke et al. 2018), on fsaverage",
    ),
    (
        r"^subjects/fsaverage[0-9]?/",
        "average subject",
        SLA,
        "MGH: fsaverage, the 40-head spherical average (Fischl et al. 1999); fsaverage5/6 its "
        "icosahedron 5 and 6 versions",
    ),
    (
        r"^mni/share/mni_autoreg/",
        "MINC model",
        "MNI's notice (license.minc.txt)",
        "MNI: mritotal's models (Collins et al. 1994)",
    ),
    (r"^fsafd/", "statistics table", SLA, "MGH: Talairach failure detection (talairach_afd)"),
    (
        r"^(etc/|build-stamp\.txt|sources\.(c)?sh|FreeSurferEnv\.(c)?sh|SetUpFreeSurfer)",
        "configuration",
        SLA,
        "FreeSurfer's run-time configuration (not data: the recon-all port reproduces it)",
    ),
]


def rule_for(path: str) -> tuple[str, str, str]:
    for pattern, kind, licence, origin in RULES:
        if re.search(pattern, path):
            return kind, licence, origin
    return "unclassified", "to be checked", "to be checked"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def load_tsv(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8") as f:
        return list(csv.DictReader(f, delimiter="\t"))


def tag_regular_files(tag: Path, sizes: set[int]) -> dict[tuple[int, str], str]:
    """(size, sha256) -> tag path, for regular files whose size is one we look for."""
    found: dict[tuple[int, str], str] = {}
    for d, _, files in os.walk(tag):
        for name in files:
            p = Path(d) / name
            if p.is_symlink() or not p.is_file():
                continue
            size = p.stat().st_size
            if size in sizes:
                found.setdefault((size, sha256(p)), str(p.relative_to(tag)))
    return found


def recorded_reads(runs: Path, fs_home: str) -> dict[str, set[str]]:
    """Package files named on recorded command lines, by program."""
    named: dict[str, set[str]] = {}
    for f in glob.glob(str(runs / "*" / "record" / "steps" / "*.json")):
        step = json.loads(Path(f).read_text(encoding="utf-8"))
        home = step["env"].get("FREESURFER_HOME", fs_home)
        prog = os.path.basename(step["argv"][0])
        for arg in step["argv"]:
            for tok in re.split(r"[,=]", arg):
                if tok.startswith(home + "/"):
                    rel = tok[len(home) + 1 :].split("#")[0]
                    if os.path.isfile(os.path.join(home, rel)):
                        named.setdefault(rel, set()).add(prog)
    return named


def stream_of(job: str) -> str:
    return {"v8": "v8", "cl": "classic", "fp": "fmriprep"}[job.split("-", 1)[0]]


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--trace", required=True, type=Path)
    ap.add_argument("--package-tsv", required=True, type=Path)
    ap.add_argument("--annex-tsv", required=True, type=Path)
    ap.add_argument("--annex-urls", required=True, type=Path)
    ap.add_argument("--tag", required=True, type=Path)
    ap.add_argument("--runs", required=True, type=Path)
    ap.add_argument("--fs-home", required=True)
    ap.add_argument("--toml", required=True, type=Path)
    args = ap.parse_args(argv)

    package = {r["path"]: r for r in load_tsv(args.package_tsv)}
    annex_by_sha: dict[str, dict[str, str]] = {}
    for r in load_tsv(args.annex_tsv):
        annex_by_sha.setdefault(r["sha256"], r)
    urls = {r["path"]: r for r in load_tsv(args.annex_urls)}

    # file -> {programs, streams, jobs, conditional}
    reads: dict[str, dict] = {}
    for f in sorted(args.trace.glob("*.json")):
        if f.name == "summary.json":
            continue
        rec = json.loads(f.read_text(encoding="utf-8"))
        stream = stream_of(rec["name"])
        cond = "-cond-" in rec["name"]
        for prog, kinds in rec["programs"].items():
            for rel in kinds.get("data:read", []):
                e = reads.setdefault(
                    rel, {"programs": set(), "streams": set(), "jobs": set(), "cond": set()}
                )
                e["programs"].add(prog)
                e["jobs"].add(rec["name"])
                (e["cond"] if cond else e["streams"]).add(stream)
    for rel, progs in recorded_reads(args.runs, args.fs_home).items():
        e = reads.setdefault(
            rel, {"programs": set(), "streams": set(), "jobs": set(), "cond": set()}
        )
        e["programs"] |= progs
        e["jobs"].add("recorded command lines")

    missing = sorted(r for r in reads if r not in package)
    for rel in missing:  # outside data_refs.py's folders (etc/, fsafd/, ...): measured here
        p = Path(args.fs_home) / rel
        if p.is_file():
            package[rel] = {"path": rel, "bytes": str(p.stat().st_size), "sha256": sha256(p)}
    missing = sorted(r for r in reads if r not in package)
    sizes = {int(package[r]["bytes"]) for r in reads if r in package}
    in_git = tag_regular_files(args.tag, sizes)

    entries = []
    for rel in sorted(reads):
        e = reads[rel]
        kind, licence, origin = rule_for(rel)
        row = package.get(rel)
        size = int(row["bytes"]) if row else 0
        digest = row["sha256"] if row else ""
        annex = annex_by_sha.get(digest)
        served, url, status = "only inside the package", "", ""
        tag_path = ""
        if annex:
            tag_path = annex["path"]
            u = urls.get(annex["path"])
            key = os.path.basename(os.readlink(args.tag / annex["path"]))
            if u:
                url, status = u["url"], u["http_status"]
            served = (
                "FreeSurfer's git-annex remote (key " + key + ")"
                if status == "200"
                else f"annexed (key {key}); remote answered {status or 'not checked'}"
            )
        elif (size, digest) in in_git:
            tag_path = in_git[(size, digest)]
            url = GITHUB_RAW + tag_path
            served = "FreeSurfer's git repository at v8.2.0 (GitHub)"
        streams = sorted(e["streams"]) or ["(conditional only)"]
        if kind == "configuration":
            home = "the recon-all port's own configuration (not shipped as data)"
        elif licence.startswith("Washington"):
            home = "fetched on first use (rule 2: terms unknown)"
        elif size > SMALL:
            home = "fetched on first use (rule 3: large)"
        elif licence.startswith("MNI"):
            home = "fetched, or the main package if MGH's conversion is judged free (rule 1)"
        else:
            home = "larmorx-freesurfer, or fetched (rule 2: FreeSurfer's terms allow it)"
        entries.append(
            {
                "path": rel,
                "kind": kind,
                "bytes": size,
                "sha256": digest,
                "programs": sorted(e["programs"]),
                "streams": streams,
                "conditional_streams": sorted(e["cond"]),
                "evidence": sorted(e["jobs"]),
                "tag_path": tag_path,
                "served": served,
                "url": url,
                "url_status": status,
                "licence": licence,
                "origin": origin,
                "proposed_home": home,
            }
        )

    lines = [
        "# SPDX-License-Identifier: Apache-2.0",
        "# Every file of FreeSurfer 8.2.0's package a default-path program reads (the plan's data rule 4).",
        "# Generated by validation/investigations/freesurfer-inventory/data_files.py; do not edit by hand.",
        "# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS.",
        'package = "freesurfer_ubuntu22-8.2.0_amd64.deb (build freesurfer-linux-ubuntu22_x86_64-8.2.0-20260314-d932c45)"',
        'package_sha256 = "0c17edb0e738a41faad76ed1435aced157a2bbf6151472d4bd9891b6a9fa1013"',
        'paths_relative_to = "FREESURFER_HOME"',
        "",
    ]
    for e in entries:
        lines.append("[[file]]")
        for k, v in e.items():
            lines.append(f"{k} = {json.dumps(v, ensure_ascii=False)}")
        lines.append("")
    args.toml.write_text("\n".join(lines), encoding="utf-8")

    # Markdown: one row per file, grouped by kind
    print(
        "| File (under `FREESURFER_HOME`) | Kind | Size (bytes) | SHA-256 (first 12) | Programs | "
        "Streams | Served on its own | Proposed home |"
    )
    print("|---|---|---:|---|---|---|---|---|")
    for e in sorted(entries, key=lambda x: (x["kind"], x["path"])):
        served = (
            "annex 200"
            if e["url_status"] == "200"
            else (
                "GitHub (git)"
                if "GitHub" in e["served"]
                else ("annex " + (e["url_status"] or "?") if "annex" in e["served"] else "no")
            )
        )
        streams = ", ".join(e["streams"] + [f"{s} (conditional)" for s in e["conditional_streams"]])
        print(
            f"| `{e['path']}` | {e['kind']} | {e['bytes']:,} | `{e['sha256'][:12]}` | "
            f"{', '.join(e['programs'])} | {streams} | {served} | {e['proposed_home']} |"
        )
    total = sum(e["bytes"] for e in entries)
    print(f"\n{len(entries)} files, {total:,} bytes; not in the package's file list: {missing}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
