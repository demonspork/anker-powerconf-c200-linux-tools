CC ?= cc
CFLAGS ?= -O2 -Wall -Wextra -Werror -std=c11

BUILD_DIR := build
SRC_DIR := src
TEST_DIR := tests

PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DESTDIR ?=
INSTALL ?= install

COMMON_SRCS := $(SRC_DIR)/c200_fov.c
COMMON_SRCS += $(SRC_DIR)/c200_vendor.c $(SRC_DIR)/c200_controls.c
COMMON_SRCS += $(SRC_DIR)/c200_area_metering.c $(SRC_DIR)/c200_enable_u8.c
COMMON_SRCS += $(SRC_DIR)/c200_device.c
CLI_SRCS := $(COMMON_SRCS) $(SRC_DIR)/c200_fov_cli.c
TEST_SRCS := $(COMMON_SRCS) $(TEST_DIR)/test_c200_fov.c
AREA_TEST_SRCS := $(COMMON_SRCS) $(TEST_DIR)/test_c200_area_metering.c
ENABLE_U8_TEST_SRCS := $(COMMON_SRCS) $(TEST_DIR)/test_c200_enable_u8.c
DEVICE_TEST_SRCS := $(COMMON_SRCS) $(TEST_DIR)/test_c200_device.c

.PHONY: all test clean install gui test-gui install-gui dist

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

install: all
	$(INSTALL) -Dm755 $(BUILD_DIR)/anker-powerconf-c200-linux-tools $(DESTDIR)$(BINDIR)/anker-powerconf-c200-linux-tools

# The desktop GUI lives in gui/ and has its own Makefile; these wrappers pass PREFIX/DESTDIR through.
gui:
	$(MAKE) -C gui

test-gui:
	$(MAKE) -C gui test

install-gui:
	$(MAKE) -C gui install PREFIX=$(PREFIX) DESTDIR=$(DESTDIR)

clean:
	rm -rf $(BUILD_DIR)
	$(MAKE) -C gui clean

# Prebuilt release tarball: bin/ and share/ laid out as `make install` would,
# plus install.sh, README and LICENSE. Fails if a binary needs a glibc newer
# than our target distributions (see packaging/check-portability.sh).
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' gui/Cargo.toml)
DIST_NAME := anker-webcam-tools-$(VERSION)-linux-$(shell uname -m)
DIST_DIR := dist/$(DIST_NAME)

dist: all gui
	rm -rf $(DIST_DIR)
	$(MAKE) install DESTDIR=$(CURDIR)/$(DIST_DIR) PREFIX=
	$(MAKE) -C gui install DESTDIR=$(CURDIR)/$(DIST_DIR) PREFIX=
	$(INSTALL) -Dm755 packaging/install.sh $(DIST_DIR)/install.sh
	$(INSTALL) -Dm644 README.md $(DIST_DIR)/README.md
	$(INSTALL) -Dm644 LICENSE $(DIST_DIR)/LICENSE
	sh packaging/check-portability.sh $(DIST_DIR)/bin/*
	tar -C dist -czf dist/$(DIST_NAME).tar.gz $(DIST_NAME)
	cd dist && sha256sum $(DIST_NAME).tar.gz > $(DIST_NAME).tar.gz.sha256
	@echo "wrote dist/$(DIST_NAME).tar.gz"
