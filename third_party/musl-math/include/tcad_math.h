/* TextCAD: one implementation of the transcendental functions on every platform (docs/adr/0014).
 *
 * Force-included into every OpenCASCADE and shim source file of the Windows and WebAssembly builds
 * (MSVC: /FI, clang: -include). It declares the vendored musl functions (libtcad_math) and renames
 * every call of sin, cos, pow, ... to them, so the platform C library's versions, which round
 * differently on each platform, are never called. sqrt, fmod, floor and the like are exact in
 * IEEE 754 and stay the platform's. */
#ifndef TCAD_MATH_H
#define TCAD_MATH_H

#ifdef __cplusplus
#include <cmath>
#endif
#include <math.h>

#ifdef __cplusplus
extern "C" {
#endif
double tcad_sin(double);
double tcad_cos(double);
double tcad_tan(double);
double tcad_asin(double);
double tcad_acos(double);
double tcad_atan(double);
double tcad_atan2(double, double);
double tcad_sinh(double);
double tcad_cosh(double);
double tcad_tanh(double);
double tcad_asinh(double);
double tcad_acosh(double);
double tcad_atanh(double);
double tcad_exp(double);
double tcad_expm1(double);
double tcad_log(double);
double tcad_log10(double);
double tcad_log1p(double);
double tcad_pow(double, double);
double tcad_cbrt(double);
float tcad_sinf(float);
float tcad_cosf(float);
float tcad_tanf(float);
float tcad_acosf(float);
float tcad_powf(float, float);
#ifdef __cplusplus
}
/* std::sin(x) becomes std::tcad_sin(x): make the renamed functions visible in std too. Float
 * arguments of the C++ overloads (std::sin(float)) go to the double version. */
namespace std {
using ::tcad_sin; using ::tcad_cos; using ::tcad_tan;
using ::tcad_asin; using ::tcad_acos; using ::tcad_atan; using ::tcad_atan2;
using ::tcad_sinh; using ::tcad_cosh; using ::tcad_tanh;
using ::tcad_asinh; using ::tcad_acosh; using ::tcad_atanh;
using ::tcad_exp; using ::tcad_expm1; using ::tcad_log; using ::tcad_log10; using ::tcad_log1p;
using ::tcad_pow; using ::tcad_cbrt;
using ::tcad_sinf; using ::tcad_cosf; using ::tcad_tanf; using ::tcad_acosf; using ::tcad_powf;
}
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
#define expm1 tcad_expm1
#define log tcad_log
#define log10 tcad_log10
#define log1p tcad_log1p
#define pow tcad_pow
#define cbrt tcad_cbrt
#define sinf tcad_sinf
#define cosf tcad_cosf
#define tanf tcad_tanf
#define acosf tcad_acosf
#define powf tcad_powf

#endif
