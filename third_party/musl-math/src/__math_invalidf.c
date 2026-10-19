#include "tcad_musl_names.h" /* TextCAD: see README.md */
#include "libm.h"

float __math_invalidf(float x)
{
	return (x - x) / (x - x);
}
