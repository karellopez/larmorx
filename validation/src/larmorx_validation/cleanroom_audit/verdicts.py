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
    (r"(^|/)prelude(/|\.rs$)", "prelude"),
)
#: What each tool is the clean-room original of (the public summary's names).
TOOL_NAMES = {
    ("AFNI", "tshift"): "3dTshift",
    ("AFNI", "automask"): "3dAutomask",
    ("AFNI", "unifize"): "3dUnifize",
    ("AFNI", "volreg"): "3dvolreg",
    ("AFNI", "afni-shared"): "larmorx-afni's shared code (datasets, FFT, messages, options)",
    ("FSL", "hmc"): "the head-motion correction (lx.mri.hmc, mcflirt's behaviour)",
    ("FSL", "bet"): "BET (bet, bet2, betsurf)",
    ("FSL", "flirt"): "FLIRT and its programs (convert_xfm, applyxfm4D, avscale, ...)",
    ("FSL", "fast"): "FAST",
    ("FSL", "topup"): "topup and applytopup",
    ("FSL", "fslmaths"): "fslmaths and fslstats",
    ("FSL", "standard_space_roi"): "standard_space_roi",
    ("FSL", "fslroi"): "fslroi",
    ("FSL", "prelude"): "prelude (phase unwrapping)",
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
        for t in ("tshift", "automask", "unifize", "volreg"):
            if re.search(rf"(^|/){t}(/|\.rs$)", p):
                return ("AFNI", t)
        return ("AFNI", "afni-shared")
    for rx, t in FSL_TOOLS:
        if re.search(rx, p):
            return ("FSL", t)
    return ("FSL", "fsl-shared")


# --- roles -------------------------------------------------------------------------------------

#: The role a launch description names (the first match wins).
ROLE_BY_WORDS = (
    ("orchestrator", r"^ORCHESTRATOR$"),
    ("audit", r"[Cc]lean-room audit"),
    ("spec writer", r"\(spec\)|spec answers|behaviour spec|[Ss]pec writer"),
    ("replica author", r"[Rr]eplica|^Port (AFNI|FSL)"),
    ("clean-room implementer", r"[Cc]lean-room|[Cc]lean room"),
)
#: The role a brief states about its reader (in its "You are ..." and "Your role ..." sentences
#: first, then anywhere), when the description names none; the first match wins.
ROLE_IN_BRIEF = (
    (
        "replica author",
        r"replica author|replica-author|relay agent of the \*\*\S+ replica|\*\*replica author",
    ),
    ("spec writer", r"\bthe spec writer\b|\*\*spec writer"),
    (
        "clean-room implementer",
        r"CLEAN-ROOM implementer|clean-room implementer|CLEAN-ROOM implementation|strict "
        r"clean-room original|## Clean-room role|You are completing larmorx's \*\*clean-room",
    ),
)
SELF = re.compile(
    r"((?:You are|Your role is|\*{0,2}Your role:|## Your role)[^\n]{0,400}?)(?=\.\s|\n|$)"
)


#: Roles settled by reading the brief where the words above would mislead (launch descriptions).
ROLE_BY_DESCRIPTION = {
    # Wired the replica program into the command line and the Python package: read the GPL
    # replica's command line to do so; not a clean-room implementer, not a replica author.
    'implementation="auto": run the GPL replica': "integration",
    # "You are porting AFNI programs": the replicas of 3dAutomask and 3dUnifize and their specs.
    "AFNI 3dAutomask and 3dUnifize": "replica author",
    # Wrote the porting guide and the defect register from the findings (source-derived ones
    # included); its brief describes the clean-room implementer's rules, it is not one.
    "Porting guide + upstream defect register": "documentation",
}


def role_of(description: str, brief: str) -> tuple[str, str]:
    """An agent's role from its launch description and brief: (role, the words that decided)."""
    if description in ROLE_BY_DESCRIPTION:
        return ROLE_BY_DESCRIPTION[description], "read from the brief (ROLE_BY_DESCRIPTION)"
    for role, rx in ROLE_BY_WORDS:
        if re.search(rx, description):
            return role, f"description: {description}"
    own = "\n".join(m[1] for m in SELF.finditer(brief))
    for text in (own, brief):
        for role, rx in ROLE_IN_BRIEF:
            m = re.search(rx, text)
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
    transcript): key, line, time, command and the head of its output."""
    best: dict[str, dict] = {}
    for m in manifest:
        if m["key"] not in best or m["bytes"] > best[m["key"]]["bytes"]:
            best[m["key"]] = m
    out = []
    for key, m in sorted(best.items()):
        pending: dict[str, dict] = {}
        with open(m["path"], encoding="utf-8", errors="replace") as fh:
            for n, raw in enumerate(fh, 1):
                if '"tool_use"' in raw and "git" in raw:
                    kind = "use"
                elif pending and '"tool_result"' in raw:
                    kind = "result"
                else:
                    continue
                try:
                    r = json.loads(raw)
                except json.JSONDecodeError:
                    continue
                content = (r.get("message") or {}).get("content")
                if not isinstance(content, list):
                    continue
                for b in content:
                    if not isinstance(b, dict):
                        continue
                    if kind == "result" and b.get("tool_use_id") in pending:
                        t = b.get("content")
                        if isinstance(t, list):
                            t = "\n".join(x.get("text", "") for x in t if isinstance(x, dict))
                        pending.pop(b["tool_use_id"])["result"] = (t or "")[:4000]
                        continue
                    if b.get("type") != "tool_use" or b.get("name") != "Bash":
                        continue
                    cmd = str((b.get("input") or {}).get("command", ""))
                    if GIT_MAKE.search(cmd) and r.get("timestamp"):
                        rec = {"key": key, "line": n, "ts": r["timestamp"], "command": cmd[:4000]}
                        rec["result"] = ""
                        out.append(rec)
                        pending[b.get("id", "")] = rec
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

    def near(key: str, when: float, lo: float = -1800, hi: float = 600):
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
        if not cands:
            # A hash printed in a commit command's own output that the scan's reader did not take
            # for a commit (another line before it).
            for key, lst in cmds.items():
                for t, k in lst:
                    # a command can run long before its commit (a build first): its output's
                    # hash is the check
                    if -1800 <= t - cd <= 600 and re.search(rf"\b{h[:7]}\b", k.get("result", "")):
                        v = (t - cd, k)
                        cands.append((0, abs(t - cd), key, v))
            cands.sort(key=lambda x: (x[0], x[1]))
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
    brief_reads: dict[str, dict] = {}
    brief_dep: dict[str, list[dict]] = collections.defaultdict(list)
    case_first_writer: dict[str, str] = {}
    for key, info in agents.items():
        for r in access_log(scan, info):
            if r.get("refused"):
                continue
            path = str(r.get("path", ""))
            if (
                key not in brief_reads
                and r["op"] == "read"
                and re.search(r"/[^/]*brief[^/]*\.md$", path, re.I)
                and path in (info.get("brief") or "")
                and r.get("call")
            ):
                brief_reads[key] = r
            rel = r.get("rel") or ""
            if r.get("area") == "larmorx" and r["op"] in TOUCH_OPS and tool_of(rel):
                edits[key].append({k: r.get(k) for k in KEEP})
            cat = r["category"]
            if cat.startswith("restricted") or cat in ("replica-build-output", "replica-metadata"):
                keep = {k: r.get(k) for k in KEEP}
                keep["command"] = (r.get("command") or "")[:2000]
                restricted[key].append(keep)
            if (
                r.get("area") == "larmorx"
                and r["op"] in ("write", "edit")
                and rel.startswith("validation/difftest/cases/")
                and rel not in case_first_writer
            ):
                case_first_writer[rel] = key
            if (
                r.get("area") == "larmorx"
                and (r["op"] in CONTENT_OPS or r["op"] == "search")
                and (
                    BRIEF_RESTRICTED.search(rel)
                    or BRIEF_FILES.search(rel)
                    or rel.startswith("validation/difftest/cases/")
                )
            ):
                keep = {k: r.get(k) for k in KEEP}
                keep["command"] = (r.get("command") or "")[:2000]
                brief_dep[key].append(keep)
            if cat == "register" and r["op"] in (
                "search",
                "read",
                "search-hit",
                "copy",
                "diff-shown",
                "script-path",
            ):
                register[key].append({k: r.get(k) for k in KEEP})
    # A prompt that names its brief's file: the role is read from that file, as the agent read
    # it (its own words about itself in the prompt first), unless the description settles it.
    want: dict[str, set[str]] = collections.defaultdict(set)
    for r in brief_reads.values():
        want[r["file"]].add(r["call"])
    brief_texts = fetch_results(want)
    for key, r in brief_reads.items():
        desc = description(agents[key])
        if desc in ROLE_BY_DESCRIPTION or any(re.search(rx, desc) for _, rx in ROLE_BY_WORDS):
            continue
        text = brief_texts.get(r["call"])
        if text:
            prompt = agents[key].get("brief") or ""
            own = "\n".join(m[1] for m in SELF.finditer(prompt))
            role, basis = role_of(desc, own + "\n" + text)
            if role != "other":
                roles[key] = (role, f"its brief file: {basis}")
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
    implementers = {k for k, v in roles.items() if v[0] == "clean-room implementer"}
    wanted: dict[str, set[str]] = collections.defaultdict(set)
    for k in touchers | implementers:
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
        for a in entry["agents"]:
            if a["role"] != "clean-room implementer":
                a["diffs"] = {h: diff_nature(repo, h) for h in a["commits"]}
    orch = {k: [e for e in v] for k, v in edits.items() if roles[k][0] == "orchestrator"}
    orch_commits = [
        {"hash": h, **{x: commits[h][x] for x in ("commit_date", "subject", "files")}, **makers[h]}
        for h in commits
        if makers[h]["maker"] and roles.get(makers[h]["maker"], ("",))[0] == "orchestrator"
    ]
    slips = implementer_slips(
        agents, roles, restricted, register, texts, reg, writer_roles, touches
    )
    cases = case_writers(repo, printed, commands, {r["rel"] for v in brief_dep.values() for r in v})
    brief_dependent = []
    for key in sorted(k for k, v in roles.items() if v[0] == "clean-room implementer"):
        for r in brief_dep.get(key, []):
            rel = r["rel"]
            if rel.startswith("validation/difftest/cases/"):
                w = cases.get(rel) or case_first_writer.get(rel)
                wrole = roles[w][0] if w in roles else "unknown"
                if w == key or wrole == "clean-room implementer":
                    continue  # a clean-room part's own case
                if w is None and (
                    rel.rsplit("/", 1)[-1].startswith("original-")
                    or re.match(r"^validation/difftest/cases/[^/]+-original(/|$)", rel)
                ):
                    continue  # named for the clean-room original that differed: its own case
                kind = f"R15: a permanent case added by a {wrole}"
            elif BRIEF_FILES.search(rel):
                if epoch(r["ts"]) < epoch(BRIEF_FILES_SINCE):
                    continue
                desc = description(agents[key])
                if any(desc.startswith(d) and rel == p for d, p in BRIEF_ALLOWED):
                    continue
                if rel.endswith("triage.toml") and r["op"] in ("script-path", "copy"):
                    continue  # the syntax check with tomllib the briefs ask for, or a backup
                kind = (
                    "R15: the random-testing triage's entries (its brief: do not read them)"
                    if rel.endswith("triage.toml")
                    else "R16: a replica part's or spec writer's investigation (its brief: never open)"
                )
            else:
                desc = description(agents[key])
                if not any(desc.startswith(p) for p in R16_FORBIDDEN_BY):
                    continue  # its brief named the replica's suite for its cases and mechanics
                kind = "R16: a replica's suite or evidence module (its brief: never read)"
            brief_dependent.append(
                {
                    "agent": key,
                    "description": description(agents[key]),
                    "kind": kind,
                    **{
                        k: r.get(k)
                        for k in ("ts", "op", "via", "rel", "file", "line", "call", "command")
                    },
                }
            )
    # The lines each read printed: a search of one file names none (its output is the check); the
    # triage's: a replica author's entry, or only clean-room ones?
    want = collections.defaultdict(set)
    for x in brief_dependent:
        if x.get("call") and (x["rel"].endswith("triage.toml") or x["op"] == "search"):
            want[x["file"]].add(x["call"])
    shown = fetch_results(want)
    kept = []
    seen_search: set[tuple[str, str]] = set()
    for x in brief_dependent:
        if x["op"] == "search":
            if any(ch in x["rel"] for ch in "*?[") or not re.search(r"\.\w+$", x["rel"]):
                continue  # a folder or a glob: the names its lines carry are the hits
            t = shown.get(x.get("call") or "", "")
            hit = any(c["call"] == x["call"] and c["op"] != "search" for c in brief_dependent)
            if hit or not t.strip() or (x["agent"], x["call"]) in seen_search:
                continue  # its lines are counted as hits, or nothing was printed
            n = lines_of_file_shown(repo, x["rel"], t)
            if not n:
                continue  # the call printed nothing of that file
            seen_search.add((x["agent"], x["call"]))
            x["kind"] += f"; a search printed {n} of its lines"
        if x["rel"].endswith("triage.toml"):
            t = shown.get(x.get("call") or "", "")
            if not REPLICA_ENTRY.search(t):
                continue  # only clean-room entries (the part's own) were printed
            x["kind"] += "; a replica author's entry printed"
        kept.append(x)
    brief_dependent = kept
    # A grep of one restricted file prints no name: its output is compared with the file's lines.
    want = collections.defaultdict(set)
    single = []
    for key in implementers:
        for r in restricted[key]:
            rel = r.get("rel") or ""
            if (
                r["op"] != "search"
                or any(ch in rel for ch in "*?[")
                or not re.search(r"\.\w+$", rel)
            ):
                continue
            want[r["file"]].add(r["call"])
            single.append((key, r))
    shown = fetch_results(want)
    single_file_scopes = {"checked": len(single), "shown": []}
    for key, r in single:
        t = shown.get(r.get("call") or "", "")
        n = lines_of_file_shown(repo, r["rel"], t) if t.strip() else 0
        if n:
            single_file_scopes["shown"].append(
                {"agent": key, "description": description(agents[key]), "lines": n, **r}
            )
    return {
        "single_file_scopes": single_file_scopes,
        "slips": slips,
        "brief_dependent": brief_dependent,
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
    """A tool's verdict: the clean-room implementers' judged accesses (the key question: did any
    of them see restricted material of this family before or while writing it?), and apart, the
    agents of other roles (the orchestrator, integration, a replica author) that touched it."""
    notes: collections.Counter = collections.Counter()
    findings = []
    other = []
    for a in entry["agents"]:
        seen = [
            r
            for r in a["restricted"]
            if r["when"] == "before or during" and r["relevant"] and r["level"] in SEVERE
        ]
        seen_reg = [
            g
            for g in a["register"]
            if g["when"] == "before or during" and g["relevant"] and g["body_lines"]
        ]
        if a["role"] != "clean-room implementer":
            other.append(
                {
                    "description": a["description"],
                    "role": a["role"],
                    "touches": a["touches"],
                    "commits": a["commits"],
                    "relevant_content_before": len(seen) + len(seen_reg),
                }
            )
            continue
        for r in seen:
            findings.append((a["description"], r["rule"], r["level"], r["show"], r["ts"]))
        for g in seen_reg:
            findings.append((a["description"], "R12", "body lines", REGISTER, g["ts"]))
        for r in a["restricted"]:
            if r["when"] == "before or during" and r not in seen:
                notes[(r["rule"], r["level"], "relevant" if r["relevant"] else "other")] += 1
        for g in a["register"]:
            if g["when"] == "before or during" and g not in seen_reg:
                kind = "body" if g["body_lines"] else "headings"
                notes[("R12", kind, "relevant" if g["relevant"] else "other")] += 1
    if findings:
        status = "review"
    elif other:
        status = "clean (implementers); edits by other roles listed"
    elif notes:
        status = "clean, with notes"
    else:
        status = "clean"
    return {
        "status": status,
        "findings": findings,
        "notes": {"/".join(k): n for k, n in sorted(notes.items())},
        "other_roles": other,
    }


#: Files a clean-room brief may forbid (R16): the replicas' parity suites and evidence modules.
BRIEF_RESTRICTED = re.compile(
    r"^validation/src/larmorx_validation/(?:parity|evidence|difftest/tools)/"
    r"(?:\w*replica\w*|fsl_mcflirt\w*|afni_volreg)\.py$"
)
CONTENT_OPS = frozenset({"read", "search-hit", "copy", "diff-shown", "script-path"})
#: Files the clean-room briefs forbid from the clean-room fixes part on (2026-10-29): the
#: random-testing triage (replica authors' entries; appending one's own is allowed) and the
#: investigations of replica parts and spec writers.
BRIEF_FILES = re.compile(
    r"^validation/difftest/triage\.toml$|^validation/investigations/(?:fsl-bet|fsl-fast|fsl-flirt|"
    r"afni-volreg|fsl-maths|fsl-mcflirt-replica|fsl-prelude|fsl-topup|flirt-spec-answers|bet-spec-answers|spec-answers-\w+|"
    r"version-\w+|specs-memory|replica-side|afni-move|replica-glibc-dbl|bet-options-site)/|"
    r"^validation/investigations/glibc-dbl/tables\.py$"
)
BRIEF_FILES_SINCE = "2026-10-29T01:20:00+01:00"
#: A replica author's triage entry in an output (TOML or a printed dict).
REPLICA_ENTRY = re.compile(r"implementation\s*=\s*\"replica\"|'implementation': 'replica'")
#: Files of those folders a brief named for its reader (launch description prefix, path).
BRIEF_ALLOWED = (
    # "the block hashes of glibc's results, ...-blocks.json (that one file only: black-box output)"
    (
        "Clean-room cbrtf for reproduce",
        "validation/investigations/specs-memory/cbrtf-glibc-2.35-x86_64-blocks.json",
    ),
    # "the spec writer's README validation/investigations/bet-spec-answers/README.md"
    ("BET clean-room part 2a", "validation/investigations/bet-spec-answers/README.md"),
)
#: The parts whose briefs forbade the replicas' suites (the clean-room catch-up, 2026-11-04: "The
#: replicas' suites run the replica as a black box; read only your originals' suites and code").
#: FLIRT's, FAST's, topup's and fslmaths' clean-room briefs named the replica's suite "for its
#: cases and mechanics"; the earlier briefs say nothing of it (no rule restricted it).
R16_FORBIDDEN_BY = ("Clean-room catch-up",)


_FILE_LINES: dict[str, set[str]] = {}


def lines_of_file_shown(repo: Path, rel: str, text: str) -> int:
    """How many lines of an output are lines of a repository file (its version on main; grep's
    ``N:`` prefixes removed; lines shorter than 8 characters ignored)."""
    if rel not in _FILE_LINES:
        try:
            body = _git(repo, "show", f"main:{rel}")
        except subprocess.CalledProcessError:
            body = ""
        _FILE_LINES[rel] = {norm_line(x) for x in body.splitlines() if len(norm_line(x)) >= 8}
    have = _FILE_LINES[rel]
    return sum(1 for raw in text.splitlines() if norm_line(PREFIX.sub("", raw, count=1)) in have)


def case_writers(repo: Path, printed, commands, paths: set[str]) -> dict[str, str | None]:
    """Who added each permanent random-testing case (the maker of the commit that added it)."""
    out: dict[str, str | None] = {}
    if not paths:
        return out
    txt = _git(
        repo,
        "log",
        "--all",
        "--diff-filter=A",
        "--format=" + SEP + "%H%x1f%cI%x1f%s",
        "--name-only",
        "--",
        "validation/difftest/cases",
    )
    added: dict[str, dict] = {}
    for chunk in txt.split(SEP)[1:]:
        head, _, body = chunk.partition("\n")
        h, cd, subject = head.split("\x1f")
        for f in body.splitlines():
            if f.strip() and f.strip() not in added:
                added[f.strip()] = {"hash": h, "commit_date": cd, "subject": subject, "files": []}
    wanted = {p: added[p] for p in paths if p in added}
    makers = attribute({c["hash"]: c for c in wanted.values()}, printed, commands, {})
    for p, c in wanted.items():
        out[p] = makers[c["hash"]]["maker"]
    return out


def implementer_slips(agents, roles, restricted, register, texts, reg, writer_roles, touches):
    """Every clean-room implementer's accesses that showed restricted material (any family, any
    time; material only clean-room implementers wrote excepted), one entry per call: the list
    the declared slips are compared with."""
    touched: dict[str, set[str]] = collections.defaultdict(set)
    for (k, tool), _ in touches.items():
        touched[k].add("/".join(tool))
    out = []
    for key in sorted(k for k, v in roles.items() if v[0] == "clean-room implementer"):
        calls: dict[str, dict] = {}
        for r in restricted[key]:
            j = judge(r, "", writer_roles, part_kind)
            if j.level not in SEVERE or not r.get("in_force"):
                continue
            c = calls.setdefault(
                r.get("call") or f"line-{r['line']}",
                {
                    "ts": r["ts"],
                    "file": r["file"],
                    "line": r["line"],
                    "command": r.get("command") or r.get("detail") or "",
                    "rules": collections.Counter(),
                    "paths": set(),
                },
            )
            c["rules"][f"{r['rule']}:{r.get('family')}:{j.level}"] += 1
            c["paths"].add(r["show"])
        for g in register[key]:
            txt = texts.get(g.get("call") or "", "")
            hits = printed_register_lines(txt, reg) if txt else []
            if not hits:
                continue
            c = calls.setdefault(
                g.get("call") or f"line-{g['line']}",
                {
                    "ts": g["ts"],
                    "file": g["file"],
                    "line": g["line"],
                    "command": g.get("command") or "",
                    "rules": collections.Counter(),
                    "paths": set(),
                },
            )
            body = sum(1 for _, x in hits if not is_heading(x))
            sections = "+".join(sorted({s.split(":")[1].split("(")[0].strip() for s, _ in hits}))
            k = f"R12:{sections}:{'body' if body else 'headings'}"
            # one call's output, counted once whatever the number of its access records
            c["rules"][k] = max(c["rules"][k], len(hits))
            c["register_lines"] = {"body": body, "headings": len(hits) - body}
            c["paths"].add(REGISTER)
            c.setdefault("register_sample", [x[:160] for _, x in hits if not is_heading(x)][:3])
        for cid, c in sorted(calls.items(), key=lambda kv: kv[1]["ts"]):
            out.append(
                {
                    "agent": key,
                    "description": description(agents[key]),
                    "tools_touched": sorted(touched.get(key, ())),
                    "call": cid,
                    **{k: v for k, v in c.items() if k not in ("rules", "paths")},
                    "rules": dict(c["rules"]),
                    "paths": sorted(c["paths"]),
                }
            )
    return out


def diff_nature(repo: Path, h: str, paths=CLEANROOM_PATHS) -> dict:
    """What a commit changed in the clean-room code: lines added and removed, and how many of
    them are comments, blank or package metadata (a merge: its own resolution, ``--cc``)."""
    txt = _git(repo, "show", "--cc", "--format=", h, "--", *paths)
    added = removed = trivial = 0
    for line in txt.splitlines():
        if line.startswith(("+++", "---", "diff ", "index ", "@@")):
            continue
        m = re.match(r"^([ +-]{1,2})(.*)$", line)
        if not m or ("+" not in m[1] and "-" not in m[1]) or m[1].strip() == "":
            continue
        body = m[2].strip()
        if "+" in m[1]:
            added += 1
        else:
            removed += 1
        if not body or body.startswith(("//", "#", "/*", "*")):
            trivial += 1
    files = re.findall(r"^diff --(?:git a/\S+ b/|cc )(\S+)$", txt, re.M)
    return {"files": files, "added": added, "removed": removed, "comments_or_blank": trivial}


def _cmd(x: dict, n: int = 240) -> str:
    c = (x.get("command") or x.get("detail") or "").replace("\n", " | ").replace("`", "'")
    return c[:n] + ("..." if len(c) > n else "")


def _ev(x: dict, key: str | None = None) -> str:
    """Evidence: session/agent, the transcript file's line, the time."""
    k = key or x.get("agent") or ""
    return f"`{k}` line {x.get('line')} ({x.get('ts', '')[:19]}Z), `{x.get('file', '')}`"


