#include "tcad_musl_names.h" /* TextCAD: see README.md */
#include "libm.h"

double __math_invalid(double x)
{
	return (x - x) / (x - x);
}
