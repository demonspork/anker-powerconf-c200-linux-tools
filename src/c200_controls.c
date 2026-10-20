#include "c200_controls.h"

#include <time.h>
#include <linux/videodev2.h>
#include <strings.h>

static const struct c200_control_info C200_CONTROLS[] = {
	{"fov", C200_CONTROL_KIND_VENDOR_FOV, 0x10, "Vendor FOV preset/value", 0, C200_MODEL_ANY, 0},
	{"hdr", C200_CONTROL_KIND_VENDOR_BOOL, 0x13, "Vendor HDR toggle", 0, C200_MODEL_ANY, 0},
	{"horizontal_flip", C200_CONTROL_KIND_VENDOR_BOOL, 0x11, "Vendor horizontal mirror", 0, C200_MODEL_ANY, 0},
	{"vertical_screen", C200_CONTROL_KIND_VENDOR_BOOL, 0x0a, "Vendor vertical screen mode (every write restarts the camera)", 0, C200_MODEL_ANY, C200_CONTROL_FLAG_RESTARTS_CAMERA},
	{"anti_flicker", C200_CONTROL_KIND_VENDOR_U8, 0x12, "Vendor anti-flicker mode", 0, C200_MODEL_ANY, 0},
	{"exposure_smoothing", C200_CONTROL_KIND_VENDOR_AREA_METERING, 0x18, "Vendor exposure smoothing / area metering (on|off[,x,y,w,h] on a 3840x2160 grid)", 0, .models = C200_MODEL_C310, .flags = 0},
	{"area_metering", C200_CONTROL_KIND_VENDOR_AREA_METERING, 0x18, "Alias of exposure_smoothing", 0, .models = C200_MODEL_C310, .flags = 0},
	{"exposure_compensation", C200_CONTROL_KIND_VENDOR_ENABLE_U8, 0x19, "Vendor auto exposure (on|off[,compensation 0..60], 30 is neutral)", .max_value = 60, .models = C200_MODEL_C310, .flags = 0},
	{"face_exposure", C200_CONTROL_KIND_VENDOR_ENABLE_U8, 0x0b, "Vendor face auto exposure / FACE AE (on|off[,value 0..60])", .max_value = 60, .models = C200_MODEL_C310, .flags = 0},
	{"brightness", C200_CONTROL_KIND_V4L2_INT, V4L2_CID_BRIGHTNESS, "Standard UVC brightness", 0, C200_MODEL_ANY, 0},
	{"contrast", C200_CONTROL_KIND_V4L2_INT, V4L2_CID_CONTRAST, "Standard UVC contrast", 0, C200_MODEL_ANY, 0},
	{"saturation", C200_CONTROL_KIND_V4L2_INT, V4L2_CID_SATURATION, "Standard UVC saturation", 0, C200_MODEL_ANY, 0},
	{"hue", C200_CONTROL_KIND_V4L2_INT, V4L2_CID_HUE, "Standard UVC hue", 0, C200_MODEL_ANY, 0},
	{"white_balance_automatic", C200_CONTROL_KIND_V4L2_BOOL, V4L2_CID_AUTO_WHITE_BALANCE, "Standard UVC auto white balance", 0, C200_MODEL_ANY, 0},
	{"gamma", C200_CONTROL_KIND_V4L2_INT, V4L2_CID_GAMMA, "Standard UVC gamma", 0, C200_MODEL_ANY, 0},
	{"power_line_frequency", C200_CONTROL_KIND_V4L2_MENU, V4L2_CID_POWER_LINE_FREQUENCY, "Standard UVC power-line frequency", 0, C200_MODEL_ANY, 0},
	{"white_balance_temperature", C200_CONTROL_KIND_V4L2_INT, V4L2_CID_WHITE_BALANCE_TEMPERATURE, "Standard UVC white balance temperature", 0, C200_MODEL_ANY, 0},
	{"sharpness", C200_CONTROL_KIND_V4L2_INT, V4L2_CID_SHARPNESS, "Standard UVC sharpness", 0, C200_MODEL_ANY, 0},
	{"auto_exposure", C200_CONTROL_KIND_V4L2_MENU, V4L2_CID_EXPOSURE_AUTO, "Standard UVC auto exposure mode", 0, C200_MODEL_ANY, 0},
	{"exposure_time_absolute", C200_CONTROL_KIND_V4L2_INT, V4L2_CID_EXPOSURE_ABSOLUTE, "Standard UVC exposure time", 0, C200_MODEL_ANY, 0},
	{"pan_absolute", C200_CONTROL_KIND_V4L2_INT, V4L2_CID_PAN_ABSOLUTE, "Standard UVC pan", 0, C200_MODEL_ANY, 0},
	{"tilt_absolute", C200_CONTROL_KIND_V4L2_INT, V4L2_CID_TILT_ABSOLUTE, "Standard UVC tilt", 0, C200_MODEL_ANY, 0},
	{"focus_absolute", C200_CONTROL_KIND_V4L2_INT, V4L2_CID_FOCUS_ABSOLUTE, "Standard UVC focus", 0, C200_MODEL_ANY, 0},
	{"focus_automatic_continuous", C200_CONTROL_KIND_V4L2_BOOL, V4L2_CID_FOCUS_AUTO, "Standard UVC autofocus", 0, C200_MODEL_ANY, 0},
	{"zoom_absolute", C200_CONTROL_KIND_V4L2_INT, V4L2_CID_ZOOM_ABSOLUTE, "Standard UVC zoom", 0, C200_MODEL_ANY, 0},
};

const struct c200_control_info *c200_find_control(const char *name)
{
	size_t i = 0;

	for (i = 0; i < sizeof(C200_CONTROLS) / sizeof(C200_CONTROLS[0]); ++i) {
		if (strcasecmp(C200_CONTROLS[i].name, name) == 0) {
			return &C200_CONTROLS[i];
		}
	}

	return NULL;
}

bool c200_control_supports_model(const struct c200_control_info *control, enum c200_model model)
{
	if (control == NULL) {
		return false;
	}

	return (control->models & (unsigned int)model) != 0;
}

const struct c200_control_info *c200_all_controls(size_t *count)
{
	if (count != NULL) {
		*count = sizeof(C200_CONTROLS) / sizeof(C200_CONTROLS[0]);
	}

	return C200_CONTROLS;
}

bool c200_parse_bool(const char *text, bool *value)
{
	if (text == NULL || value == NULL) {
		return false;
	}

	if (strcasecmp(text, "1") == 0 || strcasecmp(text, "on") == 0 || strcasecmp(text, "true") == 0 || strcasecmp(text, "yes") == 0) {
		*value = true;
		return true;
	}

	if (strcasecmp(text, "0") == 0 || strcasecmp(text, "off") == 0 || strcasecmp(text, "false") == 0 || strcasecmp(text, "no") == 0) {
		*value = false;
		return true;
	}

	return false;
}

const char *c200_format_bool(bool value)
{
	return value ? "on" : "off";
}
