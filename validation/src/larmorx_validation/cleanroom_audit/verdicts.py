# SPDX-License-Identifier: Apache-2.0
"""The verdicts: for every clean-room original, who touched its code, in which role, and whether
any of them had seen restricted material of its family before or while doing so.

    cleanroom-audit verdicts --scan <scan dir> --out <private dir> [--repo <larmorx checkout>]

From a scan (``cleanroom-audit scan``) and the repository's history:

1. **The clean-room code** (:func:`tool_of`): the three clean-room crates, by tool, and the
   clean-room files of ``larmorx-core`` (glibc's bits written from specs).
2. **Who touched it**: every edit an agent's access log records on that code, and every commit
   that changed it (all refs), attributed to the agent that printed its hash right after making it
   (``git commit -q && git log --oneline -1``), else to the agent whose commit command carries its
   subject, else to the one that committed at that second and had edited one of its files
   (:func:`attribute`). Merges whose result differs from every parent (``--cc``) are listed with
   their diff: they are the orchestrator's own edits.
3. **Roles** (:func:`role_of`), from each agent's brief (its first message).
4. **Every restricted access** of each agent that touched a tool, judged against that tool's
   family and the rules in force on the access's date (:func:`judge`): content of the family's
   restricted material before the agent's last touch is the finding that matters; names, builds,
   black-box runs of a replica program, other families' material and material written by other
   clean-room implementers (``docs/parts``, scratch notes) are reported with their nature.
5. **The register** (R12): the lines a clean-room agent's reads and searches of
   ``docs/findings/upstream-defects.md`` printed are compared with the lines that only its
   restricted sections ever held, over every version (:func:`register_lines`).

The output (``verdicts.json``, ``verdicts.md``) holds absolute paths, session ids and command
lines: it is private (``<workspace>/archive/clean-room-audit/<date>/``), never committed.
"""

from __future__ import annotations

import argparse
import collections
import gzip
import json
import re
import subprocess
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

# --- the clean-room code -----------------------------------------------------------------------

#: The clean-room files of crates that are not clean-room crates (larmorx-core's glibc bits from
#: specs; ``tanh_glibc`` is fdlibm's, written by BET's clean-room implementer: glibc never opened).
CORE_FILES = {
    "crates/larmorx-core/src/math/cbrtf_glibc.rs": ("GNU", "cbrtf_glibc"),
    "crates/larmorx-core/src/math/sincos_glibc.rs": ("GNU", "sincos_glibc"),
    "crates/larmorx-core/src/math/sincos_glibc_tables.rs": ("GNU", "sincos_glibc"),
    "crates/larmorx-core/src/math/tanh_glibc.rs": ("GNU", "tanh_glibc"),
}
CLEANROOM_PATHS = (
    "crates/larmorx-afni",
    "crates/larmorx-mri",
    "crates/larmorx-fsl",
    *CORE_FILES,
)
#: larmorx-fsl's modules and programs by tool (the first match wins; the rest is shared code).
FSL_TOOLS = (
    (r"(^|/)(bet|betsurf|bet2|bet_script|bre)(/|\.rs$)", "bet"),
    (
        r"(^|/)(flirt|convert_xfm|avscale|makerot|midtrans|pairreg|pointflirt|rmsdiff|coords|"
        r"aff2rigid|applyxfm4d|extracttxt)(/|\.rs$)",
        "flirt",
    ),
    (r"(^|/)fast(/|\.rs$)", "fast"),
    (r"(^|/)(topup|applytopup|topup_debug)(/|\.rs$)", "topup"),
    (r"(^|/)(maths/|fslmaths|fslstats)", "fslmaths"),
    (r"(^|/)ssroi\.rs$", "standard_space_roi"),
    (r"(^|/)fslroi\.rs$", "fslroi"),
)
#: What each tool is the clean-room original of (the public summary's names).
TOOL_NAMES = {
    ("AFNI", "tshift"): "3dTshift",
    ("AFNI", "automask"): "3dAutomask",
    ("AFNI", "unifize"): "3dUnifize",
    ("AFNI", "afni-shared"): "larmorx-afni's shared code (datasets, FFT, messages, options)",
    ("FSL", "hmc"): "the head-motion correction (lx.mri.hmc, mcflirt's behaviour)",
    ("FSL", "bet"): "BET (bet, bet2, betsurf)",
    ("FSL", "flirt"): "FLIRT and its programs (convert_xfm, applyxfm4D, avscale, ...)",
    ("FSL", "fast"): "FAST",
    ("FSL", "topup"): "topup and applytopup",
    ("FSL", "fslmaths"): "fslmaths and fslstats",
    ("FSL", "standard_space_roi"): "standard_space_roi",
    ("FSL", "fslroi"): "fslroi",
    ("FSL", "fsl-shared"): "larmorx-fsl's shared code (images, options, messages, data)",
    ("GNU", "cbrtf_glibc"): "cbrtf with glibc 2.35's bits (larmorx-core)",
    ("GNU", "sincos_glibc"): "sin, cos, sincos with glibc 2.35's bits (larmorx-core)",
    ("GNU", "tanh_glibc"): "tanh with glibc 2.35's bits (larmorx-core, from fdlibm)",
}


