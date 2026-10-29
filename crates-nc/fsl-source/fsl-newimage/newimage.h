/*  Templated image storage class

    Mark Jenkinson, FMRIB Image Analysis Group

    Copyright (C) 2000 University of Oxford  */

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

#if !defined(__newimage_h)
#define __newimage_h

#define volume4D volume

#include <algorithm>
#include <cstdlib>
#include <iostream>
#include <limits>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <vector>

#include "armawrap/newmatap.h"
#include "voxeliterator.h"
#include "NewNifti/NewNifti.h"
#include "miscmaths/miscmaths.h"
#include "miscmaths/kernel.h"
#include "miscmaths/splinterpolator.h"
#include "utils/threading.h"


namespace NEWIMAGE {


  // constant which may be passed to samesize to allow
  // volumes of different dimensionality to be compared
  const int SUBSET = -1;

  // used inside imthrow to determine type of action
  const bool USEASSERT = false;

  void imthrow(const std::string& msg, int nierrnum, const bool quiet=false);

  enum extrapolation { zeropad, constpad, extraslice, mirror, periodic };

  enum interpolation { nearestneighbour, trilinear, sinc, userkernel, spline };

  enum threshtype { inclusive , exclusive };

  enum constructionMode { CLONE, TEMPLATE, ALIAS };

  // Internal identifiers for supported image file types.
  enum fileType {
    FSL_TYPE_ANALYZE          = 10,
    FSL_TYPE_NIFTI            =  1,
    FSL_TYPE_NIFTI2           =  2,
    FSL_TYPE_NIFTI_PAIR       = 11,
    FSL_TYPE_NIFTI2_PAIR      = 12,
    FSL_TYPE_ANALYZE_GZ       = 100,
    FSL_TYPE_NIFTI_GZ         = 101,
    FSL_TYPE_NIFTI2_GZ        = 102,
    FSL_TYPE_NIFTI_PAIR_GZ    = 111,
    FSL_TYPE_NIFTI2_PAIR_GZ   = 112,
    FSL_TYPE_ANALYZE_ZST      = 200,
    FSL_TYPE_NIFTI_ZST        = 201,
    FSL_TYPE_NIFTI2_ZST       = 202,
    FSL_TYPE_NIFTI_PAIR_ZST   = 211,
    FSL_TYPE_NIFTI2_PAIR_ZST  = 212,
    FSL_TYPE_ANALYZE_BZ2      = 300,
    FSL_TYPE_NIFTI_BZ2        = 301,
    FSL_TYPE_NIFTI2_BZ2       = 302,
    FSL_TYPE_NIFTI_PAIR_BZ2   = 311,
    FSL_TYPE_NIFTI2_PAIR_BZ2  = 312
  };

  // Allowed image extensions. The first extension is the header-bearing file,
  // the second the data-bearing file. For non-paired images these are the same,
  // but the consistent format can make automated extension processing easier.
  const std::vector<std::pair<const std::string, const std::string>> fileExtensions = {
    { ".nii.gz",  ".nii.gz" },
    { ".nii",     ".nii" },
    { ".hdr",     ".img" },
    { ".hdr.gz",  ".img.gz" },
    { ".nii.zst", ".nii.zst" },
    { ".hdr.zst", ".img.zst" },
    { ".nii.bz2", ".nii.bz2" },
    { ".hdr.bz2", ".img.bz2" }
  };

  enum orientation {
    FSL_RADIOLOGICAL = -1,
    FSL_NEUROLOGICAL = 1,
    FSL_INCONSISTENT = 0,
    FSL_ZERODET      = -101
  };


template<class T> class ShadowVolume;
template<class T> class volume;

template <class T>
int readGeneralVolume(volume<T>& target, const std::string& filename,
		  short& dtype, const bool swap2radiological=true,
		  int64_t x0 = -1, int64_t y0 = -1, int64_t z0 = -1, int64_t t0 = -1, int64_t d50 = -1, int64_t d60 = -1, int64_t d70 = -1,
		  int64_t x1 = -1, int64_t y1 = -1, int64_t z1 = -1, int64_t t1 = -1, int64_t d51 = -1, int64_t d61 = -1, int64_t d71 = -1,
      const bool readAs4D=false);

#pragma interface
  template <class T>
  class volume {
    friend class ShadowVolume<T>;
  private:
    T* Data;
    T* DataEnd;
    bool data_owner;
    double maskDelimiter;
    int64_t nElements;
    int64_t nThreads;

    int64_t ColumnsX;
    int64_t RowsY;
    int64_t SlicesZ;
    int64_t dim4;
    int64_t dim5;
    int64_t dim6;
    int64_t dim7;
    std::vector<int64_t> originalSizes;

    float Xdim;
    float Ydim;
    float Zdim;
    float p_TR;
    float pxdim5;
    float pxdim6;
    float pxdim7;

    // the following matrices map voxel coords to mm coords (nifti conventions)
    NEWMAT::Matrix StandardSpaceCoordMat;
    NEWMAT::Matrix RigidBodyCoordMat;
    int StandardSpaceTypeCode;
    int RigidBodyTypeCode;

    int IntentCode;
    float IntentParam1;
    float IntentParam2;
    float IntentParam3;

    int SliceOrderingCode;

    int64_t no_voxels;

    float displayMaximum;
    float displayMinimum;
    char auxFile[24];

    // Extrapolation method
    extrapolation p_extrapmethod;
    // Value to return for constpad, extraslice
    T padvalue;
    // Indicates if extrapolation can produce "valid" values.
    std::tuple<bool, bool, bool> ep_valid;

    // Interpolation method
    interpolation p_interpmethod;
    // Kernel used for sinc interpolation
    MISCMATHS::kernel interpkernel;

    // Object used to perform spline interpolation
    mutable SPLINTERPOLATOR::Splinterpolator<T> splint;
    // Used to lock recalculation of spline coefficients
    mutable std::mutex splinecoef_mutex;
    // Indicates need to recalculate spline coefficients
    mutable bool splineuptodate;
    // Spline-order (typically 3 for cubic splines)
    int splineorder;

    // ShadowVolume cache, created on-demand on
    // operator[timepoint] access, and cleared
    // on all operator() writes. A cache is used
    // to avoid unnececessary re-calculation of
    // spline coefficients.
    mutable std::map<int64_t, ShadowVolume<T>> sv_cache;
    mutable std::mutex                         sv_cache_mutex;

    // Struct which may be passed to interpolating functions,
    // and which controls extrapolation behaviour. This allows
    // calling code to customise the extrapolation settings
    // when using a const volume, without having to mutate the
    // volume state via the setextrapolation* functions.
    public:
    struct extrapparams {
    private:

      // Extrapolation method to use.
      extrapolation _method;

      // Value to return when extrapolating.
      T _padvalue;

      // Extrapolation reference target,
      //
      mutable T _extrapval;

      // xyz validity, used by the valid() function
      bool _xvalid;
      bool _yvalid;
      bool _zvalid;

