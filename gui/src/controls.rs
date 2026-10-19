//! Control table (shared with the CLI through the C library) and per-control state.

use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;

use crate::ffi;

#[derive(Clone, Debug)]
pub enum ControlValue {
    VendorBool(bool),
    VendorU8(u8),
    Fov(u16),
    AreaMetering(ffi::AreaMetering),
    EnableU8(ffi::EnableU8),
    V4l2Bool(bool),
    V4l2Int {
        value: i32,
        min: i32,
        max: i32,
        step: i32,
        default: i32,
    },
    V4l2Menu {
        value: i32,
        items: Vec<(i32, String)>,
    },
}

#[derive(Clone, Debug)]
pub struct Control {
    pub name: String,
    pub description: String,
    pub kind: i32,
    pub id: u32,
    pub max_value: u8,
    pub value: Option<ControlValue>,
    /// Driver reports the control as currently inactive (e.g. manual exposure while auto is on).
    pub inactive: bool,
    pub read_only: bool,
    pub error: Option<String>,
}

/// An open camera: a control fd plus the controls the model supports.
pub struct Camera {
    file: File,
    pub controls: Vec<Control>,
}

fn last_error() -> String {
    io::Error::last_os_error().to_string()
}

impl Camera {
    pub fn open(path: &str, model: i32) -> io::Result<Camera> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let mut camera = Camera {
            file,
            controls: Vec::new(),
        };
        camera.controls = table_for_model(model);
        camera.refresh_all();
        Ok(camera)
    }

    fn fd(&self) -> i32 {
        self.file.as_raw_fd()
    }

    pub fn refresh_all(&mut self) {
        for i in 0..self.controls.len() {
            self.refresh(i);
        }
        // Drop standard controls the driver does not expose at all.
        self.controls.retain(|c| {
            !(c.value.is_none() && c.error.as_deref() == Some("not supported by driver"))
        });
    }

    pub fn refresh(&mut self, index: usize) {
        let fd = self.fd();
        let control = &mut self.controls[index];
        control.error = None;
        let result = unsafe { read_value(fd, control) };
        match result {
            Ok(value) => control.value = Some(value),
            Err(message) => {
                control.value = None;
                control.error = Some(message);
            }
        }
    }

    /// Write a new value, then re-read so the panel shows what the camera accepted.
    pub fn write(&mut self, index: usize, value: &ControlValue) {
        let fd = self.fd();
        let control = &mut self.controls[index];
        let result = unsafe { write_value(fd, control, value) };
        if let Err(message) = result {
            control.error = Some(message);
        }
        self.refresh(index);
        // Standard controls can flip others between active and inactive (auto exposure).
        if matches!(
            value,
            ControlValue::V4l2Menu { .. } | ControlValue::V4l2Bool(_)
        ) {
            self.refresh_all();
        }
    }
}

fn table_for_model(model: i32) -> Vec<Control> {
    let mut controls = Vec::new();
    let mut seen: Vec<(i32, u32)> = Vec::new();
    unsafe {
        let mut count: usize = 0;
        let table = ffi::c200_all_controls(&mut count);
        for i in 0..count {
            let info = &*table.add(i);
            let is_standard = info.kind >= ffi::KIND_V4L2_BOOL;
            // Unknown cameras get the standard UVC controls only; vendor selectors stay gated.
            let supported = if model == ffi::MODEL_UNKNOWN {
                is_standard
            } else {
                ffi::c200_control_supports_model(info, model)
            };
            if !supported {
                continue;
            }
            // Aliases (area_metering == exposure_smoothing) share kind and id.
            if seen.contains(&(info.kind, info.id)) {
                continue;
            }
            seen.push((info.kind, info.id));
            controls.push(Control {
                name: ffi::cstr_to_string(info.name),
                description: ffi::cstr_to_string(info.description),
                kind: info.kind,
                id: info.id,
                max_value: info.max_value,
                value: None,
                inactive: false,
                read_only: false,
                error: None,
            });
        }
    }
    controls
}

