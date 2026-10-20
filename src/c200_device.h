#ifndef C200_DEVICE_H
#define C200_DEVICE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define C200_USB_VENDOR_ID_ANKER 0x291a
#define C200_USB_PRODUCT_ID_C200 0x3369
#define C200_USB_PRODUCT_ID_C310 0x3367

/* Bitmask of camera models a control is known to work on. */
enum c200_model {
	C200_MODEL_UNKNOWN = 0,
	C200_MODEL_C200 = 1u << 0,
	C200_MODEL_C310 = 1u << 1,
};

#define C200_MODEL_ANY (C200_MODEL_C200 | C200_MODEL_C310)

struct c200_usb_id {
	uint16_t vendor_id;
	uint16_t product_id;
};

/* Map a USB vendor/product pair to a known model (C200_MODEL_UNKNOWN if none). */
enum c200_model c200_model_from_usb_id(const struct c200_usb_id *id);

/* Human readable name for a single model, or a comma list for a mask. */
const char *c200_model_name(enum c200_model model);
void c200_model_mask_describe(unsigned int mask, char *buffer, size_t size);

/*
 * Resolve the USB IDs of the camera behind an open V4L2 node by walking
 * /sys/dev/char/<major>:<minor>/device/../idVendor (and idProduct).
 * Returns 0 on success, -1 (errno set) if the node is not a USB device.
 */
int c200_device_get_usb_id(int fd, struct c200_usb_id *id);

/* Same lookup against an arbitrary sysfs USB device directory (for tests). */
int c200_device_read_usb_id_from_dir(const char *usb_device_dir, struct c200_usb_id *id);

/* True if the errno of a failed query means the camera dropped off the bus (self-restart or unplug). */
bool c200_device_errno_means_gone(int err);

/*
 * Wait for a camera that restarted itself to re-enumerate and return a fresh
 * descriptor for the same node. Polls until the node opens and answers
 * VIDIOC_QUERYCAP, or timeout_ms elapses (-1, errno ETIMEDOUT).
 */
int c200_device_reopen(const char *path, unsigned int timeout_ms);

#endif