      // Use volume.p_extrapmethod instead
      bool _use_default_method;

      // use volume.extrapval instead
      bool _use_default_padvalue;

      // use volume.ep_valid instead
      bool _use_default_validity;

    public:
      extrapolation method(const volume<T>& vol) const {
        if (_use_default_method) return vol.getextrapolationmethod();
        else                     return _method;
      }
      T padvalue(const volume<T>& vol) const {
        if (_use_default_padvalue) return vol.getpadvalue();
        else                       return _padvalue;
      }
      T& extrapval() const {
        return _extrapval;
      }
      bool xvalid(const volume<T>& vol) const {
        if (_use_default_validity) return vol.getextrapolationvalidity()[0];
        else                       return _xvalid;
      }
      bool yvalid(const volume<T>& vol) const {
        if (_use_default_validity) return vol.getextrapolationvalidity()[1];
        else                       return _yvalid;
      }
      bool zvalid(const volume<T>& vol) const {
        if (_use_default_validity) return vol.getextrapolationvalidity()[2];
        else                       return _zvalid;
      }

    public:

      extrapparams() :
        _use_default_method(true),
        _use_default_padvalue(true),
        _use_default_validity(true) {}

      extrapparams(extrapolation meth) :
        _method(meth),
        _use_default_method(false),
        _use_default_padvalue(true),
        _use_default_validity(true) {}

      extrapparams(extrapolation meth, T pv) :
        _method(meth),
        _padvalue(pv),
        _use_default_method(false),
        _use_default_padvalue(false),
        _use_default_validity(true) {}

      extrapparams(extrapolation meth, T pv, bool xv, bool yv, bool zv) :
        _method(meth),
        _padvalue(pv),
        _xvalid(xv),
        _yvalid(yv),
        _zvalid(zv),
        _use_default_method(false),
        _use_default_padvalue(false),
        _use_default_validity(false) {}

      extrapparams(extrapolation meth, bool xv, bool yv, bool zv) :
        _method(meth),
        _xvalid(xv),
        _yvalid(yv),
        _zvalid(zv),
        _use_default_method(false),
        _use_default_padvalue(true),
        _use_default_validity(false) {}

      extrapparams(bool xv, bool yv, bool zv) :
        _xvalid(xv),
        _yvalid(yv),
        _zvalid(zv),
        _use_default_method(true),
        _use_default_padvalue(true),
        _use_default_validity(false) {}
    };

  // initialize can be called from the ShadowVolume
  // class, hence has protected visibility
  protected:
    virtual int initialize(
      int64_t xsize, int64_t ysize, int64_t zsize,
      int64_t tsize, int64_t d5, int64_t d6, int64_t d7,
      T *d, bool d_owner, int64_t nthreads); //Master 7D

  private:

    int initialize(
      int64_t xsize, int64_t ysize, int64_t zsize,
      T *d, bool d_owner, int64_t nthreads); //3D
    int initialize(
      int64_t xsize, int64_t ysize, int64_t zsize,
      int64_t tsize, T *d, bool d_owner, int64_t nthreads); //4D

    // Internal functions
    inline T* basicptr(int64_t x, int64_t y, int64_t z) {
        return (nsfbegin() + (z*RowsY + y)*ColumnsX + x);
    }
    inline T* basicptr(int64_t x, int64_t y, int64_t z) const
      { return (Data + (z*RowsY + y)*ColumnsX + x); }

    void setdefaultproperties();
    void enforcelimits(std::vector<int>& lims) const;

    const T& extrapolate(
      int64_t x, int64_t y, int64_t z,
      const extrapparams& ep=extrapparams()) const;
    float kernelinterpolation(
      float x, float y, float z,
      const extrapparams& ep=extrapparams()) const;
    float splineinterpolate(
      float x, float y, float z,
      const extrapparams& eps=extrapparams()) const;
    float spline_interp1partial(
      float x, float y, float z,
      int dir, float *deriv,
      const extrapparams& eps=extrapparams()) const;
    float spline_interp3partial(
      float x, float y, float z,
      float *dfdx, float *dfdy, float *dfdz,
      const extrapparams& eps=extrapparams()) const;

    template <class S, class D> friend
    void copybasicproperties(const volume<S>& source, volume<D>& dest);

    template <class S> friend
    int readGeneralVolume(volume<S>& target, const std::string& filename,
		    short& dtype, const bool swap2radiological,
		    int64_t x0, int64_t y0, int64_t z0, int64_t t0, int64_t d50, int64_t d60, int64_t d70,
		    int64_t x1, int64_t y1, int64_t z1, int64_t t1, int64_t d51, int64_t d61, int64_t d71,
		    const bool readAs4D);

    template <class S> friend
      int read_volume_hdr_only(volume<S>&, const std::string&);

    void basic_swapdimensions(int dim1, int dim2, int dim3, bool keepLRorder, const bool headerOnly=false);

#ifdef EXPOSE_TREACHEROUS
  public:
#endif
    // sampling_mat should now be avoided - use newimagevox2mm_mat instead
    std::vector<NiftiIO::NiftiExtension> extensions;
    bool RadiologicalFile;
    NEWMAT::Matrix sampling_mat() const;
    void set_sform(int sform_code, const NEWMAT::Matrix& snewmat);
    void set_qform(int qform_code, const NEWMAT::Matrix& qnewmat);
    int  left_right_order() const; // see also newimagevox2mm_mat()
    void swapLRorder(const bool headerOnly=false);
    void setLRorder(int LRorder);
    void makeradiological(const bool headerOnly=false);
    void makeneurological();
    NEWMAT::Matrix sform_mat() const { return StandardSpaceCoordMat; }
    int sform_code() const { return StandardSpaceTypeCode; }
    NEWMAT::Matrix qform_mat() const { return RigidBodyCoordMat; }
    int qform_code() const { return RigidBodyTypeCode; }
    void set_data_owner(bool d_owner) { data_owner=d_owner; }
    public:
    typedef T* nonsafe_fast_iterator;
    inline nonsafe_fast_iterator nsfbegin()
      {       this->invalidateSplines(); return Data; }
    inline nonsafe_fast_iterator nsfend()
    { return DataEnd; }
   public:
    // CONSTRUCTORS AND DESTRUCTORS (including copy, = and reinitialize)
    volume(Utilities::NoOfThreads nt = Utilities::NoOfThreads());
    volume(const volume<T>& source, const bool copyData);
    volume(const volume<T>& source, const constructionMode mode=CLONE);
    volume(int64_t xsize, int64_t ysize, int64_t zsize, Utilities::NoOfThreads nt = Utilities::NoOfThreads());
    volume(int64_t xsize, int64_t ysize, int64_t zsize, int64_t tsize, Utilities::NoOfThreads nt = Utilities::NoOfThreads());
    volume(int64_t xsize, int64_t ysize, int64_t zsize, int64_t tsize, int64_t d5, int64_t d6, int64_t d7, Utilities::NoOfThreads nt = Utilities::NoOfThreads());
    virtual ~volume();
    void destroy();
    const volume<T>& operator=(const volume<T>& that) { return this->equals(that); }

