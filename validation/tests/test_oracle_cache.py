# SPDX-License-Identifier: Apache-2.0
"""The oracle-output cache (``larmorx_validation.oracle_cache``, docs/validation/oracle-cache.md):
the key (stable, sensitive to every part, blind to path names), the store (atomic, concurrent),
replays, the runs never cached, the modes, ``refresh`` and ``verify`` finding differences."""

from __future__ import annotations

import gzip
import json
import os
import stat
import subprocess
import sys
import textwrap
from pathlib import Path

import pytest

oracle_cache = pytest.importorskip("larmorx_validation.oracle_cache")

from larmorx_validation import oracle_cache as oc  # noqa: E402
from larmorx_validation.oracle_cache import compare, core, hashing, key, store  # noqa: E402
from larmorx_validation.oracle_cache.selftest import Factory, env_for, fake_suite  # noqa: E402

if sys.platform != "linux":  # the cache runs where the originals are (fcntl, ldd, /proc)
    pytest.skip("the oracle cache runs on Linux", allow_module_level=True)

#: The fake original: ``fake IN OUT [options]`` writes OUT from IN (reversed) and prints both
#: paths; its options make it fail, crash, hang, vary, or write elsewhere.
FAKE = """\
    import os, random, signal, sys, time
    args = sys.argv[1:]
    def opt(name):
        return args[args.index(name) + 1] if name in args else None
    src, dst = args[0], args[1]
    data = open(src, "rb").read()
    extra = os.environ.get("FAKE_SEED", "").encode()
    if "--random" in args:
        extra += str(random.random()).encode()
    if (secret := os.environ.get("FAKE_SECRET")) is not None:
        extra += open(secret, "rb").read()
    if (s := opt("--sleep")) is not None:
        time.sleep(float(s))
    if "--crash" in args:
        os.kill(os.getpid(), signal.SIGSEGV)
    if (stray := opt("--stray")) is not None:
        open(stray, "wb").write(b"stray")
    if (also := opt("--also")) is not None:
        open(also, "wb").write(b"second output")
    if (tmpname := opt("--write-own-path")) is not None:
        extra += dst.encode()
    open(dst, "wb").write(data[::-1] + extra)
    print("read", src, "wrote", dst)
    print("note:", os.path.basename(src), file=sys.stderr)
    sys.exit(int(opt("--exit") or 0))
"""


def make_program(directory: Path, body: str = FAKE, name: str = "fake") -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / name
    path.write_text(f"#!{sys.executable}\n" + textwrap.dedent(body), encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP)
    return path


@pytest.fixture
def cache(tmp_path, monkeypatch):
    """An empty oracle cache for the test, in ``use`` mode; the hash memo forgotten."""
    from larmorx_validation.oracle_cache import hashing

    root = tmp_path / "cache"
    monkeypatch.setenv(oracle_cache.ENV_LOCATION, str(root))
    monkeypatch.setenv(oracle_cache.ENV_MODE, "use")
    monkeypatch.delenv("FAKE_SEED", raising=False)
    monkeypatch.delenv("FAKE_SECRET", raising=False)
    hashing.clear_memo()
    oracle_cache.begin_case("test")
    yield oracle_cache.Store(root)
    oracle_cache.end_case()


@pytest.fixture
def fake(tmp_path_factory) -> Path:
    """The fake original program (in a directory of its own: a script's identity is its
    build's, every file beside it)."""
    return make_program(tmp_path_factory.mktemp("bin"))


@pytest.fixture
def workdir(tmp_path_factory):
    """A factory of fresh case directories (each in its own temporary directory, as the
    suites' cases are), with an input file written."""

    def make(content: bytes = b"hello world", name: str = "in.nii") -> tuple[Path, Path, Path]:
        d = tmp_path_factory.mktemp("lx-case-")
        (d / "out").mkdir()
        src = d / name
        src.write_bytes(content)
        return d, src, d / "out" / "result.nii"

    return make


ONE = oc.Policy(threads=1, family="itk")


def run(fake: Path, src: Path, dst: Path, *extra: str, policy=ONE, env=None, **kw):
    return oc.run([fake, src, dst, *extra], env=env or env_for(), policy=policy, **kw)


def entries(cache) -> list[dict]:
    return list(cache.entries())


def prepared(fake: Path, src: Path, dst: Path, *extra: str, env=None, **kw) -> key.Prepared:
    env = env or env_for()
    return key.prepare(
        key.program_identity(fake, env), [str(src), str(dst), *extra], env, family="itk", **kw
    )


