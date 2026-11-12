# SPDX-License-Identifier: Apache-2.0
"""The restriction table: what a clean-room implementer must never open, and since when.

Written from the rules (CLAUDE.md rule 1 and "Agent working rules", ``docs/porting-guide.md`` §1,
``docs/licensing.md``) and the dates their history in git gives. Every entry has two dates:
``since``, when its content became restricted by nature (the decision of 2026-10-09 for the
upstream sources and the replicas, a file's first commit for a source-derived finding), and
``named``, when the rules first named it (main's first-parent history). An access between the two
is reported as restricted "before the rule named it": reading source-derived content contaminates
whatever the rule said that day.

:func:`locate` turns an absolute path into a logical location (an area and a path inside it, the
form the public summary may show); :func:`classify` gives the location's category, rule and
family, for an operation (``read``, ``exec``, ...). Black-box runs of an original program are
classified as ``run``, never as reading.
"""

from __future__ import annotations

import fnmatch
import posixpath
import re
from dataclasses import dataclass, field

#: The day the strict clean-room rule was decided (decision log, 2026-10-09: "Replica and
#: original for every restricted upstream"); CLAUDE.md rule 1 from commit 8d332534.
DAY_ONE = "2026-10-09T00:00:00+02:00"


@dataclass(frozen=True)
class Rule:
    """One entry of the restriction table."""

    id: str
    title: str
    family: str
    since: str
    named: str
    basis: str


