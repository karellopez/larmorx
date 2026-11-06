# SPDX-License-Identifier: Apache-2.0
"""What one tool call opened, listed, searched, ran, wrote or committed.

:func:`call_accesses` takes a tool call as the transcript records it (the tool's name, its input,
its result text, the directory it ran in) and returns its :class:`Access` list and the commits its
output printed. The dedicated tools give their paths directly (``Read``, ``Grep``, ``Glob``,
``LS``, ``Write``, ``Edit``, ``MultiEdit``, ``NotebookEdit``, ``WebFetch``); a ``Bash`` command is
read by :mod:`.shell` and each simple command by its verb's option grammar (the verbs below).
The output adds what the command's arguments cannot say: the files a search printed lines of
(``grep``/``rg``/``git grep`` hits), the files a ``git show``/``git diff``/``git log -p`` printed
(``diff --git`` headers), the source lines a compiler diagnostic quoted (``-->``), and the
commits ``git commit`` printed (``[branch hash] message``).

Operations: ``read`` (content shown), ``search`` (a search's scope), ``search-hit`` (lines of a
file a search printed), ``search-match`` (a file a search named), ``list`` (names), ``stat``
(metadata, counts, checksums), ``copy`` (content copied, not shown), ``diff-shown`` (a diff of the
file shown), ``diagnostic``, ``exec`` (a program run: a black-box run when it is an original's),
``build`` (compiled or tested), ``script-path`` (a path a script the agent ran names: assumed
opened), ``mention`` (a path among a command's arguments whose effect the audit does not model),
``web``, ``write``, ``edit``, ``delete``, ``move``.
"""

from __future__ import annotations

import fnmatch
import posixpath
import re
from dataclasses import dataclass, field

from larmorx_validation.cleanroom_audit import shell
from larmorx_validation.cleanroom_audit.restrictions import Context

Lines = tuple[tuple[int, int | None], ...]


def _words(s: str) -> frozenset[str]:
    """A set of names written as one string."""
    return frozenset(s.split())


@dataclass
class Access:
    op: str
    path: str
    via: str
    detail: str = ""
    lines: Lines = ()
    hits: tuple[int, ...] = ()
    hint: str | None = None  # a program's family for a run by name (bet, 3dTshift, ...)
    unresolved: bool = False


@dataclass
class Commit:
    repo: str
    branch: str
    hash: str
    message: str
    inferred: bool = False  # read from a ``git log --oneline`` after a quiet commit


@dataclass
class Result:
    accesses: list[Access] = field(default_factory=list)
    commits: list[Commit] = field(default_factory=list)
    spawns: list[dict] = field(default_factory=list)
    unparsed: list[str] = field(default_factory=list)


