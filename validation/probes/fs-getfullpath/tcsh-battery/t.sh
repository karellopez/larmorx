# SPDX-License-Identifier: Apache-2.0
G=$1; shift
run() { printf '== [%s]\n' "$*"; env -i PATH=/usr/bin:/bin HOME=/nonexistent-home timeout 10 tcsh -f "$G" "$@" > o.txt 2> e.txt; echo "rc $?"; sed 's/^/ O: /' o.txt; sed 's/^/ E: /' e.txt; }
run
run d1/x.mgz
run 'd1/*.mgz'
run 'd1/z*.mgz'
run 'd1/{x,y}.mgz'
run 'a b/f'
run 'a  b/f'
run nodir/f
run afile/f
run lnk/f
run ''
run /
run .
run ..
run x
run d1/x.mgz extra
run '~/f'
run '-n'
run 'd1/-n'
run 'a\tb'
run '$HOME/f'
run '`echo hi`'
run 'd1/x.mgz;'
run '-e'
run 'd1/'
run 'd1//x'
run '[d]1/x.mgz'
run 'd1/x.mgz]'