RULES: dict[str, Rule] = {
    r.id: r
    for r in (
        Rule(
            "R01",
            "FSL's source (crates-nc/fsl-source, the installation's src, include, pkgs, its "
            "scripts and Python modules read as text)",
            "FSL",
            DAY_ONE,
            "2026-10-09T23:26:43+02:00",
            "CLAUDE.md rule 1 (8d332534); decision log 2026-10-09",
        ),
        Rule(
            "R02",
            "AFNI's source (MCW and NIH files alike; the clean-room originals stay clean after "
            "the CC BY 4.0 move of 2026-10-29)",
            "AFNI",
            DAY_ONE,
            "2026-10-09T23:26:43+02:00",
            "decision log 2026-10-09; CLAUDE.md rule 1 (8d332534)",
        ),
        Rule(
            "R03",
            "FreeSurfer's source (the clone, the tags, the installation's scripts)",
            "FreeSurfer",
            DAY_ONE,
            "2026-10-09T23:26:43+02:00",
            "CLAUDE.md rules 1 and 3; FreeSurfer clean-room originals planned 2026-10-26",
        ),
        Rule(
            "R04",
            "Connectome Workbench's source",
            "Workbench",
            DAY_ONE,
            "2026-10-09T23:26:43+02:00",
            "CLAUDE.md rule 1; reference_src/DO_NOT_READ.md",
        ),
        Rule(
            "R05",
            "MSM_HOCR's source",
            "MSM",
            DAY_ONE,
            "2026-10-09T23:26:43+02:00",
            "CLAUDE.md rule 1; reference_src/DO_NOT_READ.md",
        ),
        Rule(
            "R06",
            "glibc's and libstdc++'s source (LGPL / GPL)",
            "GNU",
            "2026-10-30T02:56:32+01:00",
            "2026-11-04T09:33:11+01:00",
            "specs/cbrtf.md (a9c584fb, the first glibc behaviour spec); CLAUDE.md rule 2 "
            "'never copied or translated' (8a11aa02)",
        ),
        Rule(
            "R07",
            "tedana's source (LGPL-2.1: the clone, tags/tedana-*, every installed copy's "
            "site-packages/tedana)",
            "tedana",
            DAY_ONE,
            "2026-11-06T00:00:00+01:00",
            "docs/licensing.md (an original from the published equations); decision log "
            "2026-11-06 (t2smap: a replica and a clean-room original); the tags and installed "
            "copies named at the t2smap replica's merge (D9, 2026-11-08)",
        ),
        Rule(
            "R10",
            "the replica crates (crates-nc, crates-gpl, crates/larmorx-afni-replica, any "
            "larmorx-nc-* or larmorx-gpl-* copy)",
            "replica",
            DAY_ONE,
            "2026-10-09T23:26:43+02:00",
            "CLAUDE.md rule 1 (8d332534); crates/larmorx-afni-replica named 2026-10-30 (05893577)",
        ),
        Rule(
            "R11",
            "findings written from the source (docs/findings, the files CLAUDE.md rule 1 lists)",
            "per file",
            DAY_ONE,
            "2026-10-10T01:07:13+02:00",
            "CLAUDE.md rule 1, extended file by file (dates in SOURCE_FINDINGS)",
        ),
        Rule(
            "R12",
            "the defect register's restricted sections (## Restricted: ...)",
            "per section",
            "2026-10-11T16:56:42+02:00",
            "2026-10-11T17:00:07+02:00",
            "docs/findings/upstream-defects.md (42c3da7c, 9cd70709); searching the whole file "
            "named a violation 2026-10-30 (d526f107)",
        ),
        Rule(
            "R13",
            "other parts' briefs, reports and handoffs (docs/parts)",
            "all",
            "2026-10-23T19:28:03+02:00",
            "2026-10-23T19:28:03+02:00",
            "CLAUDE.md rule 1 (c6ff930b); porting guide §1 (bd2ca21d)",
        ),
        Rule(
            "R14",
            "probes written from the source (validation/probes)",
            "per folder",
            "2026-10-23T11:07:07+02:00",
            "2026-11-06T00:00:00+01:00",
            "the audit's brief (2026-11-06); folder created d901a1a0",
        ),
        Rule(
            "R15",
            "the replica authors' permanent random-testing cases",
            "per tool",
            "2026-10-30T06:51:45+01:00",
            "2026-10-30T06:51:45+01:00",
            "porting guide §1 'Searching is opening' (d526f107)",
        ),
        Rule(
            "R16",
            "replicas' suite and evidence modules a brief names",
            "per tool",
            DAY_ONE,
            DAY_ONE,
            "the clean-room parts' briefs",
        ),
        Rule(
            "R17",
            "session transcripts (they hold every source an agent read)",
            "all",
            DAY_ONE,
            "2026-11-06T00:00:00+01:00",
            "by content; the audit's brief",
        ),
        Rule(
            "R18",
            "other agents' scratch notes and archived scratch",
            "all",
            "2026-10-23T19:28:03+02:00",
            "2026-10-23T19:28:03+02:00",
            "as R13 (other parts' notes and handoffs)",
        ),
        Rule(
            "R19",
            "code FreeSurfer borrows from others, at its sources (GSL, Burkardt's spline, EPFL's B-spline "
            "code, Washington University's 4dfp tools)",
            "FreeSurfer",
            "2026-11-12T00:00:00+01:00",
            "2026-11-12T00:00:00+01:00",
            "decision log 2026-11-12 (FreeSurfer's borrowed pieces: each gets a clean room); "
            "docs/freesurfer/third-party.md",
        ),
    )
}