# ------------------------------------------------------------------------------------------------
# The key


def test_key_is_stable_and_blind_to_path_names(cache, fake, workdir):
    d1, src1, dst1 = workdir(b"same bytes", "first.nii")
    d2, src2, dst2 = workdir(b"same bytes", "other-name.nii")
    assert d1 != d2
    k1, k2 = prepared(fake, src1, dst1), prepared(fake, src2, dst2)
    assert k1.key == k2.key
    assert k1.template == ["{in0}", "{d0}/out/result.nii"]
    assert k1.parts["directories"] == ["{d0}/out"]
    # the same again (another process would compute the same: no randomness in the key)
    assert prepared(fake, src1, dst1).key == k1.key


@pytest.mark.parametrize(
    "change",
    [
        "input content",
        "argument",
        "suffix",
        "seed",
        "threads",
        "preload",
        "program",
        "machine",
        "cwd",
        "companion",
        "output name",
    ],
)
def test_every_part_of_the_key_changes_it(cache, fake, workdir, tmp_path, monkeypatch, change):
    d, src, dst = workdir(b"0123456789", "image.hdr")
    (d / "image.img").write_bytes(b"companion")
    env = env_for(ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS="1", ANTS_RANDOM_SEED="42")
    base = prepared(fake, src, dst, "--opt", "1", env=env)
    args = [src, dst, "--opt", "1"]
    kw: dict = {}
    if change == "input content":
        src.write_bytes(b"9876543210")
    elif change == "argument":
        args[3] = "2"
    elif change == "suffix":
        src = src.rename(d / "image.nii")
        args[0] = src
    elif change == "seed":
        env = dict(env, ANTS_RANDOM_SEED="43")
    elif change == "threads":
        env = dict(env, ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS="2")
    elif change == "preload":
        lib = tmp_path / "libfake.so"
        lib.write_bytes(b"a library")
        env = dict(env, LD_PRELOAD=str(lib))
    elif change == "program":
        fake.write_text(fake.read_text() + "\n# changed\n")
        hashing.clear_memo()
    elif change == "machine":
        monkeypatch.setattr(key, "machine", lambda: {"arch": "x86_64", "cpu": "another"})
    elif change == "cwd":
        kw["cwd"] = d
    elif change == "companion":
        (d / "image.img").write_bytes(b"another companion")
    elif change == "output name":
        args[1] = dst.with_name("other.nii")
    hashing.clear_memo()
    other = key.prepare(
        key.program_identity(fake, env), [str(a) for a in args[:]], env, family="itk", **kw
    )
    assert other.key != base.key, change


def test_a_preloaded_library_counts_by_its_content(cache, fake, workdir, tmp_path):
    _d, src, dst = workdir()
    a, b = tmp_path / "a" / "libm.so", tmp_path / "b" / "libm.so"
    for lib in (a, b):
        lib.parent.mkdir()
        lib.write_bytes(b"the same library")
    ka = prepared(fake, src, dst, env=env_for(LD_PRELOAD=str(a)))
    kb = prepared(fake, src, dst, env=env_for(LD_PRELOAD=str(b)))
    assert ka.key == kb.key


def test_variables_outside_the_family_do_not_count(cache, fake, workdir):
    _d, src, dst = workdir()
    base = prepared(fake, src, dst)
    for name, value in (
        ("PATH", "/nowhere"),
        ("FSLDIR", "/opt/fsl"),
        ("AFNI_X", "YES"),
        ("FAKE_SEED", "1"),
        ("HOME", "/root"),
    ):
        assert prepared(fake, src, dst, env=env_for(**{name: value})).key == base.key, name
    # the family's own variables do
    assert prepared(fake, src, dst, env=env_for(NSLOTS="3")).key != base.key