unsafe fn read_value(fd: i32, control: &mut Control) -> Result<ControlValue, String> {
    match control.kind {
        ffi::KIND_VENDOR_BOOL => {
            let mut v = false;
            if ffi::c200_vendor_get_bool(fd, control.id as u8, &mut v) < 0 {
                return Err(last_error());
            }
            Ok(ControlValue::VendorBool(v))
        }
        ffi::KIND_VENDOR_U8 => {
            let mut v = 0u8;
            if ffi::c200_vendor_get_u8(fd, control.id as u8, &mut v) < 0 {
                return Err(last_error());
            }
            Ok(ControlValue::VendorU8(v))
        }
        ffi::KIND_VENDOR_FOV => {
            let mut v = 0u16;
            if ffi::c200_fov_get(fd, &mut v) < 0 {
                return Err(last_error());
            }
            Ok(ControlValue::Fov(v))
        }
        ffi::KIND_VENDOR_AREA_METERING => {
            let mut v = ffi::AreaMetering::default();
            if ffi::c200_area_metering_get(fd, &mut v) < 0 {
                return Err(last_error());
            }
            Ok(ControlValue::AreaMetering(v))
        }
        ffi::KIND_VENDOR_ENABLE_U8 => {
            let mut v = ffi::EnableU8::default();
            if ffi::c200_enable_u8_get(fd, control.id as u8, &mut v) < 0 {
                return Err(last_error());
            }
            Ok(ControlValue::EnableU8(v))
        }
        ffi::KIND_V4L2_BOOL | ffi::KIND_V4L2_INT | ffi::KIND_V4L2_MENU => {
            let mut info = ffi::CtlInfo::default();
            if ffi::awctl_query(fd, control.id, &mut info) < 0 {
                return Err("not supported by driver".to_string());
            }
            control.inactive = info.flags & ffi::V4L2_CTRL_FLAG_INACTIVE != 0;
            control.read_only = info.flags & ffi::V4L2_CTRL_FLAG_READ_ONLY != 0;
            let mut value = 0i32;
            if ffi::awctl_get(fd, control.id, &mut value) < 0 {
                return Err(last_error());
            }
            match control.kind {
                ffi::KIND_V4L2_BOOL => Ok(ControlValue::V4l2Bool(value != 0)),
                ffi::KIND_V4L2_MENU => {
                    let mut items = Vec::new();
                    for index in info.minimum..=info.maximum {
                        let mut buffer = [0i8; 64];
                        if ffi::awctl_menu_name(
                            fd,
                            control.id,
                            index as u32,
                            buffer.as_mut_ptr(),
                            buffer.len(),
                        ) == 0
                        {
                            let name = ffi::cstr_to_string(buffer.as_ptr());
                            items.push((index, name));
                        }
                    }
                    Ok(ControlValue::V4l2Menu { value, items })
                }
                _ => Ok(ControlValue::V4l2Int {
                    value,
                    min: info.minimum,
                    max: info.maximum,
                    step: info.step.max(1),
                    default: info.default_value,
                }),
            }
        }
        other => Err(format!("unknown control kind {other}")),
    }
}

unsafe fn write_value(fd: i32, control: &Control, value: &ControlValue) -> Result<(), String> {
    let rc = match value {
        ControlValue::VendorBool(v) => ffi::c200_vendor_set_bool(fd, control.id as u8, *v),
        ControlValue::VendorU8(v) => ffi::c200_vendor_set_u8(fd, control.id as u8, *v),
        ControlValue::Fov(v) => ffi::c200_fov_set(fd, *v),
        ControlValue::AreaMetering(v) => {
            if v.enable && !ffi::c200_area_metering_rect_is_valid(v) {
                return Err("rectangle must be non-empty and inside 3840x2160".to_string());
            }
            ffi::c200_area_metering_set(fd, v)
        }
        ControlValue::EnableU8(v) => {
            if v.value > control.max_value {
                return Err(format!("value must be 0..{}", control.max_value));
            }
            ffi::c200_enable_u8_set(fd, control.id as u8, v)
        }
        ControlValue::V4l2Bool(v) => ffi::awctl_set(fd, control.id, i32::from(*v)),
        ControlValue::V4l2Int { value, .. } | ControlValue::V4l2Menu { value, .. } => {
            ffi::awctl_set(fd, control.id, *value)
        }
    };
    if rc < 0 {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub fn fov_label(value: u16) -> String {
    unsafe { ffi::cstr_to_string(ffi::c200_fov_describe_value(value)) }
}

/// Enumerate the capture modes a device offers, best (largest) first.
pub fn list_modes(path: &str) -> Vec<ffi::CapMode> {
    let Ok(c_path) = CString::new(path) else {
        return Vec::new();
    };
    let mut modes = vec![ffi::CapMode::default(); 64];
    let count =
        unsafe { ffi::awcap_list_modes(c_path.as_ptr(), modes.as_mut_ptr(), modes.len() as i32) };
    if count < 0 {
        return Vec::new();
    }
    modes.truncate(count as usize);
    modes.retain(|m| m.fourcc == ffi::FOURCC_YUYV || m.fourcc == ffi::FOURCC_MJPG);
    modes.sort_by_key(|m| {
        std::cmp::Reverse((m.fourcc == ffi::FOURCC_YUYV, m.width * m.height, m.max_fps))
    });
    modes
}
