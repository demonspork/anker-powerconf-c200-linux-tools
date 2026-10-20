# anker-powerconf-c200-linux

Linux CLI tools for the Anker PowerConf C200 webcam.

This project reverse engineers the vendor camera controls used by the official Anker app and exposes them as a small command-line utility for Linux.

## Status

Confirmed on an Anker PowerConf C200 with USB VID:PID `291a:3369`.

Also works on an AnkerWork C310 with USB VID:PID `291a:3367` (firmware V2.21), which shares the same vendor extension unit.

The current binary is:

```sh
./build/anker-powerconf-c200-linux-tools
```

## Features

- Read and set the vendor FOV presets on Linux
- Access additional vendor-only controls that are not exposed as normal V4L2 controls
- Access standard V4L2/UVC controls through the same CLI

Currently exposed controls:

- `fov`
- `hdr`
- `horizontal_flip`
- `vertical_screen`
- `anti_flicker`
- `exposure_smoothing` (alias `area_metering`) – C310 only
- `exposure_compensation` – C310 only
- `face_exposure` – C310 only
- `brightness`
- `contrast`
- `saturation`
- `hue`
- `white_balance_automatic`
- `gamma`
- `power_line_frequency`
- `white_balance_temperature`
- `sharpness`
- `auto_exposure`
- `exposure_time_absolute`
- `pan_absolute`
- `tilt_absolute`
- `focus_absolute`
- `focus_automatic_continuous`
- `zoom_absolute`

`list-controls` prints which models each control applies to. Before touching a model-specific control the CLI reads the camera's USB vendor/product ID from sysfs and refuses to send it to any other device, so the C310-only selectors are never issued to a PowerConf C200 (or to a non-Anker camera):

```
$ ./build/anker-powerconf-c200-linux-tools --device /dev/video0 set face_exposure on
control face_exposure is only supported on AnkerWork C310; /dev/video0 is a PowerConf C200 (USB 291a:3369)
```

## Reverse Engineering Notes

The C200 uses a UVC Extension Unit for at least some vendor-specific camera controls.

Confirmed FOV control path:

- Unit ID: `0x06`
- Selector: `0x10`
- Linux `GET_INFO`: `0x03`
- Linux `GET_LEN`: `60`

FOV payload layout:

- bytes `0..1`: `0x0100` little-endian when setting
- bytes `2..3`: FOV value as `uint16_le`
- byte `4`: `0x00`

Known FOV presets:

- `65`: narrow
- `78`: medium
- `95`: wide

Other confirmed vendor selectors currently wired into the CLI:

- `0x0a`: `vertical_screen`
- `0x11`: `horizontal_flip`
- `0x12`: `anti_flicker`
- `0x13`: `hdr`
- `0x18`: `exposure_smoothing` (area metering) – C310 only
- `0x19`: `exposure_compensation` (vendor auto exposure + compensation) – C310 only
- `0x0b`: `face_exposure` (FACE AE) – C310 only

The three C310-only selectors were taken from the AnkerWork desktop app and verified on a C310. They have not been tested on a C200, so the CLI only accepts them when the connected camera reports USB ID `291a:3367`.

### Exposure Smoothing (Area Metering)

The AnkerWork app labels this control "Exposure Smoothing" in English, but internally it is `setAreaMetering()` / `getAreaMetering()` (the Chinese UI string is 区域测光, "area metering"). The camera meters exposure from the average brightness inside a rectangle instead of the whole frame.

Confirmed by disassembling `AWDeviceInterface.dll` from AnkerWork 3.0.4 for Windows:

- Unit ID: `0x06`
- Selector: `0x18`
- Linux `GET_LEN`: `9`

Payload layout:

- byte `0`: enable (`0x00` / `0x01`)
- bytes `1..2`: x as `uint16_le`
- bytes `3..4`: y as `uint16_le`
- bytes `5..6`: width as `uint16_le`
- bytes `7..8`: height as `uint16_le`

The rectangle is always expressed on a fixed `3840x2160` grid regardless of the streaming resolution. The app's default region is `640x360` centered, i.e. `x=1600 y=900 width=640 height=360`.

### Exposure Compensation and Face AE

Two more selectors from the same DLL share a 2-byte `{ enable, value }` layout:

- `0x19`: `setAutoExposure(enable, compensation)` – vendor auto exposure. `compensation` is 0..60 and the app treats 30 as neutral; lower values darken the image. Exposed as `exposure_compensation`.
- `0x0b`: `setAutoFaceExposure(enable, value)` – FACE AE. The camera finds the face itself and meters on it, so unlike `exposure_smoothing` it keeps working when you move. Exposed as `face_exposure`.

If your face is washed out against a darker background (classic backlit metering), `face_exposure on` is the automatic fix. `exposure_smoothing` with a rectangle over your face works too but only while you stay inside the box.

Other selectors seen in the same DLL but not yet wired in:

