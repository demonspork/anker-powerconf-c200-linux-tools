#ifndef C200_CONTROLS_H
#define C200_CONTROLS_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "c200_device.h"

enum c200_control_kind {
	C200_CONTROL_KIND_VENDOR_BOOL,
	C200_CONTROL_KIND_VENDOR_U8,
	C200_CONTROL_KIND_VENDOR_FOV,
	C200_CONTROL_KIND_VENDOR_AREA_METERING,
	C200_CONTROL_KIND_VENDOR_ENABLE_U8,
	C200_CONTROL_KIND_V4L2_BOOL,
	C200_CONTROL_KIND_V4L2_INT,
	C200_CONTROL_KIND_V4L2_MENU,
};

struct c200_control_info {
	const char *name;
	enum c200_control_kind kind;
	uint32_t id;
	const char *description;
	uint8_t max_value; /* Only used by C200_CONTROL_KIND_VENDOR_ENABLE_U8. */
	unsigned int models; /* Bitmask of enum c200_model this control is known to work on. */
};

const struct c200_control_info *c200_find_control(const char *name);
const struct c200_control_info *c200_all_controls(size_t *count);
/* True if the control is known to work on the given model. */
bool c200_control_supports_model(const struct c200_control_info *control, enum c200_model model);
bool c200_parse_bool(const char *text, bool *value);
const char *c200_format_bool(bool value);

#endif