#: Source-derived findings (R11): file -> (family, since: the file's first commit, named: the
#: first commit of main whose CLAUDE.md or porting guide lists it).
SOURCE_FINDINGS: dict[str, tuple[str, str, str]] = {
    "afni-tshift.md": ("AFNI", "2026-10-09T20:20:25+02:00", "2026-10-10T01:07:13+02:00"),
    "fsl-mcflirt.md": ("FSL", "2026-10-10T02:05:34+02:00", "2026-10-11T17:00:07+02:00"),
    "afni-automask.md": ("AFNI", "2026-10-23T10:23:47+02:00", "2026-10-24T03:16:36+02:00"),
    "afni-unifize.md": ("AFNI", "2026-10-23T10:23:47+02:00", "2026-10-24T03:16:36+02:00"),
    "afni-licence-routines.md": (
        "AFNI",
        "2026-10-24T07:11:25+02:00",
        "2026-10-24T07:11:25+02:00",
    ),
    "fsl-bet.md": ("FSL", "2026-10-24T19:01:34+02:00", "2026-10-26T05:13:39+01:00"),
    "fsl-flirt.md": ("FSL", "2026-11-01T12:37:56+01:00", "2026-11-01T14:59:07+01:00"),
    "fsl-fast.md": ("FSL", "2026-11-02T20:25:01+01:00", "2026-11-02T23:10:39+01:00"),
    "fsl-topup.md": ("FSL", "2026-11-03T05:19:40+01:00", "2026-11-03T07:38:52+01:00"),
    "fsl-maths.md": ("FSL", "2026-11-03T08:53:55+01:00", "2026-11-03T16:40:04+01:00"),
    "fsl-prelude.md": ("FSL", "2026-11-06T17:30:33+01:00", "2026-11-06T21:07:11+01:00"),
    "afni-volreg.md": ("AFNI", "2026-11-06T22:01:36+01:00", "2026-11-07T03:46:30+01:00"),
    # the t2smap replica part's brief named it restricted (2026-11-08, relay 1); first commit c04c344e
    "tedana-t2smap.md": ("tedana", "2026-11-08T17:00:33+01:00", "2026-11-08T17:00:33+01:00"),
}

#: validation/probes/<folder> -> family (R14); folders not listed: by their name's prefix.
PROBE_FAMILIES = {
    "bet-options": "FSL",
    "standard-space-roi": "FSL",
    "glibc-double-maths": "GNU",
    "libstdcxx-sort": "GNU",
    "ants-ai-blobs": None,
    "nlohmann-to-chars": None,
}

#: The clean-room crates (the originals' code; relay 2 adds the clean-room parts of other crates).
CLEANROOM_CRATES = (
    "crates/larmorx-afni/",
    "crates/larmorx-mri/",
    "crates/larmorx-fsl/",
    "crates/larmorx-tedana/",
)

#: Workspace folders holding permissively licensed upstream source (allowed; logged).
PERMISSIVE_DIRS = (
    "ANTs/",
    "ITK/",
    "nibabel/",
    "fmriprep/",
    "smriprep/",
    "niworkflows/",
    "sdcflows/",
    "nipype/",
    "nitransforms/",
    "nireports/",
    "nitime/",
    "pybids/",
    "python-client/",
    "acres/",
    "migas-py/",
    "oracles/ants-2.6.5-src/",
)
PERMISSIVE_TAGS = ("ANTs-", "ITK-", "ITKAdaptiveDenoising", "core-math", "numpy-", "scipy-")
PERMISSIVE_TAGS += ("OpenBLAS-", "nitransforms-")

#: Hosts and paths of source code on the web (WebFetch, curl, wget).
URL_RULES = (
    (re.compile(r"^https?://git\.fmrib\.ox\.ac\.uk/"), "R01"),
    (re.compile(r"^https?://(www\.)?github\.com/afni/afni/(blob|tree|raw|commit)/"), "R02"),
    (re.compile(r"^https?://raw\.githubusercontent\.com/afni/afni/"), "R02"),
    (re.compile(r"^https?://(www\.)?github\.com/freesurfer/freesurfer/(blob|tree|raw)/"), "R03"),
    (re.compile(r"^https?://raw\.githubusercontent\.com/freesurfer/"), "R03"),
    (re.compile(r"^https?://(www\.)?github\.com/Washington-University/workbench/"), "R04"),
    (re.compile(r"^https?://(sourceware\.org/git|elixir\.bootlin\.com/glibc)"), "R06"),
    (re.compile(r"^https?://(www\.)?github\.com/(bminor/glibc|gcc-mirror/gcc)/"), "R06"),
    (re.compile(r"^https?://(ftp\.gnu\.org/gnu/glibc|gcc\.gnu\.org/git)"), "R06"),
    (re.compile(r"^https?://(www\.)?github\.com/ME-ICA/tedana/(blob|tree|raw)/"), "R07"),
    (re.compile(r"^https?://raw\.githubusercontent\.com/ME-ICA/tedana/"), "R07"),
    # the sources of the code FreeSurfer borrows (2026-11-12), also through the Wayback Machine
    (
        re.compile(
            r"(savannah\.gnu\.org/(cgit|git)/gsl|ftp\.gnu\.org/gnu/gsl|github\.com/ampl/gsl)"
        ),
        "R19",
    ),
    (re.compile(r"people\.sc\.fsu\.edu/~?jburkardt|people\.math\.sc\.edu/Burkardt"), "R19"),
    (re.compile(r"bigwww\.epfl\.ch/(thevenaz|sage)"), "R19"),
    (re.compile(r"(github\.com/[^/]+/4dfp_tools|nil\.wustl\.edu/.*4dfp)"), "R19"),
)
DOC_HOSTS = re.compile(
    r"^https?://(fsl\.fmrib\.ox\.ac\.uk|afni\.nimh\.nih\.gov|surfer\.nmr\.mgh\.harvard\.edu|"
    r"[a-z0-9.-]*readthedocs\.io|docs\.|www\.humanconnectome\.org)"
)

