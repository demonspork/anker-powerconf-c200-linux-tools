//! Find compatible cameras by walking sysfs, the same way the CLI gates controls.

use std::ffi::CString;
use std::fs;

use crate::ffi;

#[derive(Clone, Debug)]
pub struct CameraDevice {
    pub path: String,
    pub name: String,
    pub model: i32,
    pub model_name: String,
    pub usb_id: ffi::UsbId,
}

impl CameraDevice {
    pub fn label(&self) -> String {
        if self.model == ffi::MODEL_UNKNOWN {
            format!("{} ({})", self.name, self.path)
        } else {
            format!("{} ({})", self.model_name, self.path)
        }
    }
}

/// Every video4linux capture node (index 0) whose USB parent is a known Anker model.
/// With `include_all`, every USB camera is listed (standard controls only).
pub fn find_cameras(include_all: bool) -> Vec<CameraDevice> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/video4linux") else {
        return found;
    };

    let mut nodes: Vec<(u32, String)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let number = name.strip_prefix("video")?.parse::<u32>().ok()?;
            Some((number, name))
        })
        .collect();
    nodes.sort();

    for (_, node) in nodes {
        let sys = format!("/sys/class/video4linux/{node}");
        let index = fs::read_to_string(format!("{sys}/index")).unwrap_or_default();
        if index.trim() != "0" {
            continue; // metadata node of the same camera
        }

        let usb_dir = format!("{sys}/device/..");
        let Ok(c_dir) = CString::new(usb_dir) else {
            continue;
        };
        let mut usb_id = ffi::UsbId::default();
        let model = unsafe {
            if ffi::c200_device_read_usb_id_from_dir(c_dir.as_ptr(), &mut usb_id) < 0 {
                continue;
            }
            ffi::c200_model_from_usb_id(&usb_id)
        };
        if model == ffi::MODEL_UNKNOWN && !include_all {
            continue;
        }

        let name = fs::read_to_string(format!("{sys}/name"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| node.clone());
        found.push(CameraDevice {
            path: format!("/dev/{node}"),
            name,
            model,
            model_name: ffi::model_name(model),
            usb_id,
        });
    }

    found
}
