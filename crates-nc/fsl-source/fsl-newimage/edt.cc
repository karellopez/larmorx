// Matthew Webster, WIN Analysis Group
//Copyright (C) 2022 University of Oxford
/*  Part of FSL - FMRIB's Software Library
    http://www.fmrib.ox.ac.uk/fsl
    fsl@fmrib.ox.ac.uk

    Developed at FMRIB (Oxford Centre for Functional Magnetic Resonance
    Imaging of the Brain), Department of Clinical Neurology, Oxford
    University, Oxford, UK


    LICENCE

    FMRIB Software Library, Release 6.0 (c) 2018, The University of
    Oxford (the "Software")

    The Software remains the property of the Oxford University Innovation
    ("the University").

    The Software is distributed "AS IS" under this Licence solely for
    non-commercial use in the hope that it will be useful, but in order
    that the University as a charitable foundation protects its assets for
    the benefit of its educational and research purposes, the University
    makes clear that no condition is made or to be implied, nor is any
    warranty given or to be implied, as to the accuracy of the Software,
    or that it will be suitable for any particular purpose or for use
    under any specific conditions. Furthermore, the University disclaims
    all responsibility for the use which is made of the Software. It
    further disclaims any liability for the outcomes arising from using
    the Software.

    The Licensee agrees to indemnify the University and hold the
    University harmless from and against any and all claims, damages and
    liabilities asserted by third parties (including claims for
    negligence) which arise directly or indirectly from the use of the
    Software or the sale of any products based on the Software.

    No part of the Software may be reproduced, modified, transmitted or
    transferred in any form or by any means, electronic or mechanical,
    without the express permission of the University. The permission of
    the University is not required if the said reproduction, modification,
    transmission or transference is done without financial return, the
    conditions of this Licence are imposed upon the receiver of the
    product, and all original and amended source code is included in any
    transmitted product. You may be held legally responsible for any
    copyright infringement that is caused or encouraged by your failure to
    abide by these terms and conditions.

    You are not permitted under this Licence to use this Software
    commercially. Use for which any financial return is received shall be
    defined as commercial use, and includes (1) integration of all or part
    of the source code or the Software into a product for sale or license
    by or on behalf of Licensee to third parties or (2) use of the
    Software or any derivative of it for research with the final aim of
    developing software products for sale or license to a third party or
    (3) use of the Software or any derivative of it for research with the
    final aim of developing non-software products for sale or license to a
    third party, or (4) use of the Software to provide any service to an
    external organisation for which payment is received. If you are
    interested in using the Software commercially, please contact Oxford
    University Innovation ("OUI"), the technology transfer company of the
    University, to negotiate a licence. Contact details are:
    fsl@innovation.ox.ac.uk quoting Reference Project 9564, FSL.*/

#include "newimagefns.h"
#include "newimageio.h"
#include "edt.h"

using namespace std;

namespace NEWIMAGE {
  //Euclidean distance transform of a 1D function f
  //All elements of f should be real, but safety features have been added to handle Inf elements
  //For images, weight should be pixdim^2
  //aux is auxillary function ( e.g. for sparse interpolation )
  template<class AUX>
  vector<double> euclideanDistanceTransform(const vector<double>& f, const float weight, AUX&& aux) {
    int64_t n(f.size());
    vector<double> d(n), newAuxillary(n); //distance metric, aux output
    vector<int64_t> v(n,0);   //Only v[0] _needs_ to be 0
    vector<double> z(n+1,numeric_limits<double>::infinity()); //Only z[1] _needs_ to be inf
    z[0] = -numeric_limits<double>::infinity();
    for (int64_t q(1), k(0); q < n; q++) {
      double s(intersection(f,q,v[k],weight));
      while ( k > 0 && s <= z[k] ) //Safety addition: test k > 0
        s = intersection(f,q,v[--k],weight);
      v[++k] = q;
      z[k] = s;
      z[k+1] = numeric_limits<float>::infinity();
    }
    for (int64_t q(0), k(0); q < n; q++) {
      while (z[k+1] < q)
        k++;
      d[q] = ( weight*(q - v[k])*(q - v[k]) ) + f[v[k]];
      if ( !aux.empty() )
        newAuxillary[q]=aux[v[k]];
    }
    if ( !aux.empty() )
      aux=newAuxillary;
    return d;
  }

  template <class T>
  volume<double> euclideanDistanceTransform(const NEWIMAGE::volume<T>& input) {
    auto[ distanceMap, temp ] = euclideanDistanceTransform(input,input);
    return distanceMap;
  }