#: Operations that show or copy content (as opposed to names, metadata or execution).
CONTENT_OPS = frozenset(
    {"read", "search", "search-hit", "copy", "diff-shown", "diagnostic", "script-path", "web"}
)
NAME_OPS = frozenset({"list", "stat", "search-match"})
WRITE_OPS = frozenset({"write", "edit", "delete", "move"})

#: Anchors that mark restricted material wherever a copy lives (a scratch clone, an archive, a
#: wheel): pattern on "/" + path, rule, family.
#: Operations that name a program a script or a command runs (a path in a ``bin`` folder named by a
#: script is the program it runs: a black-box run, not a read).
RUN_OPS = frozenset({"exec", "script-path", "mention"})
#: Build outputs of the replica workspaces (compiled code, not source).
BUILD_OUTPUT = re.compile(
    r"/crates-(nc|gpl)/target(/|$)|/(nc|gpl)-target(/|$)|/target-(nc|gpl)(/|$)"
)
#: Licence, notice and package metadata of a replica crate (not its code).
REPLICA_METADATA = re.compile(
    r"/crates-(?:nc|gpl)(?:/[^/]+)?/(?:LICENSE[^/]*|LICENSES/.*|NOTICE[^/]*|Cargo\.(?:toml|lock)|"
    r"pyproject\.toml|README\.md|\.gitignore)$|/crates/larmorx-afni-replica/(?:LICENSE[^/]*|"
    r"NOTICE[^/]*|Cargo\.toml)$"
)
TASK_OUTPUT = re.compile(r"^/tmp/claude-\d+/[^/]+/[^/]+/tasks/")

ANCHORS = (
    (re.compile(r"/crates-nc/fsl-source(/|$)"), "R01", "FSL"),
    (re.compile(r"/fsl-source(-[0-9.]+)?(/|$)"), "R01", "FSL"),
    (re.compile(r"/crates-nc(/|$)"), "R10", "FSL"),
    (re.compile(r"/larmorx[-_]nc([-_][a-z0-9]+)*(/|$)"), "R10", "FSL"),
    (re.compile(r"/crates/larmorx-afni-replica(/|$)"), "R10", "AFNI"),
    (re.compile(r"/larmorx[-_]afni[-_]replica(/|$)"), "R10", "AFNI"),
    (re.compile(r"/crates-gpl/tools(/|$)"), "R10", "GNU"),
    # tedana's t2smap replica (2026-11-08): its crate, and the program's crate (AFNI's until 2026-10-29)
    (re.compile(r"/crates-gpl/larmorx-gpl-tedana(/|$)"), "R10", "tedana"),
    (re.compile(r"/larmorx[-_]gpl[-_]tedana(/|$)"), "R10", "tedana"),
    (re.compile(r"/crates-gpl(/|$)"), "R10", "AFNI/tedana"),
    (re.compile(r"/larmorx[-_]gpl([-_][a-z0-9]+)*(/|$)"), "R10", "AFNI/tedana"),
    # tedana's source wherever a copy lives: the tags, every installed package (D9, 2026-11-08)
    (re.compile(r"/tags/tedana-[^/]+(/|$)"), "R07", "tedana"),
    (re.compile(r"/site-packages/tedana(/|$)"), "R07", "tedana"),
    # FreeSurfer's replicas (2026-11-12: a workspace of several crates, the wheel larmorx-freesurfer)
    (re.compile(r"/crates-freesurfer(/|$)"), "R10", "FreeSurfer"),
    (re.compile(r"/larmorx[-_]freesurfer([-_][a-z0-9]+)*(/|$)"), "R10", "FreeSurfer"),
    (re.compile(r"/docs/parts(/|$)"), "R13", "all"),
    (re.compile(r"/validation/probes(/|$)"), "R14", None),
)

