#ifndef C200_ENABLE_U8_H
#define C200_ENABLE_U8_H

#include <stdbool.h>
#include <stdint.h>

/*
 * Two-byte vendor controls of the form { enable, value }, as used by the
 * AnkerWork app for:
 *
 *   selector 0x19: setAutoExposure(enable, compensation)   value 0..60, app default 30
 *   selector 0x0b: setAutoFaceExposure(enable, value)      value 0..60
 *
 * Layout confirmed by disassembling AWDeviceInterface.dll 3.0.4.
 */
#define C200_ENABLE_U8_PAYLOAD_SIZE 2

struct c200_enable_u8 {
	bool enable;
	uint8_t value;
};

int c200_enable_u8_get(int fd, uint8_t selector, struct c200_enable_u8 *value);
int c200_enable_u8_set(int fd, uint8_t selector, const struct c200_enable_u8 *value);

/*
 * Parses "on", "off", "on,30", "on 30", or a bare number (keeps the current
 * enable flag). *value should be pre-filled with the current device state so
 * that fields which are not mentioned are preserved.
 */
bool c200_enable_u8_parse_value(const char *text, struct c200_enable_u8 *value, uint8_t max_value);
const char *c200_enable_u8_describe_value(const struct c200_enable_u8 *value);

#endif