READ_ALL = _words(
    "cat tac nl less more bat batcat zcat zless bzcat xzcat strings od xxd hexdump cut sort "
    "uniq paste column fold fmt rev iconv base64 yq csvlook view"
)
STAT = _words("du stat file wc md5sum sha1sum sha256sum sha512sum b2sum cksum realpath readlink")
LIST = frozenset(["ls", "tree", "exa", "eza", "dir"])
SEARCH = frozenset(["grep", "egrep", "fgrep", "zgrep", "rg", "ag", "ack"])
DELETE = frozenset(["rm", "rmdir", "unlink", "shred"])
CREATE = frozenset(["mkdir", "touch", "truncate"])
BUILD = _words(
    "cargo rustc maturin pytest ruff mypy pyright make cmake ninja gcc g++ cc c++ clang clang++ "
    "ld ar gfortran nvcc meson pip"
)
INSPECT = _words("nm objdump readelf ldd gdb valgrind perf ltrace strace")
PYTHONS = re.compile(r"^(python[0-9.]*|pypy[0-9.]*|ipython[0-9]*)$")
SHELLS = frozenset({"bash", "sh", "zsh", "dash", "source", "."})
IGNORE = _words(
    "echo printf true false sleep set unset shopt trap wait kill pkill pgrep jobs disown exit "
    "return break continue read printenv type which hash alias ulimit umask test [ [[ basename "
    "dirname date seq expr let bc dc yes clear history whoami id hostname uname nproc free "
    "uptime ps top htop sync systemctl journalctl loginctl tput stty nvidia-smi lscpu df mount "
    "export cd pushd popd local declare readonly typeset"
)
WRAPPERS = _words(
    "sudo nohup nice ionice timeout time command builtin exec env stdbuf taskset chrt xargs "
    "watch flock systemd-run boxed.sh uv poetry conda mamba micromamba srun"
)
#: FSL programs larmorx ports or its agents run by name (FSL is off the agents' PATH, so a bare
#: name is rare): a black-box run.
FSL_PROGRAMS = _words(
    "bet bet2 betsurf bet4animal flirt mcflirt fast topup applytopup fslmaths fslstats fslroi "
    "fslval fslhd fslinfo fslmerge fslsplit fslreorient2std fslorient fslswapdim fslcpgeom "
    "fslnvols fslchfiletype fslcomplex fslmeants fslsize fslversion convert_xfm "
    "standard_space_roi prelude fugue epi_reg fsl_anat robustfov applywarp fnirt invwarp "
    "convertwarp susan slicer slices aff2rigid imcp immv imtest imrm remove_ext avscale "
    "img2imgcoord std2imgcoord flirt_average fsl_prepare_fieldmap fsl_regfilt melodic overlay "
    "fsl_sub fslpython fsl_tsplot"
)
ANTS_PROGRAMS = re.compile(
    r"^(ants[A-Z]\w*|N4BiasFieldCorrection|N3BiasFieldCorrection|ImageMath|DenoiseImage|Atropos|"
    r"ThresholdImage|ResampleImage\w*|PrintHeader|CreateImage|MultiplyImages|AverageImages|"
    r"ConvertImage\w*|ExtractRegionFromImage\w*|LabelGeometryMeasures|KellyKapowski|"
    r"SmoothImage|CopyImageHeaderInformation|ComposeMultiTransform)$"
)
AFNI_PROGRAMS = re.compile(r"^(3d\w+|@\w+|afni\w*|1d\w+|nifti_tool|to3d|whereami)$")
PATHLIKE_EXT = re.compile(
    r"\.(rs|py|pyi|c|cc|cpp|cxx|h|hh|hpp|hxx|txx|md|toml|json|jsonl|sh|txt|tcl|m|nii|gz|mat|"
    r"yml|yaml|csv|tsv|log|cfg|ini|html|css|js|svg|png|patch|diff|f|f90|cmake|in)$"
)
SKIP_TARGETS = ("/dev/null", "/dev/stderr", "/dev/stdout", "/dev/tty")
# Unambiguous alternatives only (an escape or a plain character): linear on any input.
_SQ = r"'(?P<s1>(?:\\.|[^'\\\n])*)'"
_DQ = r'"(?P<s2>(?:\\.|[^"\\\n])*)"'
STRING_LITERAL = re.compile(_SQ + "|" + _DQ)
COMMIT_LINE = re.compile(
    r"^\[(?P<branch>.+?)(?: \(root-commit\))? (?P<hash>[0-9a-f]{7,40})\] (?P<msg>.*)$", re.M
)
ONELINE = re.compile(r"^(?P<hash>[0-9a-f]{7,40}) (?P<msg>\S.*)$", re.M)
DIFF_HEADER = re.compile(r"^diff --git a/(\S+) b/(\S+)$", re.M)
STAT_LINE = re.compile(r"^ (\S[^|\n]*?)[ \t]+\|[ \t]+(?:\d+|Bin)", re.M)
DIAGNOSTIC = re.compile(r"^[ \t]*--> (\S+?):(\d+):\d+", re.M)
HIT_LINE = re.compile(
    r"^(?:\x1b\[[0-9;]*m)*(?P<p>[^\s:\x1b][^:\n\x1b]*?)(?:\x1b\[[0-9;]*m)*[:-]"
    r"(?:\x1b\[[0-9;]*m)*(?P<n>\d+)(?:\x1b\[[0-9;]*m)*[:-]"
)
HIT_NOLINE = re.compile(r"^(?P<p>[^\s:][^:\n]*?):(?!\d)")
BINARY_MATCH = re.compile(r"^(?:grep: )?(?:Binary file )?(?P<p>\S+?)(?:: binary file)? matches$")


def call_accesses(
    tool: str,
    inp: dict,
    result: str,
    cwd: str,
    ctx: Context,
    scripts: dict[str, str] | None = None,
) -> Result:
    """The accesses, commits and spawned agents of one tool call."""
    scripts = {} if scripts is None else scripts
    res = Result()
    if not isinstance(inp, dict):
        return res
    if tool == "Read":
        p = _abs(inp.get("file_path", ""), cwd, ctx)
        res.accesses.append(Access("read", p, "Read", _read_detail(inp), _read_lines(inp, result)))
    elif tool in ("Write", "Edit", "MultiEdit", "NotebookEdit"):
        p = _abs(inp.get("file_path") or inp.get("notebook_path") or "", cwd, ctx)
        op = "write" if tool == "Write" else "edit"
        res.accesses.append(Access(op, p, tool))
        if tool == "Write" and isinstance(inp.get("content"), str):
            scripts[p] = inp["content"]
    elif tool == "Grep":
        _grep_tool(inp, result, cwd, ctx, res)
    elif tool == "Glob":
        base = _abs(inp.get("path") or cwd, cwd, ctx)
        pat = str(inp.get("pattern", ""))
        res.accesses.append(Access("list", base, "Glob", pat))
        for line in result.splitlines():
            line = line.strip()
            if line.startswith("/") and "\n" not in line:
                res.accesses.append(Access("list", _abs(line, cwd, ctx), "Glob", "result"))
    elif tool == "LS":
        res.accesses.append(Access("list", _abs(inp.get("path") or cwd, cwd, ctx), "LS"))
    elif tool == "WebFetch":
        res.accesses.append(Access("web", str(inp.get("url", "")), "WebFetch"))
    elif tool in ("Agent", "Task"):
        res.spawns.append(
            {
                "description": inp.get("description", ""),
                "subagent_type": inp.get("subagent_type"),
                "isolation": inp.get("isolation"),
                "prompt_chars": len(str(inp.get("prompt", ""))),
            }
        )
    elif tool in ("Bash", "Monitor"):
        cmd = inp.get("command")
        if isinstance(cmd, str) and cmd:
            _bash(cmd, result, cwd, ctx, scripts, res)
    return res


# -- dedicated tools ------------------------------------------------------------------------------


def _read_detail(inp: dict) -> str:
    parts = [f"{k}={inp[k]}" for k in ("offset", "limit", "pages") if inp.get(k) not in (None, "")]
    return " ".join(parts)


