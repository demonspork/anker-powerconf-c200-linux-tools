#include "c200_area_metering.h"
#include "c200_controls.h"
#include "c200_device.h"
#include "c200_enable_u8.h"
#include "c200_fov.h"
#include "c200_vendor.h"

#include <errno.h>
#include <fcntl.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <time.h>
#include <linux/videodev2.h>
#include <sys/ioctl.h>
#include <unistd.h>

static void usage(FILE *stream, const char *argv0)
{
	fprintf(stream, "Usage: %s [--device /dev/videoN] list-controls\n", argv0);
	fprintf(stream, "       %s [--device /dev/videoN] get <control>\n", argv0);
	fprintf(stream, "       %s [--device /dev/videoN] set <control> <value>\n", argv0);
	fprintf(stream, "       %s [--device /dev/videoN] set exposure_smoothing <on|off>[,x,y,width,height]\n", argv0);
	fprintf(stream, "       %s [--device /dev/videoN] set exposure_compensation <on|off>[,0..60]\n", argv0);
	fprintf(stream, "       %s [--device /dev/videoN] apply-profile <file>\n", argv0);
	fprintf(stream, "exposure_smoothing, exposure_compensation and face_exposure are only accepted on an AnkerWork C310.\n");
	fprintf(stream, "Compatibility aliases:\n");
	fprintf(stream, "       %s [--device /dev/videoN] get-fov\n", argv0);
	fprintf(stream, "       %s [--device /dev/videoN] set-fov <65|78|95|narrow|medium|wide|raw>\n", argv0);
}

static int open_device(const char *path, int flags)
{
	int fd = open(path, flags);

	if (fd < 0) {
		fprintf(stderr, "failed to open %s: %s\n", path, strerror(errno));
	}

	return fd;
}

/*
 * Refuse controls that are not known to work on the connected camera. Vendor
 * selectors that only the C310 firmware implements must never be sent to a
 * C200, so an unidentifiable device is treated as unsupported for them.
 */
static int check_control_supported(int fd, const struct c200_control_info *control, const char *device)
{
	struct c200_usb_id usb_id = {0};
	enum c200_model model = C200_MODEL_UNKNOWN;
	char supported[64];

	if (control->models == C200_MODEL_ANY) {
		return 0;
	}

	if (c200_device_get_usb_id(fd, &usb_id) == 0) {
		model = c200_model_from_usb_id(&usb_id);
	}

	if (c200_control_supports_model(control, model)) {
		return 0;
	}

	c200_model_mask_describe(control->models, supported, sizeof(supported));
	if (model == C200_MODEL_UNKNOWN) {
		fprintf(stderr, "control %s is only supported on %s; %s is not a recognised Anker camera (USB %04x:%04x)\n",
			control->name, supported, device, usb_id.vendor_id, usb_id.product_id);
	} else {
		fprintf(stderr, "control %s is only supported on %s; %s is a %s (USB %04x:%04x)\n",
			control->name, supported, device, c200_model_name(model), usb_id.vendor_id, usb_id.product_id);
	}
	return -1;
}

static int v4l2_get_value(int fd, uint32_t id, int32_t *value)
{
	struct v4l2_control control = {.id = id};

	if (value == NULL) {
		errno = EINVAL;
		return -1;
	}

	if (ioctl(fd, VIDIOC_G_CTRL, &control) < 0) {
		return -1;
	}

	*value = control.value;
	return 0;
}

static int v4l2_set_value(int fd, uint32_t id, int32_t value)
{
	struct v4l2_control control = {.id = id, .value = value};

	return ioctl(fd, VIDIOC_S_CTRL, &control);
}

static int parse_i32(const char *text, int32_t *value)
{
	char *end = NULL;
	long parsed = 0;

	if (text == NULL || value == NULL) {
		return -1;
	}

	errno = 0;
	parsed = strtol(text, &end, 10);
	if (errno != 0 || end == text || *end != '\0') {
		errno = EINVAL;
		return -1;
	}

	*value = (int32_t)parsed;
	return 0;
}

