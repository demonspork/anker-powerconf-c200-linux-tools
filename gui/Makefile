# Build and install the desktop GUI. Packaging-friendly: honours DESTDIR and PREFIX,
# and CARGO_FLAGS (e.g. --frozen or --offline) for builds without network access.

PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
DESTDIR ?=
INSTALL ?= install
CARGO ?= cargo
CARGO_FLAGS ?=

APP_ID := io.github.demonspork.AnkerWebcamGui
BIN := anker-webcam-gui
ICON_SIZES := 16 24 32 48 64 128 256 512

.PHONY: all test install clean

all:
	$(CARGO) build --release $(CARGO_FLAGS)

test:
	$(CARGO) test --release $(CARGO_FLAGS)

install: all
	$(INSTALL) -Dm755 target/release/$(BIN) $(DESTDIR)$(BINDIR)/$(BIN)
	$(INSTALL) -Dm644 assets/$(APP_ID).desktop $(DESTDIR)$(DATADIR)/applications/$(APP_ID).desktop
	$(INSTALL) -Dm644 assets/$(APP_ID).metainfo.xml $(DESTDIR)$(DATADIR)/metainfo/$(APP_ID).metainfo.xml
	$(INSTALL) -Dm644 assets/$(APP_ID).svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/$(APP_ID).svg
	for s in $(ICON_SIZES); do \
		$(INSTALL) -Dm644 assets/icons/$$s.png $(DESTDIR)$(DATADIR)/icons/hicolor/$${s}x$${s}/apps/$(APP_ID).png || exit 1; \
	done

clean:
	$(CARGO) clean