def _read_lines(inp: dict, result: str) -> Lines:
    nums = re.findall(r"^[ \t]*(\d+)(?:→|\t)", result[:4_000_000], re.M)
    if nums:
        return ((int(nums[0]), int(nums[-1])),)
    try:
        off = int(inp.get("offset") or 1)
        lim = int(inp.get("limit") or 2000)
    except (TypeError, ValueError):
        return ()
    return ((off, off + lim - 1),)


def _grep_tool(inp: dict, result: str, cwd: str, ctx: Context, res: Result) -> None:
    base = _abs(inp.get("path") or cwd, cwd, ctx)
    pat = str(inp.get("pattern", ""))
    glob = inp.get("glob") or inp.get("type") or ""
    res.accesses.append(Access("search", base, "Grep", f"{pat!r} {glob}".strip()))
    mode = inp.get("output_mode") or "files_with_matches"
    if mode == "content":
        hits = _hits(result, base, ctx, single=_looks_like_file(base))
        for p, lines in hits.items():
            res.accesses.append(Access("search-hit", p, "Grep", pat, hits=tuple(lines)))
    else:
        op = "stat" if mode == "count" else "search-match"
        for line in result.splitlines():
            line = line.strip()
            if mode == "count":
                line = line.rsplit(":", 1)[0]
            if line.startswith("/") or "/" in line:
                res.accesses.append(Access(op, _abs(line, base, ctx), "Grep", pat))


# -- Bash ------------------------------------------------------------------------------------------


@dataclass
class _Seen:
    search_cwds: list[tuple[str, bool]] = field(default_factory=list)
    search_scopes: list[tuple[str, str]] = field(default_factory=list)  # (cwd, path searched)
    git_diff_roots: list[str] = field(default_factory=list)
    compile_roots: list[str] = field(default_factory=list)
    commit_repos: list[str] = field(default_factory=list)
    log_after_commit: bool = False
    search_names: bool = False  # ``grep -l``/``-L``/``-c``: the output names files, no lines


def _bash(cmd: str, result: str, cwd: str, ctx: Context, scripts: dict, res: Result) -> None:
    st = shell.State(cwd, ctx.home, env={"FSLDIR": [ctx.default_fsldir]})
    seen = _Seen()
    try:
        _commands(cmd, st, ctx, scripts, res, seen, 0)
    except (RecursionError, IndexError, ValueError, KeyError) as e:  # pragma: no cover - logged
        res.unparsed.append(f"{type(e).__name__}: {cmd[:200]}")
    _outputs(result, ctx, res, seen)


def _commands(
    src: str, st: shell.State, ctx: Context, scripts: dict, res: Result, seen: _Seen, depth: int
) -> None:
    for sc in shell.simple_commands(src, st, depth):
        _simple(sc, st, ctx, scripts, res, seen, depth)


