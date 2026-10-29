/*  miscmaths.h

    Mark Jenkinson & Mark Woolrich & Christian Beckmann & Tim Behrens, FMRIB Image Analysis Group

    Copyright (C) 1999-2000 University of Oxford  */

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

// Miscellaneous maths functions




#if !defined(__miscmaths_h)
#define __miscmaths_h

#include <cmath>
#include <cstdlib>
#include <vector>
#include <iostream>
#include <assert.h>
#include <fstream>
#include <sstream>
#include <string>
#include <iterator>
#include "NewNifti/NewNifti.h"
//#include "config.h"
#include "armawrap/newmatap.h"
#include "kernel.h"

//#pragma interface

namespace MISCMATHS {


#ifndef M_PI
#define M_PI            3.14159265358979323846
#endif

  // IO/string stuff
  template <class T> std::string num2str(T n, int width=-1);

#if defined(_MSC_VER) && (_MSC_VER < 1300)
  template <class T> std::string num2str(T n) { return num2str(n -1); }
#endif

  std::string size(const NEWMAT::Matrix& mat);
  bool isNumber(const std::string& str);
  NEWMAT::ReturnMatrix read_ascii_matrix(const std::string& filename, int nrows, int ncols);
  NEWMAT::ReturnMatrix read_ascii_matrix(int nrows, int ncols, const std::string& filename);
  NEWMAT::ReturnMatrix read_ascii_matrix(const std::string& filename);
  NEWMAT::ReturnMatrix read_vest(std::string p_fname);
  int read_binary_matrix(NEWMAT::Matrix& mres, const std::string& filename);
  NEWMAT::ReturnMatrix read_binary_matrix(const std::string& filename);
  NEWMAT::ReturnMatrix read_matrix(const std::string& filename);

  int write_ascii_matrix(const NEWMAT::Matrix& mat, const std::string& filename,
			 int precision=-1);
  int write_ascii_matrix(const std::string& filename, const NEWMAT::Matrix& mat,
			 int precision=-1);
  int write_vest(const NEWMAT::Matrix& x, std::string p_fname, int precision=-1);
  int write_vest(std::string p_fname, const NEWMAT::Matrix& x, int precision=-1);
  int write_binary_matrix(const NEWMAT::Matrix& mat, const std::string& filename);

  // more basic IO calls
  std::string skip_alpha(std::ifstream& fs);
  NEWMAT::ReturnMatrix read_ascii_matrix(std::ifstream& fs, int nrows, int ncols);
  NEWMAT::ReturnMatrix read_ascii_matrix(int nrows, int ncols, std::ifstream& fs);
  NEWMAT::ReturnMatrix read_ascii_matrix(std::ifstream& fs);
  int read_binary_matrix(NEWMAT::Matrix& mres, std::ifstream& fs);
  NEWMAT::ReturnMatrix read_binary_matrix(std::ifstream& fs);
  int write_ascii_matrix(const NEWMAT::Matrix& mat, std::ofstream& fs, int precision=-1);
  int write_ascii_matrix(std::ofstream& fs, const NEWMAT::Matrix& mat, int precision=-1);
  int write_binary_matrix(const NEWMAT::Matrix& mat, std::ofstream& fs);

  // General maths

  int round(int x);
  int round(float x);
  int round(double x);
  double rounddouble(double x);

  inline int sign(int x){ if (x>0) return 1; else { if (x<0) return -1; else return 0; } }
  inline int sign(float x){ if (x>0) return 1; else { if (x<0) return -1; else return 0; } }
  inline int sign(double x){ if (x>0) return 1; else { if (x<0) return -1; else return 0; } }

  inline double pow(double x, float y) { return std::pow(x,(double) y); }
  inline double pow(float x, double y) { return std::pow((double) x,y); }
  inline double pow(double x, int y) { return std::pow(x,(double) y); }
  inline float pow(float x, int y) { return std::pow(x,(float) y); }
  inline double pow(int x, double y) { return std::pow((double)x, y); }
  inline float pow(int x, float y) { return std::pow((float)x, y); }

  inline double sqrt(int x) { return std::sqrt((double) x); }
  inline double log(int x) { return std::log((double) x); }

