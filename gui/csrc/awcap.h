#ifndef AWCAP_H
#define AWCAP_H

/*
 * Thin C shim over the V4L2 streaming and standard-control ioctls so the Rust
 * side never has to replicate kernel struct layouts. Built with the real
 * <linux/videodev2.h>.
 */

#include <stddef.h>
#include <stdint.h>

struct awcap_mode {
	uint32_t fourcc;
	uint32_t width;
	uint32_t height;
	uint32_t max_fps;
};

/* Enumerate discrete capture modes. Returns the count written (<= max) or -1. */
int awcap_list_modes(const char *path, struct awcap_mode *out, int max);

struct awcap;

/*
 * Open a capture stream. fps may be 0 for the driver default. On failure
 * returns NULL with errno set and a short stage description in err.
 */
struct awcap *awcap_open(const char *path, uint32_t fourcc, uint32_t width, uint32_t height, uint32_t fps, char *err, size_t err_len);

/* Negotiated stream parameters. */
uint32_t awcap_width(const struct awcap *cap);
uint32_t awcap_height(const struct awcap *cap);
uint32_t awcap_fourcc(const struct awcap *cap);

/*
 * Wait up to timeout_ms for a frame. Returns 1 with data/len pointing into
 * the mmap'd buffer (valid until awcap_release_frame), 0 on timeout, -1 on
 * error (errno set).
 */
int awcap_wait_frame(struct awcap *cap, int timeout_ms, const uint8_t **data, size_t *len);
int awcap_release_frame(struct awcap *cap);
void awcap_close(struct awcap *cap);

/* Standard V4L2 controls. */
struct awctl_info {
	int32_t minimum;
	int32_t maximum;
	int32_t step;
	int32_t default_value;
	uint32_t flags;
	uint32_t type;
};

int awctl_query(int fd, uint32_t id, struct awctl_info *out);
int awctl_get(int fd, uint32_t id, int32_t *value);
int awctl_set(int fd, uint32_t id, int32_t value);
/* Returns 0 and fills name for a valid menu index, -1 otherwise. */
int awctl_menu_name(int fd, uint32_t id, uint32_t index, char *name, size_t name_len);

#endif
