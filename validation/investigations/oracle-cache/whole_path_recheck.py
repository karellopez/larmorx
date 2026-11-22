# SPDX-License-Identifier: Apache-2.0
"""The re-check of a probe's cached runs after the key learned a whole path's extension.

FreeSurfer's readers take a file's format from its whole name (the text after the last dot,
folders included: ``a.b/c`` has ``b/c``, ``trail.`` none; a gzipped name by ``.mgh.gz`` or
``.nii.gz`` anywhere in it), while the oracle-output cache kept each input by its content and the
suffix of its own name: runs of the same content under such names could share an entry
(track A's relay 50 of FreeSurfer's foundations; ``policy.EXTENSION_FROM_WHOLE_PATH``). This
runs one probe script unchanged, with ``oracle_cache.run`` watched:

- ``--phase before``: the probe as it runs now (every run served from the cache where it can);
- ``--phase after``: after the key's correction, every run whose inputs' names the old key could
  confuse (an input whose suffix is empty or ``.gz``, or whose whole name holds a text the
  suffix does not: ``policy.whole_path_texts``) runs fresh (``refresh``: a difference from the
  entry it was served from is a dispute of the cache); the others are served as before.

Each run's result (exit status, both streams' SHA-256, the files under its output folders) goes
to ``<out>/<label>-<phase>.jsonl``; ``--compare <label>`` prints, run by run, what changed
between the two phases (nothing, for a probe whose results stand).

    python -P whole_path_recheck.py --phase before --label info-s1 --out <dir> -- \\
        validation/probes/fs-info/check.py --seed 1 --n 600
    python -P whole_path_recheck.py --compare info-s1 --out <dir>
"""

from __future__ import annotations

import argparse
import hashlib
import json
import runpy
import sys
from pathlib import Path, PurePosixPath

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.oracle_cache import core  # noqa: E402
from larmorx_validation.oracle_cache.key import _SPLIT, suffix  # noqa: E402

#: the names of a gzipped file that FreeSurfer's readers look for anywhere in it
GZ_MARKERS = (".mgh.gz", ".nii.gz", "nii.gz")


def confusable(argv: list[str], cwd: str | None) -> list[str]:
    """Why the old key could give this run another run's entry: each input (an existing file
    named by an argument) whose key suffix does not fix its format for FreeSurfer's readers."""
    out = []
    base = Path(cwd) if cwd else Path.cwd()
    for a in argv[1:]:
        for i, piece in enumerate(_SPLIT.split(a)):
            if i % 2 or not piece:
                continue
            p = Path(piece) if piece.startswith("/") else base / piece
            try:
                if not p.is_file():
                    continue
            except OSError:
                continue
            held = suffix(PurePosixPath(piece).name)
            if held in ("", ".gz"):
                out.append(f"{piece!r}: suffix {held!r}")
            elif "." in piece and not held.endswith(piece[piece.rfind(".") :]):
                out.append(f"{piece!r}: extension {piece[piece.rfind('.') :]!r}")
            elif any(m in piece and m not in held for m in GZ_MARKERS):
                out.append(f"{piece!r}: a gzip marker")
    return out


def files_of(paths) -> list[list[str]]:
    rows = []
    for o in paths or ():
        o = Path(o)
        if o.is_file():
            rows.append([str(o), hashlib.sha256(o.read_bytes()).hexdigest()])
        elif o.is_dir():
            for f in sorted(o.rglob("*")):
                if f.is_file() and not f.is_symlink():
                    rows.append([str(f), hashlib.sha256(f.read_bytes()).hexdigest()])
    return rows


def watch(phase: str, sink) -> None:
    original = core.run
    count = [0]

    def run(argv, **kw):
        argv = [str(a) for a in argv]
        cwd = str(kw["cwd"]) if kw.get("cwd") is not None else None
        why = confusable(argv, cwd)
        if phase == "after" and why:
            with core.using("refresh"):
                done = original(argv, **kw)
        else:
            done = original(argv, **kw)
        row = {
            "n": count[0],
            "argv": argv,
            "cwd": cwd,
            "confusable": why,
            "cached": done.cached,
            "returncode": done.returncode,
            "stdout": hashlib.sha256(done.stdout_bytes).hexdigest(),
            "stderr": hashlib.sha256(done.stderr_bytes).hexdigest(),
            "files": files_of(kw.get("outputs")),
        }
        count[0] += 1
        sink.write(json.dumps(row) + "\n")
        sink.flush()
        return done

    oracle_cache.run = run
    core.run = run


