setscale 1 force
setoption costfunction bbr
setoption optimisationtype brent
setoption boundguess 1
setoption bbrstep 200
clear U
clear UU
setrowqsform UU
setrow UU 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1
measurecost 6 UU:1-2  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 8
gridmeasurecost 6 UU:1-2  -0.07 0.07 0.07  -0.07 0.07 0.07  -0.07 0.07 0.07  -4.0 4.0 4.0  -4.0 4.0 4.0  -4.0 4.0 4.0  0.0 0.0 0.0  abs 8
print U
sort U
print U
