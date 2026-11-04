# SPDX-License-Identifier: Apache-2.0
x="a b"; [ _$x = _ ]; echo "s=$?"
x="a b c"; [ _$x = _ ]; echo "s=$?"
x="a b c d"; [ _$x = _ ]; echo "s=$?"
x="a b"; [ _$x != _ ]; echo "s=$?"
x="-d x"; [ $x = -d ]; echo "s=$?"
x="!"; [ $x = -d ]; echo "s=$?"
x="a b"; [ `echo 0 0` = 0 ]; echo "s=$?"
