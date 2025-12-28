.PHONY: build build-release sign swift-bindings clean macos embed-dylib

# Paths
DYLIB_SRC = target/release/libcider_core.dylib
DYLIB_DST = apps/macos/CiderTogether/Frameworks/libcider_core.dylib
XCODE_PROJECT = apps/macos/CiderTogether/CiderTogether.xcodeproj

# Default: build release, sign, generate bindings, embed dylib
all: build-release sign swift-bindings embed-dylib

# Build debug
build:
	cargo build

# Build release
build-release:
	cargo build --release

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
	@echo "Done! Now configure Xcode:"
	@echo "1. Add Frameworks/libcider_core.dylib to the project"
	@echo "2. Add it to 'Link Binary With Libraries'"
	@echo "3. Add 'Copy Files' build phase -> Frameworks -> libcider_core.dylib"
	@echo ""
	@echo "Then open: $(XCODE_PROJECT)"

# Open Xcode
xcode:
	open $(XCODE_PROJECT)

# Clean
clean:
	cargo clean
	rm -rf apps/macos/CiderTogether/Frameworks

# Run tests
test:
	cargo test