  template <typename T>
  tuple<volume<double>,volume<T>> euclideanDistanceTransform(const volume<T>& binary, const volume<T>& data) {
    binary.throwsIfNot3D();
    volume<double> distanceMap;
    volume<T> nearestData(data);
    copyconvert(binary,distanceMap);

    //"0D" initialisation
    for (int z=0; z<=distanceMap.maxz(); z++)
      for (int y=0; y<=distanceMap.maxy(); y++)
        for (int x=0; x<=distanceMap.maxx(); x++)
          distanceMap(x,y,z) = ( distanceMap(x,y,z) == 0) ? std::numeric_limits<float>::infinity() : 0;

    //1D:X
    vector<double> f(distanceMap.xsize());
    vector<double> aux(distanceMap.xsize());
    for (int z=0; z<=distanceMap.maxz(); z++)
      for (int y=0; y<=distanceMap.maxy(); y++) {
        for (int x=0; x<=distanceMap.maxx(); x++) {
          f[x] = distanceMap(x,y,z);
          aux[x] = nearestData(x,y,z);
        }
        vector<double> dvec( euclideanDistanceTransform(f,distanceMap.xdim()*distanceMap.xdim(),aux));
        for (int x=0; x<=distanceMap.maxx(); x++) {
          distanceMap(x,y,z) = dvec[x];
          nearestData(x,y,z) = aux[x];
        }
      }

    //2D:Y
    f.resize(distanceMap.ysize());
    aux.resize(distanceMap.ysize());
    for (int z=0; z<=distanceMap.maxz(); z++)
      for (int x=0; x<=distanceMap.maxx(); x++) {
        for (int y=0; y<=distanceMap.maxy(); y++) {
          f[y] = distanceMap(x,y,z);
          aux[y] = nearestData(x,y,z);
        }
        vector<double> dvec( euclideanDistanceTransform(f,distanceMap.ydim()*distanceMap.ydim(),aux));
        for (int y=0; y<=distanceMap.maxy(); y++) {
          distanceMap(x,y,z) = dvec[y];
          nearestData(x,y,z) = aux[y];
        }
      }

    //3D:Z
    f.resize(distanceMap.zsize());
    aux.resize(distanceMap.zsize());
    for (int x=0; x<=distanceMap.maxx(); x++)
      for (int y=0; y<=distanceMap.maxy(); y++) {
        for (int z=0; z<=distanceMap.maxz(); z++) {
          f[z] = distanceMap(x,y,z);
          aux[z] = nearestData(x,y,z);
        }
        vector<double> dvec( euclideanDistanceTransform(f,distanceMap.zdim()*distanceMap.zdim(),aux) );
        for (int z=0; z<=distanceMap.maxz(); z++) {
          distanceMap(x,y,z) = dvec[z];
          nearestData(x,y,z) = aux[z];
        }
      }

    return { distanceMap, nearestData };
  }

  //From a set of sparse data, defined by isSample > 0
  template<class T>
  volume<T> sparseInterpolate(const volume<T>& sparseData, const volume<T>& isSample, const double sigma) {

    if ( sparseData.tsize() > 1 ) {
      volume<T> iData(sparseData,TEMPLATE);
      for (int64_t t=0; t < sparseData.tsize(); t++)
        iData[t] = sparseInterpolate(sparseData[t],isSample[t],sigma);
      return iData;
    }

    volume<T> dmap, iData;
    tie(dmap,iData) = euclideanDistanceTransform(isSample,sparseData);

    auto kernel=gaussian_kernel3D(sigma,iData.xdim(),iData.ydim(),iData.zdim());
    if ( sigma > 0 )
      iData=generic_convolve(iData,kernel,true,true);
    for (int64_t z(0); z < iData.zsize(); z++)
      for (int64_t y(0); y < iData.ysize(); y++)
        for (int64_t x(0); x < iData.xsize(); x++)
              iData(x,y,z) = isSample(x,y,z) ? sparseData(x,y,z) : iData(x,y,z);
    return iData;
  }

  template<typename... Ts>
  auto edtInstantiate() {
    static auto instantiatedFunctions = std::tuple_cat(std::make_tuple(
      static_cast< volume<double>(*)(const volume<Ts>&)>(euclideanDistanceTransform<Ts>),
      static_cast< tuple<volume<double>,volume<Ts>>(*)(const volume<Ts>&, const volume<Ts>&)>(euclideanDistanceTransform<Ts>)
    )...);
    return &instantiatedFunctions;
  }

  template auto __attribute__((visibility("hidden"))) edtInstantiate<char,short,int,float,double>();

  template volume<double> sparseInterpolate(const volume<double>& data, const volume<double>& weights, const double sigma);
}