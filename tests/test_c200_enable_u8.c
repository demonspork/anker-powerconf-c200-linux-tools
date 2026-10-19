#include "c200_enable_u8.h"

#include <assert.h>
#include <string.h>

static void test_parse_toggle_keeps_value(void)
{
	struct c200_enable_u8 value = {.enable = false, .value = 30};

	assert(c200_enable_u8_parse_value("on", &value, 60));
	assert(value.enable && value.value == 30);

	assert(c200_enable_u8_parse_value("off", &value, 60));
	assert(!value.enable && value.value == 30);
}

static void test_parse_toggle_with_value(void)
{
	struct c200_enable_u8 value = {0};

	assert(c200_enable_u8_parse_value("on,20", &value, 60));
	assert(value.enable && value.value == 20);

	assert(c200_enable_u8_parse_value("on 45", &value, 60));
	assert(value.enable && value.value == 45);

	assert(c200_enable_u8_parse_value("off=0", &value, 60));
	assert(!value.enable && value.value == 0);
}

static void test_parse_bare_number_keeps_enable(void)
{
	struct c200_enable_u8 value = {.enable = true, .value = 30};

	assert(c200_enable_u8_parse_value("15", &value, 60));
	assert(value.enable && value.value == 15);
}

static void test_parse_rejects_bad_input(void)
{
	struct c200_enable_u8 value = {0};

	assert(!c200_enable_u8_parse_value("banana", &value, 60));
	assert(!c200_enable_u8_parse_value("on,61", &value, 60));
	assert(!c200_enable_u8_parse_value("on,1,2", &value, 60));
	assert(!c200_enable_u8_parse_value("", &value, 60));
	assert(!c200_enable_u8_parse_value("on,x", &value, 60));
}

static void test_describe_value(void)
{
	struct c200_enable_u8 value = {.enable = true, .value = 30};

	assert(strcmp(c200_enable_u8_describe_value(&value), "on value=30") == 0);
	value.enable = false;
	assert(strcmp(c200_enable_u8_describe_value(&value), "off value=30") == 0);
}

int main(void)
{
	test_parse_toggle_keeps_value();
	test_parse_toggle_with_value();
	test_parse_bare_number_keeps_enable();
	test_parse_rejects_bad_input();
	test_describe_value();
	return 0;
}
