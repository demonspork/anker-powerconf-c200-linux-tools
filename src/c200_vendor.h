#ifndef C200_VENDOR_H
#define C200_VENDOR_H

#include <stdbool.h>
#include <stdint.h>

#define C200_VENDOR_UNIT_ID 0x06
#define C200_VENDOR_MAX_PAYLOAD_SIZE 64

int c200_vendor_get_bool(int fd, uint8_t selector, bool *value);
int c200_vendor_set_bool(int fd, uint8_t selector, bool value);
int c200_vendor_get_u8(int fd, uint8_t selector, uint8_t *value);
int c200_vendor_set_u8(int fd, uint8_t selector, uint8_t value);
int c200_vendor_get_u16(int fd, uint8_t selector, uint16_t *value, uint16_t value_offset);
int c200_vendor_set_u16(int fd, uint8_t selector, uint16_t value, uint16_t value_offset);

/*
 * Raw access for multi-field controls. get_raw fills payload with the current
 * value and reports the device's control length in *size. set_raw writes the
 * first size bytes of payload, which must be the length reported by get_raw.
 */
int c200_vendor_get_raw(int fd, uint8_t selector, uint8_t payload[C200_VENDOR_MAX_PAYLOAD_SIZE], uint16_t *size);
int c200_vendor_set_raw(int fd, uint8_t selector, const uint8_t payload[C200_VENDOR_MAX_PAYLOAD_SIZE], uint16_t size);

#endif