def tool_of(rel: str) -> tuple[str, str] | None:
    """The (family, tool) a repository path's clean-room code belongs to, or None."""
    if rel in CORE_FILES:
        return CORE_FILES[rel]
    m = re.match(r"^crates/larmorx-(afni|mri|fsl)(?:/(.*))?$", rel)
    if not m:
        return None
    crate, p = m[1], m[2] or ""
    if crate == "mri":
        return ("FSL", "hmc")
    if crate == "afni":
        for t in ("tshift", "automask", "unifize"):
            if re.search(rf"(^|/){t}(/|\.rs$)", p):
                return ("AFNI", t)
        return ("AFNI", "afni-shared")
    for rx, t in FSL_TOOLS:
        if re.search(rx, p):
            return ("FSL", t)
    return ("FSL", "fsl-shared")


# --- roles -------------------------------------------------------------------------------------

ROLE_RULES = (
    # (role, pattern on the brief, pattern on the description); the first match wins.
    ("orchestrator", None, r"^ORCHESTRATOR$"),
    ("audit", r"clean-room audit part", r"[Cc]lean-room audit"),
    (
        "clean-room implementer",
        r"(CLEAN-ROOM implementer|clean-room implementer|CLEAN-ROOM implementation|strict "
        r"clean-room original|Clean-room role|You are completing larmorx's \*\*clean-room)",
        None,
    ),
    ("spec writer", r"\bspec writer\b|\bSPEC WRITER\b", r"\bspec\b"),
    ("replica author", r"\breplica author\b|bit-exact|Port AFNI", r"replica|Port AFNI|Port FSL"),
)


#: Roles settled by reading the brief where the words above would mislead (launch descriptions).
ROLE_BY_DESCRIPTION = {
    # Wired the replica program into the command line and the Python package: read the GPL
    # replica's command line to do so; not a clean-room implementer, not a replica author.
    'implementation="auto": run the GPL replica': "integration",
}


def role_of(description: str, brief: str) -> tuple[str, str]:
    """An agent's role from its launch description and brief: (role, the words that decided)."""
    if description in ROLE_BY_DESCRIPTION:
        return ROLE_BY_DESCRIPTION[description], "read from the brief (ROLE_BY_DESCRIPTION)"
    for role, rx_brief, rx_desc in ROLE_RULES:
        if rx_desc and re.search(rx_desc, description):
            return role, f"description: {description}"
        if rx_brief:
            m = re.search(rx_brief, brief)
            if m:
                return role, f"brief: {m[0]}"
    return "other", f"description: {description}"


# --- times ------------------------------------------------------------------------------------


def epoch(s: str) -> float:
    return datetime.fromisoformat(s.replace("Z", "+00:00")).timestamp()


# --- git -------------------------------------------------------------------------------------

SEP = "\x1e"


def _git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(repo), *args], capture_output=True, text=True, check=True
    ).stdout


def git_commits(repo: Path, paths=CLEANROOM_PATHS) -> dict[str, dict]:
    """Every commit of every ref that changed one of the paths, with its files there."""
    fmt = SEP + "%H%x1f%P%x1f%aI%x1f%cI%x1f%s"
    txt = _git(repo, "log", "--all", "--format=" + fmt, "--name-status", "--", *paths)
    out: dict[str, dict] = {}
    for chunk in txt.split(SEP)[1:]:
        head, _, body = chunk.partition("\n")
        h, parents, ad, cd, subject = head.split("\x1f")
        files = []
        for line in body.splitlines():
            parts = line.split("\t")
            if len(parts) >= 2:
                files.append({"status": parts[0], "path": parts[-1], "from": parts[1]})
        out[h] = {
            "hash": h,
            "parents": parents.split(),
            "author_date": ad,
            "commit_date": cd,
            "subject": subject,
            "files": files,
        }
    return out