def compare(out: Path, label: str) -> int:
    def load(phase):
        p = out / f"{label}-{phase}.jsonl"
        return [json.loads(x) for x in p.read_text(encoding="utf-8").splitlines()]

    before, after = load("before"), load("after")
    if len(before) != len(after):
        print(f"{label}: {len(before)} runs before, {len(after)} after")
        return 1
    changed, confusable_runs, fresh = [], 0, 0
    for b, a in zip(before, after, strict=True):
        if b["argv"] != a["argv"]:
            print(f"{label}: run {b['n']} has other arguments after")
            return 1
        confusable_runs += bool(a["confusable"])
        fresh += not a["cached"]
        what = [k for k in ("returncode", "stdout", "stderr", "files") if b[k] != a[k]]
        if what:
            changed.append({"n": b["n"], "argv": b["argv"][:6], "changed": what,
                            "confusable": a["confusable"]})  # fmt: skip
    print(json.dumps({"label": label, "runs": len(before), "confusable": confusable_runs,
                      "ran_fresh_after": fresh, "changed": len(changed),
                      "first_changed": changed[:10]}, indent=1))  # fmt: skip
    return 1 if changed else 0


def scan_open(folder: Path, seeds: list[int]) -> int:
    """Without running anything: the confusable official calls of an open probe
    (``validation/investigations/freesurfer-*-probe/compare.py``: its ``inputs`` and ``cases``) for
    each seed, the cases' working folders being empty (their inputs named by absolute paths)."""
    import importlib.util
    import inspect

    from larmorx_validation.oracle_cache.store import workspace

    spec = importlib.util.spec_from_file_location(f"open_{folder.name}", folder / "compare.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    ws = workspace()
    total = 0
    for seed in seeds:
        f = mod.inputs(ws, seed)
        if len(inspect.signature(mod.cases).parameters) == 2:
            all_cases = mod.cases(f, mod.official().parent.parent)
        else:
            all_cases = mod.cases(f)
        found = []
        for i, c in enumerate(all_cases):
            args = c[0] if isinstance(c, tuple) else c
            if why := confusable(["prog", *map(str, args)], "/nonexistent-empty-folder"):
                found.append({"case": i, "confusable": why})
        total += len(found)
        print(json.dumps({"probe": folder.name, "seed": seed, "cases": len(all_cases),
                          "confusable": len(found), "which": found[:20]}))  # fmt: skip
    return 0


def scan_golden(example: Path) -> int:
    """Without running the official program: the confusable calls of a port's golden example
    (``<example> inputs <dir>`` writes the inputs every case's folder gets a copy of; ``<example>
    cases``: ``k<TAB>args<TAB>outputs`` per case, the names relative to that folder)."""
    import subprocess
    import tempfile

    with tempfile.TemporaryDirectory() as t:
        subprocess.run([example, "inputs", t], check=True)
        lines = subprocess.run([example, "cases"], check=True, capture_output=True, text=True)
        found = []
        rows = lines.stdout.splitlines()
        for line in rows:
            k, args, _outs = line.split("\t")
            if why := confusable(["prog", *args.split()], t):
                found.append({"case": k, "confusable": why})
    print(json.dumps({"example": example.name, "cases": len(rows), "confusable": len(found),
                      "which": found[:20]}))  # fmt: skip
    return 0


def scan_cache(programs: list[str], listing: Path | None) -> int:
    """Without running anything: the entries of the named programs in the oracle-output cache
    (``LARMORX_ORACLE_CACHE``, else the workspace's; ``listing``: a file of entry paths, one per
    line, to read only those), and for each program (relay 52 of FreeSurfer's track A, when
    ``mri_mask`` and ``mri_robust_register`` joined ``policy.EXTENSION_FROM_WHOLE_PATH``):

    - ``rekeyed``: the entries whose key the tuple changes (a path the key replaces by its role
      holds a text of ``policy.whole_path_texts``; a name the key keeps as written, an output's
      ``a.b/c``, already decides its key);
    - ``could_be_served_another``: the entries another run could have been served under the old
      key, those with an input whose key suffix is empty or ``.gz`` (only such a name can hide a
      whole-path text from its suffix: a name with a suffix of its own ends the whole path's text
      with it); none means no run of the program ever got another name's result."""
    import shlex

    from larmorx_validation.oracle_cache import policy, rerun
    from larmorx_validation.oracle_cache.store import location

    root = location() / "entries"
    paths = (
        [Path(x) for x in listing.read_text(encoding="utf-8").split()]
        if listing
        else sorted(root.rglob("*.json"))
    )
    wanted = {f'/bin/{p}"' for p in programs}
    home = str(Path.home())
    rows: dict[str, dict] = {}
    for path in paths:
        raw = path.read_text(encoding="utf-8")
        if not any(w in raw for w in wanted):
            continue
        entry = json.loads(raw)
        program = str(entry.get("launcher", {}).get("program", "")).rsplit("/", 1)[-1]
        if program not in programs:
            continue
        row = rows.setdefault(program, {"entries": 0, "runners": {}, "rekeyed": [],
                                        "could_be_served_another": []})  # fmt: skip
        row["entries"] += 1
        runner = entry.get("runner", "")
        row["runners"][runner] = row["runners"].get(runner, 0) + 1
        template = entry.get("template", [])
        words = [home + w[1:] if w.startswith("~") else w for w in shlex.split(entry["command"])]
        ran = words[-len(template) :] if template else []
        texts = policy.whole_path_texts(ran)
        for t, a in zip(template, ran, strict=True):
            if rerun._PLACEHOLDER.search(t):
                kept = rerun._PLACEHOLDER.sub("", t)
                if found := [x for x in texts if x in a and x not in kept]:
                    row["rekeyed"].append({"entry": path.name, "argument": a, "texts": found})
        odd = [i["suffix"] for i in entry["parts"]["inputs"] if i["suffix"] in ("", ".gz")]
        if odd:
            row["could_be_served_another"].append({"entry": path.name,
                                                   "command": entry["command"][:200]})  # fmt: skip
    for program in programs:
        row = rows.get(program, {"entries": 0})
        print(json.dumps({"program": program, **row,
                          "rekeyed_n": len(row.get("rekeyed", [])),
                          "could_be_served_another_n": len(row.get("could_be_served_another",
                                                                   []))}))  # fmt: skip
    return 0


def main() -> int:
    argv = sys.argv[1:]
    if argv[:1] == ["--scan-cache"]:
        listing = Path(argv[3]) if argv[2:3] == ["--list"] else None
        return scan_cache(argv[1].split(","), listing)
    if argv[:1] == ["--scan-open"]:
        return scan_open(Path(argv[1]).resolve(), [int(s) for s in argv[2].split(",")])
    if argv[:1] == ["--scan-golden"]:
        return scan_golden(Path(argv[1]).resolve())
    probe = []
    if "--" in argv:
        k = argv.index("--")
        argv, probe = argv[:k], argv[k + 1 :]
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--phase", choices=("before", "after"))
    ap.add_argument("--label", required=False)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--compare", metavar="LABEL")
    a = ap.parse_args(argv)
    if a.compare:
        return compare(a.out, a.compare)
    if not (a.phase and a.label and probe):
        ap.error("--phase, --label and a probe command after -- are needed")
    a.out.mkdir(parents=True, exist_ok=True)
    with (a.out / f"{a.label}-{a.phase}.jsonl").open("w", encoding="utf-8") as sink:
        watch(a.phase, sink)
        script = Path(probe[0]).resolve()
        sys.argv = [str(script), *probe[1:]]
        sys.path.insert(0, str(script.parent))
        try:
            runpy.run_path(str(script), run_name="__main__")
        except SystemExit as e:
            code = e.code if isinstance(e.code, int) else (0 if e.code is None else 1)
            print(f"{a.label}: the probe ended with status {code}", file=sys.stderr)
            return code
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
