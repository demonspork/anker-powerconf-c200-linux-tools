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
    /// Ticked in the panel: written by "Save as" / "Overwrite". Load sets it to
    /// match the file, so a profile round-trips the same set of controls.
    pub include_in_profile: bool,
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
        camera.reset_profile_selection();
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

/// Apply order for profiles: mode switches first so dependent controls become active.
fn apply_rank(kind: i32) -> u8 {
    match kind {
        ffi::KIND_V4L2_MENU | ffi::KIND_V4L2_BOOL => 0,
        ffi::KIND_V4L2_INT => 1,
        _ => 2,
    }
}

pub struct ApplyReport {
    pub applied: usize,
    pub skipped: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProfileSelection {
    /// Vendor controls plus every active standard control (the startup default).
    Default,
    All,
    VendorOnly,
    None,
}

impl Camera {
    pub fn is_vendor(control: &Control) -> bool {
        control.kind <= ffi::KIND_VENDOR_ENABLE_U8
    }

    /// Vendor controls on, active standard controls on, inactive and read-only off.
    pub fn reset_profile_selection(&mut self) {
        self.select_for_profile(ProfileSelection::Default);
    }

    pub fn select_for_profile(&mut self, selection: ProfileSelection) {
        select_controls(&mut self.controls, selection);
    }

    /// Tick exactly the controls a loaded profile names.
    pub fn select_from_profile(&mut self, entries: &[(String, String)]) {
        select_controls_from_profile(&mut self.controls, entries);
    }

    pub fn profile_selection_count(&self) -> (usize, usize) {
        let selectable = self.controls.iter().filter(|c| !c.read_only).count();
        let selected = self
            .controls
            .iter()
            .filter(|c| c.include_in_profile && !c.read_only)
            .count();
        (selected, selectable)
    }

    /// Current values of the ticked controls, in profile order.
    pub fn export_profile(&self) -> Vec<(String, String)> {
        self.controls
            .iter()
            .filter(|c| !c.read_only && c.include_in_profile)
            .filter_map(|c| {
                c.value
                    .as_ref()
                    .map(|v| (c.name.clone(), crate::profiles::value_to_text(v)))
            })
            .collect()
    }

    /// Write profile entries to the camera; unknown or unsupported names are reported, not fatal.
    pub fn apply_profile(&mut self, entries: &[(String, String)]) -> ApplyReport {
        let mut report = ApplyReport {
            applied: 0,
            skipped: Vec::new(),
        };
        let mut planned: Vec<(usize, ControlValue)> = Vec::new();
        for (name, text) in entries {
            let Some(index) = self
                .controls
                .iter()
                .position(|c| c.name.eq_ignore_ascii_case(name))
            else {
                report.skipped.push(format!("{name} (not on this camera)"));
                continue;
            };
            let control = &self.controls[index];
            if control.read_only {
                report.skipped.push(format!("{name} (read-only)"));
                continue;
            }
            let Some(current) = control.value.as_ref() else {
                report.skipped.push(format!("{name} (unreadable)"));
                continue;
            };
            match crate::profiles::text_to_value(text, current) {
                Some(value) => planned.push((index, value)),
                None => report.skipped.push(format!("{name} (bad value '{text}')")),
            }
        }
        planned.sort_by_key(|(index, _)| apply_rank(self.controls[*index].kind));
        for (index, value) in planned {
            self.write(index, &value);
            match &self.controls[index].error {
                Some(error) => report
                    .skipped
                    .push(format!("{} ({error})", self.controls[index].name)),
                None => report.applied += 1,
            }
        }
        self.refresh_all();
        report
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
                include_in_profile: false,
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
    // Largest first, then faster, then raw YUYV ahead of MJPEG at the same size.
    modes.sort_by_key(|m| {
        std::cmp::Reverse((m.width * m.height, m.max_fps, m.fourcc == ffi::FOURCC_YUYV))
    });
    modes
}

pub fn select_controls(controls: &mut [Control], selection: ProfileSelection) {
    for control in controls {
        control.include_in_profile = !control.read_only
            && match selection {
                ProfileSelection::Default => Camera::is_vendor(control) || !control.inactive,
                ProfileSelection::All => true,
                ProfileSelection::VendorOnly => Camera::is_vendor(control),
                ProfileSelection::None => false,
            };
    }
}

pub fn select_controls_from_profile(controls: &mut [Control], entries: &[(String, String)]) {
    for control in controls {
        control.include_in_profile = !control.read_only
            && entries
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(&control.name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(name: &str, kind: i32, inactive: bool, read_only: bool) -> Control {
        Control {
            name: name.into(),
            description: String::new(),
            kind,
            id: 0,
            max_value: 0,
            value: None,
            inactive,
            read_only,
            error: None,
            include_in_profile: false,
        }
    }

    fn sample() -> Vec<Control> {
        vec![
            control("fov", ffi::KIND_VENDOR_FOV, false, false),
            control("face_exposure", ffi::KIND_VENDOR_ENABLE_U8, false, false),
            control("brightness", ffi::KIND_V4L2_INT, false, false),
            control("exposure_time_absolute", ffi::KIND_V4L2_INT, true, false),
            control("firmware", ffi::KIND_V4L2_INT, false, true),
        ]
    }

    fn ticked(controls: &[Control]) -> Vec<&str> {
        controls
            .iter()
            .filter(|c| c.include_in_profile)
            .map(|c| c.name.as_str())
            .collect()
    }

    #[test]
    fn quick_selections() {
        let mut c = sample();
        select_controls(&mut c, ProfileSelection::Default);
        assert_eq!(ticked(&c), vec!["fov", "face_exposure", "brightness"]);
        select_controls(&mut c, ProfileSelection::All);
        assert_eq!(
            ticked(&c),
            vec![
                "fov",
                "face_exposure",
                "brightness",
                "exposure_time_absolute"
            ]
        );
        select_controls(&mut c, ProfileSelection::VendorOnly);
        assert_eq!(ticked(&c), vec!["fov", "face_exposure"]);
        select_controls(&mut c, ProfileSelection::None);
        assert!(ticked(&c).is_empty());
    }

    #[test]
    fn load_ticks_exactly_the_file() {
        let mut c = sample();
        let entries = vec![
            ("FACE_EXPOSURE".to_string(), "on,35".to_string()),
            ("exposure_time_absolute".to_string(), "100".to_string()),
            ("firmware".to_string(), "1".to_string()),
            ("unknown".to_string(), "1".to_string()),
        ];
        select_controls_from_profile(&mut c, &entries);
        assert_eq!(ticked(&c), vec!["face_exposure", "exposure_time_absolute"]);
    }
}
