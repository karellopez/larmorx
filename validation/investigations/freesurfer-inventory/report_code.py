# SPDX-License-Identifier: Apache-2.0
"""The tables of ``docs/freesurfer/code.md`` from the build graph and the call graphs.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: its input is read from FreeSurfer's source.

    python -P validation/investigations/freesurfer-inventory/report_code.py \\
        docs/freesurfer/programs.toml <cmake.json> <full.json> <core.json> <core-calls.json> > tables.md

``<cmake.json>``: ``cmake_targets.py --json``; ``<full.json>``: ``callgraph.py --toml ... --json``;
``<core.json>``: the same with ``--core``; ``<core-calls.json>``: with ``--core --calls-only``.
Prints Markdown: the build table, the reach per program, the foundations per library, per file
and the shared base.
"""

from __future__ import annotations

import json
import sys
import tomllib
from collections import defaultdict
from pathlib import Path

STANDARD = {
    "utils",
    "jpeg",
    "tiff",
    "xml2",
    "minc",
    "netcdf",
    "nrrdio",
    "dicom",
    "cephes",
    "nifti",
    "dcm2niixfs",
    "gifti",
    "expat",
}
UTILS_EXTERNAL = {"z", "OpenMP::OpenMP_CXX", "crypt", "rt"}
PACKAGES = [
    "expat",
    "gifti",
    "dicom",
    "minc",
    "dcm2niix",
    "nifti",
    "tiff",
    "nrrdio",
    "cephes",
    "jpeg",
    "netcdf",
    "xml2",
]


def k(n: int) -> str:
    return f"{n:,}"


