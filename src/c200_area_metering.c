#include "c200_area_metering.h"
#include "c200_vendor.h"

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

int c200_area_metering_get(int fd, struct c200_area_metering *value)
{
	uint8_t payload[C200_VENDOR_MAX_PAYLOAD_SIZE];
	uint16_t size = 0;

	if (value == NULL) {
		errno = EINVAL;
		return -1;
	}

	if (c200_vendor_get_raw(fd, C200_AREA_METERING_SELECTOR, payload, &size) < 0) {
		return -1;
	}

	if (size < C200_AREA_METERING_PAYLOAD_SIZE) {
		errno = EMSGSIZE;
		return -1;
	}

	c200_area_metering_decode_payload(payload, value);
	return 0;
}

int c200_area_metering_set(int fd, const struct c200_area_metering *value)
{
	uint8_t payload[C200_VENDOR_MAX_PAYLOAD_SIZE];
	uint16_t size = 0;

	if (value == NULL) {
		errno = EINVAL;
		return -1;
	}

	if (c200_vendor_get_raw(fd, C200_AREA_METERING_SELECTOR, payload, &size) < 0) {
		return -1;
	}

	if (size < C200_AREA_METERING_PAYLOAD_SIZE) {
		errno = EMSGSIZE;
		return -1;
	}

	memset(payload, 0, sizeof(payload));
	c200_area_metering_encode_payload(payload, value);
	return c200_vendor_set_raw(fd, C200_AREA_METERING_SELECTOR, payload, size);
}

void c200_area_metering_encode_payload(uint8_t payload[C200_AREA_METERING_PAYLOAD_SIZE], const struct c200_area_metering *value)
{
	payload[0] = value->enable ? 0x01 : 0x00;
	payload[1] = value->x & 0xff;
	payload[2] = value->x >> 8;
	payload[3] = value->y & 0xff;
	payload[4] = value->y >> 8;
	payload[5] = value->width & 0xff;
	payload[6] = value->width >> 8;
	payload[7] = value->height & 0xff;
	payload[8] = value->height >> 8;
}

void c200_area_metering_decode_payload(const uint8_t payload[C200_AREA_METERING_PAYLOAD_SIZE], struct c200_area_metering *value)
{
	value->enable = payload[0] == 0x01;
	value->x = (uint16_t)payload[1] | ((uint16_t)payload[2] << 8);
	value->y = (uint16_t)payload[3] | ((uint16_t)payload[4] << 8);
	value->width = (uint16_t)payload[5] | ((uint16_t)payload[6] << 8);
	value->height = (uint16_t)payload[7] | ((uint16_t)payload[8] << 8);
}

bool c200_area_metering_rect_is_valid(const struct c200_area_metering *value)
{
	if (value->width == 0 || value->height == 0) {
		return false;
	}

	if ((uint32_t)value->x + value->width > C200_AREA_METERING_GRID_WIDTH) {
		return false;
	}

	if ((uint32_t)value->y + value->height > C200_AREA_METERING_GRID_HEIGHT) {
		return false;
	}

	return true;
}

static bool parse_u16(const char *text, uint16_t *value)
{
	char *end = NULL;
	unsigned long parsed = 0;

	if (text == NULL || *text == '\0') {
		return false;
	}

	errno = 0;
	parsed = strtoul(text, &end, 10);
	if (errno != 0 || end == text || *end != '\0' || parsed > UINT16_MAX) {
		return false;
	}

	*value = (uint16_t)parsed;
	return true;
}

static bool parse_enable(const char *text, bool *enable)
{
	if (strcasecmp(text, "1") == 0 || strcasecmp(text, "on") == 0 || strcasecmp(text, "true") == 0 || strcasecmp(text, "yes") == 0) {
		*enable = true;
		return true;
	}

	if (strcasecmp(text, "0") == 0 || strcasecmp(text, "off") == 0 || strcasecmp(text, "false") == 0 || strcasecmp(text, "no") == 0) {
		*enable = false;
		return true;
	}

	return false;
}

bool c200_area_metering_parse_value(const char *text, struct c200_area_metering *value)
{
	char buffer[128];
	char *fields[5] = {0};
	size_t count = 0;
	char *cursor = NULL;
	char *token = NULL;
	bool enable = false;

	if (text == NULL || value == NULL || strlen(text) >= sizeof(buffer)) {
		return false;
	}

	strcpy(buffer, text);
	cursor = buffer;
	while (*cursor != '\0') {
		/* Separators: comma, space, colon, or 'x' (as in "640x360"). */
		cursor += strspn(cursor, ", :x");
		if (*cursor == '\0') {
			break;
		}
		token = cursor;
		cursor += strcspn(cursor, ", :x");
		if (*cursor != '\0') {
			*cursor++ = '\0';
		}
		if (count >= 5) {
			return false;
		}
		fields[count++] = token;
	}

	if (count != 1 && count != 5) {
		return false;
	}

	if (!parse_enable(fields[0], &enable)) {
		return false;
	}

	if (count == 5) {
		struct c200_area_metering parsed = {.enable = enable};

		if (!parse_u16(fields[1], &parsed.x) || !parse_u16(fields[2], &parsed.y) ||
		    !parse_u16(fields[3], &parsed.width) || !parse_u16(fields[4], &parsed.height)) {
			return false;
		}

		if (!c200_area_metering_rect_is_valid(&parsed)) {
			return false;
		}

		*value = parsed;
		return true;
	}

	value->enable = enable;
	if (!c200_area_metering_rect_is_valid(value)) {
		value->x = C200_AREA_METERING_DEFAULT_X;
		value->y = C200_AREA_METERING_DEFAULT_Y;
		value->width = C200_AREA_METERING_DEFAULT_WIDTH;
		value->height = C200_AREA_METERING_DEFAULT_HEIGHT;
	}

	return true;
}

const char *c200_area_metering_describe_value(const struct c200_area_metering *value)
{
	static char buffer[96];

	snprintf(buffer, sizeof(buffer), "%s x=%u y=%u width=%u height=%u", value->enable ? "on" : "off",
		 value->x, value->y, value->width, value->height);
	return buffer;
}
