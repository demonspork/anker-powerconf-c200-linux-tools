CC ?= cc
CFLAGS ?= -O2 -Wall -Wextra -Werror -std=c11

BUILD_DIR := build
SRC_DIR := src
TEST_DIR := tests

COMMON_SRCS := $(SRC_DIR)/c200_fov.c
COMMON_SRCS += $(SRC_DIR)/c200_vendor.c $(SRC_DIR)/c200_controls.c
COMMON_SRCS += $(SRC_DIR)/c200_area_metering.c $(SRC_DIR)/c200_enable_u8.c
COMMON_SRCS += $(SRC_DIR)/c200_device.c
CLI_SRCS := $(COMMON_SRCS) $(SRC_DIR)/c200_fov_cli.c
TEST_SRCS := $(COMMON_SRCS) $(TEST_DIR)/test_c200_fov.c
AREA_TEST_SRCS := $(COMMON_SRCS) $(TEST_DIR)/test_c200_area_metering.c
ENABLE_U8_TEST_SRCS := $(COMMON_SRCS) $(TEST_DIR)/test_c200_enable_u8.c
DEVICE_TEST_SRCS := $(COMMON_SRCS) $(TEST_DIR)/test_c200_device.c

.PHONY: all test clean

all: $(BUILD_DIR)/anker-powerconf-c200-linux-tools

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(BUILD_DIR)/anker-powerconf-c200-linux-tools: $(CLI_SRCS) | $(BUILD_DIR)
	$(CC) $(CFLAGS) -I$(SRC_DIR) $(CLI_SRCS) -o $@

$(BUILD_DIR)/test_c200_fov: $(TEST_SRCS) | $(BUILD_DIR)
	$(CC) $(CFLAGS) -I$(SRC_DIR) $(TEST_SRCS) -o $@

$(BUILD_DIR)/test_c200_area_metering: $(AREA_TEST_SRCS) | $(BUILD_DIR)
	$(CC) $(CFLAGS) -I$(SRC_DIR) $(AREA_TEST_SRCS) -o $@

$(BUILD_DIR)/test_c200_enable_u8: $(ENABLE_U8_TEST_SRCS) | $(BUILD_DIR)
	$(CC) $(CFLAGS) -I$(SRC_DIR) $(ENABLE_U8_TEST_SRCS) -o $@

$(BUILD_DIR)/test_c200_device: $(DEVICE_TEST_SRCS) | $(BUILD_DIR)
	$(CC) $(CFLAGS) -I$(SRC_DIR) $(DEVICE_TEST_SRCS) -o $@

test: $(BUILD_DIR)/test_c200_fov $(BUILD_DIR)/test_c200_area_metering $(BUILD_DIR)/test_c200_enable_u8 $(BUILD_DIR)/test_c200_device
	$(BUILD_DIR)/test_c200_fov
	$(BUILD_DIR)/test_c200_area_metering
	$(BUILD_DIR)/test_c200_enable_u8
	$(BUILD_DIR)/test_c200_device

clean:
	rm -rf $(BUILD_DIR)
