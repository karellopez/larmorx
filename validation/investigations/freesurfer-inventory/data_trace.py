# SPDX-License-Identifier: Apache-2.0
"""Sub-task 4's measurement: which files of FreeSurfer's package each program opens at run time.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it lives beside tools that read FreeSurfer's source,
and its job list is built from that reading (docs/freesurfer/programs.md).

For every job of a job list (``data_trace_jobs.toml``), it rebuilds the subject directory a
recorded step started from (the step's ``before`` map, every file copied from the recording's
``record/blobs/``; ``fsaverage`` linked as recon-all links it), runs the step's command line (or
the job's own) under ``strace -f`` with FreeSurfer's environment (``SetUpFreeSurfer.sh``), stops it
after the job's time limit, and lists every file it opened, by the program that opened it (the
FreeSurfer program or script at the top of its process chain: a wrapper's children are named by
their own program). Files are split into the package's data (anything under ``FREESURFER_HOME``
that is not code), the package's code (``bin/``, ``lib/``, Python modules), the licence file and
the subject's own files; failed opens under ``FREESURFER_HOME`` (searched, not found) are kept.

    python -P validation/investigations/freesurfer-inventory/data_trace.py run \\
        --jobs validation/investigations/freesurfer-inventory/data_trace_jobs.toml \\
        --runs <workspace>/oracles/freesurfer-8.2.0/runs \\
        --fs-home <package root> --license <licence file> \\
        --box scripts/orchestration/boxed.sh --box-gib 12 --out <scratch>/trace [--only NAME ...]
    python -P validation/investigations/freesurfer-inventory/data_trace.py report \\
        --out <scratch>/trace [--json <scratch>/trace/summary.json]

Every run goes through the memory box (one job at a time); the rebuilt subject directory is
removed after each job, the strace log is kept gzip-compressed beside the job's JSON.
"""

from __future__ import annotations

import argparse
import gzip
import json
import os
import re
import shutil
import subprocess
import sys
import time
import tomllib
from pathlib import Path

WRAPPERS = {
    "fs_time",
    "fs_time.orig",
    "fspython",
    "timeout",
    "python",
    "python3",
    "python3.8",
    "bash",
    "sh",
    "dash",
    "tcsh",
    "csh",
    "env",
}
CODE_SUFFIXES = (
    ".py",
    ".pyc",
    ".so",
    ".pth",
    ".pyi",
    ".typed",
    ".egg-link",
    ".dist-info",
)
LINE = re.compile(r"^(\d+)\s+(.*)$")
EXEC = re.compile(r'^execve\("([^"]*)", \[(.*?)\](?:, 0x[0-9a-f]+)?.*\) = (-?\d+)')
OPEN2 = re.compile(r'^(?:open|openat|openat2|creat)\((?:AT_FDCWD, |\d+, )?"([^"]*)",? ?(.*)$')
RET = re.compile(r"\) = (-?\d+)(?: (E[A-Z]+))?")
CLONE = re.compile(r"^(?:clone|clone3|fork|vfork)\(.*\) = (\d+)")
RESUMED = re.compile(r"^<\.\.\. (\w+) resumed>(.*)$")


def fs_environment(fs_home: Path, license_file: Path) -> dict[str, str]:
    """FreeSurfer's environment as run_recon.sh sets it (SetUpFreeSurfer.sh), without the recorder."""
    env = {
        "HOME": os.environ.get("HOME", "/tmp"),
        "PATH": "/usr/local/bin:/usr/bin:/bin",
        "FREESURFER_HOME": str(fs_home),
        "FS_LICENSE": str(license_file),
        "LANG": "C.UTF-8",
    }
    for key in ("XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS", "USER", "LOGNAME"):
        if key in os.environ:
            env[key] = os.environ[key]
    out = subprocess.run(
        ["bash", "-c", 'source "$FREESURFER_HOME/SetUpFreeSurfer.sh" >/dev/null 2>&1; env -0'],
        env=env,
        capture_output=True,
        check=True,
    ).stdout
    result = dict(item.split("=", 1) for item in out.decode().split("\0") if "=" in item)
    for key in list(result):
        if key.startswith("LARMORX_FS_") or key in ("_", "SHLVL", "PWD", "OLDPWD"):
            del result[key]
    return result