static int handle_get_control(int fd, const struct c200_control_info *control)
{
	bool bool_value = false;
	uint8_t u8_value = 0;
	uint16_t u16_value = 0;
	int32_t v4l2_value = 0;
	struct c200_area_metering area_metering = {0};
	struct c200_enable_u8 enable_u8 = {0};

	switch (control->kind) {
	case C200_CONTROL_KIND_VENDOR_ENABLE_U8:
		if (c200_enable_u8_get(fd, (uint8_t)control->id, &enable_u8) < 0) {
			return -1;
		}
		printf("%s\n", c200_enable_u8_describe_value(&enable_u8));
		return 0;
	case C200_CONTROL_KIND_VENDOR_BOOL:
		if (c200_vendor_get_bool(fd, (uint8_t)control->id, &bool_value) < 0) {
			return -1;
		}
		printf("%s\n", c200_format_bool(bool_value));
		return 0;
	case C200_CONTROL_KIND_VENDOR_U8:
		if (c200_vendor_get_u8(fd, (uint8_t)control->id, &u8_value) < 0) {
			return -1;
		}
		printf("%u\n", u8_value);
		return 0;
	case C200_CONTROL_KIND_VENDOR_FOV:
		if (c200_fov_get(fd, &u16_value) < 0) {
			return -1;
		}
		printf("%s\n", c200_fov_describe_value(u16_value));
		return 0;
	case C200_CONTROL_KIND_VENDOR_AREA_METERING:
		if (c200_area_metering_get(fd, &area_metering) < 0) {
			return -1;
		}
		printf("%s\n", c200_area_metering_describe_value(&area_metering));
		return 0;
	case C200_CONTROL_KIND_V4L2_BOOL:
		if (v4l2_get_value(fd, control->id, &v4l2_value) < 0) {
			return -1;
		}
		printf("%s\n", c200_format_bool(v4l2_value != 0));
		return 0;
	case C200_CONTROL_KIND_V4L2_INT:
	case C200_CONTROL_KIND_V4L2_MENU:
		if (v4l2_get_value(fd, control->id, &v4l2_value) < 0) {
			return -1;
		}
		printf("%d\n", v4l2_value);
		return 0;
	}

	errno = EINVAL;
	return -1;
}

static int handle_set_control(int fd, const struct c200_control_info *control, const char *value_text)
{
	bool bool_value = false;
	uint8_t u8_value = 0;
	uint16_t u16_value = 0;
	int32_t v4l2_value = 0;
	struct c200_area_metering area_metering = {0};
	struct c200_enable_u8 enable_u8 = {0};

	switch (control->kind) {
	case C200_CONTROL_KIND_VENDOR_ENABLE_U8:
		/* Pre-fill with the current state so "on"/"off"/bare numbers keep the other field. */
		if (c200_enable_u8_get(fd, (uint8_t)control->id, &enable_u8) < 0) {
			return -1;
		}
		if (!c200_enable_u8_parse_value(value_text, &enable_u8, control->max_value)) {
			errno = EINVAL;
			return -1;
		}
		return c200_enable_u8_set(fd, (uint8_t)control->id, &enable_u8);
	case C200_CONTROL_KIND_VENDOR_BOOL:
		if (!c200_parse_bool(value_text, &bool_value)) {
			errno = EINVAL;
			return -1;
		}
		return c200_vendor_set_bool(fd, (uint8_t)control->id, bool_value);
	case C200_CONTROL_KIND_VENDOR_U8:
		if (parse_i32(value_text, &v4l2_value) < 0 || v4l2_value < 0 || v4l2_value > 255) {
			errno = EINVAL;
			return -1;
		}
		u8_value = (uint8_t)v4l2_value;
		return c200_vendor_set_u8(fd, (uint8_t)control->id, u8_value);
	case C200_CONTROL_KIND_VENDOR_FOV:
		if (!c200_fov_parse_value(value_text, &u16_value)) {
			errno = EINVAL;
			return -1;
		}
		return c200_fov_set(fd, u16_value);
	case C200_CONTROL_KIND_VENDOR_AREA_METERING:
		/* Pre-fill with the current rectangle so "on"/"off" alone keeps it. */
		if (c200_area_metering_get(fd, &area_metering) < 0) {
			return -1;
		}
		if (!c200_area_metering_parse_value(value_text, &area_metering)) {
			errno = EINVAL;
			return -1;
		}
		return c200_area_metering_set(fd, &area_metering);
	case C200_CONTROL_KIND_V4L2_BOOL:
		if (!c200_parse_bool(value_text, &bool_value)) {
			errno = EINVAL;
			return -1;
		}
		return v4l2_set_value(fd, control->id, bool_value ? 1 : 0);
	case C200_CONTROL_KIND_V4L2_INT:
	case C200_CONTROL_KIND_V4L2_MENU:
		if (parse_i32(value_text, &v4l2_value) < 0) {
			errno = EINVAL;
			return -1;
		}
		return v4l2_set_value(fd, control->id, v4l2_value);
	}

	errno = EINVAL;
	return -1;
}

