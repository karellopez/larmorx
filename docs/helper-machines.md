# Helper machines: the old laptop and the Windows laptop

From 2026-10-27 the main machine is the new laptop (Alienware m18 R1, AMD, 32 threads, 124 GiB,
Ubuntu 22.04; `docs/machine-move.md`). Every agent and every record runs there. The helpers
are reached from it by SSH with keys (never passwords, never through Claude), each with a short
name in the main machine's `~/.ssh/config`:

| Name | Machine | Used for |
|---|---|---|
| `old-laptop` | MSI GE75 Raider 8SF: Intel i7-8750H, 6 cores / 12 threads, 31 GiB, Ubuntu 22.04 | speed measurements on a quiet machine; a second Linux reference (Intel, where the main machine is AMD) |
| `win-laptop` | the Windows laptop | Windows checks: the wheel, the golden suite, the command line |
| (later) | the lab's server, 1 TB of memory, shared | only the jobs too large for the main machine (the 25 queued heavy random-testing cases) |

## The old laptop

### What it holds

Its helper kit, once the move's phase 4 has cleaned it (`docs/machine-move.md`): the `larmorx`
main checkout and its build, `.venv`, Rust and Node; every original program (ANTs, AFNI, FSL,
the CORE-MATH preload, the toolchain, the fMRIPrep image); the test data; its own oracle cache
(keyed to its CPU, so tests there reuse it); the fMRIPrep runs (so it can run any test, the data
panel included). So it can run any benchmark or parity suite on its own; only larmorx's code has
to be brought up to date, and new original programs (the newest FSL and AFNI after the version
update, FLIRT, FreeSurfer, ...) are copied to it from the main machine.

### Bringing a commit to it