def test_home_is_named_like_a_place_and_its_logs_do_not_count(cache, tmp_path, workdir):
    """AFNI prints its HOME (where it appends its crash log): replayed with this run's; the
    files it writes there and never reads are not in the key."""
    prog = make_program(
        tmp_path / "bin",
        "import os, sys\n"
        "log = os.path.join(os.environ['HOME'], '.afni.crashlog')\n"
        "open(log, 'a').write('crash\\n')\n"
        "print('** Crash log is appended to file', log, file=sys.stderr)\n",
    )
    policy = oc.Policy(threads=1, family="afni")
    homes = []
    for i in range(2):
        home = tmp_path / f"lx-gen-{i}" / "home"
        home.mkdir(parents=True)
        homes.append(home)
        oc.begin_case(f"case {i}")
        r = oc.run([prog], env=env_for(HOME=str(home)), policy=policy)
        assert r.cached == (i == 1)
        assert r.stderr == f"** Crash log is appended to file {home}/.afni.crashlog\n"
    assert len(entries(cache)) == 1
    # the first run's crash log is in its HOME; the digest of the second's (empty) is the same
    assert (homes[0] / ".afni.crashlog").is_file()


def test_afni_family_reads_home_by_its_contents(cache, fake, workdir, tmp_path):
    _d, src, dst = workdir()
    home1, home2 = tmp_path / "h1", tmp_path / "h2"
    home1.mkdir()
    home2.mkdir()

    def k(home: Path) -> str:
        env = env_for(HOME=str(home), AFNI_DONT_LOGFILE="YES")
        return key.prepare(
            key.program_identity(fake, env), [str(src), str(dst)], env, family="afni"
        ).key

    assert k(home1) == k(home2)  # two empty homes
    (home2 / ".afnirc").write_text("AFNI_X = YES\n")
    assert k(home1) != k(home2)


def test_gzip_inputs_count_by_their_decompressed_content(cache, fake, workdir):
    d1, _, dst1 = workdir()
    d2, _, dst2 = workdir()
    a, b = d1 / "x.nii.gz", d2 / "x.nii.gz"
    a.write_bytes(gzip.compress(b"voxels", mtime=1))
    b.write_bytes(gzip.compress(b"voxels", mtime=2, compresslevel=1))
    assert a.read_bytes() != b.read_bytes()
    assert prepared(fake, a, dst1).key == prepared(fake, b, dst2).key
    b.write_bytes(gzip.compress(b"other voxels", mtime=2))
    hashing.clear_memo()
    assert prepared(fake, a, dst1).key != prepared(fake, b, dst2).key


def test_gunzip_digest_of_odd_files(tmp_path):
    two = tmp_path / "two.gz"
    two.write_bytes(gzip.compress(b"ab") + gzip.compress(b"cd"))
    one = tmp_path / "one.gz"
    one.write_bytes(gzip.compress(b"abcd"))
    assert hashing.sha256_content(two) == hashing.sha256_content(one)
    truncated = tmp_path / "truncated.gz"
    truncated.write_bytes(gzip.compress(b"abcd" * 1000)[:-12])
    assert hashing.sha256_content(truncated) == hashing.sha256_file(truncated)
    plain = tmp_path / "plain.gz"
    plain.write_bytes(b"not gzip at all")
    assert hashing.sha256_content(plain) == hashing.sha256_file(plain)


def test_the_hash_memo_sees_a_changed_file(tmp_path):
    p = tmp_path / "f"
    p.write_bytes(b"one")
    first = hashing.sha256_file(p)
    st = p.stat()
    p.write_bytes(b"two")
    os.utime(p, ns=(st.st_atime_ns, st.st_mtime_ns + 1000))
    assert hashing.sha256_file(p) != first


def test_suffixes_and_companions(tmp_path):
    assert key.suffix("sub-01_T1w.nii.gz") == ".nii.gz"
    assert key.suffix("anat+orig.HEAD") == "+orig.HEAD"
    assert key.suffix("x.1D") == ".1D"
    assert key.suffix("transform0GenericAffine.mat") == ".mat"
    (tmp_path / "a.hdr").write_bytes(b"h")
    (tmp_path / "a.img").write_bytes(b"i")
    assert key.companions(tmp_path / "a.hdr") == [tmp_path / "a.img"]


# ------------------------------------------------------------------------------------------------
# Storing and replaying


def test_a_run_is_stored_then_replayed_with_this_runs_paths(cache, fake, workdir):
    _d1, src1, dst1 = workdir(b"abc")
    first = run(fake, src1, dst1)
    assert not first.cached and first.returncode == 0
    (e,) = entries(cache)
    assert e["confirmations"] == 1 and e["required"] == 1
    _d2, src2, dst2 = workdir(b"abc", "renamed.nii")
    oc.begin_case("another case")  # a case that ran it already would run it again
    second = run(fake, src2, dst2)
    assert second.cached
    assert second.stdout == f"read {src2} wrote {dst2}\n"
    assert second.stderr == "note: renamed.nii\n"
    assert dst2.read_bytes() == b"cba"
    tally = oc.end_case()
    assert (tally.served, tally.ran) == (1, 0)