/*
 * Controls flagged C200_CONTROL_FLAG_RESTARTS_CAMERA reboot the camera on
 * every write, even one that repeats the current value. Returns 1 if the
 * value is already set (the write can be skipped), 0 if not, -1 on error.
 */
static int restart_control_unchanged(int fd, const struct c200_control_info *control, const char *value_text)
{
	bool wanted = false;
	bool current = false;

	if (control->kind != C200_CONTROL_KIND_VENDOR_BOOL) {
		return 0;
	}
	if (!c200_parse_bool(value_text, &wanted)) {
		errno = EINVAL;
		return -1;
	}
	if (c200_vendor_get_bool(fd, (uint8_t)control->id, &current) < 0) {
		return -1;
	}
	return current == wanted;
}

/* The AnkerWork app refuses vertical mode unless the FOV is wide and HDR is off; say so. */
static void warn_vertical_screen_preconditions(int fd, const struct c200_control_info *control, const char *value_text)
{
	const struct c200_control_info *hdr = c200_find_control("hdr");
	bool wanted = false;
	bool hdr_on = false;
	uint16_t fov = 0;

	if (control->id != 0x0a || !c200_parse_bool(value_text, &wanted) || !wanted) {
		return;
	}
	if (c200_fov_get(fd, &fov) < 0 || hdr == NULL || c200_vendor_get_bool(fd, (uint8_t)hdr->id, &hdr_on) < 0) {
		return;
	}
	if (fov < 95 || hdr_on) {
		fprintf(stderr, "warning: AnkerWork only switches to vertical mode with fov wide (95) and hdr off; current fov %u, hdr %s\n",
			fov, c200_format_bool(hdr_on));
	}
}

/*
 * Write a control, absorbing the self-restart some controls trigger: the SET
 * fails with EPROTO as the camera drops off the bus, so wait for it to
 * re-enumerate and leave a fresh descriptor in *fd. Returns 0 on success,
 * 1 if the write was skipped because the value was already set, -1 on
 * failure (errno set; *fd may be -1 if the camera never came back).
 */
static int set_control_with_restart(int *fd, const char *device, const struct c200_control_info *control, const char *value_text)
{
	int unchanged = 0;
	int new_fd = -1;

	if (!(control->flags & C200_CONTROL_FLAG_RESTARTS_CAMERA)) {
		return handle_set_control(*fd, control, value_text);
	}

	unchanged = restart_control_unchanged(*fd, control, value_text);
	if (unchanged < 0) {
		return -1;
	}
	if (unchanged) {
		fprintf(stderr, "%s is already %s; not written because every write restarts the camera\n", control->name, value_text);
		return 1;
	}
	warn_vertical_screen_preconditions(*fd, control, value_text);

	if (handle_set_control(*fd, control, value_text) == 0) {
		return 0;
	}
	if (!c200_device_errno_means_gone(errno)) {
		return -1;
	}

	fprintf(stderr, "%s: the camera restarts itself to apply this; waiting for %s to come back...\n", control->name, device);
	close(*fd);
	*fd = -1;
	new_fd = c200_device_reopen(device, 8000);
	if (new_fd < 0) {
		return -1;
	}
	*fd = new_fd;
	return 0;
}

/*
 * Apply a settings profile: a text file of "control = value" lines as written by
 * the GUI (lines starting with '#' and the "model = ..." line are ignored).
 * Controls that are unknown, unsupported on this camera or rejected are reported
 * and skipped; the return value is the number of failures.
 */