SCRATCH = re.compile(r"^/tmp/claude-\d+/[^/]+/[^/]+/scratchpad(?:/(?P<task>[^/]+))?(?P<rest>/.*)?$")
WORKTREE = re.compile(r"^larmorx/\.claude/worktrees/(?P<id>[^/]+)(?P<rest>/.*)?$")


@dataclass(frozen=True)
class Context:
    """The machine's layout: where the workspace, the home and the FSL installations are."""

    workspace: str
    home: str
    fsldirs: tuple[str, ...] = ()
    freesurfer: str = "/usr/local/freesurfer"

    @property
    def default_fsldir(self) -> str:
        return self.fsldirs[0] if self.fsldirs else posixpath.join(self.home, "fsl")


@dataclass(frozen=True)
class Location:
    """A logical location: an area and a path inside it."""

    area: str  # larmorx, site, testdata, workspace, fsl, scratch, transcripts, home, system, url
    rel: str
    checkout: str = ""  # main, heavy, a worktree id, or a scratch task for scratch clones
    unresolved: bool = False

    def show(self) -> str:
        """The form the public summary may show (no account names, no absolute paths)."""
        if self.area == "larmorx":
            return self.rel or "."
        if self.area in ("site", "testdata"):
            return f"{self.area}:{self.rel}"
        if self.area == "workspace":
            return f"<workspace>/{self.rel}"
        if self.area == "fsl":
            return f"<FSLDIR>/{self.rel}"
        if self.area == "scratch":
            return f"<scratch>/{self.rel}"
        if self.area == "home":
            return f"~/{self.rel}"
        if self.area == "transcripts":
            return "<transcripts>/..."
        return self.rel


@dataclass(frozen=True)
class Class:
    """A location's classification for one operation."""

    category: str
    rule: str | None = None
    family: str | None = None
    note: str = ""
    extra: dict = field(default_factory=dict, compare=False, hash=False)


def locate(path: str, ctx: Context, *, unresolved: bool = False) -> Location:
    """The logical location of an absolute path (or a URL)."""
    if re.match(r"^[a-z]+://", path):
        return Location("url", path)
    p = posixpath.normpath(path) if path.startswith("/") else path
    ws = ctx.workspace.rstrip("/") + "/"
    if p.startswith(ws) or p + "/" == ws:
        rel = p[len(ws) :] if p.startswith(ws) else ""
        m = WORKTREE.match(rel)
        if m:
            return Location("larmorx", (m["rest"] or "/")[1:], m["id"], unresolved)
        for prefix, area, checkout in (
            ("larmorx", "larmorx", "main"),
            ("larmorx-heavy", "larmorx", "heavy"),
            ("larmorx_documentation", "site", "main"),
            ("larmorx-testdata", "testdata", "main"),
        ):
            if rel == prefix or rel.startswith(prefix + "/"):
                return Location(area, rel[len(prefix) + 1 :], checkout, unresolved)
        return Location("workspace", rel, "", unresolved)
    m = SCRATCH.match(p)
    if m:
        task = m["task"] or ""
        rest = (m["rest"] or "")[1:]
        for clone, area in (("larmorx_documentation", "site"), ("larmorx-testdata", "testdata")):
            if rest == clone or rest.startswith(clone + "/"):
                return Location(area, rest[len(clone) + 1 :], "scratch:" + task, unresolved)
        return Location("scratch", f"{task}/{rest}".rstrip("/"), task, unresolved)
    for fsldir in (*ctx.fsldirs, ctx.default_fsldir):
        f = fsldir.rstrip("/")
        if p == f or p.startswith(f + "/"):
            return Location("fsl", p[len(f) + 1 :], posixpath.basename(f), unresolved)
    home = ctx.home.rstrip("/") + "/"
    if p.startswith(home):
        rel = p[len(home) :]
        if re.match(r"^fsl(-[0-9.]+)?(/|$)", rel):
            top, _, rest = rel.partition("/")
            return Location("fsl", rest, top, unresolved)
        if rel.startswith(".claude/projects/"):
            return Location("transcripts", rel, "", unresolved)
        return Location("home", rel, "", unresolved)
    return Location("system", p, "", unresolved)


