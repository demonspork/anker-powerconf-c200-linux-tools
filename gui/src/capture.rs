//! Background capture thread: V4L2 frames in, RGBA `ColorImage`s out.

use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Instant;

use egui::ColorImage;

use crate::ffi;

#[derive(Default)]
pub struct Shared {
    pub frame: Option<ColorImage>,
    pub error: Option<String>,
    /// The device is streaming in another process (EBUSY on setup).
    pub busy: bool,
    pub width: u32,
    pub height: u32,
    pub fourcc: u32,
    pub fps: f32,
}

pub struct Preview {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Preview {
    pub fn start(path: &str, mode: ffi::CapMode, ctx: egui::Context) -> Preview {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let path = path.to_string();
        let handle = {
            let shared = Arc::clone(&shared);
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("capture".into())
                .spawn(move || capture_loop(&path, mode, &shared, &stop, &ctx))
                .ok()
        };
        Preview {
            shared,
            stop,
            handle,
        }
    }

    /// Take the newest frame, if the capture thread produced one since the last call.
    pub fn take_frame(&self) -> Option<ColorImage> {
        self.shared.lock().ok()?.frame.take()
    }

    pub fn status(&self) -> (Option<String>, u32, u32, u32, f32) {
        match self.shared.lock() {
            Ok(s) => (s.error.clone(), s.width, s.height, s.fourcc, s.fps),
            Err(_) => (Some("capture thread panicked".into()), 0, 0, 0, 0.0),
        }
    }

    /// True once the capture thread found the device streaming elsewhere.
    pub fn is_busy(&self) -> bool {
        self.shared.lock().map(|s| s.busy).unwrap_or(false)
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.stop();
    }
}

fn set_error(shared: &Mutex<Shared>, message: String) {
    if let Ok(mut s) = shared.lock() {
        s.error = Some(message);
    }
}

fn capture_loop(
    path: &str,
    mode: ffi::CapMode,
    shared: &Mutex<Shared>,
    stop: &AtomicBool,
    ctx: &egui::Context,
) {
    let Ok(c_path) = CString::new(path) else {
        set_error(shared, "bad device path".into());
        return;
    };
    let mut stage = [0i8; 64];
    let cap = unsafe {
        ffi::awcap_open(
            c_path.as_ptr(),
            mode.fourcc,
            mode.width,
            mode.height,
            mode.max_fps,
            stage.as_mut_ptr(),
            stage.len(),
        )
    };
    if cap.is_null() {
        let os = std::io::Error::last_os_error();
        let stage = unsafe { ffi::cstr_to_string(stage.as_ptr()) };
        let busy = os.raw_os_error() == Some(16); // EBUSY: another process is streaming
        let message = if busy {
            format!(
                "{path} is in use by another application; preview unavailable, controls still work"
            )
        } else {
            format!("{path}: {stage} failed: {os}")
        };
        eprintln!("preview: {message}");
        if let Ok(mut s) = shared.lock() {
            s.busy = busy;
            s.error = Some(message);
        }
        ctx.request_repaint();
        return;
    }

    let (width, height, fourcc) = unsafe {
        (
            ffi::awcap_width(cap),
            ffi::awcap_height(cap),
            ffi::awcap_fourcc(cap),
        )
    };
    if let Ok(mut s) = shared.lock() {
        s.width = width;
        s.height = height;
        s.fourcc = fourcc;
    }

    let mut frames_in_window = 0u32;
    let mut window_start = Instant::now();
    let mut rgba = Vec::new();
    let mut announced = false;
    let mut last_error: Option<String> = None;

    while !stop.load(Ordering::Relaxed) {
        let mut data: *const u8 = std::ptr::null();
        let mut len: usize = 0;
        let rc = unsafe { ffi::awcap_wait_frame(cap, 250, &mut data, &mut len) };
        if rc < 0 {
            set_error(
                shared,
                format!("capture error: {}", std::io::Error::last_os_error()),
            );
            ctx.request_repaint();
            break;
        }
        if rc == 0 {
            continue;
        }

        let bytes = unsafe { std::slice::from_raw_parts(data, len) };
        let converted = convert_frame(fourcc, width, height, bytes, &mut rgba);
        unsafe { ffi::awcap_release_frame(cap) };

        match converted {
            Ok(()) => {
                if !announced {
                    announced = true;
                    eprintln!(
                        "preview: {} {width}x{height} streaming from {path}",
                        ffi::fourcc_to_string(fourcc)
                    );
                }
                frames_in_window += 1;
                let elapsed = window_start.elapsed().as_secs_f32();
                let image =
                    ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);
                if let Ok(mut s) = shared.lock() {
                    s.frame = Some(image);
                    s.error = None;
                    if elapsed >= 1.0 {
                        s.fps = frames_in_window as f32 / elapsed;
                    }
                }
                if elapsed >= 1.0 {
                    frames_in_window = 0;
                    window_start = Instant::now();
                }
                ctx.request_repaint();
            }
            Err(message) => {
                if last_error.as_deref() != Some(message.as_str()) {
                    eprintln!("preview: {message}");
                    last_error = Some(message.clone());
                }
                set_error(shared, message);
                ctx.request_repaint();
            }
        }
    }