  float Sinc(const float x);
  double Sinc(const double x);

  int periodicclamp(int x, int x1, int x2);


#ifdef __ARMAWRAP_HPP__
  template<class S,class T,class U,bool V>
  inline T Min(const S &a, const armawrap::AWCallManager<T,U,V>& b ) { if (a<b) return (T) a; else return b; }
#endif

  template<class S, class T>
   inline T Min(const S &a, const T &b) { if (a<b) return (T) a; else return b; }

  template<class S, class T>
   inline T Max(const S &a, const T &b) { if (a>b) return (T) a; else return b; }

#ifdef __ARMAWRAP_HPP__
  template<class T,class U,bool V>
    inline T Sqr(const armawrap::AWCallManager<T,U,V>& x) {return x*x;}
#endif

  template<class T>
   inline T Sqr(const T& x) { return x*x; }

  NEWMAT::ColumnVector cross(const NEWMAT::ColumnVector& a, const NEWMAT::ColumnVector& b);
  NEWMAT::ColumnVector cross(const double *a, const double *b);

  inline float dot(const NEWMAT::ColumnVector& a, const NEWMAT::ColumnVector& b)
    { return Sum(SP(a,b)); }

  double norm2(const NEWMAT::ColumnVector& x);
  double norm2sq(double a, double b, double c);
  float norm2sq(float a, float b, float c);

  NEWMAT::ColumnVector seq(const int num);

  int diag(NEWMAT::Matrix& m, const float diagvals[]);
  int diag(NEWMAT::Matrix& m, const NEWMAT::ColumnVector& diagvals);
  int diag(NEWMAT::DiagonalMatrix& m, const NEWMAT::ColumnVector& diagvals);
  NEWMAT::ReturnMatrix diag(const NEWMAT::Matrix& mat);
  NEWMAT::ReturnMatrix pinv(const NEWMAT::Matrix& mat);
  int rank(const NEWMAT::Matrix& X);
  NEWMAT::ReturnMatrix sqrtaff(const NEWMAT::Matrix& mat);

  // in the following mode = "new2old" or "old2new" (see .cc for more info)
  std::vector<int> get_sortindex(const NEWMAT::Matrix& vals, const std::string& mode, int col=1);
  NEWMAT::Matrix apply_sortindex(const NEWMAT::Matrix& vals, std::vector<int> sidx, const std::string& mode);

  void reshape(NEWMAT::Matrix& r, const NEWMAT::Matrix& m, int nrows, int ncols);
  NEWMAT::ReturnMatrix reshape(const NEWMAT::Matrix& m, int nrows, int ncols);
  int addrow(NEWMAT::Matrix& m, int ncols);

  int construct_rotmat_euler(const NEWMAT::ColumnVector& params, int n, NEWMAT::Matrix& aff);
  int construct_rotmat_euler(const NEWMAT::ColumnVector& params, int n, NEWMAT::Matrix& aff,
		   const NEWMAT::ColumnVector& centre);
  int construct_rotmat_quat(const NEWMAT::ColumnVector& params, int n, NEWMAT::Matrix& aff);
  int construct_rotmat_quat(const NEWMAT::ColumnVector& params, int n, NEWMAT::Matrix& aff,
		   const NEWMAT::ColumnVector& centre);
  int make_rot(const NEWMAT::ColumnVector& angl, const NEWMAT::ColumnVector& centre,
	       NEWMAT::Matrix& rot);

  int getrotaxis(NEWMAT::ColumnVector& axis, const NEWMAT::Matrix& rotmat);
  int rotmat2euler(NEWMAT::ColumnVector& angles, const NEWMAT::Matrix& rotmat);
  int rotmat2quat(NEWMAT::ColumnVector& quaternion, const NEWMAT::Matrix& rotmat);
  int decompose_aff(NEWMAT::ColumnVector& params, const NEWMAT::Matrix& affmat,
		    int (*rotmat2params)(NEWMAT::ColumnVector& , const NEWMAT::Matrix& ));
  int decompose_aff(NEWMAT::ColumnVector& params, const NEWMAT::Matrix& affmat,
		    const NEWMAT::ColumnVector& centre,
                    int (*rotmat2params)(NEWMAT::ColumnVector& , const NEWMAT::Matrix& ));
  int compose_aff(const NEWMAT::ColumnVector& params, int n, const NEWMAT::ColumnVector& centre,
		  NEWMAT::Matrix& aff,
		  int (*params2rotmat)(const NEWMAT::ColumnVector& , int , NEWMAT::Matrix& ,
				       const NEWMAT::ColumnVector& ) );
  float rms_deviation(const NEWMAT::Matrix& affmat1, const NEWMAT::Matrix& affmat2,
		      const NEWMAT::ColumnVector& centre, const float rmax);
  float rms_deviation(const NEWMAT::Matrix& affmat1, const NEWMAT::Matrix& affmat2,
		      const float rmax=80.0);