static int apply_profile(int *fd, const char *device, const char *path)
{
	FILE *file = fopen(path, "r");
	char line[256];
	int line_number = 0;
	int applied = 0;
	int failed = 0;
	/* Controls that restart the camera go last, so the rest of the profile lands first. */
	struct {
		const struct c200_control_info *control;
		char value[64];
		int line_number;
	} deferred[4];
	int deferred_count = 0;
	int i = 0;

	if (file == NULL) {
		fprintf(stderr, "failed to open %s: %s\n", path, strerror(errno));
		return 1;
	}

	while (fgets(line, sizeof(line), file) != NULL) {
		char *key = line;
		char *value = NULL;
		char *end = NULL;
		const struct c200_control_info *control = NULL;

		++line_number;
		while (*key == ' ' || *key == '\t') {
			++key;
		}
		end = key + strlen(key);
		while (end > key && (end[-1] == '\n' || end[-1] == '\r' || end[-1] == ' ' || end[-1] == '\t')) {
			*--end = '\0';
		}
		if (*key == '\0' || *key == '#') {
			continue;
		}

		value = strchr(key, '=');
		if (value == NULL) {
			fprintf(stderr, "%s:%d: expected 'control = value'\n", path, line_number);
			++failed;
			continue;
		}
		*value++ = '\0';
		end = key + strlen(key);
		while (end > key && (end[-1] == ' ' || end[-1] == '\t')) {
			*--end = '\0';
		}
		while (*value == ' ' || *value == '\t') {
			++value;
		}

		if (strcasecmp(key, "model") == 0) {
			continue;
		}

		control = c200_find_control(key);
		if (control == NULL) {
			fprintf(stderr, "%s:%d: unknown control %s, skipped\n", path, line_number, key);
			++failed;
			continue;
		}
		if (check_control_supported(*fd, control, device) < 0) {
			++failed;
			continue;
		}
		if ((control->flags & C200_CONTROL_FLAG_RESTARTS_CAMERA) && deferred_count < (int)(sizeof(deferred) / sizeof(deferred[0])) &&
		    strlen(value) < sizeof(deferred[0].value)) {
			deferred[deferred_count].control = control;
			strcpy(deferred[deferred_count].value, value);
			deferred[deferred_count].line_number = line_number;
			++deferred_count;
			continue;
		}
		if (handle_set_control(*fd, control, value) < 0) {
			fprintf(stderr, "%s:%d: failed to set %s to '%s': %s\n", path, line_number, control->name, value, strerror(errno));
			++failed;
			continue;
		}
		++applied;
	}

	fclose(file);

	for (i = 0; i < deferred_count; ++i) {
		int rc = set_control_with_restart(fd, device, deferred[i].control, deferred[i].value);

		if (rc < 0) {
			fprintf(stderr, "%s:%d: failed to set %s to '%s': %s\n", path, deferred[i].line_number, deferred[i].control->name,
				deferred[i].value, strerror(errno));
			++failed;
			if (*fd < 0) {
				break;
			}
		} else if (rc == 0) {
			++applied;
		}
	}
	printf("applied %d control%s from %s%s\n", applied, applied == 1 ? "" : "s", path, failed ? " (with failures)" : "");
	return failed;
}

static void list_controls(void)
{
	size_t count = 0;
	size_t i = 0;
	const struct c200_control_info *controls = c200_all_controls(&count);

	for (i = 0; i < count; ++i) {
		char models[64];

		c200_model_mask_describe(controls[i].models, models, sizeof(models));
		printf("%-28s %-32s %s\n", controls[i].name, controls[i].models == C200_MODEL_ANY ? "all" : models, controls[i].description);
	}
}