    virtual const volume<T>& equals(const volume<T>& );

    void reinitialize(const volume<T>& source, const constructionMode mode=CLONE);
    int reinitialize(int64_t xsize, int64_t ysize, int64_t zsize, Utilities::NoOfThreads nt = Utilities::NoOfThreads());
    int reinitialize(int64_t xsize, int64_t ysize, int64_t zsize, int64_t tsize,T *d=nullptr, bool d_owner=true, Utilities::NoOfThreads nt = Utilities::NoOfThreads());
    int reinitialize(int64_t xsize, int64_t ysize, int64_t zsize, int64_t tsize, int64_t d5, int64_t d6, int64_t d7, Utilities::NoOfThreads nt = Utilities::NoOfThreads());
    void copyproperties(const volume<T>& source);
    int copydata(const volume<T>& source, const bool isOwner=true);

    // BASIC PROPERTIES
    inline int64_t xsize() const { return ColumnsX; }
    inline int64_t ysize() const { return RowsY; }
    inline int64_t zsize() const { return SlicesZ; }
    inline int64_t tsize() const { return dim4; }
    inline int64_t size1() const { return xsize(); }
    inline int64_t size2() const { return ysize(); }
    inline int64_t size3() const { return zsize(); }
    inline int64_t size4() const { return tsize(); }
    inline int64_t size5() const { return dim5; }
    inline int64_t size6() const { return dim6; }
    inline int64_t size7() const { return dim7; }
    int64_t size(int n) const;

    int dimensionality() const;
    void throwsIfNot3D() const { if (this->dimensionality()>3) imthrow("3D only method called by higher-dimensional volume.",75); }

    inline int64_t maxx() const { return xsize()-1; }
    inline int64_t maxy() const { return ysize()-1; }
    inline int64_t maxz() const { return zsize()-1; }
    inline int64_t maxt() const { return tsize()-1; }
    inline int64_t minx() const { return 0; }
    inline int64_t miny() const { return 0; }
    inline int64_t minz() const { return 0; }
    inline int64_t mint() const { return 0; }

    inline float xdim() const { return Xdim; }
    inline float ydim() const { return Ydim; }
    inline float zdim() const { return Zdim; }
    inline float tdim() const { return p_TR; }
    inline float TR() const { return p_TR; }
    inline float pixdim1() const { return Xdim; }
    inline float pixdim2() const { return Ydim; }
    inline float pixdim3() const { return Zdim; }
    inline float pixdim4() const { return p_TR; }
    inline float pixdim5() const { return pxdim5; }
    inline float pixdim6() const { return pxdim6; }
    inline float pixdim7() const { return pxdim7; }

    inline float getDisplayMaximum() const { return displayMaximum; }
    inline float getDisplayMinimum() const { return displayMinimum; }
    inline std::string getAuxFile() const { return std::string(auxFile); }

    inline bool hasSplines() const { return(splint.Valid()); }

    inline int64_t nthreads() const { return(nThreads); }

    void setxdim(float x) { Xdim = fabs(x); }
    void setydim(float y) { Ydim = fabs(y); }
    void setzdim(float z) { Zdim = fabs(z); }
    void settdim(float tr) { p_TR = fabs(tr); }
    void setd5dim(float d5) { pxdim5 = d5; }
    void setd6dim(float d6) { pxdim6 = d6; }
    void setd7dim(float d7) { pxdim7 = d7; }
    void setTR(float tr)   { settdim(tr); }
    void setdims(float x, float y, float z)
      { setxdim(x); setydim(y); setzdim(z); }
    void setdims(float x, float y, float z, float tr)
      { setxdim(x); setydim(y); setzdim(z); settdim(tr); }
    void setdims(float x, float y, float z, float tr, float d5, float d6, float d7)
      { setxdim(x); setydim(y); setzdim(z); settdim(tr); setd5dim(d5); setd6dim(d6); setd7dim(d7); }
    void setDisplayMaximumMinimum(float maximum, float minimum) {  displayMaximum=maximum; displayMinimum=minimum; }
    void setDisplayMaximum(float maximum) { setDisplayMaximumMinimum(maximum,displayMinimum); }
    void setDisplayMinimum(float minimum) { setDisplayMaximumMinimum(displayMaximum,minimum); }
    void setAuxFile(std::string fileName) { strncpy(auxFile,fileName.c_str(),24); }
    int64_t nvoxels() const { return no_voxels; }
    int64_t ntimepoints() const { return tsize(); }
    int64_t totalElements() const { return nElements; }

    std::vector<int64_t> ptrToCoord(size_t offset) const;

    static const T smallestPositiveT;
    // MATRIX <-> VOLUME CONVERSIONS
    NEWMAT::ReturnMatrix vec(const volume<T>& mask) const;
    NEWMAT::ReturnMatrix vec() const;
    void insert_vec(const NEWMAT::ColumnVector& pvec, const volume<T>& pmask);
    void insert_vec(const NEWMAT::ColumnVector& pvec);
    std::vector<int64_t> labelToCoord(const int64_t label) const;
    NEWMAT::ReturnMatrix matrix(const volume<T>& mask, bool using_mask=true) const;
    NEWMAT::ReturnMatrix matrix(const volume<T>& mask, std::vector<int64_t>& voxelLabels) const;
    NEWMAT::ReturnMatrix matrix() const;
    void setmatrix(const NEWMAT::Matrix& newmatrix, const volume<T>& mask,
		   const T pad=0, bool using_mask=true);
    void setmatrix(const NEWMAT::Matrix& newmatrix);
    volume<int> vol2matrixkey(const volume<T>& mask); //returns a volume with numbers in relating to matrix colnumbers
    NEWMAT::ReturnMatrix matrix2volkey(volume<T>& mask);

    // SECONDARY PROPERTIES
    // maps *NEWIMAGE* voxel coordinates to mm (consistent with FSLView mm)
    // NB: do not try to determine left-right order from this matrix
    // sampling_mat should now be avoided - use newimagevox2mm_mat instead
    NEWMAT::Matrix newimagevox2mm_mat() const;
    NEWMAT::Matrix niftivox2newimagevox_mat() const;

    int intent_code() const { return IntentCode; }
    float intent_param(int n) const;
    void set_intent(int intent_code, float p1, float p2, float p3);

    inline double maskThreshold() const { return maskDelimiter; }
    T min() const;
    T max() const;
    double sum() const;
    double sumsquares() const;
    double mean() const { return sum()/(MISCMATHS::Max(1.0,(double)totalElements()));}
    double variance() const { double n(totalElements());
		return (n/(n-1))*(sumsquares()/n - mean()*mean()); }
    double stddev() const { return sqrt(variance()); }
    T robustmin() const;
    T robustmax() const;
    std::vector<T> robustlimits(const volume<T>& mask) const;
    NEWMAT::ColumnVector principleaxis(int n) const;
    NEWMAT::Matrix principleaxes_mat() const;
    T percentile(float pvalue) const;  // argument in range [0.0 , 1.0]
    NEWMAT::ColumnVector histogram(int nbins) const;
    NEWMAT::ColumnVector histogram(int nbins, T minval, T maxval) const;
    NEWMAT::ColumnVector cog(const std::string& coordtype="voxel", bool abscog=false) const;