def rebuild(run: Path, before: dict[str, str], subjects: Path, sid: str, links: list[str]) -> Path:
    if subjects.exists():
        shutil.rmtree(subjects)
    sd = subjects / sid
    for rel, digest in before.items():
        dst = sd / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(run / "record" / "blobs" / digest[:2] / digest, dst)
    fs_home = Path(os.environ["LX_FS_HOME"])
    for name in links:
        (subjects / name).symlink_to(fs_home / "subjects" / name)
    for d in ("mri/orig", "mri/transforms", "surf", "label", "stats", "scripts", "tmp", "touch"):
        (sd / d).mkdir(parents=True, exist_ok=True)
    return sd


def expand(value: str, mapping: dict[str, str]) -> str:
    for key, repl in mapping.items():
        value = value.replace(key, repl)
    return value


def parse(log: Path) -> list[tuple[int, str, str]]:
    """(pid, kind, rest) per completed call; <unfinished> lines joined with their resumption."""
    pending: dict[int, str] = {}
    calls = []
    with gzip.open(log, "rt", encoding="utf-8", errors="replace") as f:
        for raw in f:
            m = LINE.match(raw.rstrip("\n"))
            if not m:
                continue
            pid, body = int(m.group(1)), m.group(2)
            if body.endswith("<unfinished ...>"):
                pending[pid] = body[: -len("<unfinished ...>")].rstrip()
                continue
            r = RESUMED.match(body)
            if r:
                body = pending.pop(pid, r.group(1) + "(") + r.group(2)
            calls.append((pid, body))
    return calls


def attribute(calls, fs_home: str) -> dict:
    """Name each process by its FreeSurfer program; collect opens per program."""
    parent: dict[int, int] = {}
    own: dict[int, str] = {}
    for pid, body in calls:
        c = CLONE.match(body)
        if c:
            parent[int(c.group(1))] = pid
    names: dict[int, str] = {}

    def name_of(pid: int) -> str:
        if pid in names:
            return names[pid]
        if pid in own:
            return own[pid]
        if pid in parent:
            return name_of(parent[pid])
        return "(top)"

    opens: dict[str, dict[str, dict[str, int]]] = {}
    execs: list[tuple[str, str]] = []
    for pid, body in calls:
        e = EXEC.match(body)
        if e:
            path, args, ret = e.group(1), e.group(2), int(e.group(3))
            if ret != 0:
                continue
            argv = re.findall(r'"((?:[^"\\]|\\.)*)"', args)
            base = os.path.basename(path)
            label = None
            if path.startswith(fs_home) and base not in WRAPPERS:
                label = base
            else:
                for a in argv[1:]:
                    if a.startswith(fs_home + "/") and not a.endswith(CODE_SUFFIXES[:2]):
                        b = os.path.basename(a)
                        if b not in WRAPPERS:
                            label = b
                            break
                    elif a.startswith(fs_home + "/") and a.endswith(".py"):
                        label = os.path.basename(a)[:-3]
                        break
            if label is None:
                label = name_of(parent.get(pid, -1)) if pid in parent else name_of(pid)
            own[pid] = label
            execs.append((label, " ".join(argv)[:300]))
            continue
        o = OPEN2.match(body)
        if not o:
            continue
        path, rest = o.group(1), o.group(2)
        r = RET.search(rest)
        if not r:
            continue
        ok = int(r.group(1)) >= 0
        flags = rest.split(")")[0]
        if "O_DIRECTORY" in flags:
            continue
        writing = any(x in flags for x in ("O_WRONLY", "O_RDWR", "O_CREAT"))
        prog = name_of(pid)
        kind = "read" if ok and not writing else ("write" if ok else "missing")
        opens.setdefault(prog, {}).setdefault(kind, {})
        opens[prog][kind][path] = opens[prog][kind].get(path, 0) + 1
    return {"opens": opens, "execs": execs}


