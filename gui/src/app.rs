//! The egui application: camera picker, control panel, live preview with a draggable metering box.

use egui::{Color32, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2};

use crate::capture::Preview;
use crate::controls::{fov_label, list_modes, Camera, ControlValue, ProfileSelection};
use crate::devices::{find_cameras, CameraDevice};
use crate::ffi;
use crate::profiles::{sanitize_name, ProfileStore};

const FOV_PRESETS: [(u16, &str); 3] = [(65, "Narrow"), (78, "Medium"), (95, "Wide")];

const PROFILE_TICK_HELP: &str =
    "Include this control in the profile. Ticked controls are written by \
    \"Save as\" and \"Overwrite\"; unticked ones are left out, so loading the profile later leaves \
    them untouched. \"Load\" ticks exactly the controls the file contains.";

pub struct App {
    ctx: egui::Context,
    devices: Vec<CameraDevice>,
    selected_device: usize,
    camera: Option<Camera>,
    open_error: Option<String>,

    modes: Vec<ffi::CapMode>,
    selected_mode: usize,
    preview: Option<Preview>,
    texture: Option<TextureHandle>,
    preview_wanted: bool,
    show_all_cameras: bool,
    /// The selected camera is streaming in another process; the preview pane is hidden.
    camera_busy: bool,

    /// Controls edited this frame; written once the pointer is released.
    dirty: Vec<usize>,
    /// ANKER_GUI_SCREENSHOT=<file.png>: save the window after a few seconds and quit (dev aid).
    screenshot: Option<(String, std::time::Instant, bool)>,