def _simple(
    sc: shell.Simple,
    st: shell.State,
    ctx: Context,
    scripts: dict,
    res: Result,
    seen: _Seen,
    depth: int,
) -> None:
    words = list(sc.words)
    cwd = sc.cwd
    heredoc = None
    for op, target, body in sc.redirs:
        if body is not None:
            heredoc = body
        if target is None or target.startswith("&"):
            continue
        p = _abs(target, cwd, ctx)
        if op in (">", ">>", ">|", "&>", "&>>"):
            if p not in SKIP_TARGETS:
                res.accesses.append(Access("write", p, "redirect", op))
                if body is not None:
                    scripts[p] = body
        elif op in ("<", "<>"):
            res.accesses.append(Access("read", p, "redirect", "<"))
    words = _strip_wrappers(words)
    if not words:
        return
    head = words[0]
    name = posixpath.basename(head)
    args = words[1:]
    un = bool(sc.unresolved)

    def add(op: str, path: str, detail: str = "", **kw) -> None:
        if path:
            res.accesses.append(
                Access(op, _abs(path, cwd, ctx), name, detail, unresolved="$" in path, **kw)
            )

    if name in IGNORE:
        return
    if name in READ_ALL:
        for a in _positional(args, values=set("bcdfnstwBHNSW") if name in ("od", "xxd") else set()):
            add("read", a)
    elif name in ("head", "tail"):
        spec, lines = _head_tail_lines(name, args)
        for a in _positional(args, values=set("nc"), long_values={"--lines", "--bytes"}):
            add("read", a, f"{name} -n {spec}", lines=lines)
    elif name == "sed":
        _sed(args, add)
    elif name in ("awk", "gawk", "mawk", "nawk"):
        pos = _positional(args, values=set("Fvf"))
        if "-f" not in args and pos:
            pos = pos[1:]
        for a in pos:
            if not re.match(r"^[A-Za-z_]\w*=", a):
                add("read", a)
    elif name == "jq":
        pos = _positional(args, values={"f"}, long_values={"--arg", "--argjson", "--slurpfile"})
        for a in pos[1:]:
            add("read", a)
    elif name in SEARCH:
        _search(name, args, add, cwd, seen)
    elif name == "find":
        _find(args, add, cwd, st, ctx, scripts, res, seen, depth)
    elif name in LIST:
        for a in _positional(args, values={"I", "w", "L", "P"}) or ["."]:
            add("list", a)
    elif name in STAT:
        for a in _positional(args, values={"d"}):
            add("stat", a)
    elif name in ("diff", "colordiff", "sdiff", "diff3", "cmp", "comm"):
        op = "stat" if name == "cmp" else "read"
        for a in _positional(args, values={"U", "C", "x", "X", "I", "W"}, long_values={"--label"}):
            add(op, a, name)
    elif name in ("cp", "rsync", "scp", "install", "ln"):
        _copy(name, args, add)
    elif name == "mv":
        pos = _positional(args, values={"t", "S"})
        for a in pos[:-1]:
            add("move", a)
        if pos:
            add("write", pos[-1], "mv")
    elif name in DELETE:
        for a in _positional(args):
            add("delete", a)
    elif name in CREATE:
        for a in _positional(args, values={"m", "s"}):
            add("write", a, name)
    elif name == "tee":
        for a in _positional(args):
            add("write", a, "tee")
    elif name == "tar":
        _tar(args, add)
    elif name in ("unzip", "zip", "7z", "gzip", "gunzip", "xz", "unxz", "bzip2", "zstd"):
        for a in _positional(args, values={"d", "o", "x"}):
            add("copy", a, name)
    elif name in ("curl", "wget"):
        for a in args:
            if re.match(r"^https?://", a):
                res.accesses.append(Access("web", a, name))
        for flag in ("-o", "--output", "-O", "--output-document"):
            if flag in args[:-1]:
                add("write", args[args.index(flag) + 1], name)
    elif name == "git":
        _git(args, cwd, st, ctx, res, seen, un)
    elif PYTHONS.match(name):
        _python(args, heredoc, cwd, ctx, scripts, add, res, sc, st, seen, depth)
    elif name in SHELLS:
        _shell_cmd(name, args, heredoc, cwd, st, ctx, scripts, res, seen, depth, add)
    elif name in ("perl", "ruby", "node", "Rscript"):
        if "-e" in args[:-1]:
            _literals(args[args.index("-e") + 1], add, f"{name} -e")
        if name == "perl" and any(a.startswith("-pi") or a == "-i" for a in args):
            for a in _positional(args, values={"e"})[0:]:
                add("edit", a, "perl -i")
        pos = _positional(args, values={"e", "I"})
        if pos and "-e" not in args:
            add("exec", pos[0])
            _run_script(pos[0], cwd, ctx, scripts, add, res, st, seen, depth)
    elif name in BUILD:
        _build(name, args, cwd, add)
        manifest = next(
            (args[k + 1] for k, a in enumerate(args[:-1]) if a == "--manifest-path"), None
        )
        root = posixpath.dirname(_abs(manifest, cwd, ctx)) if manifest else cwd
        seen.compile_roots.append(root)
    elif name in INSPECT:
        for a in _positional(args, values={"j", "M"}):
            add("read", a, name)
    elif name in ("apptainer", "singularity"):
        for a in args:
            if a.endswith((".sif", ".simg")):
                add("exec", a, name)
    elif name in FSL_PROGRAMS and "/" not in head:
        res.accesses.append(Access("exec", name, name, " ".join(args)[:200], hint="FSL"))
    elif AFNI_PROGRAMS.match(name) and "/" not in head:
        res.accesses.append(Access("exec", name, name, " ".join(args)[:200], hint="AFNI"))
    elif ANTS_PROGRAMS.match(name) and "/" not in head:
        res.accesses.append(Access("exec", name, name, " ".join(args)[:200], hint="ANTs"))
    else:
        if "/" in head:
            add("exec", head)
            _run_script(head, cwd, ctx, scripts, add, res, st, seen, depth)
        for a in args:
            _mention(a, add)


def _strip_wrappers(words: list[str]) -> list[str]:
    for _ in range(6):
        if not words:
            return words
        name = posixpath.basename(words[0])
        if name not in WRAPPERS:
            return words
        rest = words[1:]
        if name == "boxed.sh":
            rest = rest[1:]
        elif name in ("timeout", "taskset", "chrt", "watch", "flock", "nice", "ionice", "srun"):
            values = {"timeout": "sk", "taskset": "c", "nice": "n", "ionice": "cn", "watch": "n"}
            rest = _after_options(rest, set(values.get(name, "")))
            if name in ("timeout", "flock") or (name == "taskset" and "-c" not in words[1:]):
                rest = rest[1:]
            if name == "chrt":
                rest = rest[1:]
        elif name == "env":
            rest = _after_options(rest, {"u", "C", "S"})
            while rest and re.match(r"^[A-Za-z_]\w*=", rest[0]):
                rest = rest[1:]
        elif name == "xargs":
            rest = _after_options(rest, set("nIPdaLsE"), long_values=set())
            if not rest:
                rest = ["echo"]
        elif name == "systemd-run":
            rest = rest[rest.index("--") + 1 :] if "--" in rest else _after_options(rest, {"p"})
        elif name in ("uv", "poetry", "conda", "mamba", "micromamba"):
            if rest and rest[0] == "run":
                rest = _after_options(rest[1:], {"n", "p"}, long_values={"--with", "--project"})
            else:
                return words
        elif name == "stdbuf" or name in ("sudo", "nohup", "time", "command", "builtin", "exec"):
            rest = _after_options(rest, set())
        words = rest
    return words


def _after_options(
    args: list[str], values: set[str], long_values: set[str] | None = None
) -> list[str]:
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--":
            return args[i + 1 :]
        if not a.startswith("-") or a == "-":
            return args[i:]
        if a.startswith("--"):
            if "=" not in a and long_values and a in long_values:
                i += 1
        elif a[-1] in values and len(a) == 2:
            i += 1
        i += 1
    return []