def write_markdown(res: dict, path: Path, scan: str, repo_head: str) -> None:
    """The private dossier: every verdict with its evidence (session, record line, time)."""
    L: list[str] = []
    w = L.append
    w("# The clean-room audit: verdicts (private: paths, sessions, command lines)\n")
    w(f"Scan: `{scan}`; repository at `{repo_head}`. Generated by `cleanroom-audit verdicts`.\n")
    w("## Per clean-room original\n")
    w("| tool | family | agents (implementers / other roles) | status | findings |")
    w("|---|---|---|---|---|")
    for _name, e in sorted(res["tools"].items()):
        n_impl = sum(1 for a in e["agents"] if a["role"] == "clean-room implementer")
        n_other = len(e["agents"]) - n_impl
        v = e["verdict"]
        w(
            f"| {e['name']} | {e['family']} | {n_impl} / {n_other} | {v['status']} | {len(v['findings'])} |"
        )
    for name, e in sorted(res["tools"].items()):
        v = e["verdict"]
        w(f"\n### {e['name']} (`{name}`): {v['status']}\n")
        w("| agent | role | first touch | last touch | touches | commits |")
        w("|---|---|---|---|---|---|")
        for a in sorted(e["agents"], key=lambda a: a["first_touch"]):
            w(
                f"| {a['description']} (`{a['key']}`) | {a['role']} | {a['first_touch'][:16]} | "
                f"{a['last_touch'][:16]} | {a['touches']} | {' '.join(a['commits'][:6])}"
                f"{' ...' if len(a['commits']) > 6 else ''} |"
            )
        if v["findings"]:
            w(
                "\nFindings (an implementer saw restricted material of this family before or while writing it):\n"
            )
            for a in e["agents"]:
                for r in a["restricted"]:
                    if r["when"] == "before or during" and r["relevant"] and r["level"] in SEVERE:
                        w(
                            f"- {a['description']}: {r['rule']} {r['level']} `{r['show']}`; {_ev(r, a['key'])}; `{_cmd(r)}`"
                        )
                for g in a["register"]:
                    if g["when"] == "before or during" and g["relevant"] and g["body_lines"]:
                        w(
                            f"- {a['description']}: R12 {g['body_lines']} body lines, {g['headings']} "
                            f"headings of {', '.join(g['sections'])}; {_ev(g, a['key'])}; e.g. "
                            f"{'; '.join(g['sample'][:2])}"
                        )
        if v["other_roles"]:
            w("\nEdits by agents of other roles:\n")
            for o in v["other_roles"]:
                a = next(x for x in e["agents"] if x["description"] == o["description"])
                diffs = "; ".join(
                    f"{h}: +{d['added']}/-{d['removed']} ({d['comments_or_blank']} comments or blank)"
                    for h, d in a.get("diffs", {}).items()
                )
                ex = "; ".join(
                    f"{t['how']} {t['rel']} {t['ts'][:16]}" for t in a["touch_examples"][:3]
                )
                w(
                    f"- {o['description']} ({o['role']}): {o['touches']} touches; restricted content of "
                    f"this family seen before: {o['relevant_content_before']}; {diffs or 'no commit'}; {ex}"
                )
        if v["notes"]:
            w(
                "\nOther accesses of the implementers before or during the work (rule/level/family): "
                + ", ".join(f"{k} {n}" for k, n in v["notes"].items())
            )
    w("\n## Every clean-room implementer's accesses that showed restricted material (any family)\n")
    for x in res["slips"]:
        sample = f"; e.g. {'; '.join(x['register_sample'][:2])}" if x.get("register_sample") else ""
        w(
            f"- {x['description']} (touched: {', '.join(x['tools_touched']) or 'no clean-room code'}): "
            f"{x['rules']} {', '.join(x['paths'][:4])}; {_ev(x)}; `{_cmd(x)}`{sample}"
        )
    w("\n## Files the implementers' briefs forbade (R15, R16)\n")
    for x in res["brief_dependent"]:
        w(f"- {x['description']}: {x['kind']}; `{x['rel']}` ({x['op']}, {x['via']}); {_ev(x)}")
    sf = res.get("single_file_scopes", {"checked": 0, "shown": []})
    w(
        f"\nGreps of one restricted file by implementers (no file name printed): {sf['checked']}; "
        f"with lines of that file (on main) in their output: {len(sf['shown'])}."
    )
    for x in sf["shown"]:
        w(
            f"- {x['description']}: {x['rule']} `{x['rel']}`, {x['lines']} lines; {_ev(x, x['agent'])}"
        )
    w("\n## The orchestrator's own edits of clean-room code\n")
    for k, lst in res["orchestrator_edits"].items():
        for e in lst:
            w(f"- `{k}`: {e['op']} ({e['via']}) `{e['rel']}`; {_ev(e, k)}")
    for c in res["orchestrator_commits"]:
        files = ", ".join(f["path"] for f in c["files"][:6])
        w(
            f"- commit `{c['hash'][:9]}` {c['commit_date'][:16]} ({c['how']}, line {c['line']}): {c['subject'][:160]}; {files}"
        )
    for m in res["merge_edits"]:
        w(
            f"- merge `{m['hash'][:9]}` {m['commit_date'][:16]}: its own resolution in {', '.join(m['files'])} "
            f"(+{m['lines_added']}/-{m['lines_removed']}): {m['subject'][:120]}"
        )
        w("\n```diff\n" + m["diff"][:3000] + "\n```")
    w("\n## Commits of clean-room code and their makers\n")
    how = collections.Counter(c["how"] for c in res["commits"].values())
    w(
        f"{len(res['commits'])} commits; attributed by: {dict(how)}; unattributed: {len(res['unattributed'])}.\n"
    )
    w("| commit | date | maker | how | subject |")
    w("|---|---|---|---|---|")
    for h, c in sorted(res["commits"].items(), key=lambda kv: kv[1]["commit_date"]):
        role = res["roles"].get(c["maker"] or "", {})
        w(
            f"| `{h[:9]}` | {c['commit_date'][:16]} | {role.get('description', '?')} ({role.get('role', '?')}) "
            f"`{c['maker']}` | {c['how']} (line {c['line']}) | {c['subject'][:90]} |"
        )
    path.write_text("\n".join(L) + "\n", encoding="utf-8")


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
    head = _git(repo, "rev-parse", "--short", "HEAD").strip()
    write_markdown(res, out / "verdicts.md", str(args.scan), head)
    (out / "verdicts.json").write_text(
        json.dumps(res, indent=1, sort_keys=True, default=list) + "\n"
    )
    for name, e in sorted(res["tools"].items()):
        print(f"{name:28s} {len(e['agents']):3d} agents  {e['verdict']['status']}")
    print(
        f"unattributed commits: {len(res['unattributed'])}; merges with own edits: {len(res['merge_edits'])}"
    )
    return 0
