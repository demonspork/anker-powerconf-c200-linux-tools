#ifndef C200_AREA_METERING_H
#define C200_AREA_METERING_H

#include <stdbool.h>
#include <stdint.h>

/*
 * Vendor "Exposure Smoothing" control (internally "Area Metering" in the
 * AnkerWork app). Confirmed by disassembling AWDeviceInterface.dll 3.0.4:
 * setAreaMetering()/getAreaMetering() use extension unit 0x06, selector 0x18,
 * and a 9-byte payload:
 *
 *   byte 0   : enable (0x00 / 0x01)
 *   bytes 1-2: x      uint16_le
 *   bytes 3-4: y      uint16_le
 *   bytes 5-6: width  uint16_le
 *   bytes 7-8: height uint16_le
 *
 * Coordinates are expressed on a fixed 3840x2160 grid regardless of the
 * streaming resolution. The app's default region is 640x360 centered.
 */
#define C200_AREA_METERING_SELECTOR 0x18
#define C200_AREA_METERING_PAYLOAD_SIZE 9

#define C200_AREA_METERING_GRID_WIDTH 3840
#define C200_AREA_METERING_GRID_HEIGHT 2160

#define C200_AREA_METERING_DEFAULT_WIDTH 640
#define C200_AREA_METERING_DEFAULT_HEIGHT 360
#define C200_AREA_METERING_DEFAULT_X ((C200_AREA_METERING_GRID_WIDTH - C200_AREA_METERING_DEFAULT_WIDTH) / 2)
#define C200_AREA_METERING_DEFAULT_Y ((C200_AREA_METERING_GRID_HEIGHT - C200_AREA_METERING_DEFAULT_HEIGHT) / 2)

struct c200_area_metering {
	bool enable;
	uint16_t x;
	uint16_t y;
	uint16_t width;
	uint16_t height;
};

int c200_area_metering_get(int fd, struct c200_area_metering *value);
int c200_area_metering_set(int fd, const struct c200_area_metering *value);

void c200_area_metering_encode_payload(uint8_t payload[C200_AREA_METERING_PAYLOAD_SIZE], const struct c200_area_metering *value);
void c200_area_metering_decode_payload(const uint8_t payload[C200_AREA_METERING_PAYLOAD_SIZE], struct c200_area_metering *value);

/*
 * Parses "on", "off", or "on[,x,y,width,height]" / "on x y width height".
 * When only "on" is given, *value keeps whatever rectangle it already holds
 * (callers pre-fill it with the current device state) and falls back to the
 * app default if that rectangle is empty.
 */
bool c200_area_metering_parse_value(const char *text, struct c200_area_metering *value);
bool c200_area_metering_rect_is_valid(const struct c200_area_metering *value);
const char *c200_area_metering_describe_value(const struct c200_area_metering *value);

#endif
