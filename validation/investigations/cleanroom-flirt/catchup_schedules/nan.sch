setscale 1 force
setoption costfunction bbr
setoption optimisationtype brent
setoption boundguess 1
clear U
clear UU
setrow UU 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1
measurecost 6 UU:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 8
print U
clear U
measurecost 12 UU:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 8
print U
clear U
measurecost 6 UU:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  abs 8
print U
clear U
measurecost 6 UU:1  0.1   0.0   0.0   1.0   0.0   0.0   0.0  rel 8
print U
clear U
optimise 6 UU:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1
print U
optimise 12 U:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1
print U
