# Cider Listen Together

[![Rust](https://img.shields.io/badge/Rust-f74c00?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Swift](https://img.shields.io/badge/Swift-FA7343?style=for-the-badge&logo=swift&logoColor=white)](https://swift.org/)
[![C#](https://img.shields.io/badge/C%23-512BD4?style=for-the-badge&logo=csharp&logoColor=white)](https://docs.microsoft.com/en-us/dotnet/csharp/)
[![macOS](https://img.shields.io/badge/macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://developer.apple.com/macos/)
[![Windows](https://img.shields.io/badge/Windows-0078D4?style=for-the-badge&logo=windows&logoColor=white)](https://www.microsoft.com/windows)
[![libp2p](https://img.shields.io/badge/libp2p-469EA2?style=for-the-badge&logo=libp2p&logoColor=white)](https://libp2p.io/)

Listen to music together with friends using [Cider](https://cider.sh). One person hosts a room, others join, and everyone's music stays in sync.

## Features

- **P2P Sync** - No server required, direct peer-to-peer connection via libp2p
- **Cross-Platform** - Native apps for macOS (SwiftUI) and Windows (WinUI 3)
- **Real-time** - Sub-second synchronization

> **Why no Linux?** Each app is built with native UI frameworks (SwiftUI, WinUI 3), and I don't use Linux personally. PRs welcome!

## How It Works

1. Everyone needs [Cider](https://cider.sh) running with an Apple Music subscription
2. One person creates a room and shares the 8-character code
3. Others join with the code
4. The host controls playback, everyone stays in sync

## Building

### Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- Xcode 26+ (for macOS)
- Visual Studio 2026 with .NET 10 and Windows App SDK (for Windows)

### macOS

```bash
# Build Rust library and generate Swift bindings
make macos

# Open in Xcode
make xcode
```

### Windows

```bash
# Build Rust library and generate C# bindings
make windows

# Open in Visual Studio
start apps/windows/CiderTogether/CiderTogether.sln
```

### Rebuild Core Library

After making changes to `cider-core/`:

```bash
# Rebuild and copy to both platforms
make all
```

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
│   └── windows/             # WinUI 3 app
└── Makefile                 # Build commands
```

## Cider API

This app uses Cider's REST API on `localhost:10767`. Make sure to:
1. Enable the API in Cider: Settings → Connectivity → Manage External Application Access
2. Generate an API token (or disable authentication for local use)