    // SECONDARY PROPERTIES (using mask)
    T min(const volume<T>& mask) const;
    T max(const volume<T>& mask) const;
    T min(const volume<T>& mask, std::vector<int64_t>& coords) const;
    T max(const volume<T>& mask, std::vector<int64_t>& coords) const;
    double sum(const volume<T>& mask) const;
    double sumsquares(const volume<T>& mask) const;
    double mean(const volume<T>& mask) const;
    double variance(const volume<T>& mask) const;
    double stddev(const volume<T>& mask) const { return sqrt(variance(mask)); }
    T robustmin(const volume<T>& mask) const;
    T robustmax(const volume<T>& mask) const;
    T percentile(float pvalue, const volume<T>& mask) const;  // arg in [0,1]
    NEWMAT::ColumnVector histogram(int nbins, const volume<T>& mask) const;
    NEWMAT::ColumnVector histogram(int nbins, T minval, T maxval, const volume<T>& mask)
      const;

    // VOXEL COORDINATE ITERATORS
    // see voxeliterator.h for details
    voxelIterator vbegin() const {return voxelIterator(xsize(), ysize(), zsize(), 0); };
    voxelIterator vend()   const {return voxelIterator(xsize(), ysize(), zsize(), nvoxels()); };

    // DATA ACCESS FUNCTIONS (iterators)

    typedef const T* fast_const_iterator;

    inline fast_const_iterator fbegin() const { return Data; }
    inline fast_const_iterator fend() const { return DataEnd; }
    inline fast_const_iterator fbegin(const int64_t offset) const { return Data + offset*nvoxels(); }
    inline fast_const_iterator fend(const int64_t offset) const { return Data + (offset+1)*nvoxels(); }

    // BASIC DATA ACCESS FUNCTIONS
    template<class A, class B, class C>
    inline bool in_bounds( A x, B y, C z) const {
      if ( std::is_integral<A>::value && std::is_integral<B>::value && std::is_integral<C>::value )
	return ( (x>=0) && (y>=0) && (z>=0) && (x<ColumnsX) && (y<RowsY) && (z<SlicesZ) );
      return ( (floor(x)>=0) && (floor(y)>=0) && (floor(z)>=0) && (ceil(x)<ColumnsX) && (ceil(y)<RowsY) && (ceil(z)<SlicesZ) );
    }

    inline bool in_bounds(int64_t t) const
      { return ( (t>=0) && (t<this->tsize()) ); }

    template<class A, class B, class C, class D>
    inline bool in_bounds(A x, B y, C z, D t) const
      { return ( this->in_bounds(t) && this->in_bounds(x,y,z) ); }

    inline bool in_bounds(int64_t x, int64_t y, int64_t z, int64_t t, int64_t d5, int64_t d6=0, int64_t d7=0) const {
      return (this->in_bounds(t) && this->in_bounds(x,y,z) && (d5>=0) && (d6>=0) && (d7>=0) && (d5<dim5) && (d6<dim6) && (d7<dim7));}

    bool in_extraslice_bounds(float x, float y, float z) const
    {
      int64_t ix=((int64_t) floor(x));
      int64_t iy=((int64_t) floor(y));
      int64_t iz=((int64_t) floor(z));
      return((ix>=-1) && (iy>=-1) && (iz>=-1) && (ix<ColumnsX) && (iy<RowsY) && (iz<SlicesZ));
    }
    inline bool valid(int64_t x, int64_t y, int64_t z,
                      const extrapparams& ep=extrapparams()) const
    {
      bool xvalid = ep.xvalid(*this);
      bool yvalid = ep.yvalid(*this);
      bool zvalid = ep.zvalid(*this);
      return((xvalid || (x>=0 && x<ColumnsX)) &&
             (yvalid || (y>=0 && y<RowsY)) &&
             (zvalid || (z>=0 && z<SlicesZ)));
    }
    bool valid(float x, float y, float z, double tol=1e-8,
               const extrapparams& ep=extrapparams()) const
    {
      bool xvalid = ep.xvalid(*this);
      bool yvalid = ep.yvalid(*this);
      bool zvalid = ep.zvalid(*this);
      return((xvalid || (x+tol >= 0.0 && x <= ColumnsX-1+tol)) &&
             (yvalid || (y+tol >= 0.0 && y <= RowsY-1+tol)) &&
             (zvalid || (z+tol >= 0.0 && z <= SlicesZ-1+tol)));
    }

    inline T& operator()(int64_t x, int64_t y, int64_t z,
                         const extrapparams& ep=extrapparams()) {
      this->invalidateSplines();
      if (in_bounds(x,y,z)) return *(basicptr(x,y,z));
      else                  return const_cast<T& > (extrapolate(x,y,z,ep));
    }

    inline const T& operator()(int64_t x, int64_t y, int64_t z,
                               const extrapparams& ep=extrapparams()) const {
      if (in_bounds(x,y,z)) return *(basicptr(x,y,z));
      else                  return extrapolate(x,y,z,ep);
    }

    inline T& operator()(int64_t x, int64_t y, int64_t z, int64_t t,
                         const extrapparams& ep=extrapparams()) {
      this->invalidateSplines();
      if (!in_bounds(t)) imthrow("Out of Bounds (time index)",5);
      else if (!in_bounds(x,y,z)) return const_cast<T& > (operator[](t).extrapolate(x,y,z,ep));
      return *(Data + ((zsize()*t + z)*ysize() + y)*xsize() + x);
    }

    inline const T& operator()(int64_t x, int64_t y, int64_t z, int64_t t,
                               const extrapparams& ep=extrapparams()) const
    { if (!in_bounds(t)) imthrow("Out of Bounds (time index)",5);
      else if (!in_bounds(x,y,z)) return const_cast<T& > (operator[](t).extrapolate(x,y,z,ep));
      return *(Data + ((zsize()*t + z)*ysize() + y)*xsize() + x);
    }

    inline T& operator()(int64_t x, int64_t y, int64_t z, int64_t t, int64_t d5, int64_t d6=0, int64_t d7=0) {
      this->invalidateSplines();
      if (!in_bounds(x,y,z,t,d5,d6,d7)) imthrow("Out of Bounds (7D index)",5);
      return *(Data + ((((((size6()*d7+d6)*size5()+d5)*tsize()+t)*zsize()*t+z)*ysize()+y)*xsize()+ x));
    }

