#include "tcad_musl_names.h" /* TextCAD: see README.md */
#include "libm.h"

double __math_xflow(uint32_t sign, double y)
{
	return eval_as_double(fp_barrier(sign ? -y : y) * y);
}
