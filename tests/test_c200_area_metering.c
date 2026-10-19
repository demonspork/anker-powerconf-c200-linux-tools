#include "c200_area_metering.h"

#include <assert.h>
#include <stdint.h>
#include <string.h>

static void test_encode_payload(void)
{
	uint8_t payload[C200_AREA_METERING_PAYLOAD_SIZE];
	struct c200_area_metering value = {.enable = true, .x = 1600, .y = 900, .width = 640, .height = 360};

	memset(payload, 0xaa, sizeof(payload));
	c200_area_metering_encode_payload(payload, &value);

	/* Layout taken from AWDeviceInterface.dll setAreaMetering(): */
	assert(payload[0] == 0x01);
	assert(payload[1] == 0x40 && payload[2] == 0x06); /* 1600 */
	assert(payload[3] == 0x84 && payload[4] == 0x03); /* 900 */
	assert(payload[5] == 0x80 && payload[6] == 0x02); /* 640 */
	assert(payload[7] == 0x68 && payload[8] == 0x01); /* 360 */
}

static void test_decode_payload(void)
{
	const uint8_t payload[C200_AREA_METERING_PAYLOAD_SIZE] = {0x01, 0x40, 0x06, 0x84, 0x03, 0x80, 0x02, 0x68, 0x01};
	struct c200_area_metering value = {0};

	c200_area_metering_decode_payload(payload, &value);
	assert(value.enable);
	assert(value.x == 1600);
	assert(value.y == 900);
	assert(value.width == 640);
	assert(value.height == 360);
}

static void test_parse_enable_keeps_existing_rect(void)
{
	struct c200_area_metering value = {.enable = false, .x = 100, .y = 200, .width = 300, .height = 400};

	assert(c200_area_metering_parse_value("on", &value));
	assert(value.enable);
	assert(value.x == 100 && value.y == 200 && value.width == 300 && value.height == 400);

	assert(c200_area_metering_parse_value("off", &value));
	assert(!value.enable);
	assert(value.x == 100 && value.y == 200 && value.width == 300 && value.height == 400);
}

static void test_parse_enable_defaults_empty_rect(void)
{
	struct c200_area_metering value = {0};

	assert(c200_area_metering_parse_value("on", &value));
	assert(value.enable);
	assert(value.x == C200_AREA_METERING_DEFAULT_X);
	assert(value.y == C200_AREA_METERING_DEFAULT_Y);
	assert(value.width == C200_AREA_METERING_DEFAULT_WIDTH);
	assert(value.height == C200_AREA_METERING_DEFAULT_HEIGHT);
	assert(value.x == 1600 && value.y == 900);
}

static void test_parse_full_rect(void)
{
	struct c200_area_metering value = {0};

	assert(c200_area_metering_parse_value("on,10,20,30,40", &value));
	assert(value.enable && value.x == 10 && value.y == 20 && value.width == 30 && value.height == 40);

	assert(c200_area_metering_parse_value("off 0 0 3840 2160", &value));
	assert(!value.enable && value.x == 0 && value.y == 0 && value.width == 3840 && value.height == 2160);

	assert(c200_area_metering_parse_value("1,100,100,200x100", &value));
	assert(value.enable && value.width == 200 && value.height == 100);
}

static void test_parse_rejects_bad_input(void)
{
	struct c200_area_metering value = {0};

	assert(!c200_area_metering_parse_value("banana", &value));
	assert(!c200_area_metering_parse_value("on,1,2", &value));
	assert(!c200_area_metering_parse_value("on,1,2,3,4,5", &value));
	assert(!c200_area_metering_parse_value("on,0,0,0,10", &value));
	assert(!c200_area_metering_parse_value("on,3800,0,100,100", &value));
	assert(!c200_area_metering_parse_value("on,0,2100,100,100", &value));
	assert(!c200_area_metering_parse_value("on,a,b,c,d", &value));
}

static void test_describe_value(void)
{
	struct c200_area_metering value = {.enable = true, .x = 1600, .y = 900, .width = 640, .height = 360};

	assert(strcmp(c200_area_metering_describe_value(&value), "on x=1600 y=900 width=640 height=360") == 0);
	value.enable = false;
	assert(strcmp(c200_area_metering_describe_value(&value), "off x=1600 y=900 width=640 height=360") == 0);
}

int main(void)
{
	test_encode_payload();
	test_decode_payload();
	test_parse_enable_keeps_existing_rect();
	test_parse_enable_defaults_empty_rect();
	test_parse_full_rect();
	test_parse_rejects_bad_input();
	test_describe_value();
	return 0;
}