    inline const T& operator()(int64_t x, int64_t y, int64_t z, int64_t t, int64_t d5, int64_t d6=0, int64_t d7=0) const
    { if (!in_bounds(x,y,z,t,d5,d6,d7)) imthrow("Out of Bounds (7D index)",5);
        return *(Data + ((((((size6()*d7+d6)*size5()+d5)*tsize()+t)*zsize()*t+z)*ysize()+y)*xsize()+ x)); }

    inline T splineCoef(int64_t x, int64_t y, int64_t z) const
    {
      if (!in_bounds(x,y,z)) imthrow("Out of Bounds",5);
      return(splint.Coef(static_cast<unsigned int>(x),static_cast<unsigned int>(y),static_cast<unsigned int>(z)));
    }

    float interpolate(
      float x, float y, float z,
      const extrapparams& ep=extrapparams()) const;
    float interp1partial(
      // Input
      float x, float y, float z,  // Co-ordinates to get value for
      int dir,                    // Direction for partial, 0->x, 1->y, 2->z
      // Output
      float *pderiv,              // Derivative returned here
      const extrapparams& eps=extrapparams()) const;
    float interp3partial(
      // Input
      float x, float y, float z,             // Co-ordinate to get value for
      // Output
      float *dfdx, float *dfdy, float *dfdz, // Partials
      const extrapparams& eps=extrapparams()) const;

    inline T& value(int64_t x, int64_t y, int64_t z)
      { return *(basicptr(x,y,z)); }
    inline const T& value(int64_t x, int64_t y, int64_t z) const
      { return *(basicptr(x,y,z)); }

    inline T& value(int64_t x, int64_t y, int64_t z, int64_t t)
    { return *(Data + ((zsize()*t + z)*ysize() + y)*xsize() + x); }
    inline const T& value(int64_t x, int64_t y, int64_t z, int64_t t) const
    { return *(Data + ((zsize()*t + z)*ysize() + y)*xsize() + x); }
    inline T& value(int64_t x, int64_t y, int64_t z, int64_t t, int64_t d5, int64_t d6=0, int64_t d7=0)
    { return *(Data + ((((((size6()*d7+d6)*size5()+d5)*tsize()+t)*zsize()+z)*ysize()+y)*xsize()+ x)); }
    inline const T& value(int64_t x, int64_t y, int64_t z, int64_t t, int64_t d5, int64_t d6=0, int64_t d7=0) const
    { return *(Data + ((((((size6()*d7+d6)*size5()+d5)*tsize()+t)*zsize()+z)*ysize()+y)*xsize()+ x)); }

    float interpolatevalue(float x, float y, float z) const;
    NEWMAT::ColumnVector ExtractRow(int j, int k) const;
    NEWMAT::ColumnVector ExtractColumn(int i, int k) const;
    void SetRow(int j, int k, const NEWMAT::ColumnVector& row);
    void SetColumn(int j, int k, const NEWMAT::ColumnVector& col);

    // ROI and higher dimensional accesses
    void addvolume(const volume<T>& source);
    void deletevolume(int64_t t);
    void clear();  // deletes all volumes
    NEWMAT::ReturnMatrix voxelts(int64_t x, int64_t y, int64_t z) const;
    void setvoxelts(const NEWMAT::ColumnVector& ts, int64_t x, int64_t y, int64_t z);
    void copySubVolume(const int64_t t, volume<T>& newVolume);
    ShadowVolume<T>& operator[](const int64_t t);
    const ShadowVolume<T>& operator[](const int64_t t) const;
    const volume<T> constSubVolume(const int64_t t) const;
    void replaceSubVolume(const int64_t t, const volume<T>& newVolume);
    void copyROI(const volume<T>& source,const int64_t minx, const int64_t miny, const int64_t minz, const int64_t mint, const int64_t maxx, const int64_t maxy, const int64_t maxz, const int64_t maxt);
    volume<T> ROI(const int64_t minx, const int64_t miny, const int64_t minz, const int64_t mint, const int64_t maxx, const int64_t maxy, const int64_t maxz, const int64_t maxt) const;

    // SECONDARY FUNCTIONS
    virtual void setextrapolationmethod(extrapolation extrapmethod) { p_extrapmethod = extrapmethod; }
    virtual extrapolation getextrapolationmethod() const { return(p_extrapmethod); }
    virtual void setpadvalue(T padval) { padvalue = padval; }
    virtual T getpadvalue() const { return padvalue; }
    T backgroundval() const;
    virtual void setinterpolationmethod(interpolation interpmethod);
    virtual interpolation getinterpolationmethod() const { return p_interpmethod; }
    virtual void setsplineorder(int order);
    inline void invalidateSplines() {
      splineuptodate = false;
      if (p_interpmethod == spline) {
        std::lock_guard<std::mutex> lg(sv_cache_mutex);
        sv_cache.clear();
      }
    }
    virtual int getsplineorder() const { return(splineorder); }
    void forcesplinecoefcalculation() const;

    virtual void setextrapolationvalidity(bool xv, bool yv, bool zv) { ep_valid = {xv, yv, zv}; }

    virtual std::vector<bool> getextrapolationvalidity() const {
      std::vector<bool> vv{std::get<0>(ep_valid),
                           std::get<1>(ep_valid),
                           std::get<2>(ep_valid)};
      return(vv);
    }
    virtual void definekernelinterpolation(
      const NEWMAT::ColumnVector& kx,
      const NEWMAT::ColumnVector& ky,
      const NEWMAT::ColumnVector& kz,
      int wx, int wy, int wz);  // full-width
    virtual void definesincinterpolation(
      const std::string& sincwindowtype,
      int w, int nstore=1201);  // full-width
    virtual void definesincinterpolation(
      const std::string& sincwindowtype,
      int wx, int wy, int wz, int nstore=1201); // full-width

    inline void getneighbours(int64_t x, int64_t y, int64_t z,
			      T &v000, T &v001, T &v010,
			      T &v011, T &v100, T &v101,
			      T &v110, T &v111) const;
    inline void getneighbours(int64_t x, int64_t y, int64_t z,
			      T &v000, T &v010,
			      T &v100, T &v110) const;

    // ARITHMETIC FUNCTIONS
    T operator=(T val);
    const volume<T>& operator+=(T val);
    const volume<T>& operator-=(T val);
    const volume<T>& operator*=(T val);
    const volume<T>& operator/=(T val);
    const volume<T>& operator+=(const volume<T>& source);
    const volume<T>& operator-=(const volume<T>& source);
    const volume<T>& operator*=(const volume<T>& source);
    const volume<T>& operator/=(const volume<T>& source);

    volume<T> operator+(T num) const;
    volume<T> operator-(T num) const;
    volume<T> operator*(T num) const;
    volume<T> operator/(T num) const;
    volume<T> operator+(const volume<T>& vol2) const;
    volume<T> operator-(const volume<T>& vol2) const;
    volume<T> operator*(const volume<T>& vol2) const;
    volume<T> operator/(const volume<T>& vol2) const;

