.PHONY: all build build-release sign swift-bindings csharp-bindings clean macos windows embed-dylib embed-dll xcode test

# Paths - macOS
DYLIB_SRC = target/release/libcider_core.dylib
DYLIB_DST = apps/macos/CiderTogether/Frameworks/libcider_core.dylib
XCODE_PROJECT = apps/macos/CiderTogether/CiderTogether.xcodeproj

# Paths - Windows
DLL_SRC = target/release/cider_core.dll
DLL_DST = apps/windows/CiderTogether/CiderTogether/CiderTogether/Bridge/cider_core.dll

# Default: build for current platform
all: build-release
ifeq ($(OS),Windows_NT)
	$(MAKE) csharp-bindings embed-dll
else
	$(MAKE) sign swift-bindings embed-dylib
endif
	@echo ""
	@echo "Done! Library rebuilt for current platform."

# Build debug
build:
	cargo build

# Build release
build-release:
	cargo build --release

# === macOS ===

# Fix install name and sign the dylib
sign:
	@echo "Fixing install name..."
	install_name_tool -id @rpath/libcider_core.dylib $(DYLIB_SRC)
	@echo "Signing dylib..."
	@IDENTITY=$$(security find-identity -v -p codesigning 2>/dev/null | grep "Apple Development" | head -1 | sed 's/.*"\(.*\)".*/\1/'); \
	if [ -z "$$IDENTITY" ]; then IDENTITY="-"; fi; \
	codesign --force --sign "$$IDENTITY" $(DYLIB_SRC)

# Generate Swift bindings
swift-bindings:
	cargo run --bin uniffi-bindgen generate \
		--library $(DYLIB_SRC) \
		--language swift \
		--out-dir apps/macos/CiderTogether/CiderTogether/Bridge
	cp apps/macos/CiderTogether/CiderTogether/Bridge/cider_coreFFI.h \
		apps/macos/CiderTogether/CiderTogether/

# Copy dylib to Xcode project for embedding
embed-dylib:
	@echo "Copying dylib for embedding..."
	mkdir -p apps/macos/CiderTogether/Frameworks
	cp $(DYLIB_SRC) $(DYLIB_DST)
	@echo "Dylib ready at $(DYLIB_DST)"

# Build everything for macOS
macos: build-release sign swift-bindings embed-dylib
	@echo ""
	@echo "Done! Open Xcode: make xcode"

# Open Xcode
xcode:
	open $(XCODE_PROJECT)

# === Windows ===

# Generate C# bindings
csharp-bindings:
	cargo run --bin uniffi-bindgen generate \
		--library $(DLL_SRC) \
		--language csharp \
		--out-dir apps/windows/CiderTogether/CiderTogether/CiderTogether/Bridge

# Copy DLL to Windows project
embed-dll:
	@echo "Copying DLL..."
ifeq ($(OS),Windows_NT)
	copy /Y "$(subst /,\,$(DLL_SRC))" "$(subst /,\,$(DLL_DST))"
else
	cp $(DLL_SRC) $(DLL_DST)
endif
	@echo "DLL ready at $(DLL_DST)"

# Build everything for Windows
windows: build-release csharp-bindings embed-dll
	@echo ""
	@echo "Done! Open Visual Studio: start apps/windows/CiderTogether/CiderTogether.sln"

# === Common ===

# Clean
clean:
	cargo clean
	rm -rf apps/macos/CiderTogether/Frameworks

# Run tests
test:
	cargo test
