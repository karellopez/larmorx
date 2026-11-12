// SPDX-License-Identifier: Apache-2.0
// What an uncaught std::stol exception does (FS-05, docs/findings/freesurfer-core.md): build with
// any GCC (`g++ -O2 stol_probe.cpp -o stol_probe`) and run `./stol_probe abc`,
// `./stol_probe 99999999999999999999`, `./stol_probe " 12x"`; it prints a line first, as
// randomNumber does. README.md beside this file.
#include <string>
#include <iostream>
int main(int c,char**v){std::cout<<"x"<<std::endl; long s=std::stol(std::string(v[1])); std::cout<<s<<std::endl;}