    template <class S>
    friend volume<S> operator+(S num, const volume<S>& vol);
    template <class S>
    friend volume<S> operator-(S num, const volume<S>& vol);
    template <class S>
    friend volume<S> operator*(S num, const volume<S>& vol);
    template <class S>
    friend volume<S> operator/(S num, const volume<S>& vol);
    template <class S>
    friend volume<S> operator-(const volume<S>& vol);

    // Comparisons. These are used for "spatial" purposes
    // so that if data is identical and all the "spatial
    // fields of the header are identical then the volumes
    // are considered identical.
    template <class S>
    friend bool operator==(const volume<S>& v1, const volume<S>& v2);
    template <class S>
    friend bool operator!=(const volume<S>& v1, const volume<S>& v2); // { return(!(v1==v2)); }

    // GENERAL MANIPULATION

    //binarise: Values within range are set to 1, values outside to 0
    //if invertRange is true this behaviour is reversed
    void binarise(const T lower = smallestPositiveT,
                  const T upper = std::numeric_limits<T>::max(),
                  const threshtype tt = inclusive,
                  const bool invertRange = false);

    //threshold: Values outside of the range will be zeroed
    //if invertRange is true values inside the range will be zeroed
    void threshold( const T lower,
                    const T upper = std::numeric_limits<T>::max(),
                    const threshtype tt = inclusive,
                    const bool invertRange = false);

    // valid entries for dims are +/- 1, 2, 3 (and for newx, etc they are x, -x, y, -y, z, -z)
    void swapdimensions(int dim1, int dim2, int dim3, bool keepLRorder=false);
    void swapdimensions(const std::string& newx, const std::string& newy, const std::string& newz, const bool keepLRorder=false);
    NEWMAT::Matrix swapmat(int dim1, int dim2, int dim3) const;
    NEWMAT::Matrix swapmat(const std::string& newx, const std::string& newy, const std::string& newz) const;


    // CONVERSION FUNCTIONS
    template <class S, class D> friend
      void copyconvert(const volume<S>& source, volume<D>& dest, const bool copyData);

    // Deprecated functions which change the state of a const volume
    [[deprecated("Calling setextrapolationmethod on a const volume is not thread safe!")]]
    void setextrapolationmethod(extrapolation extrapmethod) const {
      const_cast<volume<T>*>(this)->setextrapolationmethod(extrapmethod);
    }
    [[deprecated("Calling setinterpolationmethod on a const volume is not thread safe!")]]
    void setinterpolationmethod(interpolation interpmethod) const {
      const_cast<volume<T>*>(this)->setinterpolationmethod(interpmethod);
    }
    [[deprecated("Calling setextrapolationvalidity on a const volume is not thread safe!")]]
    void setextrapolationvalidity(bool xv, bool yv, bool zv) const {
      const_cast<volume<T>*>(this)->setextrapolationvalidity(xv, yv, zv);
    }
    [[deprecated("Calling setsplineorder on a const volume is not thread safe!")]]
    void setsplineorder(int order) const {
      const_cast<volume<T>*>(this)->setsplineorder(order);
    }
  };

template <typename T>
const T volume<T>::smallestPositiveT = std::is_integral<T>::value ? 1 : std::numeric_limits<T>::min();

template<class T>
class ShadowVolume : public volume<T> {

private:
  bool             _assigned;
  int64_t          _t;
  const volume<T>* _owner;

  const volume<T>& owner() const;

protected:
  virtual int initialize(
    int64_t xsize, int64_t ysize, int64_t zsize,
    int64_t tsize, int64_t d5, int64_t d6, int64_t d7,
    T *d, bool d_owner, int64_t nt);

public:

  using volume<T>::operator=;

  // Calling interp/extrap setting getters
  // on a ShadowVolume will delegate to the
  // owning volume instance
  virtual interpolation getinterpolationmethod() const;
  virtual extrapolation getextrapolationmethod() const;
  virtual T getpadvalue() const;
  virtual std::vector<bool> getextrapolationvalidity() const;
  virtual int getsplineorder() const;

  // Calling interp/extrap setting getters
  // on a ShadowVolume will result in an error
  [[deprecated("Calling setinterpolationmethod on a ShadowVolume will result in runtime errors!")]]
  virtual void setinterpolationmethod(interpolation interp);
  [[deprecated("Calling setextrapolationmethod on a ShadowVolume will result in runtime errors!")]]
  virtual void setextrapolationmethod(extrapolation extrapmethod) const;
  [[deprecated("Calling setextrapolationmethod on a ShadowVolume will result in runtime errors!")]]
  virtual void setextrapolationmethod(extrapolation extrapmethod);
  [[deprecated("Calling setpadvalue on a ShadowVolume will result in runtime errors!")]]
  virtual void setpadvalue(T value);
  [[deprecated("Calling setextrapolationvalidity on a ShadowVolume will result in runtime errors!")]]
  virtual void setextrapolationvalidity(bool xv, bool yv, bool zv);
  [[deprecated("Calling definekernelinterpolation on a ShadowVolume will result in runtime errors!")]]
  virtual void definekernelinterpolation(const NEWMAT::ColumnVector& kx, const NEWMAT::ColumnVector& ky, const NEWMAT::ColumnVector& kz, int wx, int wy, int wz);
  [[deprecated("Calling definesincinterpolation on a ShadowVolume will result in runtime errors!")]]
  virtual void definesincinterpolation(const std::string& sincwindowtype, int w, int nstore=1201);
  [[deprecated("Calling definesincinterpolation on a ShadowVolume will result in runtime errors!")]]
  virtual void definesincinterpolation(const std::string& sincwindowtype, int wx, int wy, int wz, int nstore=1201);
  [[deprecated("Calling setsplineorder on a ShadowVolume will result in runtime errors!")]]
  virtual void setsplineorder(int order);

  const volume<T>& equals(const volume<T>& source);

