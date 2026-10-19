//! Hand-written bindings to the C library in ../src and the capture shim in csrc.
#![allow(dead_code)]

use std::ffi::{c_char, c_int, c_uint, CStr};

pub const KIND_VENDOR_BOOL: c_int = 0;
pub const KIND_VENDOR_U8: c_int = 1;
pub const KIND_VENDOR_FOV: c_int = 2;
pub const KIND_VENDOR_AREA_METERING: c_int = 3;
pub const KIND_VENDOR_ENABLE_U8: c_int = 4;
pub const KIND_V4L2_BOOL: c_int = 5;
pub const KIND_V4L2_INT: c_int = 6;
pub const KIND_V4L2_MENU: c_int = 7;

pub const MODEL_UNKNOWN: c_int = 0;
pub const MODEL_C200: c_int = 1;
pub const MODEL_C310: c_int = 2;

pub const AREA_GRID_WIDTH: u16 = 3840;
pub const AREA_GRID_HEIGHT: u16 = 2160;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct UsbId {
    pub vendor_id: u16,
    pub product_id: u16,
}

#[repr(C)]
pub struct ControlInfo {
    pub name: *const c_char,
    pub kind: c_int,
    pub id: u32,
    pub description: *const c_char,
    pub max_value: u8,
    pub models: c_uint,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AreaMetering {
    pub enable: bool,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EnableU8 {
    pub enable: bool,
    pub value: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CapMode {
    pub fourcc: u32,
    pub width: u32,
    pub height: u32,
    pub max_fps: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CtlInfo {
    pub minimum: i32,
    pub maximum: i32,
    pub step: i32,
    pub default_value: i32,
    pub flags: u32,
    pub type_: u32,
}

pub const V4L2_CTRL_FLAG_INACTIVE: u32 = 0x0010;
pub const V4L2_CTRL_FLAG_READ_ONLY: u32 = 0x0004;
pub const V4L2_CTRL_TYPE_MENU: u32 = 3;
pub const V4L2_CTRL_TYPE_INTEGER_MENU: u32 = 9;

#[repr(C)]
pub struct Awcap {
    _private: [u8; 0],
}

extern "C" {
    pub fn c200_all_controls(count: *mut usize) -> *const ControlInfo;
    pub fn c200_control_supports_model(control: *const ControlInfo, model: c_int) -> bool;
    pub fn c200_model_from_usb_id(id: *const UsbId) -> c_int;
    pub fn c200_model_name(model: c_int) -> *const c_char;
    pub fn c200_device_read_usb_id_from_dir(dir: *const c_char, id: *mut UsbId) -> c_int;

    pub fn c200_vendor_get_bool(fd: c_int, selector: u8, value: *mut bool) -> c_int;
    pub fn c200_vendor_set_bool(fd: c_int, selector: u8, value: bool) -> c_int;
    pub fn c200_vendor_get_u8(fd: c_int, selector: u8, value: *mut u8) -> c_int;
    pub fn c200_vendor_set_u8(fd: c_int, selector: u8, value: u8) -> c_int;

    pub fn c200_fov_get(fd: c_int, value: *mut u16) -> c_int;
    pub fn c200_fov_set(fd: c_int, value: u16) -> c_int;
    pub fn c200_fov_describe_value(value: u16) -> *const c_char;

    pub fn c200_area_metering_get(fd: c_int, value: *mut AreaMetering) -> c_int;
    pub fn c200_area_metering_set(fd: c_int, value: *const AreaMetering) -> c_int;
    pub fn c200_area_metering_rect_is_valid(value: *const AreaMetering) -> bool;

    pub fn c200_enable_u8_get(fd: c_int, selector: u8, value: *mut EnableU8) -> c_int;
    pub fn c200_enable_u8_set(fd: c_int, selector: u8, value: *const EnableU8) -> c_int;

    pub fn awcap_list_modes(path: *const c_char, out: *mut CapMode, max: c_int) -> c_int;
    pub fn awcap_open(
        path: *const c_char,
        fourcc: u32,
        width: u32,
        height: u32,
        fps: u32,
        err: *mut c_char,
        err_len: usize,
    ) -> *mut Awcap;
    pub fn awcap_width(cap: *const Awcap) -> u32;
    pub fn awcap_height(cap: *const Awcap) -> u32;
    pub fn awcap_fourcc(cap: *const Awcap) -> u32;
    pub fn awcap_wait_frame(
        cap: *mut Awcap,
        timeout_ms: c_int,
        data: *mut *const u8,
        len: *mut usize,
    ) -> c_int;
    pub fn awcap_release_frame(cap: *mut Awcap) -> c_int;
    pub fn awcap_close(cap: *mut Awcap);

    pub fn awctl_query(fd: c_int, id: u32, out: *mut CtlInfo) -> c_int;
    pub fn awctl_get(fd: c_int, id: u32, value: *mut i32) -> c_int;
    pub fn awctl_set(fd: c_int, id: u32, value: i32) -> c_int;
    pub fn awctl_menu_name(
        fd: c_int,
        id: u32,
        index: u32,
        name: *mut c_char,
        name_len: usize,
    ) -> c_int;
}

/// Copy a C string the library owns (static or long-lived) into a Rust String.
///
/// # Safety
/// `ptr` must be null or point at a NUL-terminated string.
pub unsafe fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

pub fn model_name(model: c_int) -> String {
    unsafe { cstr_to_string(c200_model_name(model)) }
}

pub fn fourcc_to_string(fourcc: u32) -> String {
    fourcc
        .to_le_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_graphic() {
                *b as char
            } else {
                '?'
            }
        })
        .collect()
}

pub const fn fourcc(code: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*code)
}

pub const FOURCC_YUYV: u32 = fourcc(b"YUYV");
pub const FOURCC_MJPG: u32 = fourcc(b"MJPG");