def _positional(
    args: list[str], values: set[str] | None = None, long_values: set[str] | None = None
) -> list[str]:
    """The positional arguments: options (and the values of those in ``values``) left out."""
    values = values or set()
    long_values = long_values or set()
    out: list[str] = []
    i = 0
    end = False
    while i < len(args):
        a = args[i]
        if end or not a.startswith("-") or a == "-" or re.match(r"^-\d+$", a):
            if a != "-" and not re.match(r"^-\d+$", a):
                out.append(a)
            i += 1
            continue
        if a == "--":
            end = True
        elif a.startswith("--"):
            if "=" not in a and a in long_values:
                i += 1
        else:
            letters = a[1:]
            for k, ch in enumerate(letters):
                if ch in values:
                    if k == len(letters) - 1:
                        i += 1
                    break
        i += 1
    return out


def _head_tail_lines(name: str, args: list[str]) -> tuple[str, Lines]:
    """``head``/``tail``'s line count and the lines it shows: ``(start, end)``, ``end`` None for
    the end of the file, a negative ``start`` for the last lines."""
    spec = "10"
    for k, a in enumerate(args):
        m = re.match(r"^-(\d+)$", a) or re.match(r"^-n([+-]?\d+)$", a)
        m = m or re.match(r"^--lines=([+-]?\d+)$", a)
        if m:
            spec = m[1]
            break
        if a in ("-n", "--lines") and k + 1 < len(args):
            spec = args[k + 1]
            break
    if not re.fullmatch(r"[+-]?\d+", spec):
        return spec, ()
    if name == "head":
        return spec, ((1, None),) if spec.startswith("-") else ((1, int(spec.lstrip("+"))),)
    if spec.startswith("+"):
        return spec, ((int(spec[1:]), None),)
    return spec, ((-abs(int(spec)), None),)


def _sed(args: list[str], add) -> None:
    scripts: list[str] = []
    files: list[str] = []
    inplace = False
    i = 0
    while i < len(args):
        a = args[i]
        if a in ("-e", "--expression") and i + 1 < len(args):
            scripts.append(args[i + 1])
            i += 2
            continue
        if a.startswith("--expression="):
            scripts.append(a.split("=", 1)[1])
        elif a in ("-f", "--file") and i + 1 < len(args):
            i += 2
            continue
        elif a.startswith("-i") or a.startswith("--in-place"):
            inplace = True
        elif a.startswith("-") and a != "-":
            if re.fullmatch(r"-[nErsuz]+i?", a) and a.endswith("i"):
                inplace = True
        else:
            files.append(a)
        i += 1
    if not scripts and files:
        scripts.append(files.pop(0))
    lines: list[tuple[int, int | None]] = []
    for s in scripts:
        for m in re.finditer(r"(\d+)(?:,(\d+|\$))?\s*p", s):
            end = None if m[2] == "$" else int(m[2] or m[1])
            lines.append((int(m[1]), end))
    for f in files:
        add("edit" if inplace else "read", f, "sed " + "; ".join(scripts)[:120], lines=tuple(lines))


def _search(name: str, args: list[str], add, cwd: str, seen: _Seen) -> None:
    if name == "rg":
        values = set("efgtTmABCMj")
        long_values = {"--glob", "--type", "--type-not", "--max-count", "--max-depth", "--regexp"}
        long_values |= {"--file", "--after-context", "--before-context", "--context", "--iglob"}
        long_values |= {"--max-columns", "--threads", "--sort", "--type-add", "--encoding"}
    else:
        values = set("efmABCdD")
        long_values = {"--regexp", "--file", "--max-count", "--after-context"}
        long_values |= {"--before-context", "--context", "--label"}
    pos = _positional(args, values, long_values)
    explicit = any(
        a in ("-e", "-f", "--regexp", "--file") or a.startswith("--regexp=") for a in args
    )
    pattern = ""
    if not explicit and pos:
        pattern, pos = pos[0], pos[1:]
    recursive = "--recursive" in args or any(re.match(r"^-[A-Za-z]*[rR]", a) for a in args)
    if not pos and (name in ("rg", "ag", "ack") or recursive):
        pos = ["."]
    if not pos:  # a search of its input (a pipe): the files are the earlier command's
        return
    single = pos[0] if len(pos) == 1 and _looks_like_file(pos[0]) and not _glob(pos[0]) else ""
    if any(
        a in ("--files-with-matches", "--files-without-match", "--count")
        or re.fullmatch(r"-[A-Za-z]*[lLc][A-Za-z]*", a)
        for a in args
    ):
        seen.search_names = True
        single = ""
    seen.search_cwds.append((cwd, single))
    for a in pos:
        add("search", a, f"{name} {pattern!r}"[:160])
        seen.search_scopes.append((cwd, a))


def _find(args, add, cwd, st, ctx, scripts, res, seen, depth) -> None:
    paths: list[str] = []
    i = 0
    while i < len(args) and not (args[i].startswith(("-", "(", "!")) and args[i] != "-"):
        paths.append(args[i])
        i += 1
    paths = paths or ["."]
    for p in paths:
        add("list", p, "find")
    if "-exec" in args or "-execdir" in args:
        k = args.index("-exec") if "-exec" in args else args.index("-execdir")
        inner = []
        for a in args[k + 1 :]:
            if a in (";", "+", "\\;"):
                break
            inner.append(a)
        if inner:
            inner_name = posixpath.basename(inner[0])
            op = {"cat": "read", "head": "read", "tail": "read", "sed": "read"}.get(inner_name)
            if inner_name in SEARCH:
                op = "search"
                seen.search_cwds.append((cwd, ""))
                seen.search_scopes.extend((cwd, p) for p in paths)
            if op:
                for p in paths:
                    add(op, p, "find -exec " + " ".join(inner)[:120])