- `0x1c`: frame offset (`x, y, width, height` as `uint16_le`, 8 bytes)
- `0x0c`: firmware version string, read-only
- `0x0e`: serial number string, read-only

### AnkerWork C310 (A3367)

The C310 (`291a:3367`) exposes the same unit `0x06` extension unit GUID (`41769ea2-04de-e347-8b2b-f4341aff003b`) and the C200 selectors above work on it unchanged. `exposure_smoothing`, `exposure_compensation` and `face_exposure` are gated to the C310. It also carries three further extension units (IDs `3`, `7`, `8`) that this tool does not touch. Avoid issuing `GET_CUR` on unit `8` selector `0x08` (a 512-byte control): on firmware V2.21 it times out and leaves the camera unresponsive until it is power-cycled.

## GUI

`gui/` holds a small desktop app (Rust + egui) with a live preview and a panel for every control the connected model supports. It links the C sources in `src/` directly, so the CLI and the GUI share one control table and one protocol implementation, including the per-model gating.

```sh
cd gui
cargo build --release
./target/release/anker-webcam-gui
```

The binary depends only on libc at link time and uses the system's Wayland or X11 and OpenGL libraries at runtime, so it runs unchanged on KDE, GNOME and wlroots compositors such as Hyprland. It needs no GTK or Qt.

Features:

- Lists every connected PowerConf C200 / AnkerWork C310 (tick "Show all cameras" to manage the standard UVC controls of any other webcam).
- Live preview in YUYV or MJPEG at any mode the camera offers; MJPEG is decoded in pure Rust.
- The exposure smoothing rectangle is drawn over the preview and can be dragged into place; the size is set with the spinners.
- Standard UVC controls show the driver's range and default, and grey out when another control overrides them (manual exposure under auto exposure, for example).
- Values are written when you release the slider or box, then read back so the panel shows what the camera accepted.
- Settings profiles: save the camera's current settings under a name, load them back, overwrite or delete them. Profiles live in `~/.config/anker-webcam-gui/profiles/*.profile` (or under `$XDG_CONFIG_HOME`).

### Profiles

A profile is a plain text file of `control = value` lines using the same value syntax as the CLI, for example:

```
# Anker webcam settings profile
model = AnkerWork C310
fov = 95
hdr = off
face_exposure = on,35
exposure_compensation = on,30
exposure_smoothing = off,1600,900,640,360
auto_exposure = 3
brightness = 0
```

**What gets saved.** Each control line in the panel has a tick box. "Save as" and "Overwrite" write only the ticked controls, so a profile pins exactly what you chose and nothing else. The row under the profile bar shows how many controls are ticked and offers four quick selections:

- `default`: the Anker vendor controls plus every active standard control. Inactive controls (greyed out because another control overrides them, like manual exposure time under auto exposure) are left out. This is the selection when the app starts.
- `all`: every writable control, inactive ones included.
- `vendor only`: just the Anker controls (FOV, HDR, flip, anti-flicker, exposure smoothing, Face AE, exposure compensation).
- `none`: untick everything and pick by hand.

The tick boxes form their own column at the right edge of the panel, under a "profile" header, so they are easy to tell apart from the on/off switches that belong to the controls themselves. Read-only controls can never be ticked. Hovering the tick box, the buttons and the quick selections shows the same explanations as tooltips.

**What loading does.** "Load" applies only the controls listed in the file; every other control keeps its current value. Mode switches (menus and toggles) are applied before dependent values, controls the current camera does not have are skipped and reported in the status line, and the panel re-reads everything afterwards. Load also ticks exactly the controls the file contains, so "Overwrite" after a Load writes the same set back, and a profile you edited by hand stays in step with the GUI.

**Profiles layer.** Because a profile only touches what it lists, you can keep a small "base" profile (say FOV and HDR) for the udev rule to apply on every plug-in, and separate situational profiles (Face AE and compensation for a backlit desk) to load on top. The CLI applies the same files:

```sh
./build/anker-powerconf-c200-linux-tools --device /dev/video0 apply-profile ~/.config/anker-webcam-gui/profiles/meeting.profile
```

Because the vendor settings reset on every replug, pointing the udev rule below at a profile re-applies all of them at once. `ANKER_GUI_PROFILE=meeting` loads a profile when the GUI starts.

### Installing

`make install` in `gui/` installs the binary, the `.desktop` entry, the AppStream metainfo and the icon (SVG plus hicolor PNG sizes) under `PREFIX` (default `/usr/local`), honouring `DESTDIR`. From the repository root:

```sh
make                              # CLI
make gui                          # GUI (cargo build --release)
sudo make install install-gui     # both, PREFIX=/usr/local
```

Packaging files are in `packaging/`:

- `packaging/arch/PKGBUILD`: a `-git` package; `cd packaging/arch && makepkg -si`.
- `packaging/fedora/anker-webcam-tools.spec`: uses the Fedora Rust macros with a vendored crate tarball; run `packaging/fedora/make-sources.sh` (needs the `v<version>` tag) then `rpmbuild -ba packaging/fedora/anker-webcam-tools.spec`.