def main(argv: list[str]) -> int:
    toml_path, cmake_path, full_path, core_path, calls_path = (Path(a) for a in argv[:5])
    table = tomllib.loads(toml_path.read_text(encoding="utf-8"))["program"]
    streams = {p["name"]: p["streams"] for p in table}
    cmake = json.loads(cmake_path.read_text(encoding="utf-8"))
    full = json.loads(full_path.read_text(encoding="utf-8"))
    core = json.loads(core_path.read_text(encoding="utf-8"))
    calls = json.loads(calls_path.read_text(encoding="utf-8"))
    out: list[str] = []
    w = out.append

    # 1. the build table
    w(
        "| Program | Streams | Directory | Own files | Own lines | Links besides `utils` and its packages |"
    )
    w("|---|---|---|---:|---:|---|")
    for name, p in sorted(cmake["programs"].items(), key=lambda kv: kv[0].lower()):
        if p is None:
            continue
        extra = [x for x in p["internal_libraries"] if x not in STANDARD]
        ext = [x for x in p["external_libraries"] if x not in UTILS_EXTERNAL]
        links = ", ".join(f"`{x}`" for x in extra + ext) or "-"
        if not p["internal_libraries"]:
            links = "nothing of FreeSurfer's" + (
                f"; {', '.join(f'`{x}`' for x in ext)}" if ext else ""
            )
        st = ", ".join(streams.get(name, [])) or "(none)"
        w(
            f"| `{name}` | {st} | `{p['directory']}` | {p['own_files']} | {k(p['own_lines'])} | {links} |"
        )
    out.append("")

    # 2. the reach per program
    w(
        "| Program | Core reach: functions | lines in its own files | in `utils` and `include/` | in the packages "
        "| all lines (core) | calls only (core) | static reach, every format |"
    )
    w("|---|---:|---:|---:|---:|---:|---:|---:|")
    rows = []
    for name, v in core["programs"].items():
        pl = v["per_library"]
        own = pl.get("own", {}).get("function_lines", 0)
        ui = sum(pl.get(x, {}).get("function_lines", 0) for x in ("utils", "include"))
        pk = sum(
            d.get("function_lines", 0)
            for x, d in pl.items()
            if x not in ("own", "utils", "include")
        )
        tables = sum(d.get("table_lines", 0) for d in pl.values())
        fns = sum(d.get("functions", 0) for d in pl.values())
        total = own + ui + pk
        c = sum(d.get("function_lines", 0) for d in calls["programs"][name]["per_library"].values())
        f = sum(d.get("function_lines", 0) for d in full["programs"][name]["per_library"].values())
        rows.append((total, name, fns, own, ui, pk, c, f, tables))
    for total, name, fns, own, ui, pk, c, f, _tables in sorted(rows, key=lambda r: (-r[0], r[1])):
        w(f"| `{name}` | {k(fns)} | {k(own)} | {k(ui)} | {k(pk)} | {k(total)} | {k(c)} | {k(f)} |")
    out.append("")

    # 3. the foundations per library
    f = core["foundations"]
    tiers = f["tiers"]
    n = len(f["programs"])
    w(
        f"| Library | Lines of functions indexed | reached by 1+ of the {n} programs | 2+ | "
        f"{tiers['25%']}+ (25 %) | {tiers['50%']}+ (50 %) | {tiers['90%']}+ (90 %) |"
    )
    w("|---|---:|---:|---:|---:|---:|---:|")
    libs = f["per_library"]
    for lib in ["utils", "include", *PACKAGES]:
        v = libs.get(lib, {})
        w(
            f"| `{lib}` | {k(v.get('indexed_lines', 0))} | {k(v.get('1+', 0))} | {k(v.get('2+', 0))} | "
            f"{k(v.get('25%', 0))} | {k(v.get('50%', 0))} | {k(v.get('90%', 0))} |"
        )
    tot = {
        t: sum(v.get(t, 0) for v in libs.values())
        for t in ("indexed_lines", "1+", "2+", "25%", "50%", "90%")
    }
    w(
        f"| **all** | {k(tot['indexed_lines'])} | {k(tot['1+'])} | {k(tot['2+'])} | {k(tot['25%'])} | "
        f"{k(tot['50%'])} | {k(tot['90%'])} |"
    )
    out.append("")

    # 4. per file of utils and include: the programs reaching each
    reach_by_file: dict[str, set[str]] = defaultdict(set)
    for name, v in core["programs"].items():
        if not streams.get(name):
            continue
        for file, d in v["per_file"].items():
            if d.get("functions", 0) or d.get("tables", 0):
                reach_by_file[file].add(name)
    w(
        "| File | Lines (file) | Lines of functions indexed | reached (union) | 2+ programs | 50 % | 90 % | Programs reaching it |"
    )
    w("|---|---:|---:|---:|---:|---:|---:|---:|")
    files = core["files"]
    per_file = f["per_file"]
    rows2 = []
    for file, v in per_file.items():
        if not file.startswith(("utils/", "include/")) or v.get("1+", 0) == 0:
            continue
        rows2.append((v.get("1+", 0), file, v))
    for union, file, v in sorted(rows2, key=lambda r: (-r[0], r[1])):
        if union < 400:
            continue
        w(
            f"| `{file}` | {k(files.get(file, {}).get('lines', 0))} | {k(v.get('indexed_lines', 0))} | {k(union)} | "
            f"{k(v.get('2+', 0))} | {k(v.get('50%', 0))} | {k(v.get('90%', 0))} | {len(reach_by_file.get(file, ()))} |"
        )
    small = [r for r in rows2 if r[0] < 400]
    w(
        f"| ({len(small)} more files, under 400 lines reached each) | | | {k(sum(r[0] for r in small))} | | | | |"
    )
    unreached = [x for x in per_file if x.startswith("utils/") and per_file[x].get("1+", 0) == 0]
    out.append("")
    w(
        f"Files of `utils/` none of the {n} programs reaches (core view): {len(unreached)}, "
        f"{k(sum(per_file[x].get('indexed_lines', 0) for x in unreached))} lines of functions: "
        + ", ".join(f"`{Path(x).name}`" for x in sorted(unreached))
        + "."
    )
    out.append("")

    # 5. the shared base: functions reached by 90 % of the programs, by file
    base: dict[str, list[int]] = defaultdict(lambda: [0, 0])
    for key, cnt in f["functions"].items():
        if cnt >= tiers["90%"]:
            file = key.split(":", 1)[0]
            base[file][0] += 1
            base[file][1] += core["functions"][key]["lines"]
    w(f"| File | Functions reached by {tiers['90%']}+ programs | their lines |")
    w("|---|---:|---:|")
    for file, (cnt, lines) in sorted(base.items(), key=lambda kv: -kv[1][1])[:30]:
        w(f"| `{file}` | {cnt} | {k(lines)} |")
    rest = sorted(base.items(), key=lambda kv: -kv[1][1])[30:]
    w(
        f"| ({len(rest)} more files) | {sum(c for _, (c, _) in rest)} | {k(sum(lines for _, (_, lines) in rest))} |"
    )
    print("\n".join(out))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