def _anchor(path: str) -> tuple[str, str | None] | None:
    s = path if path.startswith("/") else "/" + path
    for rx, rule, family in ANCHORS:
        if rx.search(s):
            if rule == "R14":
                m = re.search(r"/validation/probes/([^/]+)", s)
                return rule, _probe_family(m[1]) if m else None
            return rule, family
    m = re.search(r"/docs/findings/([^/]+\.md)$", s)
    if m and m[1] in SOURCE_FINDINGS:
        return "R11", SOURCE_FINDINGS[m[1]][0]
    return None


def _probe_family(folder: str) -> str | None:
    if folder in PROBE_FAMILIES:
        return PROBE_FAMILIES[folder]
    if folder.startswith(("fsl-", "bet-")):
        return "FSL"
    if folder.startswith("afni-"):
        return "AFNI"
    if folder.startswith(("glibc-", "libstdc")):
        return "GNU"
    return None


def _glob_hits_findings(pattern: str) -> list[str]:
    """Source-derived findings a glob over docs/findings could match."""
    m = re.search(r"(?:^|/)docs/findings/([^/]*)$", pattern)
    if not m or not any(ch in m[1] for ch in "*?["):
        return []
    return [f for f in SOURCE_FINDINGS if fnmatch.fnmatchcase(f, m[1])]


def classify(loc: Location, op: str) -> Class:
    """The category, rule and family of an operation on a location."""
    if loc.area == "url":
        return _classify_url(loc.rel)
    rel = loc.rel
    if loc.area == "system" and TASK_OUTPUT.match(rel):
        return Class("task-output")
    # Restricted material wherever a copy lives.
    if loc.area in ("larmorx", "site", "testdata", "scratch", "workspace", "home", "system"):
        s = ("/" + rel) if loc.area != "system" else rel
        if BUILD_OUTPUT.search(s):
            return Class("replica-build-output", "R10", "FSL" if "nc" in s else "AFNI/tedana")
        if REPLICA_METADATA.search(s) and op not in ("build", "exec"):
            return Class("replica-metadata", "R10", "FSL" if "crates-nc" in s else "AFNI/tedana")
        hit = _anchor(s)
        if hit and not (loc.area == "workspace" and rel.startswith("archive/")):
            rule, family = hit
            if rule == "R14" and family is None:
                return Class("probe-permissive", None, None)
            return restricted(rule, family, op, rel)
    if loc.area == "larmorx":
        return _classify_repo(rel, op)
    if loc.area in ("site", "testdata"):
        return Class("repo-" + loc.area)
    if loc.area == "fsl":
        return _classify_fsl(rel, op)
    if loc.area == "workspace":
        return _classify_workspace(rel, op)
    if loc.area == "transcripts":
        if re.match(r"^\.claude/projects/[^/]+/[^/]+/tool-results/", rel):
            return Class("tool-output")
        if re.match(r"^\.claude/projects/[^/]+/memory/", rel):
            return Class("memory")
        return restricted("R17", "all", op, rel)
    if loc.area == "scratch":
        return Class("scratch", note=loc.checkout)
    if loc.area == "system":
        if rel.startswith("/usr/include/c++/"):
            return restricted("R06", "GNU", op, rel)
        if rel.startswith("/usr/local/freesurfer/"):
            if op in RUN_OPS and "/bin/" in rel:
                return Class("run", None, "FreeSurfer")
            if op in CONTENT_OPS and re.search(r"/(bin|python|lib)/", rel):
                return restricted("R03", "FreeSurfer", op, rel)
            return Class("freesurfer-install")
        if rel.startswith(("/usr/share/doc/", "/usr/share/man/")):
            return Class("docs")
        return Class("system")
    return Class(loc.area)


