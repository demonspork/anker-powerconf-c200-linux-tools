# Fedora / RPM spec. Build offline with the Rust packaging macros:
#   packaging/fedora/make-sources.sh     # produces Source0 and the vendored crates tarball
#   rpmbuild -ba packaging/fedora/anker-webcam-tools.spec
# For an official Fedora package, generate crate dependencies with rust2rpm instead of vendoring.

%global appid io.github.demonspork.AnkerWebcamGui
%global srcname anker-powerconf-c200-linux-tools

Name:           anker-webcam-tools
Version:        0.3.0
Release:        1%{?dist}
Summary:        CLI and GUI for Anker PowerConf C200 / AnkerWork C310 webcam vendor controls
License:        MIT
URL:            https://github.com/demonspork/%{srcname}
Source0:        %{url}/archive/v%{version}/%{srcname}-%{version}.tar.gz
# Produced by make-sources.sh with `cargo vendor` inside gui/
Source1:        %{name}-%{version}-vendor.tar.xz

BuildRequires:  cargo-rpm-macros >= 24
BuildRequires:  gcc
BuildRequires:  make
BuildRequires:  desktop-file-utils
BuildRequires:  libappstream-glib
# Runtime libraries the GUI loads with dlopen (not detected by RPM's auto-requires)
Requires:       libxkbcommon
Requires:       libwayland-client
Requires:       libglvnd-egl
Requires:       libglvnd-opengl
Requires:       libX11
Requires:       libXcursor
Requires:       libXi
Requires:       libXrandr
Requires:       hicolor-icon-theme

%description
Reverse-engineered vendor controls for Anker PowerConf C200 and AnkerWork C310
webcams on Linux: field of view presets, HDR, horizontal flip, anti-flicker,
exposure smoothing (area metering), Face AE and exposure compensation, next to
the standard UVC controls. Ships a command-line tool and a desktop GUI with a
live preview and settings profiles.

%prep
%autosetup -n %{srcname}-%{version}
pushd gui
%cargo_prep -v vendor
popd

%build
%make_build
pushd gui
%cargo_build
popd

%install
%make_install PREFIX=%{_prefix}
%make_install PREFIX=%{_prefix} install-gui CARGO_FLAGS="--frozen"

%check
%make_build test
pushd gui
%cargo_test
popd
desktop-file-validate %{buildroot}%{_datadir}/applications/%{appid}.desktop
appstream-util validate-relax --nonet %{buildroot}%{_metainfodir}/%{appid}.metainfo.xml

%files
%license LICENSE
%doc README.md
%{_bindir}/anker-powerconf-c200-linux-tools
%{_bindir}/anker-webcam-gui
%{_datadir}/applications/%{appid}.desktop
%{_metainfodir}/%{appid}.metainfo.xml
%{_datadir}/icons/hicolor/*/apps/%{appid}.*

%changelog
* Tue Oct 20 2026 Michael Suiter <demonspork@gmail.com> - 0.3.0-1
- Handle the C310's self-restart on vertical_screen writes; GUI reconnects
- Profile tick column, pill switches, own title row on bare compositors
- Portable release tarball (make dist) with a glibc 2.39 ceiling

* Sun Oct 19 2026 Michael Suiter <demonspork@gmail.com> - 0.2.0-1
- CLI with C310 controls and apply-profile, egui GUI with live preview and profiles
