# glibc 2.35's `scanf` conversions, measured

What it shows: how the C library FreeSurfer's official binaries run on (glibc 2.35, Ubuntu 22.04)
reads `%f` and `%d` with `fscanf` and `sscanf`: each input's return value, the bits assigned, `errno`
and how many bytes were read. These black-box runs (no glibc source read) are the behaviour
`crates-freesurfer/larmorx-freesurfer-core/src/cscan.rs` reproduces; its unit tests hold the numbers.

- `scanf_probe.c`: `%f` on 100 inputs (numbers in every form, hexadecimal ones, `inf`, `nan`, broken
  and edge inputs, overflow and underflow, the tininess cases) and `%d` on 8.
- `fail_probe.c`: how many characters a failing `%f` reads (a number leaves the mismatching
  character unread, an unfinished `inf`, `infinity` or `nan` reads it).
- `seq_probe.c`, `eof_probe.c`, `eof2.c`, `file_probe.c`: `errno` across successive calls and at the
  end of the input (a conversion that meets the end before its item sets `errno` to 0, unless an
  earlier item of the call ended there).

Run: `gcc -O1 -o /tmp/p scanf_probe.c && /tmp/p` (each file alone; Linux with glibc 2.35).