def test_stdout_bytes_are_what_subprocess_text_mode_gives(cache, tmp_path, workdir):
    prog = make_program(tmp_path / "bin", "import sys\nsys.stdout.write('a\\r\\nb\\rc\\xe9')\n")
    _d, _src, _dst = workdir()
    r = oc.run([prog], env=env_for(), policy=ONE)
    assert r.stdout == "a\nb\nc\xe9"
    oc.begin_case("another case")
    again = oc.run([prog], env=env_for(), policy=ONE)
    assert again.cached and again.stdout_bytes == r.stdout_bytes


def test_off_mode_never_reads_nor_writes(cache, fake, workdir):
    with oc.using("off"):
        _d, src, dst = workdir()
        assert not run(fake, src, dst).cached
        assert not run(fake, src, dst).cached
    assert entries(cache) == []


def test_exit_status_and_failures_are_cached(cache, fake, workdir):
    _d, src, dst = workdir()
    r = run(fake, src, dst, "--exit", "3")
    assert r.returncode == 3
    _d2, src2, dst2 = workdir()
    oc.begin_case("another case")
    again = run(fake, src2, dst2, "--exit", "3")
    assert again.cached and again.returncode == 3


def test_outputs_named_only_by_a_declared_place(cache, fake, workdir):
    d, src, dst = workdir()
    extra = d / "out" / "second.nii"
    r = run(fake, src, dst, "--also", extra)
    assert r.returncode == 0
    d2, src2, dst2 = workdir()
    extra2 = d2 / "out" / "second.nii"
    oc.begin_case("another case")
    again = run(fake, src2, dst2, "--also", extra2)
    assert again.cached and extra2.read_bytes() == b"second output"


def test_a_file_written_outside_the_places_is_never_stored(cache, fake, workdir, tmp_path):
    _d, src, dst = workdir()
    elsewhere = Path.cwd() / ".lx-oracle-cache-stray-test"
    try:
        run(fake, src, dst, "--stray", elsewhere.name)
    finally:
        elsewhere.unlink(missing_ok=True)
    assert entries(cache) == []
    assert any("outside the places" in why for why in oc.end_case().fresh)


def test_outputs_that_hold_a_temporary_path_are_never_stored(cache, fake, workdir):
    _d, src, dst = workdir()
    run(fake, src, dst, "--write-own-path", "1")
    assert entries(cache) == []


def test_nifti_extensions_may_hold_paths_when_the_policy_says_so(cache, fake, workdir):
    _d, src, dst = workdir()
    nii = dst.with_name("out.nii")
    policy = oc.Policy(threads=1, family="afni", nifti_extensions="test: not compared")
    run(fake, src, nii, "--write-own-path", "1", policy=policy)
    assert len(entries(cache)) == 1


# ------------------------------------------------------------------------------------------------
# What is never cached


def test_a_never_policy_always_runs(cache, fake, workdir):
    _d, src, dst = workdir()
    for _ in range(2):
        r = run(fake, src, dst, policy=oc.Policy(threads=1, never="the test says so"))
        assert not r.cached
    assert entries(cache) == []
    assert oc.end_case().fresh == {"never cached: the test says so": 2}


def test_programs_that_vary_above_one_thread_are_never_cached(cache, tmp_path, workdir):
    prog = make_program(tmp_path / "bin", name="antsRegistration")
    _d, src, dst = workdir()
    run(prog, src, dst, policy=oc.Policy(threads=4))
    assert entries(cache) == []
    run(prog, src, dst, policy=oc.Policy(threads=1))
    assert len(entries(cache)) == 1


def test_above_one_thread_a_run_is_served_after_three_identical_runs(cache, fake, workdir):
    four = oc.Policy(threads=4, family="itk")
    served = []
    for _ in range(4):
        oc.begin_case("a case")
        _d, src, dst = workdir()
        served.append(run(fake, src, dst, policy=four).cached)
    assert served == [False, False, False, True]
    (e,) = entries(cache)
    assert e["confirmations"] == 3 == e["required"]


def test_an_unknown_thread_count_needs_confirmations_and_keys_the_cpus(cache, fake, workdir):
    _d, src, dst = workdir()
    run(fake, src, dst, policy=oc.Policy(threads=None))
    (e,) = entries(cache)
    assert e["required"] == oc.CONFIRMATIONS and "cpus" in e["parts"]["machine"]