def _copy(name: str, args: list[str], add) -> None:
    if name == "rsync":
        values = {"e", "f"}
        long_values = {"--exclude", "--include", "--filter", "--exclude-from", "--include-from"}
        long_values |= {"--rsh", "--chmod", "--chown", "--partial-dir", "--log-file"}
    else:
        values, long_values = {"t", "S"}, {"--target-directory", "--suffix"}
    pos = _positional(args, values, long_values)
    target = None
    for k, a in enumerate(args):
        if a == "-t" and k + 1 < len(args):
            target = args[k + 1]
        elif a.startswith("--target-directory="):
            target = a.split("=", 1)[1]
    if target is None and len(pos) >= 2:
        target, pos = pos[-1], pos[:-1]
    elif target is not None:
        pos = [p for p in pos if p != target]
    for a in pos:
        add("copy", a, name)
    if target:
        add("write", target, name)


def _tar(args: list[str], add) -> None:
    if not args:
        return
    first = args[0]
    mode = ""
    archive = None
    directory = None
    rest = args
    if not first.startswith("-") and re.fullmatch(r"[A-Za-z]+", first):
        mode = first
        rest = args[1:]
        if "f" in first and rest:
            archive, rest = rest[0], rest[1:]
    pos: list[str] = []
    i = 0
    while i < len(rest):
        a = rest[i]
        if a in ("-f", "--file") and i + 1 < len(rest):
            archive = rest[i + 1]
            i += 2
            continue
        if a in ("-C", "--directory") and i + 1 < len(rest):
            directory = rest[i + 1]
            i += 2
            continue
        if a.startswith("-") and not a.startswith("--"):
            mode += a[1:]
            if a.endswith("f") and i + 1 < len(rest):
                archive = rest[i + 1]
                i += 2
                continue
        elif not a.startswith("-"):
            pos.append(a)
        i += 1
    if "t" in mode:
        if archive:
            add("list", archive, "tar t")
        return
    if "x" in mode:
        if archive:
            add("copy", archive, "tar x")
        if directory:
            add("write", directory, "tar x")
        return
    base = directory
    for a in pos:
        add("copy", posixpath.join(base, a) if base and not a.startswith("/") else a, "tar c")
    if archive:
        add("write", archive, "tar c")


def _pathspecs(specs: list[str]) -> list[str]:
    """Git pathspecs as paths: exclusions (``:!p``, ``:^p``, ``:(exclude)p``) name what the
    command leaves out, so they are dropped; other magic (``:(top)p``, ``:/p``) is removed."""
    out = []
    for p in specs:
        if re.match(r"^:(!|\^|\([^)]*\bexclude\b[^)]*\))", p):
            continue
        out.append(re.sub(r"^:(\([^)]*\)|/)", "", p) or ".")
    return out


def _git(args, cwd, st, ctx, res, seen, unresolved) -> None:
    i = 0
    gcwd = cwd
    while i < len(args) and args[i].startswith("-"):
        if args[i] == "-C" and i + 1 < len(args):
            gcwd = shell.resolve(args[i + 1], gcwd, ctx.home)
            i += 2
            continue
        if args[i] in ("-c", "--git-dir", "--work-tree") and i + 1 < len(args):
            i += 2
            continue
        i += 1
    if i >= len(args):
        return
    sub, rest = args[i], args[i + 1 :]
    root = repo_root(gcwd, ctx)

    def add(op: str, path: str, detail: str, base: str) -> None:
        res.accesses.append(
            Access(op, _abs(path, base, ctx), "git " + sub, detail, unresolved=unresolved)
        )

    after = _pathspecs(rest[rest.index("--") + 1 :]) if "--" in rest else []
    before = rest[: rest.index("--")] if "--" in rest else rest
    if sub in ("show", "cat-file"):
        for a in before:
            m = re.match(r"^[^-:][^:]*:(.+)$", a)
            if m and not re.match(r"^[a-z]+://", a):
                p = m[1]
                base = gcwd if p.startswith(("./", "../")) else root
                add("read", p, f"git {sub} {a.split(':', 1)[0]}", base)
        for p in after:
            add("diff-shown", p, f"git {sub}", gcwd)
        seen.git_diff_roots.append(root)
    elif sub in ("diff", "log", "whatchanged", "format-patch", "range-diff", "stash"):
        names_only = any(
            a in ("--stat", "--name-only", "--name-status", "--shortstat", "--numstat")
            or a.startswith("--stat=")
            for a in rest
        )
        patch = not names_only and (
            sub == "diff" or any(a in ("-p", "--patch", "-u") or a.startswith("-L") for a in rest)
        )
        for p in after:
            add("diff-shown" if patch else "stat", p, f"git {sub}", gcwd)
        seen.git_diff_roots.append(root)
    elif sub == "grep":
        pos = _positional(before, set("efABCm"), {"--regexp", "--max-count"})
        explicit = "-e" in before or "-f" in before
        if not explicit and pos:
            pos = pos[1:]
        scope = after or [p for p in pos if not re.match(r"^[0-9a-f]{7,40}$|^HEAD|^main$", p)]
        for p in scope or ["."]:
            add("search", p, "git grep", gcwd)
            seen.search_scopes.append((gcwd, p))
        seen.search_cwds.append((gcwd, ""))
    elif sub in ("ls-files", "ls-tree"):
        pos = _positional(before, set("x"))
        if sub == "ls-tree" and pos:
            pos = pos[1:]
        for p in (pos + after) or ["."]:
            add("list", p, f"git {sub}", gcwd)
    elif sub in ("blame", "annotate"):
        pos = _positional(before, set("LCM"))
        for p in after or pos[-1:]:
            add("read", p, f"git {sub}", gcwd)
    elif sub in ("checkout", "restore") and after:
        for p in after:
            add("write", p, f"git {sub}", gcwd)
    elif sub in ("apply", "am"):
        for p in _positional(before, {"p", "C"}):
            add("read", p, f"git {sub}", gcwd)
        add("write", root, f"git {sub} (targets from the patch)", root)
    elif sub == "rm":
        for p in _positional(before) + after:
            add("delete", p, "git rm", gcwd)
    if sub in ("commit", "cherry-pick", "revert", "merge", "am", "rebase"):
        seen.commit_repos.append(root)
    elif sub == "log" and seen.commit_repos and "--oneline" in rest:
        seen.log_after_commit = True