  void get_axis_orientations(const NEWMAT::Matrix& sform_mat, int sform_code,
			     const NEWMAT::Matrix& qform_mat, int qform_code,
			     int& icode, int& jcode, int& kcode);

  // 1D lookup table with linear interpolation
  float interp1(const NEWMAT::ColumnVector& x, const NEWMAT::ColumnVector& y, float xi);

  float quantile(const NEWMAT::ColumnVector& in, int which);
  float percentile(const NEWMAT::ColumnVector& in, float p);
  inline float median(const NEWMAT::ColumnVector& in){ return quantile(in,2);}
  inline float iqr(const NEWMAT::ColumnVector &in) { return quantile(in,3) - quantile(in,1); }

  NEWMAT::ReturnMatrix quantile(const NEWMAT::Matrix& in, int which);
  NEWMAT::ReturnMatrix percentile(const NEWMAT::Matrix& in, float p);
  inline NEWMAT::ReturnMatrix median(const NEWMAT::Matrix& in){ return quantile(in,2);}
  inline NEWMAT::ReturnMatrix iqr(const NEWMAT::Matrix& in){ NEWMAT::Matrix res = quantile(in,3) - quantile(in,1); res.Release(); return res;}

  void cart2sph(const NEWMAT::ColumnVector& dir, float& th, float& ph);// cartesian to sperical polar coordinates
  void cart2sph(const NEWMAT::Matrix& dir,NEWMAT::ColumnVector& th,NEWMAT::ColumnVector& ph);//ditto
  void cart2sph(const std::vector<NEWMAT::ColumnVector>& dir,NEWMAT::ColumnVector& th,NEWMAT::ColumnVector& ph);// same but in a vector

  // geometry function
  inline float point_plane_distance(const NEWMAT::ColumnVector& X,const NEWMAT::ColumnVector& P){//plane defined by a,b,c,d with a^2+b^2+c^2=1
    return( dot(X,P.SubMatrix(1,3,1,1))+P(4) );
  }

  // returns the first P such that 2^P >= abs(N).
  int nextpow2(int n);

  // Auto-correlation function estimate of columns of p_ts
  // gives unbiased estimate - scales the raw correlation by 1/(N-abs(lags))
  void xcorr(const NEWMAT::Matrix& p_ts, NEWMAT::Matrix& ret, int lag = 0, int p_zeropad = 0);
  NEWMAT::ReturnMatrix xcorr(const NEWMAT::Matrix& p_ts, int lag = 0, int p_zeropad = 0);

  // removes trend from columns of p_ts
  // if p_level==0 it just removes the mean
  // if p_level==1 it removes linear trend
  // if p_level==2 it removes quadratic trend
  void detrend(NEWMAT::Matrix& p_ts, int p_level=1);