def test_crashes_and_timeouts_are_never_stored(cache, fake, workdir):
    _d, src, dst = workdir()
    crash = run(fake, src, dst, "--crash")
    assert crash.returncode < 0
    hang = run(fake, src, dst, "--sleep", "30", timeout=0.5)
    assert hang.timed_out and hang.returncode is None
    assert entries(cache) == []
    fresh = oc.end_case().fresh
    assert any("signal" in w for w in fresh) and any("timed out" in w for w in fresh)


def test_a_case_that_runs_a_command_again_runs_it_and_compares(cache, fake, workdir):
    run(fake, *workdir()[1:])
    oc.end_case()
    oc.begin_case("measures its spread")
    first = run(fake, *workdir()[1:])  # each run in a directory of its own, as the suites do
    second = run(fake, *workdir()[1:])
    assert first.cached and not second.cached
    again = "the case runs it again (served once 3 runs agree)"
    assert oc.end_case().fresh == {again: 1}
    (e,) = entries(cache)
    assert [h["event"] for h in e["history"]] == ["same"]
    # once three runs agreed, a case that runs it again is served it too
    oc.begin_case("a later run of the case")
    assert run(fake, *workdir()[1:]).cached
    assert not run(fake, *workdir()[1:]).cached
    oc.begin_case("and the next")
    assert run(fake, *workdir()[1:]).cached
    assert run(fake, *workdir()[1:]).cached
    assert core.identical_runs(cache.read(e["key"])) == 3


def test_an_output_there_before_the_run_is_an_input(cache, fake, workdir):
    """A file the arguments name that exists is an input (its content is in the key), and the
    run may write it: its rewrite is captured and put back."""
    _d, src, dst = workdir()
    dst.write_bytes(b"there before")
    run(fake, src, dst)
    (e,) = entries(cache)
    assert len(e["parts"]["inputs"]) == 2
    assert [o["path"] for o in e["result"]["outputs"]] == ["result.nii"]
    oc.begin_case("again")
    _d2, src2, dst2 = workdir()
    dst2.write_bytes(b"there before")
    assert run(fake, src2, dst2).cached
    assert dst2.read_bytes() == b"dlrow olleh"


def test_a_varying_original_run_twice_in_a_case_is_a_finding(cache, fake, workdir, capsys):
    run(fake, *workdir()[1:], "--random")
    run(fake, *workdir()[1:], "--random")  # the same command, again in the case: compared
    tally = oc.end_case()
    assert len(tally.differences) == 1
    (e,) = entries(cache)
    assert e["disputed"]
    assert "ORACLE CACHE" in capsys.readouterr().err
    oc.begin_case("later")
    assert not run(fake, *workdir()[1:], "--random").cached  # a disputed entry is never served


