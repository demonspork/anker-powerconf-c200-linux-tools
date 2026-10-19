#include "c200_enable_u8.h"
#include "c200_controls.h"
#include "c200_vendor.h"

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int c200_enable_u8_get(int fd, uint8_t selector, struct c200_enable_u8 *value)
{
	uint8_t payload[C200_VENDOR_MAX_PAYLOAD_SIZE];
	uint16_t size = 0;

	if (value == NULL) {
		errno = EINVAL;
		return -1;
	}

	if (c200_vendor_get_raw(fd, selector, payload, &size) < 0) {
		return -1;
	}

	if (size < C200_ENABLE_U8_PAYLOAD_SIZE) {
		errno = EMSGSIZE;
		return -1;
	}

	value->enable = payload[0] == 0x01;
	value->value = payload[1];
	return 0;
}

int c200_enable_u8_set(int fd, uint8_t selector, const struct c200_enable_u8 *value)
{
	uint8_t payload[C200_VENDOR_MAX_PAYLOAD_SIZE];
	uint16_t size = 0;

	if (value == NULL) {
		errno = EINVAL;
		return -1;
	}

	if (c200_vendor_get_raw(fd, selector, payload, &size) < 0) {
		return -1;
	}

	if (size < C200_ENABLE_U8_PAYLOAD_SIZE) {
		errno = EMSGSIZE;
		return -1;
	}

	memset(payload, 0, sizeof(payload));
	payload[0] = value->enable ? 0x01 : 0x00;
	payload[1] = value->value;
	return c200_vendor_set_raw(fd, selector, payload, size);
}

static bool parse_u8(const char *text, uint8_t max_value, uint8_t *value)
{
	char *end = NULL;
	unsigned long parsed = 0;

	if (text == NULL || *text == '\0') {
		return false;
	}

	errno = 0;
	parsed = strtoul(text, &end, 10);
	if (errno != 0 || end == text || *end != '\0' || parsed > max_value) {
		return false;
	}

	*value = (uint8_t)parsed;
	return true;
}

bool c200_enable_u8_parse_value(const char *text, struct c200_enable_u8 *value, uint8_t max_value)
{
	char buffer[64];
	char *fields[2] = {0};
	size_t count = 0;
	char *cursor = NULL;
	bool enable = false;
	uint8_t number = 0;

	if (text == NULL || value == NULL || strlen(text) >= sizeof(buffer)) {
		return false;
	}

	strcpy(buffer, text);
	cursor = buffer;
	while (*cursor != '\0') {
		cursor += strspn(cursor, ", :=");
		if (*cursor == '\0') {
			break;
		}
		if (count >= 2) {
			return false;
		}
		fields[count++] = cursor;
		cursor += strcspn(cursor, ", :=");
		if (*cursor != '\0') {
			*cursor++ = '\0';
		}
	}

	if (count == 0) {
		return false;
	}

	if (c200_parse_bool(fields[0], &enable)) {
		value->enable = enable;
		if (count == 2) {
			if (!parse_u8(fields[1], max_value, &number)) {
				return false;
			}
			value->value = number;
		}
		return true;
	}

	/* A bare number: adjust the value and leave the enable flag as-is. */
	if (count != 1 || !parse_u8(fields[0], max_value, &number)) {
		return false;
	}

	value->value = number;
	return true;
}

const char *c200_enable_u8_describe_value(const struct c200_enable_u8 *value)
{
	static char buffer[32];

	snprintf(buffer, sizeof(buffer), "%s value=%u", value->enable ? "on" : "off", value->value);
	return buffer;
}