def classify(path: str, fs_home: str, subjects: str, license_file: str) -> tuple[str, str]:
    if path == license_file:
        return "licence", path
    real = os.path.realpath(path) if os.path.isabs(path) else path
    if not os.path.isabs(path):
        return "relative", path
    if real.startswith(fs_home + "/") or path.startswith(fs_home + "/"):
        rel = (real if real.startswith(fs_home + "/") else path)[len(fs_home) + 1 :]
        top = rel.split("/", 1)[0]
        if top in ("bin", "lib"):
            return "code", rel
        if top == "python":
            if rel.endswith(CODE_SUFFIXES) or any(
                x in rel for x in ("/__pycache__/", ".dist-info/", ".egg-info/")
            ):
                return "code", rel
            return "python-other", rel
        return "data", rel
    if path.startswith(subjects + "/"):
        return "subject", path[len(subjects) + 1 :]
    return "system", path


def run_jobs(args) -> int:
    jobs = tomllib.loads(Path(args.jobs).read_text(encoding="utf-8"))["job"]
    if args.only:
        jobs = [j for j in jobs if j["name"] in args.only]
    fs_home = Path(args.fs_home).resolve()
    os.environ["LX_FS_HOME"] = str(fs_home)
    base_env = fs_environment(fs_home, Path(args.license))
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    for job in jobs:
        target = out / f"{job['name']}.json"
        if target.exists() and not args.again:
            continue
        run = Path(args.runs) / job["run"]
        steps = sorted((run / "record" / "steps").glob(f"{job['step']:04d}-*.json"))
        step = json.loads(steps[0].read_text(encoding="utf-8"))
        old_subjects = step["env"]["SUBJECTS_DIR"]
        info = (run / "RUN_INFO.txt").read_text(encoding="utf-8")
        sid = job.get("subject") or re.search(r"^subject: (\S+)$", info, re.M).group(1)
        subjects = out / "work" / "subjects"
        tmp = out / "work" / "tmp"
        if tmp.exists():
            shutil.rmtree(tmp)
        tmp.mkdir(parents=True)
        links = job.get("links", ["fsaverage"])
        sd = rebuild(run, step["before"], subjects, sid, links)
        mapping = {
            old_subjects: str(subjects),
            "{SD}": str(sd),
            "{SUBJECTS_DIR}": str(subjects),
            "{FS}": str(fs_home),
            "{TMP}": str(tmp),
        }
        argv = [expand(a, mapping) for a in job.get("argv", step["argv"])]
        cwd = expand(job.get("cwd", step["cwd"]), mapping)
        env = dict(base_env)
        for key in (
            "OMP_NUM_THREADS",
            "FS_OMP_NUM_THREADS",
            "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS",
        ):
            if key in step["env"]:
                env[key] = str(min(int(step["env"][key]), args.max_threads))
        env["SUBJECTS_DIR"] = str(subjects)
        env.update({k: expand(v, mapping) for k, v in job.get("env", {}).items()})
        log = out / f"{job['name']}.strace"
        limit = int(job.get("timeout", args.timeout))
        cmd = [
            str(Path(args.box).resolve()),
            str(args.box_gib),
            "strace",
            "-f",
            "-qq",
            "--seccomp-bpf",
            "-e",
            "trace=execve,open,openat,openat2,creat,clone,clone3,fork,vfork",
            "-e",
            "signal=none",
            "-s",
            "1024",
            "-o",
            str(log),
            "--",
            "timeout",
            "-s",
            "KILL",
            str(limit),
            *argv,
        ]
        t0 = time.time()
        proc = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True)
        seconds = time.time() - t0
        with log.open("rb") as src, gzip.open(str(log) + ".gz", "wb") as dst:
            shutil.copyfileobj(src, dst)
        log.unlink()
        result = attribute(parse(Path(str(log) + ".gz")), str(fs_home))
        files: dict[str, dict[str, list[str]]] = {}
        for prog, kinds in result["opens"].items():
            for kind, paths in kinds.items():
                for path in paths:
                    cls, rel = classify(path, str(fs_home), str(subjects), args.license)
                    if cls in ("system", "relative") or (cls == "code" and kind == "missing"):
                        continue
                    if cls == "subject" and kind == "read":
                        cls = "subject-read"
                    key = f"{cls}:{kind}" if cls not in ("subject-read",) else cls
                    files.setdefault(prog, {}).setdefault(key, [])
                    if rel not in files[prog][key]:
                        files[prog][key].append(rel)
        record = {
            "name": job["name"],
            "run": job["run"],
            "step": job["step"],
            "argv": [a.replace(str(out), "<out>").replace(str(fs_home), "<FS>") for a in argv],
            "exit": proc.returncode,
            "seconds": round(seconds, 1),
            "limit": limit,
            "stopped_at_limit": proc.returncode in (137, -9) and seconds >= limit - 1,
            "note": job.get("note", ""),
            "programs": {p: {k: sorted(v) for k, v in d.items()} for p, d in sorted(files.items())},
            "executed": sorted({label for label, _ in result["execs"]}),
            "stderr_tail": proc.stderr.decode(errors="replace")[-400:],
        }
        target.write_text(json.dumps(record, indent=1) + "\n", encoding="utf-8")
        print(f"{job['name']}: exit {proc.returncode}, {seconds:.0f} s", flush=True)
        shutil.rmtree(out / "work")
    print("DATA-TRACE DONE", flush=True)
    return 0


