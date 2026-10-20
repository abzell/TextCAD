/* TextCAD: renames the vendored musl functions to tcad_* (public) and tcad_musl_* (internal) so
 * they never collide with the platform C library. Included first by every vendored source. */
#ifndef TCAD_MUSL_NAMES_H
#define TCAD_MUSL_NAMES_H
#define _USE_MATH_DEFINES
#include <math.h>  /* the platform declarations stay untouched */
#ifndef hidden
#define hidden
#endif
#ifndef M_PI_2
#define M_PI_2 1.57079632679489661923
#endif
#ifndef M_PI_4
#define M_PI_4 0.78539816339744830962
#endif
#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif
#define sin tcad_sin
#define cos tcad_cos
#define tan tcad_tan
#define asin tcad_asin
#define acos tcad_acos
#define atan tcad_atan
#define atan2 tcad_atan2
#define sinh tcad_sinh
#define cosh tcad_cosh
#define tanh tcad_tanh
#define asinh tcad_asinh
#define acosh tcad_acosh
#define atanh tcad_atanh
#define exp tcad_exp
#define log tcad_log
#define log10 tcad_log10
#define log1p tcad_log1p
#define expm1 tcad_expm1
#define pow tcad_pow
#define cbrt tcad_cbrt
#define sinf tcad_sinf
#define cosf tcad_cosf
#define tanf tcad_tanf
#define acosf tcad_acosf
#define powf tcad_powf
#define __sin tcad_musl__sin
#define __cos tcad_musl__cos
#define __tan tcad_musl__tan
#define __rem_pio2 tcad_musl__rem_pio2
#define __rem_pio2_large tcad_musl__rem_pio2_large
#define __expo2 tcad_musl__expo2
#define __rem_pio2f tcad_musl__rem_pio2f
#define __sindf tcad_musl__sindf
#define __cosdf tcad_musl__cosdf
#define __tandf tcad_musl__tandf
#define __expo2f tcad_musl__expo2f
#define __math_xflow tcad_musl__math_xflow
#define __math_uflow tcad_musl__math_uflow
#define __math_oflow tcad_musl__math_oflow
#define __math_divzero tcad_musl__math_divzero
#define __math_invalid tcad_musl__math_invalid
#define __math_xflowf tcad_musl__math_xflowf
#define __math_uflowf tcad_musl__math_uflowf
#define __math_oflowf tcad_musl__math_oflowf
#define __math_divzerof tcad_musl__math_divzerof
#define __math_invalidf tcad_musl__math_invalidf
#define __exp_data tcad_musl__exp_data
#define __log_data tcad_musl__log_data
#define __pow_log_data tcad_musl__pow_log_data
#define __exp2f_data tcad_musl__exp2f_data
#define __powf_log2_data tcad_musl__powf_log2_data
/* prototypes under the new names (the platform's <math.h> declares only the original ones) */
double sin(double); double cos(double); double tan(double);
double asin(double); double acos(double); double atan(double); double atan2(double, double);
double sinh(double); double cosh(double); double tanh(double);
double asinh(double); double acosh(double); double atanh(double);
double exp(double); double expm1(double); double log(double); double log10(double); double log1p(double);
double pow(double, double); double cbrt(double);
float sinf(float); float cosf(float); float tanf(float); float acosf(float); float powf(float, float);
#endif
