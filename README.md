# Cider Listen Together

[![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Swift](https://img.shields.io/badge/Swift-FA7343?style=flat&logo=swift&logoColor=white)](https://swift.org/)
[![macOS](https://img.shields.io/badge/macOS-Native-000000?style=flat&logo=apple&logoColor=white)](https://developer.apple.com/macos/)
[![libp2p](https://img.shields.io/badge/libp2p-P2P-blue?style=flat)](https://libp2p.io/)

Listen to music together with friends using [Cider](https://cider.sh). One person hosts a room, others join, and everyone's music stays in sync.

## Features

- **P2P Sync** - No server required, direct peer-to-peer connection via libp2p
- **Cross-Platform** - macOS, Linux, and Windows (native UI on each)
- **Host Transfer** - Pass control to any participant
- **Real-time** - Sub-second synchronization

## How It Works

1. Everyone needs [Cider](https://cider.sh) running with an Apple Music subscription
2. One person creates a room and shares the 6-character code
3. Others join with the code
4. The host controls playback, everyone stays in sync

## Building

### Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- Xcode 15+ (for macOS)
- GTK4 (for Linux)

### macOS

```bash
# Build everything, sign, and generate Swift bindings
make macos

# Open in Xcode
make xcode
```

### Manual Build

```bash
# Build Rust library
cargo build --release

# Sign dylibs (required for macOS)
# Find your signing identity:
security find-identity -v -p codesigning

# Sign all copies:
find target -name "libcider_core.dylib" -exec codesign --force --sign "YOUR_IDENTITY" {} \;

# Generate Swift bindings
cargo run --bin uniffi-bindgen generate \
    --library target/release/libcider_core.dylib \
    --language swift \
    --out-dir apps/macos/CiderTogether/CiderTogether/Bridge
```

### Xcode Setup

After building the Rust library:

1. Open `apps/macos/CiderTogether/CiderTogether.xcodeproj`
2. Build Settings:
   - **Library Search Paths**: `$(PROJECT_DIR)/../../../../target/release`
   - **Header Search Paths**: `$(PROJECT_DIR)/CiderTogether`
   - **Objective-C Bridging Header**: `CiderTogether/CiderTogether-Bridging-Header.h`
3. Build Phases → Link Binary With Libraries:
   - Add `libcider_core.dylib` from `target/release/`
4. Build Phases → Copy Files (Destination: Frameworks):
   - Add `libcider_core.dylib`
   - Check "Code Sign On Copy"
5. Signing & Capabilities:
   - Remove App Sandbox (or enable Outgoing Connections)

## Project Structure

```
cider-listen-together/
├── cider-core/              # Rust core library
│   ├── src/
│   │   ├── cider/           # Cider API client
│   │   ├── network/         # libp2p P2P networking
│   │   ├── sync/            # Sync protocol
│   │   └── ffi/             # uniffi bindings
├── apps/
│   ├── macos/               # SwiftUI app
│   ├── linux/               # GTK4 app (planned)
│   └── windows/             # WinUI app (planned)
└── Makefile                 # Build commands
```

## Cider API

This app uses Cider's REST API on `localhost:10767`. Make sure to:
1. Enable the API in Cider: Settings → Connectivity → Manage External Application Access
2. Generate an API token (or disable authentication for local use)