  NEWMAT::ReturnMatrix zeros(const int dim1, const int dim2 = -1);
  NEWMAT::ReturnMatrix ones(const int dim1, const int dim2 = -1);
  NEWMAT::ReturnMatrix repmat(const NEWMAT::Matrix& mat, const int rows = 1, const int cols = 1);
  NEWMAT::ReturnMatrix dist2(const NEWMAT::Matrix& mat1, const NEWMAT::Matrix& mat2);
  NEWMAT::ReturnMatrix abs(const NEWMAT::Matrix& mat);
  void abs_econ(NEWMAT::Matrix& mat);
  NEWMAT::ReturnMatrix sqrt(const NEWMAT::Matrix& mat);
  void sqrt_econ(NEWMAT::Matrix& mat);
  NEWMAT::ReturnMatrix sqrtm(const NEWMAT::Matrix& mat);
  NEWMAT::ReturnMatrix log(const NEWMAT::Matrix& mat);
  void log_econ(NEWMAT::Matrix& mat);
  NEWMAT::ReturnMatrix exp(const NEWMAT::Matrix& mat);
  void exp_econ(NEWMAT::Matrix& mat);
  NEWMAT::ReturnMatrix expm(const NEWMAT::Matrix& mat);
  NEWMAT::ReturnMatrix tanh(const NEWMAT::Matrix& mat);
  void tanh_econ(NEWMAT::Matrix& mat);
  NEWMAT::ReturnMatrix pow(const NEWMAT::Matrix& mat, const double exp);
  void pow_econ(NEWMAT::Matrix& mat, const double exp);
  NEWMAT::ReturnMatrix sum(const NEWMAT::Matrix& mat, const int dim = 1);
  NEWMAT::ReturnMatrix mean(const NEWMAT::Matrix& mat, const int dim = 1);
  NEWMAT::ReturnMatrix mean(const NEWMAT::Matrix& mat, const NEWMAT::RowVector& weights, const int dim=1);
  NEWMAT::ReturnMatrix var(const NEWMAT::Matrix& mat, const int dim = 1);
  NEWMAT::ReturnMatrix max(const NEWMAT::Matrix& mat);
  NEWMAT::ReturnMatrix max(const NEWMAT::Matrix& mat,NEWMAT::ColumnVector& index);
  NEWMAT::ReturnMatrix min(const NEWMAT::Matrix& mat);
  NEWMAT::ReturnMatrix gt(const NEWMAT::Matrix& mat1,const NEWMAT::Matrix& mat2);
  NEWMAT::ReturnMatrix lt(const NEWMAT::Matrix& mat1,const NEWMAT::Matrix& mat2);
  NEWMAT::ReturnMatrix geqt(const NEWMAT::Matrix& mat1,const NEWMAT::Matrix& mat2);
  NEWMAT::ReturnMatrix geqt(const NEWMAT::Matrix& mat1,const float a);
  NEWMAT::ReturnMatrix leqt(const NEWMAT::Matrix& mat1,const NEWMAT::Matrix& mat2);
  NEWMAT::ReturnMatrix eq(const NEWMAT::Matrix& mat1,const NEWMAT::Matrix& mat2);
  NEWMAT::ReturnMatrix neq(const NEWMAT::Matrix& mat1,const NEWMAT::Matrix& mat2);
  NEWMAT::ReturnMatrix SD(const NEWMAT::Matrix& mat1,const NEWMAT::Matrix& mat2); // Schur (element-wise) divide
  void SD_econ(NEWMAT::Matrix& mat1,const NEWMAT::Matrix& mat2); // Schur (element-wise) divide
  void SP_econ(NEWMAT::Matrix& mat1,const NEWMAT::Matrix& mat2); // Schur (element-wise) divide

  NEWMAT::ReturnMatrix vox_to_vox(const NEWMAT::ColumnVector& xyz1,const NEWMAT::ColumnVector& dims1,const NEWMAT::ColumnVector& dims2,const NEWMAT::Matrix& xfm);
  NEWMAT::ReturnMatrix mni_to_imgvox(const NEWMAT::ColumnVector& mni,const NEWMAT::ColumnVector& mni_origin,const NEWMAT::Matrix& mni2img, const NEWMAT::ColumnVector& img_dims);

  void remmean_econ(NEWMAT::Matrix& mat, const int dim = 1);
  void remmean(NEWMAT::Matrix& mat, NEWMAT::Matrix& Mean, const int dim = 1);
  void remmean(const NEWMAT::Matrix& mat, NEWMAT::Matrix& demeanedmat, NEWMAT::Matrix& Mean,  const int dim = 1);
  NEWMAT::ReturnMatrix remmean(const NEWMAT::Matrix& mat, const int dim = 1);