def _python(args, heredoc, cwd, ctx, scripts, add, res, sc, st, seen, depth) -> None:
    i = 0
    while i < len(args):
        a = args[i]
        if a == "-c" and i + 1 < len(args):
            _literals(args[i + 1], add, "python -c")
            return
        if a == "-m" and i + 1 < len(args):
            mod = args[i + 1]
            for b in args[i + 2 :]:
                _mention(b, add, f"python -m {mod}")
            if mod in ("pytest", "ruff", "maturin", "pip", "mypy"):
                return
            return
        if a in ("-W", "-X", "-Q") and i + 1 < len(args):
            i += 2
            continue
        if a == "-":
            if heredoc:
                _literals(heredoc, add, "python - <<")
            return
        if a.startswith("-"):
            i += 1
            continue
        add("exec", a, "python script")
        _run_script(a, cwd, ctx, scripts, add, res, st, seen, depth)
        for b in args[i + 1 :]:
            _mention(b, add, "python script argument")
        return
    if heredoc:
        _literals(heredoc, add, "python <<")


def _shell_cmd(name, args, heredoc, cwd, st, ctx, scripts, res, seen, depth, add) -> None:
    if "-c" in args:
        k = args.index("-c")
        if k + 1 < len(args) and depth < 5:
            _commands(args[k + 1], st.copy(), ctx, scripts, res, seen, depth + 1)
        return
    pos = _positional(args, {"o", "O"})
    if pos:
        add("exec", pos[0], f"{name} script")
        _run_script(pos[0], cwd, ctx, scripts, add, res, st, seen, depth)
        for b in pos[1:]:
            _mention(b, add, f"{name} script argument")
    elif heredoc and depth < 5:
        _commands(heredoc, st.copy(), ctx, scripts, res, seen, depth + 1)


def _run_script(path, cwd, ctx, scripts, add, res, st, seen, depth) -> None:
    """A script the agent wrote earlier in the transcript: the paths its text names."""
    p = _abs(path, cwd, ctx)
    content = scripts.get(p)
    if content is None or depth >= 5:
        return
    if p.endswith((".sh", ".bash")) or content.startswith(("#!/bin/bash", "#!/usr/bin/env bash")):
        _commands(content, st.copy(), ctx, scripts, res, seen, depth + 1)
    else:
        _literals(content, add, "script " + posixpath.basename(p))


def _build(name: str, args: list[str], cwd: str, add) -> None:
    for k, a in enumerate(args):
        if a == "--manifest-path" and k + 1 < len(args):
            add("build", args[k + 1], name)
        elif a.startswith("--manifest-path="):
            add("build", a.split("=", 1)[1], name)
        elif a in ("-p", "--package") and k + 1 < len(args) and name == "cargo":
            add("build", "crates/" + args[k + 1], "cargo -p")
    if name == "cargo" and args and args[0] in ("build", "test", "clippy", "check", "run", "bench"):
        add("build", ".", "cargo " + args[0])
    for a in args:
        if name in ("gcc", "g++", "cc", "c++", "clang", "clang++", "gfortran", "nvcc"):
            if a.endswith((".c", ".cc", ".cpp", ".cxx", ".f", ".f90", ".cu")):
                add("build", a, name)
            elif a.startswith("-I") and len(a) > 2:
                add("build", a[2:], name + " -I")
        else:
            _mention(a, add, name)


def _literals(code: str, add, via: str) -> None:
    seen: set[str] = set()
    for m in STRING_LITERAL.finditer(code[:2_000_000]):
        s = m["s1"] if m["s1"] is not None else m["s2"]
        if "\n" in s or len(s) > 400 or s in seen:
            continue
        if (s.startswith(("/", "~/")) and len(s) > 1) or (
            "/" in s and re.match(r"^[\w.~-][\w./~+-]*$", s) and PATHLIKE_EXT.search(s)
        ):
            if re.match(r"^/[a-z]+/?$", s) or s.startswith("//"):
                continue
            seen.add(s)
            add("script-path", s, via)