    unsafe { ffi::awcap_close(cap) };
}

fn convert_frame(
    fourcc: u32,
    width: u32,
    height: u32,
    bytes: &[u8],
    rgba: &mut Vec<u8>,
) -> Result<(), String> {
    match fourcc {
        ffi::FOURCC_YUYV => yuyv_to_rgba(width as usize, height as usize, bytes, rgba),
        ffi::FOURCC_MJPG => mjpeg_to_rgba(width, height, bytes, rgba),
        other => Err(format!(
            "unsupported pixel format {}",
            ffi::fourcc_to_string(other)
        )),
    }
}

fn clamp_u8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

/// Packed YUV 4:2:2 (Y0 U Y1 V) to RGBA, BT.601 limited range.
fn yuyv_to_rgba(
    width: usize,
    height: usize,
    bytes: &[u8],
    rgba: &mut Vec<u8>,
) -> Result<(), String> {
    let needed = width * height * 2;
    if bytes.len() < needed {
        return Err(format!(
            "short YUYV frame: {} of {} bytes",
            bytes.len(),
            needed
        ));
    }
    rgba.clear();
    rgba.reserve(width * height * 4);
    for chunk in bytes[..needed].as_chunks::<4>().0 {
        let u = i32::from(chunk[1]) - 128;
        let v = i32::from(chunk[3]) - 128;
        let r_off = (91881 * v + 32768) >> 16;
        let g_off = (-(22554 * u) - (46802 * v) + 32768) >> 16;
        let b_off = (116130 * u + 32768) >> 16;
        for &y in &[chunk[0], chunk[2]] {
            let c = ((i32::from(y) - 16) * 76309 + 32768) >> 16;
            rgba.extend_from_slice(&[
                clamp_u8(c + r_off),
                clamp_u8(c + g_off),
                clamp_u8(c + b_off),
                255,
            ]);
        }
    }
    Ok(())
}

fn mjpeg_to_rgba(width: u32, height: u32, bytes: &[u8], rgba: &mut Vec<u8>) -> Result<(), String> {
    let decoded = match image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg) {
        Ok(img) => img,
        Err(first) => {
            if has_dht(bytes) {
                return Err(format!("JPEG decode failed: {first}"));
            }
            // UVC cameras commonly omit the Huffman tables; splice in the standard ones.
            let mut patched = Vec::with_capacity(bytes.len() + 512);
            patched.extend_from_slice(&bytes[..2]);
            patched.extend_from_slice(standard_dht());
            patched.extend_from_slice(&bytes[2..]);
            image::load_from_memory_with_format(&patched, image::ImageFormat::Jpeg)
                .map_err(|e| format!("JPEG decode failed: {e}"))?
        }
    };
    if decoded.width() != width || decoded.height() != height {
        return Err(format!(
            "JPEG is {}x{} but stream is {}x{}",
            decoded.width(),
            decoded.height(),
            width,
            height
        ));
    }
    *rgba = decoded.into_rgba8().into_raw();
    Ok(())
}