  ShadowVolume();
  ShadowVolume(const ShadowVolume<T>& source);
  ShadowVolume(const volume<T>& source, int64_t t=0);
};

template<class T>
class baseIterator {
protected:
  T* ptr;
  T* const ptrBegin;
  T* const ptrEnd;
  const volume<T>* const source;
  baseIterator() : ptr(nullptr), ptrBegin(nullptr),  ptrEnd(nullptr), source(nullptr) { };
public:
  baseIterator(const volume<T>& input) : ptr((T*)input.fbegin()), ptrBegin(ptr), ptrEnd((T*)input.fend()), source(&input) {}
  baseIterator& operator++() { ++ptr; return *this; }
  baseIterator operator++(int) {baseIterator temp(*this); operator++(); return temp;}
  inline T& operator*() { return *ptr;}
  inline bool isValid() { return ptr != ptrEnd; }
  inline size_t offset() { return ptr - ptrBegin; }
};

template<class T>
  class cyclicIterator : public baseIterator<T> {
 private:
  cyclicIterator();
 public:
 cyclicIterator(const volume<T>& input) : baseIterator<T>(input) {};
  cyclicIterator& operator++() { if ( ++baseIterator<T>::ptr ==  baseIterator<T>::ptrEnd ) baseIterator<T>::ptr=baseIterator<T>::ptrBegin;  return *this; }
  cyclicIterator operator++(int) {cyclicIterator temp(*this); operator++(); return temp;}
 };

template<class T,class U>
class maskedIterator : public baseIterator<T> {
private:
  const U* mPtr;
  const U* firstM;
  const U* const mPtrEnd;
  T* firstP;
  const volume<U>* const mask;
  bool usingMask;
  maskedIterator() : mPtr(nullptr) {};
public:
  maskedIterator(const volume<T>& data,const volume<U>& dataMask,const std::string& exceptionSource="");
  maskedIterator& operator++();
  maskedIterator operator++(int) {maskedIterator temp(*this); operator++(); return temp;}
  inline bool operator!=(const T* oPtr) { return baseIterator<T>::ptr != oPtr;}
  maskedIterator& begin();
  const T* end() { return baseIterator<T>::ptrEnd; }
 };

template<class T, class U>
maskedIterator<T,U>& maskedIterator<T,U>::begin() {
  baseIterator<T>::ptr=firstP;
  mPtr=firstM;
  return *this;
}

template<class T, class U>
maskedIterator<T,U>::maskedIterator(const volume<T>& data,const volume<U>& dataMask, const std::string& exceptionSource) : baseIterator<T>(data), mPtrEnd(dataMask.fend()), mask(&dataMask)
{
  mPtr=mask->fbegin(); //Should be safe even for NULL mask
  usingMask=( mask->totalElements() != 0 );

  if ( usingMask && !samesize(data,dataMask,SUBSET) )
    throw std::runtime_error(exceptionSource+"maskedIterator: mask and volume must be the same size");
  if ( usingMask && *mPtr <= mask->maskThreshold() )   //forward data pointer to first valid mask voxel
    operator++();
  firstP=baseIterator<T>::ptr;
  firstM=mPtr;
}


template<class T, class U>
maskedIterator<T,U>& maskedIterator<T,U>::operator++() {
  //Increment the data pointer if currently valid
  while ( this->isValid() && ++baseIterator<T>::ptr && usingMask ) {
    //Seek/Cycle to the next valid mask voxel
    if ( ++mPtr == mPtrEnd )
      mPtr=mask->fbegin();
    if ( *mPtr > mask->maskThreshold() ) //Is the candidate masked?
      break;
  }
  return *this;
}

template<class T>
  class safeLazyIterator : public baseIterator<T> { //Safe: cannot dereference if invalid, Lazy: invalidate source lazy flag on dereference
public:
  bool isValid() { return  baseIterator<T>::ptr != nullptr &&  baseIterator<T>::ptr != baseIterator<T>::source->fend(); }
  safeLazyIterator(const volume<T>& input) : baseIterator<T>(input) {}
  safeLazyIterator& operator++() { ++baseIterator<T>::ptr ; return *this; }
  safeLazyIterator operator++(int) {safeLazyIterator temp(*this); operator++(); return temp;}
  T& operator*() {
    this->invalidateSplines();
    if(isValid()) return *baseIterator<T>::ptr;
    else throw std::runtime_error("Attempted to dereference an invalid safeLazyIterator");
  }
 };

template<class T>
  class lazyIterator : public baseIterator<T> {
public:
  lazyIterator(const volume<T>& input) : baseIterator<T>(input) {}
  lazyIterator& operator++() { ++baseIterator<T>::ptr ; return *this; }
  lazyIterator operator++(int) {lazyIterator temp(*this); operator++(); return temp;}
  T& operator*() {
    this->invalidateSplines();
    return *baseIterator<T>::ptr;
  } //Need to invalidate source
};



  // HELPER FUNCTIONS
  template <class S, class D>
  void convertbuffer(const S* source, D* dest, size_t len, float slope=1.0, float intercept=0.0);

  template <class S1, class S2>
  bool samesize(const volume<S1>& vol1, const volume<S2>& vol2);
  template <class S1, class S2>
  bool samesize(const volume<S1>& vol1, const volume<S2>& vol2, bool checkdim);
  template <class S1, class S2>
  bool samesize(const volume<S1>& vol1, const volume<S2>& vol2, int ndims, bool checkdim=false);

  template <class S1, class S2>
  bool samedim(const volume<S1>& vol1, const volume<S2>& vol2, int ndims);

//////////////////////////////////////////////////////////////////////
///////////////////////// INLINE DEFINITIONS /////////////////////////
//////////////////////////////////////////////////////////////////////

  template <class T>
  inline void volume<T>::getneighbours(int64_t x, int64_t y, int64_t z,
					    T &v000, T &v001, T &v010,
					    T &v011, T &v100, T &v101,
					    T &v110, T &v111) const {
    T *ptr = basicptr(x,y,z);
    v000 = *ptr;
    ptr++;
    v100 = *ptr;
    ptr+= ColumnsX;
    v110 = *ptr;
    ptr--;
    v010 = *ptr;
    ptr += RowsY * ColumnsX;
    v011 = *ptr;
    ptr++;
    v111 = *ptr;
    ptr-= ColumnsX;
    v101 = *ptr;
    ptr--;
    v001 = *ptr;
  }


  template <class T>
  inline void volume<T>::getneighbours(int64_t x, int64_t y, int64_t z,
					    T &v000, T &v010,
					    T &v100, T &v110) const {
    T *ptr = basicptr(x,y,z);
    v000 = *ptr;
    ptr++;
    v100 = *ptr;
    ptr+= ColumnsX;
    v110 = *ptr;
    ptr--;
    v010 = *ptr;
  }





////////////////////////////////////////////////////////////////////////
/////////////////////////// HELPER FUNCTIONS ///////////////////////////
////////////////////////////////////////////////////////////////////////

  template <class T>
  int64_t no_mask_voxels(const volume<T>&  mask)
  {
     int64_t n(0);
     for(maskedIterator<T,T> it(mask,mask);it.isValid();++it)
       n++;
     return n;
  }


  template <class S, class D>
  void convertbuffer(const S* source, D* dest, size_t len, float slope, float intercept)
  {
    if ( slope == 1.0 && intercept == 0.0)
     for (const S* sptr=source; sptr<(source+len); sptr++)
      *(dest++) = (D) *sptr;
    else
     for (const S* sptr=source; sptr<(source+len); sptr++)
      *(dest++) = (D) ((*sptr) * slope + intercept);
  }