def _mention(a: str, add, detail: str = "") -> None:
    if "=" in a and (a.startswith("-") or re.match(r"^[A-Za-z_]\w*=", a)):
        a = a.split("=", 1)[1]
    if not a or a.startswith("-") or re.match(r"^[a-z]+://", a) or " " in a or "\n" in a:
        return
    if (
        a.startswith(("/", "~/", "./", "../"))
        or ("/" in a and not a.startswith("$("))
        or PATHLIKE_EXT.search(a)
    ):
        add("mention", a, detail)


# -- outputs ---------------------------------------------------------------------------------------


def _outputs(result: str, ctx: Context, res: Result, seen: _Seen) -> None:
    if not result:
        return
    text = result[:8_000_000]
    scopes = [shell.resolve(p, c, ctx.home).rstrip("/") for c, p in seen.search_scopes]

    def in_scope(p: str) -> bool:
        # A hit is a file the search looked at: a ``path:line:`` text in a searched file's own
        # lines (a program's message in a log) is not one.
        return not scopes or any(
            p == s or p.startswith(s + "/") or (_glob(s) and fnmatch.fnmatchcase(p, s))
            for s in scopes
        )

    for cwd, single in seen.search_cwds:
        if seen.search_names:
            for line in text.splitlines():
                name = line.split(":", 1)[0].strip()
                if name and _pathlike(name) and in_scope(_abs(name, cwd, ctx)):
                    res.accesses.append(Access("search-match", _abs(name, cwd, ctx), "output"))
            break
        for p, lines in _hits(text, cwd, ctx, single=single or None).items():
            if in_scope(p):
                res.accesses.append(Access("search-hit", p, "output", "", hits=tuple(lines)))
        break
    for root in seen.git_diff_roots[:1]:
        for _a, b in DIFF_HEADER.findall(text):
            res.accesses.append(Access("diff-shown", _abs(b, root, ctx), "output", "diff --git"))
        for p in STAT_LINE.findall(text):
            p = p.strip()
            if "=>" not in p and not p.startswith("..."):
                res.accesses.append(Access("stat", _abs(p, root, ctx), "output", "--stat"))
    if seen.compile_roots:
        base = seen.compile_roots[0]
        for p, line in DIAGNOSTIC.findall(text):
            res.accesses.append(
                Access("diagnostic", _abs(p, base, ctx), "output", "", hits=(int(line),))
            )
    if seen.commit_repos:
        found = list(COMMIT_LINE.finditer(text))
        for k, m in enumerate(found):
            repo = seen.commit_repos[min(k, len(seen.commit_repos) - 1)]
            res.commits.append(Commit(repo, m["branch"], m["hash"], m["msg"][:200]))
        if not found and seen.log_after_commit:
            # ``git commit -q ... && git log --oneline -1``: the hash the log printed (to be
            # checked against the repository's history: an older commit can be listed too).
            for m in ONELINE.finditer(text):
                repo = seen.commit_repos[-1]
                res.commits.append(Commit(repo, "", m["hash"], m["msg"][:200], inferred=True))


def _hits(text: str, base: str, ctx: Context, single: str | bool | None) -> dict[str, list[int]]:
    """Files whose lines a search printed, from ``path:line:`` (or ``path:``) prefixes; a search
    of one file prints no names, so its hits are that file's."""
    out: dict[str, list[int]] = {}
    any_named = False
    for line in text.splitlines():
        m = HIT_LINE.match(line)
        if m and _pathlike(m["p"]):
            out.setdefault(_abs(m["p"], base, ctx), []).append(int(m["n"]))
            any_named = True
            continue
        m = BINARY_MATCH.match(line)
        if m and _pathlike(m["p"]):
            out.setdefault(_abs(m["p"], base, ctx), [])
            any_named = True
            continue
        m = HIT_NOLINE.match(line)
        if m and _pathlike(m["p"]) and not m["p"].startswith(("http", "Error", "error", "warning")):
            out.setdefault(_abs(m["p"], base, ctx), [])
            any_named = True
    if not any_named and isinstance(single, str) and single and text.strip():
        nums = [int(n) for n in re.findall(r"^(\d+)[:-]", text, re.M)]
        out[_abs(single, base, ctx)] = nums
    return out


def _glob(p: str) -> bool:
    return any(ch in p for ch in "*?[")


def _pathlike(p: str) -> bool:
    return " " not in p and len(p) < 400 and ("/" in p or bool(PATHLIKE_EXT.search(p)))


def _looks_like_file(p: str) -> bool:
    return bool(PATHLIKE_EXT.search(p)) or posixpath.basename(p) in ("Makefile", "Dockerfile")


def _abs(path: str, cwd: str, ctx: Context) -> str:
    if not path:
        return ""
    return shell.resolve(path, cwd, ctx.home)


def repo_root(path: str, ctx: Context) -> str:
    """The checkout a path lies in (a worktree, the main checkout, the site, a scratch clone), or
    the path itself."""
    ws = ctx.workspace.rstrip("/")
    m = re.match(re.escape(ws) + r"/larmorx/\.claude/worktrees/[^/]+", path)
    if m:
        return m[0]
    for repo in ("larmorx_documentation", "larmorx-testdata", "larmorx-heavy", "larmorx"):
        r = f"{ws}/{repo}"
        if path == r or path.startswith(r + "/"):
            return r
    m = re.match(r"^(/tmp/claude-\d+/[^/]+/[^/]+/scratchpad/[^/]+/(?:[^/]+))", path)
    if m and posixpath.basename(m[1]) in ("larmorx_documentation", "larmorx-testdata", "larmorx"):
        return m[1]
    return path
