#include "c200_device.h"

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>

enum c200_model c200_model_from_usb_id(const struct c200_usb_id *id)
{
	if (id == NULL || id->vendor_id != C200_USB_VENDOR_ID_ANKER) {
		return C200_MODEL_UNKNOWN;
	}

	switch (id->product_id) {
	case C200_USB_PRODUCT_ID_C200:
		return C200_MODEL_C200;
	case C200_USB_PRODUCT_ID_C310:
		return C200_MODEL_C310;
	default:
		return C200_MODEL_UNKNOWN;
	}
}

const char *c200_model_name(enum c200_model model)
{
	switch (model) {
	case C200_MODEL_C200:
		return "PowerConf C200";
	case C200_MODEL_C310:
		return "AnkerWork C310";
	case C200_MODEL_UNKNOWN:
	default:
		return "unknown";
	}
}

void c200_model_mask_describe(unsigned int mask, char *buffer, size_t size)
{
	static const enum c200_model MODELS[] = {C200_MODEL_C200, C200_MODEL_C310};
	size_t i = 0;
	size_t used = 0;

	if (buffer == NULL || size == 0) {
		return;
	}

	buffer[0] = '\0';
	for (i = 0; i < sizeof(MODELS) / sizeof(MODELS[0]); ++i) {
		int written = 0;

		if ((mask & MODELS[i]) == 0) {
			continue;
		}
		written = snprintf(buffer + used, size - used, "%s%s", used > 0 ? ", " : "", c200_model_name(MODELS[i]));
		if (written < 0 || (size_t)written >= size - used) {
			return;
		}
		used += (size_t)written;
	}

	if (used == 0) {
		snprintf(buffer, size, "%s", c200_model_name(C200_MODEL_UNKNOWN));
	}
}

static int read_hex_u16(const char *path, uint16_t *value)
{
	FILE *file = fopen(path, "r");
	char text[16] = {0};
	char *end = NULL;
	unsigned long parsed = 0;

	if (file == NULL) {
		return -1;
	}

	if (fgets(text, sizeof(text), file) == NULL) {
		fclose(file);
		errno = EINVAL;
		return -1;
	}
	fclose(file);

	errno = 0;
	parsed = strtoul(text, &end, 16);
	if (errno != 0 || end == text || (*end != '\0' && *end != '\n') || parsed > 0xffff) {
		errno = EINVAL;
		return -1;
	}

	*value = (uint16_t)parsed;
	return 0;
}

int c200_device_read_usb_id_from_dir(const char *usb_device_dir, struct c200_usb_id *id)
{
	char path[512];

	if (usb_device_dir == NULL || id == NULL) {
		errno = EINVAL;
		return -1;
	}

	if (snprintf(path, sizeof(path), "%s/idVendor", usb_device_dir) >= (int)sizeof(path)) {
		errno = ENAMETOOLONG;
		return -1;
	}
	if (read_hex_u16(path, &id->vendor_id) < 0) {
		return -1;
	}

	snprintf(path, sizeof(path), "%s/idProduct", usb_device_dir);
	return read_hex_u16(path, &id->product_id);
}

int c200_device_get_usb_id(int fd, struct c200_usb_id *id)
{
	struct stat st;
	char dir[128];

	if (id == NULL) {
		errno = EINVAL;
		return -1;
	}

	if (fstat(fd, &st) < 0) {
		return -1;
	}
	if (!S_ISCHR(st.st_mode)) {
		errno = ENOTTY;
		return -1;
	}

	/*
	 * /sys/dev/char/81:N -> .../usbX/X-Y/X-Y:1.0/video4linux/videoN
	 * "device" is the USB interface, its parent is the USB device.
	 */
	snprintf(dir, sizeof(dir), "/sys/dev/char/%u:%u/device/..", major(st.st_rdev), minor(st.st_rdev));
	return c200_device_read_usb_id_from_dir(dir, id);
}