def test_policy_rules():
    assert oc.ants_seed_never("antsRegistration", ["-d", "3"], {}) != ""
    assert oc.ants_seed_never("antsRegistration", ["-d", "3"], {"ANTS_RANDOM_SEED": "1"}) == ""
    assert oc.ants_seed_never("antsRegistration", ["--random-seed", "5"], {}) == ""
    assert oc.ants_seed_never("antsAI", ["-m", "Mattes[f,m,32,Regular,0.25]"], {}) == ""
    assert "ANTS-58" in oc.ants_seed_never("antsAI", ["-m", "Mattes[f,m,32,Random,0.25]"], {})
    assert "ANTS-58" in oc.ants_seed_never("antsAI", ["--random-seed", "0"], {})
    assert oc.ants_seed_never("Atropos", ["-d", "3"], {}) != ""
    assert oc.ants_seed_never("Atropos", ["-r", "0"], {}) == ""
    assert oc.ants_seed_never("Atropos", ["--use-random-seed", "1"], {}) != ""
    assert oc.never_for("an expected divergence (ANTS-72): two runs differ").startswith("ANTS-72")
    assert oc.never_for("ANTS-32 corrected", "no defect") == ""
    assert oc.itk_threads({"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "4", "NSLOTS": "8"}) == 4
    assert oc.itk_threads({"NSLOTS": "8"}) == 8
    assert oc.itk_threads({}) is None
    assert oc.decide(oc.Policy(threads=4), "DenoiseImage").cache is False
    assert oc.decide(oc.Policy(threads=1), "DenoiseImage").cache is True


def test_the_suites_case_rules():
    """Each runner's per-case rule (docs/validation/oracle-cache.md, the table of rules)."""
    pytest.importorskip("larmorx_testdata")
    from larmorx_validation.parity import afni_replica, ants_atropos, ants_programs
    from larmorx_validation.parity import ants_registration as reg

    c = ants_programs.AntsCase("x", "c", "d", "ImageMath", ())
    assert ants_programs.case_never(c) == ""
    assert "never wrote" in ants_programs.case_never(
        ants_programs.AntsCase("x", "c", "d", "ImageMath", (), undefined_values="ANTS-28")
    )
    r = reg.RegCase("x", "c", "SyN then Affine (ANTS-72)", ())
    assert reg.case_never(r).startswith("ANTS-72")
    assert reg.case_never(reg.RegCase("x", "c", "d", (), expect="hang")) != ""
    assert reg.case_never(reg.RegCase("x", "c", "d", ())) == ""
    a = ants_atropos.AtroposCase("x", "c", "d", (), expect="random")
    assert ants_atropos.case_never(a) != ""
    s = afni_replica.Scenario(("{0}",), ("in",), ("out.nii",), undefined="AFNI-45")
    assert "never wrote" in afni_replica.case_never(s)
    assert afni_replica.case_never(afni_replica.Scenario(("{0}",), ("in",), ("o",))) == ""


# ------------------------------------------------------------------------------------------------
# refresh and verify


def test_refresh_reports_a_difference_and_keeps_both(cache, fake, workdir, monkeypatch, capsys):
    run(fake, *workdir()[1:])
    oc.end_case()
    oc.begin_case("refresh")
    with oc.using("refresh"):
        r = run(fake, *workdir()[1:])
        assert not r.cached
        oc.begin_case("refresh, later")
        # FAKE_SEED is not in the key: what refresh is there to catch
        run(fake, *workdir()[1:], env=env_for(FAKE_SEED="drift"))
    tally = oc.end_case()
    assert len(tally.differences) == 1
    (e,) = entries(cache)
    assert e["disputed"] and [h["event"] for h in e["history"]] == ["refreshed", "differs"]
    disputes = list((cache.root / "disputes").glob("*/*.json"))
    assert len(disputes) == 1
    record = json.loads(disputes[0].read_text())
    assert record["stored"]["outputs"][0]["sha256"] != record["fresh"]["outputs"][0]["sha256"]
    assert cache.has_blob(record["stored"]["outputs"][0]["sha256"])
    assert cache.has_blob(record["fresh"]["outputs"][0]["sha256"])
    findings = (cache.root / "findings.jsonl").read_text().splitlines()
    assert len(findings) == 1 and json.loads(findings[0])["key"] == e["key"]
    assert "ORACLE CACHE" in capsys.readouterr().err


def test_verify_finds_drift_the_key_misses(cache, fake, workdir, tmp_path, monkeypatch):
    from larmorx_validation.oracle_cache import rerun

    secret = tmp_path / "secret"
    secret.write_bytes(b"one")
    _d, src, dst = workdir()
    run(fake, src, dst, env=env_for(FAKE_SECRET=str(secret)))
    (e,) = entries(cache)
    monkeypatch.setenv("FAKE_SECRET", str(secret))
    assert rerun.rerun(cache, e).status == "same"
    secret.write_bytes(b"two")  # read by the program, not part of the key
    outcome = rerun.rerun(cache, cache.read(e["key"]))
    assert outcome.status == "differs"
    assert cache.read(e["key"])["disputed"]


def test_verify_skips_an_entry_whose_program_changed(cache, fake, workdir):
    from larmorx_validation.oracle_cache import rerun

    _d, src, dst = workdir()
    run(fake, src, dst)
    (e,) = entries(cache)
    fake.write_text(fake.read_text() + "\n# rebuilt\n")
    hashing.clear_memo()
    assert rerun.rerun(cache, e).status == "skipped"


def test_verify_command(cache, fake, workdir, capsys):
    from larmorx_validation.__main__ import main

    for content in (b"a", b"b", b"c"):
        _d, src, dst = workdir(content)
        run(fake, src, dst)
    assert main(["oracle-cache", "verify", "--sample", "2", "--seed", "1"]) == 0
    out = capsys.readouterr().out
    assert "2 same" in out
    assert main(["oracle-cache", "stats"]) == 0
    assert "3 entries: 3 served" in capsys.readouterr().out


def test_confirm_command_serves_runs_above_one_thread(cache, fake, workdir, capsys):
    from larmorx_validation.__main__ import main

    _d, src, dst = workdir()
    run(fake, src, dst, policy=oc.Policy(threads=2, family="itk"))
    assert main(["oracle-cache", "confirm"]) == 0
    (e,) = entries(cache)
    assert e["confirmations"] == e["required"] == oc.CONFIRMATIONS
    oc.begin_case("served")
    _d2, src2, dst2 = workdir()
    assert run(fake, src2, dst2, policy=oc.Policy(threads=2, family="itk")).cached


# ------------------------------------------------------------------------------------------------
# Atomic writes and concurrent workers


def test_atomic_write_leaves_nothing_on_failure(tmp_path, monkeypatch):
    target = tmp_path / "x" / "entry.json"
    store.write_atomic(target, b"first")

    def boom(*a, **k):
        raise OSError("disk full")

    monkeypatch.setattr(store.os, "replace", boom)
    with pytest.raises(OSError):
        store.write_atomic(target, b"second")
    assert target.read_bytes() == b"first"
    assert [p.name for p in target.parent.iterdir()] == ["entry.json"]


#: One worker: the fake program through the cache (``python -c``, another process).
WORKER = """
import sys
from larmorx_validation import oracle_cache as oc
from larmorx_validation.oracle_cache.selftest import env_for
oc.begin_case("worker")
r = oc.run(sys.argv[1:4], env=env_for(), policy=oc.Policy(threads=int(sys.argv[4]), family="itk"))
print("served" if r.cached else "ran", r.returncode)
"""


def test_concurrent_workers_store_one_entry(cache, fake, workdir):
    """Six processes run the same command at once (as ``--jobs`` workers can): one entry, never
    a torn file, and every run that ran either wrote it or confirmed it."""
    procs = []
    for _ in range(6):
        _d, src, dst = workdir(b"shared input")
        procs.append(
            subprocess.Popen(
                [sys.executable, "-c", WORKER, str(fake), str(src), str(dst), "2"],
                stdout=subprocess.PIPE,
                text=True,
                env={**os.environ, "PYTHONPATH": os.pathsep.join(sys.path)},
            )
        )
    outs = [p.communicate(timeout=120)[0].split() for p in procs]
    assert all(p.returncode == 0 for p in procs) and all(o[1] == "0" for o in outs)
    (e,) = entries(cache)
    assert not e.get("disputed")
    ran = sum(o[0] == "ran" for o in outs)
    assert ran >= 1
    assert e["confirmations"] == min(ran, e["required"])
    assert not [p for p in cache.root.rglob("*") if p.name.endswith(".tmp")]


def test_jobs_workers_through_the_harness(cache, fake):
    """``parity --jobs``: the cases in spawned workers, their tallies summed in the parent."""
    from larmorx_validation.__main__ import oracle_summary
    from larmorx_validation.parity import harness

    suite = fake_suite(str(fake))
    factory = Factory(str(fake), str(cache.root))
    first = harness.run(suite, "smoke", jobs=3, factory=factory)
    assert [r.status for r in first] == ["pass"] * 4
    second = harness.run(suite, "smoke", jobs=3, factory=factory)
    assert [r.status for r in second] == ["pass"] * 4
    assert (oracle_summary(first, "use")["ran"], oracle_summary(first, "use")["served"]) == (4, 0)
    summary = oracle_summary(second, "use")
    assert (summary["served"], summary["ran"]) == (4, 0)
    assert summary["location"]  # named in the record's header


# ------------------------------------------------------------------------------------------------
# Comparing runs, and the records


def test_same_file_ignores_compression_and_optionally_nifti_extensions(tmp_path):
    a, b = tmp_path / "a.nii.gz", tmp_path / "b.nii.gz"
    a.write_bytes(gzip.compress(b"data", mtime=1))
    b.write_bytes(gzip.compress(b"data", mtime=2))
    assert compare.same_file(a, b)[0]
    import numpy as np

    header = bytearray(348)
    header[:4] = (348).to_bytes(4, "little")
    header[344:348] = b"n+1\x00"

    def nifti(ext: bytes) -> bytes:
        h = bytearray(header)
        h[108:112] = np.array([352 + len(ext)], "<f4").tobytes()
        return bytes(h) + b"\x00" * 4 + ext + b"VOXELS"

    x, y = tmp_path / "x.nii", tmp_path / "y.nii"
    x.write_bytes(nifti(b"ext one"))
    y.write_bytes(nifti(b"another extension"))
    assert not compare.same_file(x, y)[0]
    assert compare.same_file(x, y, nifti_extensions=False)[0]


def test_volatile_text_is_masked():
    a = b"ITK ERROR: Filter(0x55d1aa)\nElapsed time (stage 0): 1.234\nElapsed = 0.5 sec\n"
    b = b"ITK ERROR: Filter(0x7f00bb)\nElapsed time (stage 0): 9.87\nElapsed = 1.25 sec\n"
    assert compare.mask(a) == compare.mask(b)
    assert compare.mask(b"value 1.0") != compare.mask(b"value 2.0")


def test_hdf5_strings_compare_by_their_contents(tmp_path):
    """Variable-length strings: their array's bytes are object pointers, never compared."""
    h5py = pytest.importorskip("h5py")
    paths = []
    for name in ("a.h5", "b.h5"):
        path = tmp_path / name
        with h5py.File(path, "w") as f:
            f.create_dataset("TransformGroup/0/TransformType", data=["AffineTransform_double_3_3"])
            f.attrs["note"] = ["x", "y"]
        paths.append(path)
    assert compare.same_file(*paths)[0]


def test_hdf5_outputs_compare_by_their_datasets(tmp_path):
    h5py = pytest.importorskip("h5py")
    a, b = tmp_path / "a.h5", tmp_path / "b.h5"
    for path, track in ((a, True), (b, False)):
        with h5py.File(path, "w", track_order=False) as f:
            f.create_dataset("TransformGroup/0/Parameters", data=[1.0, 2.0], track_times=track)
    assert a.read_bytes() != b.read_bytes()
    assert compare.same_file(a, b)[0]


def test_the_record_names_the_original_runs():
    from larmorx_validation.parity.render import oracle_line

    line = oracle_line(
        {
            "served": 7,
            "ran": 3,
            "fresh": {"never cached: x": 2, "refresh": 1},
            "location": "<workspace>/oracles/cache",
            "mode": "use",
        }
    )
    assert line == (
        "- **Original programs:** 10 runs: 7 from the oracle cache, 3 run (2 never cached: the "
        "original varies); cache `<workspace>/oracles/cache`, mode `use`"
    )


def test_store_refuses_another_format(tmp_path):
    s = store.Store(tmp_path / "c")
    s.ensure()
    (tmp_path / "c" / "FORMAT").write_text("999\n")
    with pytest.raises(RuntimeError):
        s.ensure()


def test_location_defaults_to_the_workspace(monkeypatch):
    monkeypatch.delenv(oc.ENV_LOCATION, raising=False)
    ws = store.workspace()
    if ws is None:
        pytest.skip("no workspace with oracles/ above this checkout")
    assert oc.location() == ws / "oracles" / "cache"


def test_mode_must_be_known(monkeypatch):
    monkeypatch.setenv(oc.ENV_MODE, "sometimes")
    with pytest.raises(ValueError):
        core.mode()


# ------------------------------------------------------------------------------------------------
# No duplicate runs: the golden recording reuses a passing smoke tier


def _record_golden():
    import importlib.util

    root = Path(__file__).resolve().parents[2]
    spec = importlib.util.spec_from_file_location(
        "lx_record_golden", root / "scripts" / "record_golden.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_record_golden_reuses_the_smoke_marker_of_this_commit_and_build(cache):
    pytest.importorskip("larmorx._core")
    import hashlib

    import larmorx._core as core

    rg = _record_golden()
    commit = rg.git("rev-parse", "HEAD")
    assert rg.smoke_marker() is None  # none written yet
    built = hashlib.sha256(Path(core.__file__).read_bytes()).hexdigest()
    path = cache.root / "markers" / f"parity-smoke-{commit}.json"
    marker = {
        "commit": commit,
        "time": "2026-10-24T12:00:00+00:00",
        "result": "passed",
        "extension_sha256": built,
    }
    store.write_atomic(path, store.dumps(marker))
    found = rg.smoke_marker()
    assert found is not None and found[1] == path
    store.write_atomic(path, store.dumps(marker | {"extension_sha256": "another build"}))
    assert rg.smoke_marker() is None
    store.write_atomic(path, store.dumps(marker | {"result": "failed"}))
    assert rg.smoke_marker() is None
