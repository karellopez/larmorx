# glibc-scanf-probe: `sscanf` and `fscanf` at the end of the input

**Open** (it calls the C library only; no glibc or FreeSurfer code).

`scanf_errno.c` measures glibc 2.35's `sscanf` and `fscanf` as a black box: for the directives
FreeSurfer's surface text readers use (text that must match, white space, `%d`, `%f`, `%s` and the
suppressed forms), each call's return value and the `errno` it leaves (5 before each call), at and
near the end of the input. The behaviour it shows is written in `specs/freesurfer-surfio.md` §56.

```
gcc -O0 -o scanf_errno scanf_errno.c && ./scanf_errno
```

Observed (2026-11-13, glibc 2.35, Ubuntu 22.04): after a call first meets the end of its input,
every later attempt of the same call to read a character sets `errno` back to its value at that
moment; an end met while a conversion skips the white space before its item is remembered as 0; a
number and `inf` look one character past themselves, a complete `nan` or `infinity` does not:

```
[POINTS] [POINTS %d float\n] r=-1 errno=0
[POINT] [POINTS %d float\n] r=-1 errno=5
[POINTSX] [POINTS %d float\n] r=0 errno=5
[99999999999 1e99 2 3 4] [%*d %*f %*f %*f %f\n] r=1 errno=34
[1 2 3 4 1e99] [%*d %*f %*f %*f %f\n] r=1 errno=5
[FIELD] [FIELD %*s %d\n] r=-1 errno=0
[FIELD a] [FIELD %*s %d\n] r=-1 errno=5
[] [%d] r=-1 errno=0
[] [x%d] r=-1 errno=5
[1e99] [%f %d] r=1 errno=5
[1e99 ] [%f %d] r=1 errno=0
[1e99 x] [%f %d] r=1 errno=34
[inf] [%f %d] r=1 errno=5
[nan] [%f %d] r=1 errno=0
[infinity] [%f %d] r=1 errno=0
fscanf %f r=1 errno=34          (a file holding 1e99)
fscanf %f at the end r=-1 errno=0
fscanf x at the end r=-1 errno=5
fscanf \n at the end r=0 errno=5
```
