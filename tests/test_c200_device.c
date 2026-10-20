#define _DEFAULT_SOURCE
#include "c200_controls.h"
#include "c200_device.h"

#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static void test_model_from_usb_id(void)
{
	struct c200_usb_id c200 = {.vendor_id = 0x291a, .product_id = 0x3369};
	struct c200_usb_id c310 = {.vendor_id = 0x291a, .product_id = 0x3367};
	struct c200_usb_id other_anker = {.vendor_id = 0x291a, .product_id = 0x0001};
	struct c200_usb_id other_vendor = {.vendor_id = 0x046d, .product_id = 0x3367};

	assert(c200_model_from_usb_id(&c200) == C200_MODEL_C200);
	assert(c200_model_from_usb_id(&c310) == C200_MODEL_C310);
	assert(c200_model_from_usb_id(&other_anker) == C200_MODEL_UNKNOWN);
	assert(c200_model_from_usb_id(&other_vendor) == C200_MODEL_UNKNOWN);
	assert(c200_model_from_usb_id(NULL) == C200_MODEL_UNKNOWN);
}

static void test_model_names(void)
{
	char text[64];

	assert(strcmp(c200_model_name(C200_MODEL_C200), "PowerConf C200") == 0);
	assert(strcmp(c200_model_name(C200_MODEL_C310), "AnkerWork C310") == 0);

	c200_model_mask_describe(C200_MODEL_C310, text, sizeof(text));
	assert(strcmp(text, "AnkerWork C310") == 0);
	c200_model_mask_describe(C200_MODEL_ANY, text, sizeof(text));
	assert(strcmp(text, "PowerConf C200, AnkerWork C310") == 0);
	c200_model_mask_describe(0, text, sizeof(text));
	assert(strcmp(text, "unknown") == 0);
}

static void test_control_gating(void)
{
	static const char *const C310_ONLY[] = {"exposure_smoothing", "area_metering", "exposure_compensation", "face_exposure"};
	static const char *const SHARED[] = {"fov", "hdr", "horizontal_flip", "vertical_screen", "anti_flicker", "brightness", "zoom_absolute"};
	size_t i = 0;
	size_t count = 0;
	const struct c200_control_info *all = c200_all_controls(&count);

	for (i = 0; i < sizeof(C310_ONLY) / sizeof(C310_ONLY[0]); ++i) {
		const struct c200_control_info *control = c200_find_control(C310_ONLY[i]);

		assert(control != NULL);
		assert(c200_control_supports_model(control, C200_MODEL_C310));
		assert(!c200_control_supports_model(control, C200_MODEL_C200));
		assert(!c200_control_supports_model(control, C200_MODEL_UNKNOWN));
	}

	for (i = 0; i < sizeof(SHARED) / sizeof(SHARED[0]); ++i) {
		const struct c200_control_info *control = c200_find_control(SHARED[i]);

		assert(control != NULL);
		assert(c200_control_supports_model(control, C200_MODEL_C310));
		assert(c200_control_supports_model(control, C200_MODEL_C200));
	}

	/* Every table entry must declare at least one model; a zero mask would be unreachable. */
	for (i = 0; i < count; ++i) {
		assert(all[i].models != 0);
	}

	/* The C310-only selectors must never be sent to a C200. */
	for (i = 0; i < count; ++i) {
		if (all[i].kind == C200_CONTROL_KIND_VENDOR_AREA_METERING || all[i].kind == C200_CONTROL_KIND_VENDOR_ENABLE_U8) {
			assert((all[i].models & C200_MODEL_C200) == 0);
		}
	}

	assert(!c200_control_supports_model(NULL, C200_MODEL_C310));
}

static void write_file(const char *dir, const char *name, const char *content)
{
	char path[512];
	FILE *file = NULL;

	snprintf(path, sizeof(path), "%s/%s", dir, name);
	file = fopen(path, "w");
	assert(file != NULL);
	fputs(content, file);
	fclose(file);
}

static void test_read_usb_id_from_dir(void)
{
	char dir[] = "/tmp/c200_device_test_XXXXXX";
	char path[512];
	struct c200_usb_id id = {0};

	assert(mkdtemp(dir) != NULL);

	/* Missing files -> failure. */
	assert(c200_device_read_usb_id_from_dir(dir, &id) < 0);

	/* sysfs style: lower-case hex followed by a newline. */
	write_file(dir, "idVendor", "291a\n");
	write_file(dir, "idProduct", "3367\n");
	assert(c200_device_read_usb_id_from_dir(dir, &id) == 0);
	assert(id.vendor_id == 0x291a && id.product_id == 0x3367);
	assert(c200_model_from_usb_id(&id) == C200_MODEL_C310);

	write_file(dir, "idProduct", "3369\n");
	assert(c200_device_read_usb_id_from_dir(dir, &id) == 0);
	assert(c200_model_from_usb_id(&id) == C200_MODEL_C200);

	write_file(dir, "idProduct", "zz\n");
	assert(c200_device_read_usb_id_from_dir(dir, &id) < 0);

	assert(c200_device_read_usb_id_from_dir(NULL, &id) < 0);
	assert(c200_device_read_usb_id_from_dir(dir, NULL) < 0);

	snprintf(path, sizeof(path), "%s/idVendor", dir);
	unlink(path);
	snprintf(path, sizeof(path), "%s/idProduct", dir);
	unlink(path);
	rmdir(dir);
}

static void test_get_usb_id_rejects_non_device(void)
{
	struct c200_usb_id id = {0};
	int fd = open("/dev/null", O_RDONLY);

	/* /dev/null is a char device but has no USB parent in sysfs. */
	assert(fd >= 0);
	assert(c200_device_get_usb_id(fd, &id) < 0);
	close(fd);

	assert(c200_device_get_usb_id(-1, &id) < 0);
	assert(c200_device_get_usb_id(0, NULL) < 0);
}

static void test_restart_helpers(void)
{
	const struct c200_control_info *vertical = c200_find_control("vertical_screen");
	const struct c200_control_info *hdr = c200_find_control("hdr");

	assert(vertical != NULL && (vertical->flags & C200_CONTROL_FLAG_RESTARTS_CAMERA));
	assert(hdr != NULL && hdr->flags == 0);

	assert(c200_device_errno_means_gone(ENODEV));
	assert(c200_device_errno_means_gone(EPROTO));
	assert(!c200_device_errno_means_gone(EINVAL));
	assert(!c200_device_errno_means_gone(0));

	errno = 0;
	assert(c200_device_reopen("/nonexistent/anker-webcam-video", 200) == -1);
	assert(errno == ETIMEDOUT);
}

int main(void)
{
	test_model_from_usb_id();
	test_model_names();
	test_control_gating();
	test_read_usb_id_from_dir();
	test_get_usb_id_rejects_non_device();
	test_restart_helpers();
	return 0;
}