  template <class S1, class S2>
  bool samesize(const volume<S1>& vol1, const volume<S2>& vol2, int ndims, bool checkdim)
  {
    int n=ndims;

    if (ndims==SUBSET) { n = MISCMATHS::Min(vol1.dimensionality(),vol2.dimensionality()); }
    bool same(vol1.xsize()==vol2.xsize());
    if ( ndims == SUBSET )
      same &= ( vol1.dimensionality() >= vol2.dimensionality() );
    if (n>=2) same = same && ( vol1.ysize()==vol2.ysize());
    if (n>=3) same = same && ( vol1.zsize()==vol2.zsize());
    if (n>=4) same = same && ( vol1.tsize()==vol2.tsize());
    if (n>=5) same = same && ( vol1.size5()==vol2.size5());
    if (n>=6) same = same && ( vol1.size6()==vol2.size6());
    if (n>=7) same = same && ( vol1.size7()==vol2.size7());
    if (checkdim)
      same = same && samedim(vol1,vol2,n);
    return(same);
  }

  template <class S1, class S2>
  bool samesize(const volume<S1>& vol1, const volume<S2>& vol2, bool checkdim)
  {
    return samesize(vol1,vol2,7,checkdim);
  }

  template <class S1, class S2>
  bool samesize(const volume<S1>& vol1, const volume<S2>& vol2)
  {
    return samesize(vol1,vol2,7,false);
  }

  //The definition of samedim is if the difference in pixdim[i] is no greater than 0.01% of mean pixdim[i]
  //fabs( dim_image1 - dim_image2 ) / ( ( dim_image1 + dim_image2 ) / 2) <= 0.0001
  //The test is refactored to cater for the edge case of both pixdims being 0
  template <class S1, class S2>
  bool samedim(const volume<S1>& v1, const volume<S2>& v2, int ndims)
  {
    bool same=true;
    same = ( std::fabs( v1.xdim()-v2.xdim() ) <= 5e-5 * ( v1.xdim()+v2.xdim() ) );
    if (ndims>=2)
      same = same && ( std::fabs( v1.ydim()-v2.ydim() ) <= 5e-5 * ( v1.ydim()+v2.ydim() ) );
    if (ndims>=3)
      same = same && ( std::fabs( v1.zdim()-v2.zdim() ) <= 5e-5 * ( v1.zdim()+v2.zdim() ) );
    if (ndims>=4)
      same = same && ( std::fabs( v1.tdim()-v2.tdim() ) <= 5e-5 * ( v1.tdim()+v2.tdim() ) );
    if (ndims>=5)
      same = same && ( std::fabs( v1.pixdim5()-v2.pixdim5() ) <= 5e-5 * ( v1.pixdim5()+v2.pixdim5() ) );
    if (ndims>=6)
      same = same && ( std::fabs( v1.pixdim6()-v2.pixdim6() ) <= 5e-5 * ( v1.pixdim6()+v2.pixdim6() ) );
    if (ndims>=7)
      same = same && ( std::fabs( v1.pixdim7()-v2.pixdim7() ) <= 5e-5 * ( v1.pixdim7()+v2.pixdim7() ) );
    return same;
  }



////////////////////////////////////////////////////////////////////////
/////////////////////////// FRIEND FUNCTIONS ///////////////////////////
////////////////////////////////////////////////////////////////////////

  template <class S, class D>
  void copybasicproperties(const volume<S>& source, volume<D>& dest)
  {
    // set up properties
    dest.Xdim = source.Xdim;
    dest.Ydim = source.Ydim;
    dest.Zdim = source.Zdim;
    dest.p_TR = source.p_TR;
    dest.pxdim5 = source.pxdim5;
    dest.pxdim6 = source.pxdim6;
    dest.pxdim7 = source.pxdim7;

    dest.originalSizes = source.originalSizes;

    dest.StandardSpaceCoordMat = source.StandardSpaceCoordMat;
    dest.RigidBodyCoordMat = source.RigidBodyCoordMat;
    dest.StandardSpaceTypeCode = source.StandardSpaceTypeCode;
    dest.RigidBodyTypeCode = source.RigidBodyTypeCode;

    dest.RadiologicalFile = source.RadiologicalFile;

    dest.IntentCode = source.IntentCode;
    dest.IntentParam1 = source.IntentParam1;
    dest.IntentParam2 = source.IntentParam2;
    dest.IntentParam3 = source.IntentParam3;

    dest.SliceOrderingCode = source.SliceOrderingCode;

    dest.interpkernel = source.interpkernel;
    dest.p_interpmethod = source.p_interpmethod;
    dest.p_extrapmethod = source.p_extrapmethod;

    dest.padvalue = (D) source.padvalue;
    dest.splineorder = source.splineorder;
    dest.ep_valid = source.ep_valid;

    dest.displayMaximum=source.displayMaximum;
    dest.displayMinimum=source.displayMinimum;
    dest.setAuxFile(source.getAuxFile());

    dest.extensions=source.extensions;
  }


  template <class S, class D>
  void copyconvert(const volume<S>& source, volume<D>& dest, const bool copyData=true)
  {
    // set up basic size and data storage
    if ( dest.totalElements() != source.totalElements() )
      dest.reinitialize(source.xsize(),source.ysize(),source.zsize(),source.tsize(),source.size5(),source.size6(),source.size7());
    // set up properties (except lazy ones)
    copybasicproperties(source,dest);
    // now copy across the data
    if ( copyData )
      convertbuffer(source.Data, dest.Data, source.totalElements() );
  }

  template <class S>
  volume<S> operator+(S num, const volume<S>& vol)
    { return (vol + num); }

  template <class S>
  volume<S> operator-(S num, const volume<S>& vol)
  {
    volume<S> tmp = vol;
    tmp=num;
    tmp-=vol;
    return tmp;
  }

  template <class S>
  volume<S> operator*(S num, const volume<S>& vol)
    { return (vol * num); }

  template <class S>
  volume<S> operator/(S num, const volume<S>& vol)
  {
    volume<S> tmp = vol;
    tmp=num;
    tmp/=vol;
    return tmp;
  }

  template <class S>
  volume<S> operator-(const volume<S>& vol)
  {
    return(vol * (static_cast<S>(-1)));
  }

  template <class S>
  bool operator==(const volume<S>& v1,
		  const volume<S>& v2)
  {
    // Check relevant parts of header
    if (!samesize(v1,v2,true)) return(false);
    if (v1.sform_code() != v2.sform_code()) return(false);
    if (v1.sform_mat() != v2.sform_mat()) return(false);
    if (v1.qform_code() != v2.qform_code()) return(false);
    if (v1.qform_mat() != v2.qform_mat()) return(false);
    // Check data
    for (typename volume<S>::fast_const_iterator it1=v1.fbegin(), it_end=v1.fend(), it2=v2.fbegin(); it1 != it_end; ++it1, ++it2) {
      if ((*it1) != (*it2)) return(false);
    }

    return(true);
  }
  template <class S>
  bool operator!=(const volume<S>& v1, const volume<S>& v2) {return(!(v1==v2));}

  template < class S, class V >
  std::vector<double> calculateSums(const S& inputVolume, const V& mask );
  template < class T, class V>
  std::vector<T> calculateExtrema(const volume<T>& inputVolume, std::vector<int64_t>& coordinates, const volume<V>& mask );

}  // end namespace

#endif