    profiles: ProfileStore,
    profile_names: Vec<String>,
    selected_profile: Option<usize>,
    new_profile_name: String,
    profile_status: Option<(String, bool)>,
    confirm_delete: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_zoom_factor(1.0);
        let mut app = App {
            ctx: cc.egui_ctx.clone(),
            devices: Vec::new(),
            selected_device: 0,
            camera: None,
            open_error: None,
            modes: Vec::new(),
            selected_mode: 0,
            preview: None,
            texture: None,
            preview_wanted: true,
            show_all_cameras: std::env::var_os("ANKER_GUI_SHOW_ALL").is_some(),
            camera_busy: false,
            dirty: Vec::new(),
            screenshot: std::env::var("ANKER_GUI_SCREENSHOT")
                .ok()
                .filter(|p| !p.is_empty())
                .map(|p| (p, std::time::Instant::now(), false)),
            profiles: ProfileStore::new(),
            profile_names: Vec::new(),
            selected_profile: None,
            new_profile_name: String::new(),
            profile_status: None,
            confirm_delete: false,
        };
        app.refresh_profiles();
        app.rescan();
        app
    }

    fn rescan(&mut self) {
        let previous = self
            .devices
            .get(self.selected_device)
            .map(|d| d.path.clone());
        self.devices = find_cameras(self.show_all_cameras);
        self.selected_device = previous
            .and_then(|p| self.devices.iter().position(|d| d.path == p))
            .unwrap_or(0);
        self.open_selected();
    }

    fn open_selected(&mut self) {
        self.preview = None;
        self.texture = None;
        self.camera = None;
        self.open_error = None;
        self.camera_busy = false;
        self.modes.clear();
        let Some(device) = self.devices.get(self.selected_device).cloned() else {
            return;
        };
        match Camera::open(&device.path, device.model) {
            Ok(camera) => self.camera = Some(camera),
            Err(e) => {
                self.open_error = Some(format!("{}: {e}", device.path));
                return;
            }
        }
        if let Some(name) = std::env::var("ANKER_GUI_PROFILE")
            .ok()
            .filter(|n| !n.is_empty())
        {
            self.load_profile(&name);
            self.selected_profile = self.profile_names.iter().position(|n| *n == name);
        }
        self.modes = list_modes(&device.path);
        self.selected_mode =
            requested_mode(&self.modes).unwrap_or_else(|| default_mode(&self.modes));
        if self.preview_wanted {
            self.start_preview();
        }
    }

    fn start_preview(&mut self) {
        self.preview = None;
        self.texture = None;
        self.camera_busy = false;
        let (Some(device), Some(mode)) = (
            self.devices.get(self.selected_device),
            self.modes.get(self.selected_mode),
        ) else {
            return;
        };
        self.preview = Some(Preview::start(&device.path, *mode, self.ctx.clone()));
    }

    fn flush_dirty(&mut self, ctx: &egui::Context) {
        if self.dirty.is_empty() || ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        let Some(camera) = self.camera.as_mut() else {
            self.dirty.clear();
            return;
        };
        self.dirty.sort_unstable();
        self.dirty.dedup();
        for index in std::mem::take(&mut self.dirty) {
            if let Some(value) = camera.controls.get(index).and_then(|c| c.value.clone()) {
                camera.write(index, &value);
            }
        }
    }

    fn refresh_profiles(&mut self) {
        let previous = self
            .selected_profile
            .and_then(|i| self.profile_names.get(i).cloned());
        self.profile_names = self.profiles.list();
        self.selected_profile =
            previous.and_then(|p| self.profile_names.iter().position(|n| *n == p));
        self.confirm_delete = false;
    }

    fn set_profile_status(&mut self, message: String, ok: bool) {
        self.profile_status = Some((message, ok));
    }

    fn save_profile(&mut self, name: &str) {
        let (Some(camera), Some(device)) =
            (self.camera.as_ref(), self.devices.get(self.selected_device))
        else {
            self.set_profile_status("no camera open".into(), false);
            return;
        };
        let entries = camera.export_profile();
        let model = if device.model == ffi::MODEL_UNKNOWN {
            device.name.clone()
        } else {
            device.model_name.clone()
        };
        let result = self.profiles.save(name, &model, &entries);
        match result {
            Ok(()) => {
                let count = entries.len();
                self.set_profile_status(format!("saved '{name}' ({count} controls)"), true);
                self.refresh_profiles();
                self.selected_profile = self.profile_names.iter().position(|n| n == name);
                self.new_profile_name.clear();
            }
            Err(e) => self.set_profile_status(format!("could not save '{name}': {e}"), false),
        }
    }

    fn load_profile(&mut self, name: &str) {
        let profile = match self.profiles.load(name) {
            Ok(profile) => profile,
            Err(e) => {
                self.set_profile_status(format!("could not read '{name}': {e}"), false);
                return;
            }
        };
        let Some(camera) = self.camera.as_mut() else {
            self.set_profile_status("no camera open".into(), false);
            return;
        };
        let report = camera.apply_profile(&profile.entries);
        camera.select_from_profile(&profile.entries);
        let mut message = format!("loaded '{name}': {} applied", report.applied);
        if !report.skipped.is_empty() {
            message.push_str(&format!(", skipped {}", report.skipped.join(", ")));
        }
        let ok = report.skipped.is_empty();
        self.set_profile_status(message, ok);
    }

    fn delete_profile(&mut self, name: &str) {
        let result = self.profiles.delete(name);
        match result {
            Ok(()) => self.set_profile_status(format!("deleted '{name}'"), true),
            Err(e) => self.set_profile_status(format!("could not delete '{name}': {e}"), false),
        }
        self.refresh_profiles();
    }

    fn profile_bar(&mut self, ui: &mut egui::Ui) {
        let mut action: Option<ProfileAction> = None;
        ui.horizontal(|ui| {
            let current = self
                .selected_profile
                .and_then(|i| self.profile_names.get(i).cloned())
                .unwrap_or_else(|| "(none)".to_string());
            ui.label("Profile:").on_hover_text(format!(
                "Profiles are plain text files in {}",
                self.profiles.dir().display()
            ));
            egui::ComboBox::from_id_salt("profile")
                .selected_text(current)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for (i, name) in self.profile_names.iter().enumerate() {
                        if ui
                            .selectable_label(self.selected_profile == Some(i), name)
                            .clicked()
                        {
                            self.selected_profile = Some(i);
                            self.confirm_delete = false;
                        }
                    }
                });
            let has_selection = self.selected_profile.is_some();
            let has_camera = self.camera.is_some();
            if ui
                .add_enabled(has_selection && has_camera, egui::Button::new("Load"))
                .on_hover_text(
                    "Apply the profile to the camera. Only the controls listed in the file change; \
                     everything else keeps its current value. Also ticks those controls in the panel.",
                )
                .clicked()
            {
                action = Some(ProfileAction::Load);
            }
            if ui
                .add_enabled(has_selection && has_camera, egui::Button::new("Overwrite"))
                .on_hover_text(
                    "Replace the selected profile with the current values of the ticked controls.",
                )
                .clicked()
            {
                action = Some(ProfileAction::Overwrite);
            }
            if self.confirm_delete {
                ui.label("delete?");
                if ui.button("Yes").clicked() {
                    action = Some(ProfileAction::Delete);
                }
                if ui.button("No").clicked() {
                    self.confirm_delete = false;
                }
            } else if ui
                .add_enabled(has_selection, egui::Button::new("Delete"))
                .clicked()
            {
                self.confirm_delete = true;
            }
            ui.separator();
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.new_profile_name)
                    .hint_text("new profile name")
                    .desired_width(160.0),
            );
            let name_ok = sanitize_name(&self.new_profile_name).is_some();
            let submitted = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui
                .add_enabled(name_ok && has_camera, egui::Button::new("Save as"))
                .on_hover_text(
                    "Write a new profile containing the current values of the ticked controls only.",
                )
                .clicked()
                || (submitted && name_ok && has_camera)
            {
                action = Some(ProfileAction::SaveAs);
            }
            if ui
                .button("Reload list")
                .on_hover_text(format!(
                    "Re-read the profiles folder: {}",
                    self.profiles.dir().display()
                ))
                .clicked()
            {
                action = Some(ProfileAction::Refresh);
            }
        });
        ui.horizontal(|ui| {
            let (selected, selectable) = self
                .camera
                .as_ref()
                .map(Camera::profile_selection_count)
                .unwrap_or((0, 0));
            ui.label(format!("Saving {selected} of {selectable} controls."))
                .on_hover_text(
                    "The tick box next to each control decides whether it goes into the profile.",
                );
            ui.label("Tick:");
            let mut choice = None;
            if ui
                .small_button("default")
                .on_hover_text("Vendor controls plus every active standard control; inactive (greyed) ones off.")
                .clicked()
            {
                choice = Some(ProfileSelection::Default);
            }
            if ui
                .small_button("all")
                .on_hover_text("Every writable control, including inactive ones.")
                .clicked()
            {
                choice = Some(ProfileSelection::All);
            }
            if ui
                .small_button("vendor only")
                .on_hover_text("Only the Anker vendor controls (FOV, HDR, Face AE, exposure smoothing, ...).")
                .clicked()
            {
                choice = Some(ProfileSelection::VendorOnly);
            }
            if ui
                .small_button("none")
                .on_hover_text("Untick everything; pick controls by hand.")
                .clicked()
            {
                choice = Some(ProfileSelection::None);
            }
            if let (Some(choice), Some(camera)) = (choice, self.camera.as_mut()) {
                camera.select_for_profile(choice);
            }
        });
        if let Some((message, ok)) = &self.profile_status {
            ui.colored_label(
                if *ok {
                    Color32::LIGHT_GREEN
                } else {
                    Color32::KHAKI
                },
                message,
            );
        }

        let selected = self
            .selected_profile
            .and_then(|i| self.profile_names.get(i).cloned());
        match action {
            Some(ProfileAction::Load) => {
                if let Some(name) = selected {
                    self.load_profile(&name);
                }
            }
            Some(ProfileAction::Overwrite) => {
                if let Some(name) = selected {
                    self.save_profile(&name);
                }
            }
            Some(ProfileAction::Delete) => {
                if let Some(name) = selected {
                    self.delete_profile(&name);
                }
            }
            Some(ProfileAction::SaveAs) => {
                if let Some(name) = sanitize_name(&self.new_profile_name) {
                    self.save_profile(&name);
                }
            }
            Some(ProfileAction::Refresh) => self.refresh_profiles(),
            None => {}
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Camera:");
            let current = self
                .devices
                .get(self.selected_device)
                .map(|d| d.label())
                .unwrap_or_else(|| "none found".to_string());
            let mut changed = None;
            egui::ComboBox::from_id_salt("camera")
                .selected_text(current)
                .width(320.0)
                .show_ui(ui, |ui| {
                    for (i, device) in self.devices.iter().enumerate() {
                        if ui.selectable_label(i == self.selected_device, device.label()).clicked() {
                            changed = Some(i);
                        }
                    }
                });
            if let Some(i) = changed {
                self.selected_device = i;
                self.open_selected();
            }
            if ui.button("Rescan").clicked() {
                self.rescan();
            }
            if ui
                .checkbox(&mut self.show_all_cameras, "Show all cameras")
                .on_hover_text("List any UVC camera with its standard controls. Anker vendor controls stay limited to the models they were verified on.")
                .changed()
            {
                self.rescan();
            }
            if ui.button("Reload values").clicked() {
                if let Some(camera) = self.camera.as_mut() {
                    camera.refresh_all();
                }
            }

        });
        ui.horizontal(|ui| {
            ui.label("Preview:");
            let mode_label = |m: &ffi::CapMode| {
                format!(
                    "{} {}x{} @{}",
                    ffi::fourcc_to_string(m.fourcc),
                    m.width,
                    m.height,
                    m.max_fps
                )
            };
            let current_mode = self
                .modes
                .get(self.selected_mode)
                .map(mode_label)
                .unwrap_or_default();
            let mut mode_changed = false;
            egui::ComboBox::from_id_salt("mode")
                .selected_text(current_mode)
                .show_ui(ui, |ui| {
                    for (i, mode) in self.modes.iter().enumerate() {
                        if ui
                            .selectable_label(i == self.selected_mode, mode_label(mode))
                            .clicked()
                            && i != self.selected_mode
                        {
                            self.selected_mode = i;
                            mode_changed = true;
                        }
                    }
                });
            if mode_changed && self.preview.is_some() {
                self.start_preview();
            }
            let running = self.preview.is_some();
            let button = if running {
                "Stop preview"
            } else if self.camera_busy {
                "Retry preview"
            } else {
                "Start preview"
            };
            if ui.button(button).clicked() {
                self.preview_wanted = !running;
                if running {
                    self.preview = None;
                    self.texture = None;
                } else {
                    self.start_preview();
                }
            }
        });
        if self.camera_busy {
            ui.colored_label(
                Color32::KHAKI,
                "Camera in use by another application: preview hidden, controls still work.",
            );
        }
    }

    fn control_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(error) = &self.open_error {
            ui.colored_label(Color32::LIGHT_RED, error);
            return;
        }
        let Some(camera) = self.camera.as_mut() else {
            ui.label("No compatible camera found. Plug in a PowerConf C200 or AnkerWork C310 and press Rescan, or tick \"Show all cameras\".");
            return;
        };
        if let Some(device) = self.devices.get(self.selected_device) {
            ui.label(format!(
                "{} · USB {:04x}:{:04x} · {}",
                device.name, device.usb_id.vendor_id, device.usb_id.product_id, device.path
            ));
        }
        ui.separator();

        // Column header: control on the left, the profile tick on the right.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.small("profile").on_hover_text(PROFILE_TICK_HELP);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.small("control");
                });
            });
        });

        let mut section = "";
        for index in 0..camera.controls.len() {
            let is_vendor = camera.controls[index].kind <= ffi::KIND_VENDOR_ENABLE_U8;
            let heading = if is_vendor {
                "Anker vendor controls"
            } else {
                "Standard UVC controls"
            };
            if heading != section {
                section = heading;
                ui.add_space(8.0);
                ui.heading(heading);
                ui.add_space(2.0);
            }
            ui.separator();
            let changed = {
                let control = &mut camera.controls[index];
                let title = control.name.replace('_', " ");
                let enabled = !control.inactive && !control.read_only;
                let mut changed = false;
                ui.horizontal_top(|ui| {
                    // Pin the profile tick to the right edge, then lay the control out in what is left.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.add_enabled(
                            !control.read_only,
                            egui::Checkbox::without_text(&mut control.include_in_profile),
                        )
                        .on_hover_text(PROFILE_TICK_HELP);
                        ui.add_space(6.0);
                        ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                            ui.label(egui::RichText::new(title).strong())
                                .on_hover_text(&control.description);
                            ui.add_enabled_ui(enabled, |ui| {
                                if let Some(value) = control.value.as_mut() {
                                    changed =
                                        control_widget(ui, &control.name, control.max_value, value);
                                }
                            });
                            if control.inactive {
                                ui.small("inactive: another control overrides it");
                            }
                            if let Some(error) = &control.error {
                                ui.colored_label(Color32::LIGHT_RED, error);
                            }
                        });
                    });
                });
                ui.add_space(2.0);
                changed
            };
            if changed {
                self.dirty.push(index);
            }
        }
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        let Some(preview) = self.preview.as_ref() else {
            ui.centered_and_justified(|ui| ui.label("Preview stopped"));
            return;
        };
        if let Some(frame) = preview.take_frame() {
            match self.texture.as_mut() {
                Some(texture) => texture.set(frame, TextureOptions::LINEAR),
                None => {
                    self.texture = Some(ui.ctx().load_texture(
                        "preview",
                        frame,
                        TextureOptions::LINEAR,
                    ))
                }
            }
        }
        let (error, width, height, fourcc, fps) = preview.status();
        if let Some(error) = error {
            ui.colored_label(Color32::LIGHT_RED, error);
        }
        if width > 0 {
            ui.small(format!(
                "{} {width}x{height} · {fps:.1} fps",
                ffi::fourcc_to_string(fourcc)
            ));
        }

        let Some(texture) = self.texture.as_ref() else {
            ui.centered_and_justified(|ui| ui.label("Waiting for first frame…"));
            return;
        };

        let available = ui.available_size();
        let image_size = texture.size_vec2();
        let scale = (available.x / image_size.x)
            .min(available.y / image_size.y)
            .max(0.01);
        let shown = image_size * scale;
        let response = ui.add(
            egui::Image::new(texture)
                .fit_to_exact_size(shown)
                .sense(Sense::drag()),
        );
        let rect = response.rect;

        // Metering box overlay, draggable, on the camera's fixed 3840x2160 grid.
        let Some(camera) = self.camera.as_mut() else {
            return;
        };
        let Some(index) = camera
            .controls
            .iter()
            .position(|c| c.kind == ffi::KIND_VENDOR_AREA_METERING)
        else {
            return;
        };
        let Some(ControlValue::AreaMetering(area)) = camera.controls[index].value.as_mut() else {
            return;
        };
        let grid_scale = Vec2::new(
            rect.width() / f32::from(ffi::AREA_GRID_WIDTH),
            rect.height() / f32::from(ffi::AREA_GRID_HEIGHT),
        );
        if area.width == 0 || area.height == 0 {
            return;
        }
        let box_rect = Rect::from_min_size(
            rect.min
                + Vec2::new(
                    f32::from(area.x) * grid_scale.x,
                    f32::from(area.y) * grid_scale.y,
                ),
            Vec2::new(
                f32::from(area.width) * grid_scale.x,
                f32::from(area.height) * grid_scale.y,
            ),
        );
        let color = if area.enable {
            Color32::YELLOW
        } else {
            Color32::from_gray(160)
        };
        let painter = ui.painter_at(rect);
        painter.rect_filled(box_rect, 0.0, color.gamma_multiply(0.12));
        painter.rect_stroke(
            box_rect,
            0.0,
            Stroke::new(2.0_f32, color),
            StrokeKind::Inside,
        );
        painter.text(
            box_rect.left_top() + Vec2::new(4.0, 2.0),
            egui::Align2::LEFT_TOP,
            if area.enable {
                "metering area (drag to move)"
            } else {
                "metering area (off)"
            },
            egui::FontId::proportional(12.0),
            color,
        );

        if response.dragged() {
            let delta = response.drag_delta();
            let max_x = ffi::AREA_GRID_WIDTH - area.width;
            let max_y = ffi::AREA_GRID_HEIGHT - area.height;
            let new_x = (f32::from(area.x) + delta.x / grid_scale.x)
                .round()
                .clamp(0.0, f32::from(max_x));
            let new_y = (f32::from(area.y) + delta.y / grid_scale.y)
                .round()
                .clamp(0.0, f32::from(max_y));
            area.x = new_x as u16;
            area.y = new_y as u16;
        }
        if response.drag_stopped() {
            self.dirty.push(index);
        }
    }
}