def report(args) -> int:
    out = Path(args.out)
    summary: dict[str, dict] = {}
    for f in sorted(out.glob("*.json")):
        if f.name == "summary.json":
            continue
        rec = json.loads(f.read_text(encoding="utf-8"))
        for prog, kinds in rec["programs"].items():
            s = summary.setdefault(
                prog,
                {
                    "jobs": [],
                    "data": set(),
                    "missing": set(),
                    "python_other": set(),
                    "licence": False,
                    "writes_package": set(),
                },
            )
            s["jobs"].append(rec["name"])
            s["data"].update(kinds.get("data:read", []))
            s["python_other"].update(kinds.get("python-other:read", []))
            s["missing"].update(kinds.get("data:missing", []))
            s["writes_package"].update(kinds.get("data:write", []) + kinds.get("code:write", []))
            s["licence"] |= bool(kinds.get("licence:read"))
        flag = " (stopped at its limit)" if rec["stopped_at_limit"] else ""
        print(f"# {rec['name']}: exit {rec['exit']}, {rec['seconds']} s{flag}")
    for prog, s in sorted(summary.items()):
        print(f"\n## {prog}  [{', '.join(s['jobs'])}]  licence read: {s['licence']}")
        for p in sorted(s["data"]):
            print(f"  data  {p}")
        for p in sorted(s["python_other"]):
            print(f"  py    {p}")
        for p in sorted(s["missing"]):
            print(f"  miss  {p}")
        for p in sorted(s["writes_package"]):
            print(f"  WRITE {p}")
    if args.json:
        Path(args.json).write_text(
            json.dumps(
                {
                    p: {k: (sorted(v) if isinstance(v, set) else v) for k, v in s.items()}
                    for p, s in summary.items()
                },
                indent=1,
            )
            + "\n",
            encoding="utf-8",
        )
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="mode", required=True)
    r = sub.add_parser("run")
    r.add_argument("--jobs", required=True)
    r.add_argument("--runs", required=True)
    r.add_argument("--fs-home", required=True)
    r.add_argument("--license", required=True)
    r.add_argument("--box", required=True)
    r.add_argument("--box-gib", type=int, default=12)
    r.add_argument("--out", required=True)
    r.add_argument("--timeout", type=int, default=150)
    r.add_argument("--max-threads", type=int, default=4, help="cap on the recorded thread counts")
    r.add_argument("--only", nargs="*")
    r.add_argument("--again", action="store_true", help="re-run jobs that already have a result")
    p = sub.add_parser("report")
    p.add_argument("--out", required=True)
    p.add_argument("--json")
    args = parser.parse_args(argv)
    return run_jobs(args) if args.mode == "run" else report(args)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
