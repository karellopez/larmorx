# The easy command line: when a tool's messages reach the user

`stream_timing.py` runs one long, talkative command (`N4BiasFieldCorrection -v` on a generated
192³ image, 4 threads) through one or more `larmorx` command lines, reads its standard output and
error as pipes, and records when each line arrives: the run's duration, the first and last line's
time, the median line's time and the share of the lines that arrived in the first half of the
run.

```
python -P validation/investigations/easy-cli/stream_timing.py --work <scratch folder> \
    --larmorx "python -c 'import sys; from larmorx.cli import main; sys.exit(main())'" \
    --larmorx target/release/larmorx
```

(with `PYTHONPATH=<checkout>/python`, inside `scripts/orchestration/boxed.sh`; a `larmorx`
without the options, 0.1.0rc1, is run with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` instead).

## What it showed (2026-11-11, main machine)

| command line | run (s) | first line (s) | median line (s) | lines in the first half |
|---|---|---|---|---|
| console script, main `7cd35c8c` (0.1.0rc1's code) | 12.55 | 12.54 | 12.54 | 0 % |
| console script, this branch | 12.59 | 0.01 | 12.44 | 0.4 % |
| standalone program, this branch | 12.61 | 0.00 | 12.46 | 0.4 % |

0.1.0rc1's console script (`pip install larmorx`'s `larmorx`) collected the tool's output and
wrote it when the tool ended: nothing appeared for the whole run. The console script now passes
each piece to `sys.stdout` and `sys.stderr` as the tool writes it (`crates/larmorx-py/src/stream.rs`;
standard output line-buffered and standard error unbuffered, as the standalone program's
streams; a replica program's output read from its pipes as it comes,
`larmorx_cli::replica::Output::Stream`), and its lines arrive when the standalone program's do,
with the same bytes (`tests/python/test_cli_options.py`). (N4 itself writes most of its lines
during its last, full-resolution level, so most arrive near the end in both.)

## The docs' examples, as a user types them (2026-11-11, relay 2)

The README's First steps (this branch: the options before the family) and the documentation
site's installation page (0.1.0rc1 from PyPI: the environment variables) each hold one example
from start to end, for bash and zsh and for Windows PowerShell, on OpenNeuro's ds000005 sub-01
(public domain). Every line was run as typed:

- `run_examples.py`: a document's block after a marker, fed line by line to an interactive
  `bash --norc --noprofile -i` or `zsh -f -i` through a terminal (`script`), each line followed by
  a line printing its exit status, in a clean environment (a new home folder, the system's `PATH`
  with only a Python 3.12 as `python3` in front, no FSL or larmorx variables). zsh was
  conda-forge's 5.9.2, installed user-level (`micromamba create -p <scratch>/zsh-env -c
  conda-forge zsh`); the session prints a probe (`echo zsh-comment-probe # visible-in-zsh`) that
  shows INTERACTIVE_COMMENTS is off, as at macOS's prompt.

  ```
  python -P run_examples.py --doc README.md --marker "**An example, start to end**" \
      --shell <zsh> --work <new folder> --python <python3.12> \
      --replace "pip install larmorx=pip install <this branch's wheel>"
  ```

- `win_examples.py` and `win_examples.sh`: the PowerShell blocks run line by line in Windows
  PowerShell 5.1 on the Windows helper, in a new folder of the profile that is removed and
  checked gone afterwards (pip's cache, the temporary folder and Python's bytecode inside it;
  Python 3.12 first on `PATH`, as an installed Python is). The README's run installs this
  branch's Windows wheel, cross-built by `win_build.sh` (Rust's `x86_64-pc-windows-gnu` target
  and zig, no container); the site's installs 0.1.0rc1 from PyPI.

  ```
  bash win_examples.sh <new folder name> <wheel> <site checkout> <log>
  ```

| run | lines | failed |
|---|---|---|
| README, interactive bash (branch wheel) | 17 | 0 |
| README, interactive zsh (branch wheel) | 17 | 0 |
| site, interactive bash (0.1.0rc1 from PyPI) | 17 | 0 |
| site, interactive zsh (0.1.0rc1 from PyPI) | 17 | 0 |
| README, Windows PowerShell 5.1 (branch wheel, cross-built) | 17 | 0 |
| site, Windows PowerShell 5.1 (0.1.0rc1 from PyPI) | 22 | 0 |

The four Linux runs wrote the same bytes (`t1w_n4`, `t1w_n4_ants`, both masks, `bold2t1w.mat`,
`t1w_brain`, `bold_mcf`): the options (`--n-threads 4`, `--implementation clean-room`,
`--upstream-defects reproduce`) give what 0.1.0rc1's variables give. The first steps' other
lines too: the `sysconfig` line (and its `--user` form) in bash, zsh and PowerShell 5.1, and
`python -m larmorx --n-threads 2 --implementation=clean-room afni 3dAutomask ...` (the mask
byte-identical to the console script's).

`site/shots.sh` took the site's screenshots before (main) and after (branch `easy-cli-pages`):
the installation page whole, the home page's command-line block with its new sentence, and the
four pages whose shell blocks lost their comments, 1440 and 390 px, light and dark. The page
inventory (`validation/investigations/afni-volreg-original/site_inventory.py`): no count of any
page went down; the installation page 1,676 -> 2,629 words, the home page 5,076 -> 5,100, the
FAST, FLIRT, mcflirt and validation pages +4 to +8 words (each comment's text now in the prose).