/// Honour ANKER_GUI_MODE, e.g. "MJPG 3840x2160", for the initial preview mode.
fn requested_mode(modes: &[ffi::CapMode]) -> Option<usize> {
    let wanted = std::env::var("ANKER_GUI_MODE").ok()?;
    let mut parts = wanted.split_whitespace();
    let fourcc = parts.next()?.to_ascii_uppercase();
    let (w, h) = parts.next()?.split_once('x')?;
    let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
    modes
        .iter()
        .position(|m| ffi::fourcc_to_string(m.fourcc) == fourcc && m.width == w && m.height == h)
}

/// Pick a preview mode: smooth (>= 24 fps) first, then closest to 720p, then raw YUYV over MJPEG.
fn default_mode(modes: &[ffi::CapMode]) -> usize {
    modes
        .iter()
        .enumerate()
        .filter(|(_, m)| (480..=1080).contains(&m.height))
        .max_by_key(|(_, m)| {
            (
                m.max_fps >= 24,
                -(m.height as i32 - 720).abs(),
                m.fourcc == ffi::FOURCC_YUYV,
                m.max_fps,
            )
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

impl App {
    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some((path, started, requested)) = self.screenshot.as_mut() else {
            return;
        };
        if !*requested && started.elapsed().as_secs_f32() > 3.0 {
            *requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = shot {
            let [w, h] = image.size;
            let bytes = image.as_raw();
            match image::save_buffer(&*path, bytes, w as u32, h as u32, image::ColorType::Rgba8) {
                Ok(()) => eprintln!("screenshot: wrote {path}"),
                Err(e) => eprintln!("screenshot: {e}"),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

enum ProfileAction {
    Load,
    Overwrite,
    Delete,
    SaveAs,
    Refresh,
}

/// A switch for feature on/off values, visually distinct from the square profile tick boxes.
fn toggle(ui: &mut egui::Ui, on: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let size = egui::vec2(36.0, 18.0);
        let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
        if response.clicked() {
            *on = !*on;
            response.mark_changed();
            changed = true;
        }
        let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let visuals = ui.style().interact_selectable(&response, *on);
        let rect = rect.expand(visuals.expansion);
        let radius = 0.5 * rect.height();
        ui.painter().rect(
            rect,
            radius,
            visuals.bg_fill,
            visuals.bg_stroke,
            StrokeKind::Inside,
        );
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        ui.painter().circle(
            egui::pos2(x, rect.center().y),
            0.75 * radius,
            visuals.fg_stroke.color,
            visuals.fg_stroke,
        );
        ui.label(if *on { "on" } else { "off" });
    });
    changed
}

/// Draw the widget for one control value. Returns true if the user changed it.
fn control_widget(ui: &mut egui::Ui, name: &str, max_value: u8, value: &mut ControlValue) -> bool {
    match value {
        ControlValue::VendorBool(v) | ControlValue::V4l2Bool(v) => toggle(ui, v),
        ControlValue::VendorU8(v) => ui.add(egui::DragValue::new(v).range(0..=255)).changed(),
        ControlValue::Fov(v) => {
            let mut changed = false;
            ui.horizontal(|ui| {
                for (preset, label) in FOV_PRESETS {
                    if ui.selectable_label(*v == preset, label).clicked() && *v != preset {
                        *v = preset;
                        changed = true;
                    }
                }
                ui.label("raw:");
                changed |= ui.add(egui::DragValue::new(v).range(1..=360)).changed();
            });
            ui.small(fov_label(*v));
            changed
        }
        ControlValue::AreaMetering(a) => {
            let mut changed = toggle(ui, &mut a.enable);
            if a.enable && (a.width == 0 || a.height == 0) {
                *a = ffi::AreaMetering {
                    enable: true,
                    x: 1600,
                    y: 900,
                    width: 640,
                    height: 360,
                };
            }
            ui.horizontal(|ui| {
                ui.label("x");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut a.x)
                            .range(0..=ffi::AREA_GRID_WIDTH - 1)
                            .speed(8),
                    )
                    .changed();
                ui.label("y");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut a.y)
                            .range(0..=ffi::AREA_GRID_HEIGHT - 1)
                            .speed(8),
                    )
                    .changed();
            });
            ui.horizontal(|ui| {
                ui.label("w");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut a.width)
                            .range(1..=ffi::AREA_GRID_WIDTH)
                            .speed(8),
                    )
                    .changed();
                ui.label("h");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut a.height)
                            .range(1..=ffi::AREA_GRID_HEIGHT)
                            .speed(8),
                    )
                    .changed();
                if ui.small_button("center").clicked() {
                    a.x = (ffi::AREA_GRID_WIDTH - a.width.min(ffi::AREA_GRID_WIDTH)) / 2;
                    a.y = (ffi::AREA_GRID_HEIGHT - a.height.min(ffi::AREA_GRID_HEIGHT)) / 2;
                    changed = true;
                }
            });
            // Keep the box inside the grid whatever was typed.
            a.width = a.width.clamp(1, ffi::AREA_GRID_WIDTH);
            a.height = a.height.clamp(1, ffi::AREA_GRID_HEIGHT);
            a.x = a.x.min(ffi::AREA_GRID_WIDTH - a.width);
            a.y = a.y.min(ffi::AREA_GRID_HEIGHT - a.height);
            ui.small("3840x2160 grid; drag the box on the preview to move it");
            changed
        }
        ControlValue::EnableU8(e) => {
            let mut changed = toggle(ui, &mut e.enable);
            let label = if name == "exposure_compensation" {
                "compensation (30 = neutral)"
            } else {
                "value"
            };
            ui.horizontal(|ui| {
                changed |= ui
                    .add(egui::Slider::new(&mut e.value, 0..=max_value).text(label))
                    .changed();
            });
            changed
        }
        ControlValue::V4l2Int {
            value,
            min,
            max,
            step,
            default,
        } => {
            let mut changed = false;
            ui.horizontal(|ui| {
                changed |= ui
                    .add(egui::Slider::new(value, *min..=*max).step_by(f64::from(*step)))
                    .changed();
                if ui.small_button("default").clicked() && value != default {
                    *value = *default;
                    changed = true;
                }
            });
            changed
        }
        ControlValue::V4l2Menu { value, items } => {
            let current = items
                .iter()
                .find(|(i, _)| i == value)
                .map(|(_, n)| n.clone())
                .unwrap_or_else(|| value.to_string());
            let mut changed = false;
            egui::ComboBox::from_id_salt(name)
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (i, label) in items.iter() {
                        if ui.selectable_label(i == value, label).clicked() && i != value {
                            *value = *i;
                            changed = true;
                        }
                    }
                });
            changed
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_screenshot(ctx);
        if self.preview.as_ref().is_some_and(|p| p.is_busy()) {
            self.preview = None;
            self.texture = None;
            self.camera_busy = true;
        }
        if self.preview_wanted
            && self.preview.is_none()
            && !self.camera_busy
            && self.camera.is_some()
            && !self.modes.is_empty()
        {
            self.start_preview();
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(4.0);
            self.top_bar(ui);
            ui.add_space(2.0);
            self.profile_bar(ui);
            ui.add_space(4.0);
        });
        if self.camera_busy {
            // No preview to show: let the controls take the whole window.
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.set_max_width(520.0);
                    self.control_panel(ui);
                    ui.add_space(12.0);
                });
            });
        } else {
            egui::SidePanel::left("controls")
                .default_width(430.0)
                .min_width(340.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.control_panel(ui);
                        ui.add_space(12.0);
                    });
                });
            egui::CentralPanel::default().show(ctx, |ui| {
                self.preview_panel(ui);
            });
        }

        self.flush_dirty(ctx);
    }
}