def restricted(rule: str, family: str | None, op: str, rel: str) -> Class:
    if op in NAME_OPS:
        return Class("restricted-names", rule, family)
    if op in WRITE_OPS:
        return Class("restricted-write", rule, family)
    if op in ("exec", "build"):
        return Class("restricted-" + op, rule, family)
    if op == "mention":
        return Class("restricted-mention", rule, family)
    return Class("restricted", rule, family)


def _classify_repo(rel: str, op: str) -> Class:
    if rel == "docs/findings/upstream-defects.md":
        return Class("register", "R12", None)
    # FreeSurfer's preparation (2026-11-12): the inventory, cards, queue and findings come from its source
    if (
        rel.startswith(
            (
                "docs/freesurfer/",
                "validation/investigations/freesurfer-inventory/",
                "validation/investigations/freesurfer-unit-oracle/",
                # the replay tool and the unit oracle's harness, written from FreeSurfer's source: a FreeSurfer
                # clean-room implementer runs them (black box) but never reads them (2026-11-12)
                "validation/src/larmorx_validation/freesurfer/",
                "validation/src/larmorx_validation/freesurfer_unit.py",
            )
        )
        or re.match(r"^docs/findings/freesurfer-[^/]+\.md$", rel)
    ) and not rel.endswith("-clean-room.md"):
        return restricted("R11", "FreeSurfer", op, rel)
    m = re.match(r"^docs/findings/((?:fsl|afni)-[^/]+\.md)$", rel)
    if m and not m[1].endswith("-original.md"):
        return Class("findings-unlisted", None, m[1].split("-", 1)[0].upper())
    hits = _glob_hits_findings(rel)
    if hits:
        fam = sorted({SOURCE_FINDINGS[h][0] for h in hits})
        return restricted("R11", "/".join(fam), op, rel)
    m = re.match(r"^validation/difftest/cases/([^/]+)", rel)
    if m:
        return Class("difftest-cases", None, None, note=m[1])
    if rel.startswith("specs/"):
        return Class("spec")
    if rel.startswith(CLEANROOM_CRATES):
        return Class("cleanroom")
    return Class("repo")


def _classify_fsl(rel: str, op: str) -> Class:
    top = rel.split("/", 1)[0]
    if op in RUN_OPS and top in ("bin", "share", "libexec"):
        return Class("run", None, "FSL")
    if top in ("src", "include", "pkgs"):
        return restricted("R01", "FSL", op, rel)
    if top == "bin":
        return restricted("R01", "FSL", op, rel) if op in CONTENT_OPS else Class("fsl-bin")
    if re.match(r"^lib/python[0-9.]*/site-packages/fsl", rel):
        return restricted("R01", "FSL", op, rel)
    if top in ("doc", "man"):
        return Class("docs", None, "FSL")
    if top == "data":
        return Class("data", None, "FSL")
    if top == "LICENCE.FSL":
        return Class("docs", None, "FSL")
    return Class("fsl-install")