def merge_edits(repo: Path, paths=CLEANROOM_PATHS) -> list[dict]:
    """Merges whose result differs from every parent in the paths: the merger's own edits."""
    txt = _git(
        repo, "log", "--all", "--merges", "--cc", "--format=" + SEP + "%H %cI %s", "--", *paths
    )
    out = []
    for chunk in txt.split(SEP)[1:]:
        head, _, body = chunk.partition("\n")
        h, cd, subject = head.split(" ", 2)
        files = re.findall(r"^diff --cc (\S+)$", body, re.M)
        added = [
            x[1:]
            for x in body.splitlines()
            if re.match(r"^[ +-]{2}", x) and x[:2] in ("++", " +", "+ ")
        ]
        removed = [x for x in body.splitlines() if x[:2] in ("--", " -", "- ")]
        out.append(
            {
                "hash": h,
                "commit_date": cd,
                "subject": subject,
                "files": files,
                "lines_added": len(added),
                "lines_removed": len(removed),
                "diff": body[:20_000],
            }
        )
    return out


# --- the transcripts' git commands -----------------------------------------------------------

GIT_MAKE = re.compile(r"\bgit\b[^|;&\n]*\b(commit|merge|cherry-pick|am|rebase)\b")


def git_commands(manifest: list[dict]) -> list[dict]:
    """Every Bash call that commits, merges, cherry-picks or rebases (largest copy of each
    transcript): key, line, time and command."""
    best: dict[str, dict] = {}
    for m in manifest:
        if m["key"] not in best or m["bytes"] > best[m["key"]]["bytes"]:
            best[m["key"]] = m
    out = []
    for key, m in sorted(best.items()):
        with open(m["path"], encoding="utf-8", errors="replace") as fh:
            for n, raw in enumerate(fh, 1):
                if '"tool_use"' not in raw or "git" not in raw:
                    continue
                try:
                    r = json.loads(raw)
                except json.JSONDecodeError:
                    continue
                content = (r.get("message") or {}).get("content")
                if not isinstance(content, list):
                    continue
                for b in content:
                    if not (isinstance(b, dict) and b.get("type") == "tool_use"):
                        continue
                    if b.get("name") != "Bash":
                        continue
                    cmd = str((b.get("input") or {}).get("command", ""))
                    if GIT_MAKE.search(cmd) and r.get("timestamp"):
                        out.append(
                            {"key": key, "line": n, "ts": r["timestamp"], "command": cmd[:4000]}
                        )
    return out


def _norm(s: str) -> str:
    return re.sub(r"[^a-z0-9]+", "", s.lower())


def attribute(
    commits: dict[str, dict],
    printed: list[dict],
    commands: list[dict],
    edits: dict[str, list[tuple[float, str]]],
) -> dict[str, dict]:
    """Each commit's maker: ``printed`` (the hash printed by an agent that ran a commit command
    within ten minutes of the commit's date), ``subject`` (a commit command carrying its subject),
    ``time`` (a commit command within two minutes by an agent that edited one of its files in the
    three hours before), ``printed-only`` (the first print within thirty minutes), or None.

    ``edits``: agent key -> [(time, repository path)] of its edits.
    """
    by_prefix: dict[str, list[dict]] = collections.defaultdict(list)
    for r in printed:
        by_prefix[r["hash"][:7]].append(r)
    cmds: dict[str, list[tuple[float, dict]]] = collections.defaultdict(list)
    for c in commands:
        cmds[c["key"]].append((epoch(c["ts"]), c))

    def near(key: str, when: float, lo: float = -15, hi: float = 600):
        best = None
        for t, c in cmds.get(key, []):
            d = t - when
            if lo <= d <= hi and (best is None or abs(d) < abs(best[0])):
                best = (d, c)
        return best

    out: dict[str, dict] = {}
    for h, c in commits.items():
        cd = epoch(c["commit_date"])
        files = {f["path"] for f in c["files"]}
        res = {"maker": None, "how": None, "delta_s": None, "line": None, "printed_by": []}
        cands = []
        for r in by_prefix.get(h[:7], []):
            if h.startswith(r["hash"]):
                v = near(r["key"], cd)
                cands.append((0 if v else 1, abs(epoch(r["ts"]) - cd), r["key"], v))
        cands.sort(key=lambda x: (x[0], x[1]))
        res["printed_by"] = sorted({x[2] for x in cands})
        if cands and cands[0][0] == 0:
            d, cmd = cands[0][3]
            res.update(maker=cands[0][2], how="printed", delta_s=round(d), line=cmd["line"])
        else:
            subj = _norm(c["subject"])[:50]
            best = None
            for key, lst in cmds.items():
                for t, k in lst:
                    d = t - cd
                    if not -15 <= d <= 900:
                        continue
                    by_subject = bool(subj) and subj in _norm(k["command"])
                    edited = any(
                        p in files and cd - 3 * 3600 <= te <= cd + 60
                        for te, p in edits.get(key, [])
                    )
                    if by_subject:
                        sc = (0, abs(d))
                    elif edited and abs(d) <= 120:
                        sc = (1, abs(d))
                    else:
                        continue
                    if best is None or sc < best[0]:
                        best = (sc, key, d, k["line"])
            if best:
                how = "subject" if best[0][0] == 0 else "time"
                res.update(maker=best[1], how=how, delta_s=round(best[2]), line=best[3])
            elif cands and cands[0][1] <= 1800:
                res.update(maker=cands[0][2], how="printed-only", delta_s=round(cands[0][1]))
        out[h] = res
    return out


