# SPDX-License-Identifier: Apache-2.0
r() { P=$1; shift; printf '== %s [%s]\n' "$(basename $P)" "$*"; env -i PATH=/usr/bin:/bin HOME=/nonexistent-home timeout 10 tcsh -f $P "$@" 2>&1 | cat -A | sed 's/^/  /'; }
S=$1; G=$2
for a in 'x _y' 'x y_z' 'x y-z' 'x y = z' 'x y =' 'x y[1]=z' 'x Y' 'x é' "$(printf 'x\ny')" "$(printf 'x\n')" "$(printf '\nx')" '*' '.*' '[[:alpha:]]' '[a-]' '=1/f' '=/f' '\x'; do r $S/p2.csh "$a"; done
for a in '[*]s' 'q*' '*' ; do r $S/p3.csh "$a"; done
for a in 'a\bb' 'a\fb' 'a\rb' 'a\vb' 'a\101b' 'a\x41b' 'a\0b' 'a\08b' 'a\c' 'a\' 'a\q' 'a\0141\0142' '-n' '-n x' '-e' 'a\\nb' 'zz*' 'a*' '-' '--'; do r $S/p4.csh "$a"; done
r $S/p5.csh
for a in '+/f' '+0/f' '+1/f' '+x/f' '=1/f' '[*]s' 'q*' './-/f' '.h' '.' '/.' '//' '///x' 'a/.' 'a//'; do r $G "$a"; done