def _classify_workspace(rel: str, op: str) -> Class:
    top = rel.split("/", 1)[0]
    if rel.startswith("afni/doc/"):
        return Class("docs", None, "AFNI")
    if (
        top == "afni"
        or re.match(r"^tags/AFNI-", rel)
        or re.match(r"^oracles/afni-[^/]+/build", rel)
    ):
        return restricted("R02", "AFNI", op, rel)
    m = re.match(r"^oracles/(afni|ants)-[^/]+/(?:[^/]+/)?bin/", rel)
    if m and op in RUN_OPS:
        return Class("run", None, "AFNI" if m[1] == "afni" else "ANTs")
    if m and m[1] == "afni" and op in CONTENT_OPS:
        return restricted("R02", "AFNI", op, rel)
    if top == "freesurfer" or rel.startswith("tags/freesurfer"):
        return restricted("R03", "FreeSurfer", op, rel)
    # an unpacked FreeSurfer package (oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/): running its programs
    # is a black-box run; its scripts, Python and libraries are FreeSurfer's source; its data is data
    m = re.match(r"^oracles/freesurfer-[^/]+/root/usr/local/freesurfer/[^/]+/(?P<sub>.*)$", rel)
    if m:
        sub = m["sub"]
        if op in RUN_OPS and sub.startswith("bin/"):
            return Class("run", None, "FreeSurfer")
        if op in CONTENT_OPS and (
            re.match(r"^(bin|python|lib|etc|fsfast|matlab|tktools|fsafd)/", sub)
            or re.match(r"^[^/]+\.(c?sh|py)$", sub)
        ):
            return restricted("R03", "FreeSurfer", op, rel)
        return Class("freesurfer-install")
    # the unit oracle's build tree (track B, 2026-11-12): FreeSurfer's library built from its source
    if re.match(r"^oracles/freesurfer-[^/]+-unit/", rel):
        if op in RUN_OPS:
            return Class("run", None, "FreeSurfer")
        return restricted("R03", "FreeSurfer", op, rel)
    # a recorded run: recon-all copies itself and its configuration into the subject's scripts/
    if re.match(
        r"^oracles/freesurfer-[^/]+/runs/.*/scripts/(recon-all\.local-copy|recon-config\.yaml)$",
        rel,
    ):
        return restricted("R03", "FreeSurfer", op, rel)
    if rel.startswith("reference_src/workbench"):
        return restricted("R04", "Workbench", op, rel)
    if rel.startswith("reference_src/MSM_HOCR"):
        return restricted("R05", "MSM", op, rel)
    if rel.startswith("tags/glibc"):
        return restricted("R06", "GNU", op, rel)
    if top == "tedana":
        return restricted("R07", "tedana", op, rel)
    if top == "archive":
        name = rel.split("/")[1] if "/" in rel else ""
        if "session-transcripts" in name:
            return restricted("R17", "all", op, rel)
        if "fsl-source" in name:
            return restricted("R01", "FSL", op, rel)
        if "afni-replica" in name:
            return restricted("R02", "AFNI", op, rel)
        if "replica" in name or "scratch" in name:
            return restricted("R18", "all", op, rel)
        hit = _anchor("/" + rel)
        if hit:
            return restricted(hit[0], hit[1], op, rel)
        return Class("archive")
    if rel.startswith(PERMISSIVE_DIRS) or any(rel.startswith("tags/" + t) for t in PERMISSIVE_TAGS):
        return Class("permissive-source")
    if top == "oracles":
        return Class("oracle")
    return Class("workspace")


def _classify_url(url: str) -> Class:
    for rx, rule in URL_RULES:
        if rx.search(url):
            return Class("restricted", rule, RULES[rule].family)
    if DOC_HOSTS.search(url):
        return Class("docs")
    return Class("web")


def in_force(cls: Class, rel: str, when: str) -> tuple[bool, bool]:
    """Whether a restriction applied at ``when`` (ISO): (by content, named by the rules)."""
    if not cls.rule:
        return False, False
    since, named = RULES[cls.rule].since, RULES[cls.rule].named
    if cls.rule == "R11":
        name = rel.rsplit("/", 1)[-1]
        if name in SOURCE_FINDINGS:
            _, since, named = SOURCE_FINDINGS[name]
    if cls.rule == "R10" and "larmorx-afni-replica" in rel:
        since, named = "2026-10-29T23:28:01+01:00", "2026-10-30T01:29:18+01:00"
    return _ge(when, since), _ge(when, named)


def _ge(a: str, b: str) -> bool:
    from datetime import datetime

    def parse(s: str) -> datetime:
        return datetime.fromisoformat(s.replace("Z", "+00:00"))

    return parse(a) >= parse(b)