/// True if a DHT marker appears before the scan starts.
fn has_dht(bytes: &[u8]) -> bool {
    let mut i = 2;
    while i + 4 <= bytes.len() && bytes[i] == 0xFF {
        let marker = bytes[i + 1];
        match marker {
            0xD8 | 0x01 | 0xD0..=0xD7 => {
                i += 2;
                continue;
            }
            0xC4 => return true,
            0xDA => return false,
            _ => {}
        }
        let length = usize::from(u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]));
        i += 2 + length;
    }
    false
}

/// The standard Huffman tables from ITU-T T.81 Annex K.3, as one DHT segment.
fn standard_dht() -> &'static [u8] {
    static DHT: OnceLock<Vec<u8>> = OnceLock::new();
    DHT.get_or_init(|| {
        const DC_LUM_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
        const DC_CHR_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
        const DC_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        const AC_LUM_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
        const AC_LUM_VALS: [u8; 162] = [
            0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51,
            0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1,
            0x15, 0x52, 0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18,
            0x19, 0x1a, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39,
            0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57,
            0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75,
            0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92,
            0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
            0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
            0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8,
            0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2,
            0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
        ];
        const AC_CHR_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
        const AC_CHR_VALS: [u8; 162] = [
            0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07,
            0x61, 0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09,
            0x23, 0x33, 0x52, 0xf0, 0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25,
            0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38,
            0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56,
            0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74,
            0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
            0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
            0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba,
            0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6,
            0xd7, 0xd8, 0xd9, 0xda, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2,
            0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
        ];

        let tables: [(u8, &[u8], &[u8]); 4] = [
            (0x00, &DC_LUM_BITS, &DC_VALS),
            (0x10, &AC_LUM_BITS, &AC_LUM_VALS),
            (0x01, &DC_CHR_BITS, &DC_VALS),
            (0x11, &AC_CHR_BITS, &AC_CHR_VALS),
        ];
        let length: usize = 2 + tables
            .iter()
            .map(|(_, bits, vals)| 1 + bits.len() + vals.len())
            .sum::<usize>();
        let mut segment = vec![0xFF, 0xC4, (length >> 8) as u8, (length & 0xFF) as u8];
        for (class_id, bits, vals) in tables {
            segment.push(class_id);
            segment.extend_from_slice(bits);
            segment.extend_from_slice(vals);
        }
        segment
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dht_segment_is_well_formed() {
        let dht = standard_dht();
        assert_eq!(&dht[..2], &[0xFF, 0xC4]);
        let length = usize::from(u16::from_be_bytes([dht[2], dht[3]]));
        assert_eq!(length + 2, dht.len());
        // 4 tables: 2 x (1 + 16 + 12) + 2 x (1 + 16 + 162) + 2 bytes of length
        assert_eq!(length, 2 + 2 * 29 + 2 * 179);
    }

    #[test]
    fn dht_detection() {
        let without = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x02, 0xFF, 0xDA, 0x00, 0x02];
        assert!(!has_dht(&without));
        let mut with = vec![0xFF, 0xD8];
        with.extend_from_slice(standard_dht());
        with.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02]);
        assert!(has_dht(&with));
    }

    #[test]
    fn yuyv_grey_and_white() {
        let mut out = Vec::new();
        // Two pixels: Y=16 (black) and Y=235 (white), neutral chroma.
        yuyv_to_rgba(2, 1, &[16, 128, 235, 128], &mut out).unwrap();
        assert_eq!(out, vec![0, 0, 0, 255, 255, 255, 255, 255]);
    }
}