# --- the register ----------------------------------------------------------------------------

REGISTER = "docs/findings/upstream-defects.md"


def norm_line(s: str) -> str:
    return re.sub(r"\s+", " ", s.strip())


def split_register(text: str) -> tuple[list[tuple[str, str]], list[str]]:
    """(restricted lines with their section, open lines) of one version of the register."""
    restricted: list[tuple[str, str]] = []
    open_: list[str] = []
    section = None
    for line in text.splitlines():
        if line.startswith("## "):
            section = line[3:].strip() if line.startswith("## Restricted") else None
        if section:
            restricted.append((section, line))
        else:
            open_.append(line)
    return restricted, open_


def register_lines(texts: list[str], min_len: int = 40) -> dict[str, str]:
    """The lines only the restricted sections ever held (normalised -> section), over versions."""
    restricted: dict[str, str] = {}
    open_: set[str] = set()
    for text in texts:
        r, o = split_register(text)
        for sec, line in r:
            n = norm_line(line)
            if len(n) >= min_len:
                restricted.setdefault(n, sec)
        open_.update(norm_line(x) for x in o)
    return {n: s for n, s in restricted.items() if n not in open_}


def register_versions(repo: Path) -> list[str]:
    hs = _git(repo, "log", "--all", "--format=%H", "--", REGISTER).split()
    out = []
    for h in hs:
        try:
            out.append(_git(repo, "show", f"{h}:{REGISTER}"))
        except subprocess.CalledProcessError:
            continue
    return out


PREFIX = re.compile(r"^(?:\s*\d+[\t→]|(?:[^\s:]+:)?\d+[:-])")


def printed_register_lines(text: str, restricted: dict[str, str]) -> list[tuple[str, str]]:
    """The restricted-only lines an output shows (Read's numbers and grep's prefixes removed)."""
    hits = []
    for raw in text.splitlines():
        line = PREFIX.sub("", raw, count=1)
        n = norm_line(line)
        if n in restricted:
            hits.append((restricted[n], n))
    return hits


def is_heading(line: str) -> bool:
    """A section or entry heading: the entry's title, which the open summary table describes."""
    return line.startswith("#")


# --- results of calls -------------------------------------------------------------------------


def fetch_results(wanted: dict[str, set[str]]) -> dict[str, str]:
    """The result text of each call (file -> call ids), long outputs read from their file."""
    out: dict[str, str] = {}
    for path, ids in wanted.items():
        ids = set(ids)
        root = Path(path).parent
        while (
            root.name != "subagents"
            and root != root.parent
            and not (root / "tool-results").is_dir()
        ):
            root = root.parent
        with open(path, encoding="utf-8", errors="replace") as fh:
            for raw in fh:
                if not ids:
                    break
                if '"tool_result"' not in raw:
                    continue
                try:
                    r = json.loads(raw)
                except json.JSONDecodeError:
                    continue
                content = (r.get("message") or {}).get("content")
                if not isinstance(content, list):
                    continue
                for b in content:
                    if isinstance(b, dict) and b.get("tool_use_id") in ids:
                        t = b.get("content")
                        if isinstance(t, list):
                            t = "\n".join(x.get("text", "") for x in t if isinstance(x, dict))
                        t = t or ""
                        m = re.search(r"Full output saved to: (\S+/tool-results/[^\s/]+)", t)
                        if m:
                            p = Path(m[1])
                            for cand in (p, Path(path).parent.parent / "tool-results" / p.name):
                                if cand.is_file():
                                    t = cand.read_text(encoding="utf-8", errors="replace")
                                    break
                        out[b["tool_use_id"]] = t
                        ids.discard(b["tool_use_id"])
    return out