The preview holds the capture node open, so stop it (or close the app) before a video call needs the camera. If another application already holds the camera, the preview pane is hidden and the controls still work.

The preview mode list is ordered largest first; on a C310 that puts `MJPG 3840x2160 @30` at the top. Three environment variables tweak startup: `ANKER_GUI_MODE="MJPG 3840x2160"` picks the initial preview mode, `ANKER_GUI_PROFILE=name` loads a profile, and `ANKER_GUI_SHOW_ALL=1` starts with "Show all cameras" ticked. For bug reports, `ANKER_GUI_SCREENSHOT=/path/shot.png` saves a picture of the window after three seconds and exits.

## Releases

Prebuilt binaries are published on the GitHub Releases page:

- [GitHub Releases](https://github.com/erans/anker-powerconf-c200-linux-tools/releases)

Download the release tarball for your architecture if you do not want to build from source.

## Build

```sh
make
```

Run tests:

```sh
make test
```

## Usage

List controls:

```sh
./build/anker-powerconf-c200-linux-tools list-controls
```

Read a control:

```sh
./build/anker-powerconf-c200-linux-tools get fov
./build/anker-powerconf-c200-linux-tools get hdr
./build/anker-powerconf-c200-linux-tools get brightness
```

Apply a saved profile (see the GUI section for the file format):

```sh
./build/anker-powerconf-c200-linux-tools apply-profile meeting.profile
```

Set a control:

```sh
./build/anker-powerconf-c200-linux-tools set fov narrow
./build/anker-powerconf-c200-linux-tools set fov wide
./build/anker-powerconf-c200-linux-tools set hdr on
./build/anker-powerconf-c200-linux-tools set horizontal_flip off
./build/anker-powerconf-c200-linux-tools set brightness 60
```

Exposure smoothing takes an on/off value and an optional metering rectangle on the `3840x2160` grid. Without a rectangle, `on` keeps the camera's current rectangle (or the app default `1600 900 640 360` if none is set):

```sh
./build/anker-powerconf-c200-linux-tools get exposure_smoothing
./build/anker-powerconf-c200-linux-tools set exposure_smoothing on
./build/anker-powerconf-c200-linux-tools set exposure_smoothing on 1600 900 640 360
./build/anker-powerconf-c200-linux-tools set exposure_smoothing on,1600,900,640,360
./build/anker-powerconf-c200-linux-tools set exposure_smoothing off
```

Face AE and exposure compensation take `on`/`off`, optionally followed by a value (0..60). A bare number changes only the value:

```sh
./build/anker-powerconf-c200-linux-tools get face_exposure
./build/anker-powerconf-c200-linux-tools set face_exposure on
./build/anker-powerconf-c200-linux-tools set exposure_compensation on 20
./build/anker-powerconf-c200-linux-tools set exposure_compensation 25
```

### Re-applying settings automatically

The vendor controls do not survive a power cycle: after a replug the camera comes back with `exposure_smoothing` cleared, `face_exposure` off and `exposure_compensation` at 30. To re-apply your preferred settings whenever the camera is plugged in, install the binary somewhere stable and add a udev rule, for example `/etc/udev/rules.d/99-anker-webcam.rules`:

```
ACTION=="add", SUBSYSTEM=="video4linux", ATTRS{idVendor}=="291a", ATTRS{idProduct}=="3367", ATTR{index}=="0", \
  RUN+="/usr/local/bin/anker-powerconf-c200-linux-tools --device /dev/%k set face_exposure on"
```

To restore a whole saved profile instead of one control, copy it somewhere root can read (udev runs as root, before your home directory is necessarily available) and use `apply-profile`:

```
ACTION=="add", SUBSYSTEM=="video4linux", ATTRS{idVendor}=="291a", ATTRS{idProduct}=="3367", ATTR{index}=="0", \
  RUN+="/usr/local/bin/anker-powerconf-c200-linux-tools --device /dev/%k apply-profile /etc/anker-webcam/c310.profile"
```

Use `idProduct` `3369` for the C200. `ATTR{index}=="0"` restricts the rule to the capture node (the second node of each UVC camera is metadata only and rejects extension-unit queries). Reload with `sudo udevadm control --reload` and replug the camera.

Use a different device node:

```sh
./build/anker-powerconf-c200-linux-tools --device /dev/video0 get fov
```

Compatibility aliases are also available:

```sh
./build/anker-powerconf-c200-linux-tools get-fov
./build/anker-powerconf-c200-linux-tools set-fov narrow
```

## Notes

- The standard `zoom_absolute` control is separate from the vendor FOV preset control.
- On this machine, `/dev/video0` is the node that accepts the camera extension-unit queries.
- The vendor macOS code uses a 64-byte staging buffer for FOV, but Linux reports the real control length as `60`, and the Linux ioctl path must use the reported length.
