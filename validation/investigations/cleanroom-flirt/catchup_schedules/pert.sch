setscale 4 force
clear U
clear UU
setrow UU 1.2 0 0 0  0.1 1 0 0  0 0 0.9 0  0 0 0 1
measurecost 6 UU:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 8
print U
clear U
measurecost 6 UU:1  1.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 8
print U
clear U
measurecost 7 UU:1  0.0   0.0   0.0   0.0   0.0   0.0   1.0  rel 8
print U
clear U
measurecost 12 UU:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 8
print U
clear U
gridmeasurecost 6 UU:1  0.0 0.1 0.1 abs 8
print U