# --- the judgement ----------------------------------------------------------------------------

TOUCH_OPS = {"write", "edit", "delete", "move"}
#: How each kind of restricted access weighs (the first word is the level).
LEVELS = {
    "content": "content of restricted material shown or copied",
    "possible": "a path of restricted material in a script the agent ran (the file may have been opened)",
    "scope": "restricted material inside a search's scope, no line of it printed",
    "names": "names of restricted files only",
    "build": "the replica compiled or its tests run (no file shown)",
    "mention": "a restricted path named in a command's arguments (nothing shown)",
    "replica-run": "the replica program run as a black box (its compiled output)",
    "build-cache": "the replica's build outputs copied, listed or removed (a build cache)",
    "metadata": "a replica crate's licence, notice or package metadata",
    "clean-writer": "notes or reports written only by clean-room implementers (and the briefs)",
}


@dataclass
class Judged:
    level: str
    relevant: bool  # of the tool's family (or of every family) and in force by content
    nature: str
    extra: dict = field(default_factory=dict)


def level_of(r: dict) -> str:
    cat, op = r["category"], r["op"]
    if cat == "replica-build-output":
        return "replica-run" if op in ("exec", "mention", "script-path") else "build-cache"
    if cat == "replica-metadata":
        return "metadata"
    if cat == "restricted-names":
        return "names"
    if cat in ("restricted-build", "restricted-exec"):
        return "build"
    if cat == "restricted-mention":
        return "mention"
    if cat == "restricted-write":
        return "names"  # writing into restricted material shows nothing of it
    if op == "search":
        return "scope"
    if op == "script-path":
        return "possible"
    return "content"


def judge(r: dict, family: str, writer_roles: dict[str, str], part_role) -> Judged:
    """One restricted access against a tool's family (writer_roles: agent key -> role; part_role:
    a docs/parts folder -> 'clean-room' or another kind)."""
    level = level_of(r)
    fam = r.get("family") or ""
    in_force = bool(r.get("in_force"))
    nature = LEVELS[level]
    extra: dict = {}
    if r["rule"] == "R18" and r.get("writers"):
        roles = sorted({writer_roles.get(w, "unknown") for w in r["writers"]})
        extra["writer_roles"] = roles
        if all(x in ("clean-room implementer", "orchestrator", "audit") for x in roles):
            return Judged("clean-writer" if level == "content" else level, False, nature, extra)
        return Judged(level, in_force, nature, extra)
    if r["rule"] == "R13":
        kind = part_role(r.get("part") or "")
        extra["part_kind"] = kind
        if kind == "clean-room":
            return Judged("clean-writer" if level == "content" else level, False, nature, extra)
        return Judged(level, in_force, nature, extra)
    fams = set(re.split(r"[/+]", fam)) if fam else set()
    relevant = in_force and (family in fams or "all" in fams or r["rule"] == "R17")
    return Judged(level, relevant, nature, extra)


def part_kind(name: str) -> str:
    """A docs/parts folder's kind from its name: the clean-room parts are named so."""
    if re.search(r"clean-?room|cleanroom", name):
        return "clean-room"
    if name in ("", "README.md"):
        return "index"
    return "other"


# --- the run ----------------------------------------------------------------------------------


def load_scan(scan: Path) -> tuple[dict, list[dict], list[dict]]:
    agents = json.loads((scan / "agents.json").read_text())
    printed = [json.loads(x) for x in (scan / "commits.jsonl").read_text().splitlines() if x]
    manifest = json.loads((scan / "manifest.json").read_text())
    return agents, printed, manifest


def access_log(scan: Path, info: dict):
    with gzip.open(scan / info["access_log"], "rt", encoding="utf-8") as fh:
        for line in fh:
            yield json.loads(line)


