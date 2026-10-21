/*
 * Portability shims for prebuilt binaries.
 *
 * glibc 2.43 gave acosf and atan2f new symbol versions (GLIBC_2.43), so an
 * executable linked on a 2.43 host refuses to start on distributions that
 * ship an older glibc (Fedora 42/43, Ubuntu 24.04) even though the functions
 * exist there. Defining them here binds the Rust code to these copies, which
 * only need the double-precision versions that every glibc has had since
 * GLIBC_2.2.5. Harmless on hosts where the issue does not arise.
 */
#include <math.h>

float acosf(float x)
{
	return (float)acos((double)x);
}

float atan2f(float y, float x)
{
	return (float)atan2((double)y, (double)x);
}