On the main machine, once (a git remote that points at the old laptop's repository):

```bash
cd /home/karelo/VScode_projects/super_fmriprep/larmorx
git remote add old-laptop old-laptop:/home/karelo/VScode_projects/super_fmriprep/larmorx
```

For each measurement, push the commit to a branch there (pushing to a branch that is not checked
out there is allowed, and nothing on the old laptop changes until it is checked out):

```bash
git push old-laptop <commit>:refs/heads/bench/<topic>
ssh old-laptop 'cd ~/VScode_projects/super_fmriprep/larmorx && git status --short | head -3 \
  && git checkout --detach bench/<topic> && source scripts/orchestration/env.sh \
  && maturin develop --release && scripts/orchestration/build-gpl.sh . && scripts/orchestration/build-nc.sh .'
```

The old laptop's main checkout is its own: `maturin develop` there points the old laptop's venv at
it, which is what is wanted (the rule against `maturin develop` concerns agent worktrees on the
main machine). The tree must be clean before the checkout (`git status` prints nothing), so a
record made there is not stamped `-dirty`. The build takes 10 to 20 minutes on the old laptop.

### Measuring speed there

A speed measurement wants a quiet machine:

- plugged in, lid open, the network cable in (or the Wi-Fi left alone);
- nothing else running: no agent session, no difftest night, no parity run, VS Code closed or idle;
- the same power profile at every measurement (`powerprofilesctl get`; `performance` if offered);
- inside a memory box: `scripts/orchestration/boxed.sh 24 ...` (the machine has 31 GiB);
- the thread counts the reports use: `--threads 1 4 0` (0 = all 12 there).

Run it in `tmux`, so it outlives the SSH connection, and fetch the report back:

```bash
ssh old-laptop 'cd ~/VScode_projects/super_fmriprep/larmorx && tmux new -d -s bench \
  "source scripts/orchestration/env.sh && scripts/orchestration/boxed.sh 24 \
   python -m larmorx_validation bench ants-n4 --repeats 3 --threads 1 4 0 --out /tmp/bench-out \
   > oracles/logs/bench-ants-n4-old-laptop.log 2>&1; echo BENCH-EXIT \$? >> oracles/logs/bench-ants-n4-old-laptop.log"'
# wait for the last line: ssh old-laptop tail -1 ~/VScode_projects/super_fmriprep/larmorx/oracles/logs/bench-ants-n4-old-laptop.log
rsync -a old-laptop:/tmp/bench-out/ <scratch>/bench-old-laptop/
```

Every report records the machine it ran on (`larmorx_validation.environment`), so a report from
the old laptop says so. The benchmark reports in `docs/benchmarks/` made before the move were
measured on this laptop; re-measuring a tool there keeps its numbers comparable with them.

### As a second Linux reference

The old laptop is an Intel CPU (AVX2), the main machine an AMD one (AVX-512). larmorx must give
the same bits on both (no fast-math, deterministic reductions, the same answer from every SIMD
path chosen at run time). After a change to a numerical kernel or to SIMD dispatch, run the
golden suite and the changed tool's smoke parity there too:

```bash
ssh old-laptop 'cd ~/VScode_projects/super_fmriprep/larmorx && source scripts/orchestration/env.sh \
  && pytest -q tests/golden && pytest -q tests/parity -k <tool>'
```

A difference between the two machines is a finding (`docs/findings/`), explained before merging.

### Keeping it current

Only larmorx's repository changes there (by the pushes above). A new original program installed
or built on the main machine (the newest FSL and AFNI, FLIRT, FreeSurfer, ...) is copied over with
`rsync -a <dir>/ old-laptop:<same dir>/` before the old laptop measures it. Nothing else is
kept there: no agent sessions, no scratch, no archive.

## The Windows laptop

Set up on 2026-10-28: Windows 11 (German), Intel i5-1335U (x64, 12 threads), 16 GiB; a domain
account that is a local administrator; on the same local network as the main machine. Git and
`uv` are installed there; `python` is only the Microsoft Store's stub; no Rust. In the main
machine's `~/.ssh/config` (the real address and account are only there, never in a repository):

```
Host win-laptop
    HostName <windows address>
    User <user>@<domain>
    ServerAliveInterval 30
```

Its address comes from the network's DHCP: if `ssh win-laptop` times out, `ipconfig | findstr IPv4`
there gives the new one for `HostName`.

How it was set up (to redo it on another Windows machine), on the Windows laptop in **PowerShell
run as administrator** (Start menu, type PowerShell, right-click, "Run as administrator"):

```powershell
# 1. The OpenSSH server (part of Windows)
Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0
Start-Service sshd
Set-Service -Name sshd -StartupType Automatic
# 2. The main machine's public key (`cat ~/.ssh/id_ed25519.pub` there), for an administrator account.
#    The groups by their SIDs, not their names: a German Windows calls them "Administratoren".
Add-Content -Force -Path "$env:ProgramData\ssh\administrators_authorized_keys" -Value 'PASTE-THE-KEY-HERE'
icacls.exe "$env:ProgramData\ssh\administrators_authorized_keys" /inheritance:r /grant "*S-1-5-32-544:F" /grant "*S-1-5-18:F"
# 3. The firewall: the installation's rule covers only "Private" networks, and a domain laptop
#    is on a "DomainAuthenticated" one. Every profile, but only from the local network:
Set-NetFirewallRule -Name OpenSSH-Server-In-TCP -Profile Any -RemoteAddress <local network, e.g. 192.168.1.0/24>
# 4. Keys only, no passwords over SSH (a copy of the settings kept beside them):
$f = "$env:ProgramData\ssh\sshd_config"
Copy-Item $f "$f.before-larmorx"
(Get-Content $f) -replace '^#PasswordAuthentication yes$', 'PasswordAuthentication no' | Set-Content $f -Encoding ascii
Restart-Service sshd
# The address and the user name for the main machine's ~/.ssh/config
ipconfig | findstr IPv4
whoami
```

(For an account that is not an administrator, `whoami /groups | findstr S-1-5-32-544` prints
nothing, and the key goes into `C:\Users\<user>\.ssh\authorized_keys` instead, with no `icacls`
step.) Then, on the main machine, `ssh -o BatchMode=yes win-laptop hostname` must print the
Windows laptop's name without asking for a password. PowerShell over SSH: the login shell is
`cmd.exe`, whose quoting breaks most PowerShell lines; send them encoded instead:

```bash
winps() { ssh -o BatchMode=yes win-laptop "powershell -NoProfile -NonInteractive -EncodedCommand $(printf '%s' "$1" | iconv -t UTF-16LE | base64 -w0)"; }
winps 'Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version'
```

Claude Code's auto mode refused to change the Windows laptop's SSH settings from the main machine
(2026-10-28: step 4 above); such steps go to the user as commands to run there.

What the orchestrator then sets up over SSH, when a Windows check is first needed (no
administrator rights needed): Python 3.12 through `uv` (`uv python install 3.12`), Rust (rustup, the MSVC toolchain needs the Visual Studio Build Tools,
which the user installs once if a local build is wanted), a clone of the repository, and a
check that the CI's Windows wheel installs and passes `pytest tests/golden` there. Windows
checks never replace CI's Windows jobs; they reproduce a CI failure on a real machine.