  NEWMAT::ReturnMatrix stdev(const NEWMAT::Matrix& mat, const int dim = 1);
  NEWMAT::ReturnMatrix cov(const NEWMAT::Matrix& mat, const bool sampleCovariance = false, const int econ=20000);
  NEWMAT::ReturnMatrix cov_r(const NEWMAT::Matrix& mat, const bool sampleCovariance = false, const int econ=20000);
  NEWMAT::ReturnMatrix cov_r(const NEWMAT::Matrix& data, const NEWMAT::Matrix& weights, int econ=20000);



  NEWMAT::ReturnMatrix oldcov(const NEWMAT::Matrix& mat, const bool norm = false);
  NEWMAT::ReturnMatrix corrcoef(const NEWMAT::Matrix& mat, const bool norm = false);
  void symm_orth(NEWMAT::Matrix &Mat);
  void powerspectrum(const NEWMAT::Matrix &Mat1, NEWMAT::Matrix &Result, bool useLog);
  void element_mod_n(NEWMAT::Matrix& Mat,double n); //represent each element in modulo n (useful for wrapping phases (n=2*M_PI))

  // matlab-like flip function
  NEWMAT::ReturnMatrix flipud(const NEWMAT::Matrix& mat);
  NEWMAT::ReturnMatrix fliplr(const NEWMAT::Matrix& mat);

  // ols
  // data is t x v
  // des is t x ev (design matrix)
  // tc is cons x ev (contrast matrix)
  // cope and varcope will be cons x v
  // but will be resized if they are wrong
  void ols(const NEWMAT::Matrix& data,const NEWMAT::Matrix& des,const NEWMAT::Matrix& tc, NEWMAT::Matrix& cope,NEWMAT::Matrix& varcope);
  float ols_dof(const NEWMAT::Matrix& des);


  // Conjugate Gradient methods to solve for x in:   A * x = b
  // A must be symmetric and positive definite
  int conjgrad(NEWMAT::ColumnVector& x, const NEWMAT::Matrix& A, const NEWMAT::ColumnVector& b,
	       int maxit=3);
  // allow specification of reltol = relative tolerance of residual error
  //  (stops when error < reltol * initial error)
  int conjgrad(NEWMAT::ColumnVector& x, const NEWMAT::Matrix& A, const NEWMAT::ColumnVector& b,
	       int maxit, float reltol);

  float csevl(const float x, const NEWMAT::ColumnVector& cs, const int n);
  float digamma(const float x);
  void glm_vb(const NEWMAT::Matrix& X, const NEWMAT::ColumnVector& Y, NEWMAT::ColumnVector& B, NEWMAT::SymmetricMatrix& ilambda_B, int niters=20);

  std::vector<float> ColumnVector2vector(const NEWMAT::ColumnVector& col);

  ///////////////////////////////////////////////////////////////////////////
  // Uninteresting byte swapping functions
  void Swap_2bytes ( int n , void *ar ) ;
  void Swap_4bytes ( int n , void *ar ) ;
  void Swap_8bytes ( int n , void *ar ) ;
  void Swap_16bytes( int n , void *ar ) ;
  void Swap_Nbytes ( int n , int siz , void *ar ) ;

  ///////////////////////////////////////////////////////////////////////////
  ///////////////////////////////////////////////////////////////////////////

  // TEMPLATE DEFINITIONS //

  template<class t> NEWMAT::ReturnMatrix vector2ColumnVector(const std::vector<t>& vec)
  {
    NEWMAT::ColumnVector col(vec.size());

    for(unsigned int c = 0; c < vec.size(); c++)
      col(c+1) = vec[c];

    col.Release();
    return col;
  }

  template<class t> void write_vector(const std::string& fname, const std::vector<t>& vec)
  {
    std::ofstream out;
    out.open(fname.c_str(), std::ios::out);
    copy(vec.begin(), vec.end(), std::ostream_iterator<t>(out, " "));
  }

  template<class t> void write_vector(const std::vector<t>& vec, const std::string& fname)
  {
    write_vector(fname,vec);
  }

  template <class T>
  std::string num2str(T n, int width)
  {
    std::ostringstream os;
    if (width>0) {
      os.fill('0');
      os.width(width);
      os.setf(std::ios::internal, std::ios::adjustfield);
    }
    os << n;
    return os.str();
  }

}

#endif