def description(info: dict) -> str:
    meta = info.get("meta") if isinstance(info.get("meta"), dict) else {}
    return "ORCHESTRATOR" if info.get("agent") == "main" else str(meta.get("description", ""))


KEEP = (
    "ts", "op", "via", "tool", "category", "rule", "family", "in_force", "named", "rel", "show",
    "area", "checkout", "file", "line", "call", "hits", "lines", "writers", "part", "unresolved",
    "detail", "path",
)  # fmt: skip


def run(scan: Path, repo: Path) -> dict:
    agents, printed, manifest = load_scan(scan)
    roles = {k: role_of(description(v), v.get("brief") or "") for k, v in agents.items()}
    # Each agent's edits of clean-room code, its restricted accesses and its register accesses.
    edits: dict[str, list[dict]] = collections.defaultdict(list)
    restricted: dict[str, list[dict]] = collections.defaultdict(list)
    register: dict[str, list[dict]] = collections.defaultdict(list)
    for key, info in agents.items():
        for r in access_log(scan, info):
            if r.get("refused"):
                continue
            rel = r.get("rel") or ""
            if r.get("area") == "larmorx" and r["op"] in TOUCH_OPS and tool_of(rel):
                edits[key].append({k: r.get(k) for k in KEEP})
            cat = r["category"]
            if cat.startswith("restricted") or cat in ("replica-build-output", "replica-metadata"):
                keep = {k: r.get(k) for k in KEEP}
                keep["command"] = (r.get("command") or "")[:2000]
                restricted[key].append(keep)
            elif cat == "register" and r["op"] in (
                "read",
                "search-hit",
                "copy",
                "diff-shown",
                "script-path",
            ):
                register[key].append({k: r.get(k) for k in KEEP})
    commits = git_commits(repo)
    commands = git_commands(manifest)
    edit_times = {k: [(epoch(e["ts"]), e["rel"]) for e in v] for k, v in edits.items()}
    makers = attribute(commits, printed, commands, edit_times)
    merges = merge_edits(repo)
    # Touches: (agent, tool) -> times and what.
    touches: dict[tuple[str, tuple[str, str]], list[dict]] = collections.defaultdict(list)
    for key, lst in edits.items():
        for e in lst:
            touches[(key, tool_of(e["rel"]))].append(
                {
                    "ts": e["ts"],
                    "how": f"{e['op']} ({e['via']})",
                    "rel": e["rel"],
                    "line": e["line"],
                }
            )
    for h, c in commits.items():
        m = makers[h]["maker"]
        if not m:
            continue
        for f in c["files"]:
            t = tool_of(f["path"])
            if t:
                touches[(m, t)].append(
                    {
                        "ts": c["commit_date"],
                        "how": f"commit {h[:9]} ({makers[h]['how']})",
                        "rel": f["path"],
                        "line": makers[h]["line"],
                    }
                )
    # The register's restricted-only lines, and what each clean-room toucher's outputs showed.
    reg = register_lines(register_versions(repo))
    touchers = {k for k, _ in touches}
    wanted: dict[str, set[str]] = collections.defaultdict(set)
    for k in touchers:
        for r in register[k]:
            if r.get("call") and r.get("file"):
                wanted[r["file"]].add(r["call"])
    texts = fetch_results(wanted)
    writer_roles = {k: v[0] for k, v in roles.items()}
    tools_out: dict[str, dict] = {}
    for (key, tool), lst in sorted(touches.items(), key=lambda kv: (kv[0][1], kv[0][0])):
        family = tool[0]
        tname = "/".join(tool)
        first = min(x["ts"] for x in lst)
        last = max(x["ts"] for x in lst)
        found = []
        for r in restricted[key]:
            j = judge(r, family, writer_roles, part_kind)
            when = "before or during" if epoch(r["ts"]) <= epoch(last) else "after"
            found.append({**r, "level": j.level, "relevant": j.relevant, "when": when, **j.extra})
        reg_hits = []
        for r in register[key]:
            txt = texts.get(r.get("call") or "", "")
            hits = printed_register_lines(txt, reg) if txt else []
            if hits:
                sec = sorted({s for s, _ in hits})
                fam = sorted({"AFNI" if "AFNI" in s else "FSL" for s in sec})
                when = "before or during" if epoch(r["ts"]) <= epoch(last) else "after"
                body = [x for _, x in hits if not is_heading(x)]
                reg_hits.append(
                    {
                        "ts": r["ts"],
                        "body_lines": len(body),
                        "headings": len(hits) - len(body),
                        "file": r["file"],
                        "line": r["line"],
                        "call": r["call"],
                        "op": r["op"],
                        "via": r["via"],
                        "lines": len(hits),
                        "sections": sec,
                        "families": fam,
                        "relevant": family in fam,
                        "when": when,
                        "sample": [x[:160] for x in (body or [x for _, x in hits])[:3]],
                    }
                )
        entry = tools_out.setdefault(
            tname,
            {
                "family": family,
                "tool": tool[1],
                "name": TOOL_NAMES.get(tool, tname),
                "agents": [],
            },
        )
        entry["agents"].append(
            {
                "key": key,
                "description": description(agents[key]),
                "role": roles[key][0],
                "role_basis": roles[key][1][:300],
                "first_touch": first,
                "last_touch": last,
                "touches": len(lst),
                "commits": sorted(
                    {x["how"].split()[1] for x in lst if x["how"].startswith("commit")}
                ),
                "touch_examples": lst[:5],
                "restricted": found,
                "register": reg_hits,
            }
        )
    for entry in tools_out.values():
        entry["verdict"] = verdict(entry)
    orch = {k: [e for e in v] for k, v in edits.items() if roles[k][0] == "orchestrator"}
    orch_commits = [
        {"hash": h, **{x: commits[h][x] for x in ("commit_date", "subject", "files")}, **makers[h]}
        for h in commits
        if makers[h]["maker"] and roles.get(makers[h]["maker"], ("",))[0] == "orchestrator"
    ]
    return {
        "tools": tools_out,
        "commits": {h: {**commits[h], **makers[h]} for h in commits},
        "merge_edits": merges,
        "orchestrator_edits": orch,
        "orchestrator_commits": orch_commits,
        "roles": {
            k: {"role": v[0], "basis": v[1][:300], "description": description(agents[k])}
            for k, v in roles.items()
        },
        "register_restricted_lines": len(reg),
        "unattributed": sorted(h for h in commits if not makers[h]["maker"]),
    }