int main(int argc, char **argv)
{
	const char *device = "/dev/video0";
	const char *command = NULL;
	const struct c200_control_info *control = NULL;
	const char *value_text = NULL;
	char joined_value[128];
	int argi = 1;
	int fd = -1;

	while (argi < argc && strcmp(argv[argi], "--device") == 0) {
		if (argi + 1 >= argc) {
			usage(stderr, argv[0]);
			return 2;
		}
		device = argv[argi + 1];
		argi += 2;
	}

	if (argi >= argc) {
		usage(stderr, argv[0]);
		return 2;
	}

	command = argv[argi++];
	if (strcmp(command, "list-controls") == 0) {
		if (argi != argc) {
			usage(stderr, argv[0]);
			return 2;
		}

		list_controls();
		return 0;
	}

	if (strcmp(command, "apply-profile") == 0) {
		int failures = 0;

		if (argi + 1 != argc) {
			usage(stderr, argv[0]);
			return 2;
		}

		fd = open_device(device, O_RDWR);
		if (fd < 0) {
			return 1;
		}

		failures = apply_profile(&fd, device, argv[argi]);
		if (fd >= 0) {
			close(fd);
		}
		return failures ? 1 : 0;
	}

	if (strcmp(command, "get-fov") == 0) {
		command = "get";
		control = c200_find_control("fov");
	}

	if (strcmp(command, "set-fov") == 0) {
		command = "set";
		control = c200_find_control("fov");
	}

	if (strcmp(command, "get") == 0) {
		if (control == NULL) {
			if (argi + 1 != argc) {
				usage(stderr, argv[0]);
				return 2;
			}
			control = c200_find_control(argv[argi]);
		}

		if (control == NULL) {
			fprintf(stderr, "unknown control: %s\n", argv[argi]);
			return 2;
		}

		fd = open_device(device, O_RDWR);
		if (fd < 0) {
			return 1;
		}

		if (check_control_supported(fd, control, device) < 0) {
			close(fd);
			return 1;
		}

		if (handle_get_control(fd, control) < 0) {
			fprintf(stderr, "failed to read %s from %s: %s\n", control->name, device, strerror(errno));
			close(fd);
			return 1;
		}

		close(fd);
		return 0;
	}

	if (strcmp(command, "set") == 0) {
		if (control == NULL) {
			if (argi + 2 > argc) {
				usage(stderr, argv[0]);
				return 2;
			}
			control = c200_find_control(argv[argi]);
			value_text = argv[argi + 1];
			if (argi + 2 != argc) {
				/* Allow "set exposure_smoothing on 1600 900 640 360" or "set exposure_compensation on 20" as separate words. */
				int extra = 0;
				size_t used = 0;

				if (control == NULL ||
				    (control->kind != C200_CONTROL_KIND_VENDOR_AREA_METERING &&
				     control->kind != C200_CONTROL_KIND_VENDOR_ENABLE_U8)) {
					usage(stderr, argv[0]);
					return 2;
				}
				joined_value[0] = '\0';
				for (extra = argi + 1; extra < argc; ++extra) {
					size_t needed = strlen(argv[extra]) + 2;

					if (used + needed >= sizeof(joined_value)) {
						usage(stderr, argv[0]);
						return 2;
					}
					if (used > 0) {
						joined_value[used++] = ',';
					}
					memcpy(joined_value + used, argv[extra], strlen(argv[extra]) + 1);
					used += strlen(argv[extra]);
				}
				value_text = joined_value;
			}
		} else {
			if (argi + 1 != argc) {
				usage(stderr, argv[0]);
				return 2;
			}
			value_text = argv[argi];
		}

		if (control == NULL) {
			fprintf(stderr, "unknown control: %s\n", argv[argi]);
			return 2;
		}

		fd = open_device(device, O_RDWR);
		if (fd < 0) {
			return 1;
		}

		if (check_control_supported(fd, control, device) < 0) {
			close(fd);
			return 1;
		}

		if (set_control_with_restart(&fd, device, control, value_text) < 0) {
			fprintf(stderr, "failed to set %s on %s: %s\n", control->name, device, strerror(errno));
			if (fd >= 0) {
				close(fd);
			}
			return 1;
		}

		if (handle_get_control(fd, control) < 0) {
			fprintf(stderr, "warning: set succeeded but readback for %s failed: %s\n", control->name, strerror(errno));
			close(fd);
			return 1;
		}

		close(fd);
		return 0;
	}

	usage(stderr, argv[0]);
	return 2;
}