SEVERE = ("content", "possible")


def verdict(entry: dict) -> dict:
    """A tool's verdict from its touchers' judged accesses (the dossier's words)."""
    notes = collections.Counter()
    findings = []
    non_clean = []
    for a in entry["agents"]:
        if a["role"] not in ("clean-room implementer",):
            non_clean.append(
                {"description": a["description"], "role": a["role"], "touches": a["touches"]}
            )
        for r in a["restricted"]:
            if r["when"] != "before or during":
                continue
            if r["relevant"] and r["level"] in SEVERE:
                findings.append((a["description"], r["rule"], r["level"], r["show"], r["ts"]))
            else:
                notes[(r["rule"], r["level"], "relevant" if r["relevant"] else "other")] += 1
        for g in a["register"]:
            if g["when"] != "before or during":
                continue
            if g["relevant"] and g["body_lines"]:
                findings.append((a["description"], "R12", "content", REGISTER, g["ts"]))
            else:
                kind = "body" if g["body_lines"] else "headings"
                notes[("R12", kind, "relevant" if g["relevant"] else "other")] += 1
    if findings:
        status = "review"
    elif non_clean:
        status = "clean code, edits by other roles listed"
    elif notes:
        status = "clean, with notes"
    else:
        status = "clean"
    return {
        "status": status,
        "findings": findings,
        "notes": {"/".join(k): n for k, n in sorted(notes.items())},
        "other_roles": non_clean,
    }


def add_arguments(sub) -> None:
    v = sub.add_parser("verdicts", help="the verdicts per clean-room original, from a scan")
    v.add_argument("--scan", required=True, help="a scan's output directory")
    v.add_argument("--out", required=True, help="a private directory (never committed)")
    v.add_argument("--repo", help="the larmorx checkout whose history is read (default: this one)")


def main(args: argparse.Namespace) -> int:
    repo = Path(args.repo) if args.repo else Path(__file__).resolve().parents[4]
    res = run(Path(args.scan), repo)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / "verdicts.json").write_text(
        json.dumps(res, indent=1, sort_keys=True, default=list) + "\n"
    )
    for name, e in sorted(res["tools"].items()):
        print(f"{name:28s} {len(e['agents']):3d} agents  {e['verdict']['status']}")
    print(
        f"unattributed commits: {len(res['unattributed'])}; merges with own edits: {len(res['merge_edits'])}"
    )
    return 0
