# Cider Listen Together

[![Rust](https://img.shields.io/badge/Rust-f74c00?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Swift](https://img.shields.io/badge/Swift-FA7343?style=for-the-badge&logo=swift&logoColor=white)](https://swift.org/)
[![C#](https://img.shields.io/badge/C%23-512BD4?style=for-the-badge&logo=csharp&logoColor=white)](https://docs.microsoft.com/en-us/dotnet/csharp/)
[![macOS](https://img.shields.io/badge/macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://developer.apple.com/macos/)
[![Windows](https://img.shields.io/badge/Windows-0078D4?style=for-the-badge&logo=windows&logoColor=white)](https://www.microsoft.com/windows)
[![libp2p](https://img.shields.io/badge/libp2p-469EA2?style=for-the-badge&logo=libp2p&logoColor=white)](https://libp2p.io/)
[![Relay](https://img.shields.io/uptimerobot/status/m802156379-276a215c7896ece157c9a450?style=for-the-badge&label=Relay)](https://stats.uptimerobot.com/FvhdrIGkHE)

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

<details>
<summary><h2>Nerd mode</h2></summary>

### Build Pipeline

```mermaid
flowchart LR
    subgraph src["Source"]
        rust["Rust + #[uniffi::export]"]
    end

    subgraph build["cargo build --release"]
        compile["rustc → LLVM"]
    end

    subgraph artifacts["Artifacts"]
        dylib["libcider_core.dylib<br/>(macOS arm64)"]
        dll["cider_core.dll<br/>(Windows x64)"]
    end

    subgraph bindgen["uniffi-bindgen generate"]
        swift["cider_core.swift"]
        header["cider_coreFFI.h"]
        csharp["CiderCore.cs"]
    end

    subgraph sign["Code Signing"]
        codesign["codesign --sign 'Apple Development'"]
    end

    subgraph apps["Native Apps"]
        xcode["Xcode → .app bundle"]
        vs["Visual Studio → .msix"]
    end

    rust --> compile --> dylib & dll
    dylib --> bindgen --> swift & header
    dll --> bindgen --> csharp
    dylib --> codesign --> xcode
    swift & header --> xcode
    dll & csharp --> vs
```

### libp2p Protocol Stack

The `CiderBehaviour` struct composes 6 libp2p protocols:

```mermaid
flowchart TB
    subgraph Application["Application Layer"]
        session["Session (uniffi::Object)"]
        network["NetworkManager (tokio task)"]
    end

    subgraph Swarm["libp2p::Swarm&lt;CiderBehaviour&gt;"]
        subgraph Behaviours["#[derive(NetworkBehaviour)]"]
            gossipsub["gossipsub::Behaviour<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>Pub/sub room messaging<br/>Topic: cider-room-{code}"]
            relay["relay::client::Behaviour<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>Circuit Relay v2<br/>NAT traversal"]
            dcutr["dcutr::Behaviour<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>Direct Connection Upgrade<br/>Hole punching after relay"]
            mdns["mdns::tokio::Behaviour<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>LAN discovery<br/>Multicast 224.0.0.251:5353"]
            identify["identify::Behaviour<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>Protocol negotiation<br/>/cider-together/1.0.0"]
            ping["ping::Behaviour<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>Keep-alive<br/>Interval: 15s"]
        end
    end

    subgraph Transport["Transport Layer"]
        tcp["TCP<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>+ Noise (encryption)<br/>+ Yamux (multiplexing)"]
        quic["QUIC<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>Built-in TLS 1.3<br/>UDP-based, 0-RTT"]
    end

    session --> network
    network --> Swarm
    Behaviours --> tcp & quic
```

### Peer Discovery: 2-Layer Strategy

```mermaid
flowchart LR
    subgraph Local["Layer 1: LAN"]
        mdns_d["mDNS<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>224.0.0.251:5353<br/>Instant discovery"]
    end

    subgraph Signal["Layer 2: Signaling"]
        ntfy["ntfy.sh HTTP<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>POST/GET polling<br/>Relay addresses"]
    end

    mdns_d -->|"~0ms"| connect["Dial Peer"]
    ntfy -->|"~500ms"| connect
```

> **Architecture note:** This is a WebRTC-style architecture using libp2p primitives. The signaling layer (ntfy.sh) exchanges relay addresses, the relay server enables NAT traversal, and DCUtR performs hole punching for direct connections.

### Connection Flow

```mermaid
sequenceDiagram
    participant H as Host
    participant R as Relay Server
    participant S as ntfy.sh
    participant L as Listener

    Note over H: create_room("ABCD-1234")

    H->>R: Connect + Reserve relay slot
    R-->>H: ReservationReqAccepted
    H->>S: POST /cider-together-abcd1234<br/>{"peer_id", "relay_addresses"}

    Note over L: join_room("ABCD-1234")

    L->>R: Connect + Reserve relay slot
    R-->>L: ReservationReqAccepted
    L->>S: GET /cider-together-abcd1234/json
    S-->>L: {"peer_id", "relay_addresses"}

    alt Same LAN (mDNS found peer)
        L->>H: TCP/QUIC direct
    else Behind NAT (most common)
        L->>R: Connect via circuit<br/>/p2p-circuit/p2p/{host_id}
        R->>H: Relay connection
        Note over L,H: DCUtR hole punch attempt
        L->>H: Direct QUIC connection<br/>(bypasses relay)
    end

    L->>H: SyncMessage::JoinRequest<br/>{"display_name"}
    H->>L: SyncMessage::RoomState<br/>{room_code, participants,<br/>current_track, playback}
```

### SyncMessage Protocol

All messages are JSON-serialized and sent via Gossipsub to topic `cider-room-{code}`:

```rust
pub enum SyncMessage {
    // Room Management
    RoomState { room_code, host_peer_id, participants, current_track, playback },
    JoinRequest { display_name },
    JoinResponse { accepted, room_code, reason },
    ParticipantJoined(Participant),
    ParticipantLeft { peer_id },
    TransferHost { new_host_peer_id },

    // Playback (host → listeners)
    Play { track: TrackInfo, position_ms, timestamp_ms },
    Pause { position_ms, timestamp_ms },
    Seek { position_ms, timestamp_ms },
    TrackChange { track: TrackInfo, position_ms, timestamp_ms },

    // Clock Sync (RTT measurement)
    Ping { sent_at_ms },
    Pong { ping_sent_at_ms, received_at_ms },

    // Periodic
    Heartbeat { track_id, playback: PlaybackInfo },
}
```

### Playback Sync Algorithm

Listeners use an adaptive **seek calibrator** (EMA-based) that learns the optimal offset:

```mermaid
flowchart LR
    subgraph Host["Host (authority)"]
        cider_h["Cider API<br/>:10767"]
        detect["Detect state change"]
        broadcast["Gossipsub publish<br/>Play/Pause/Seek/TrackChange"]
    end

    subgraph Network["Network"]
        topic((("cider-room-XXXX")))
    end

    subgraph Listener["Listener"]
        receive["Receive message"]
        seek["Cider seek to:<br/>position_ms + offset_ms"]
        heartbeat["Heartbeat received"]
        measure["Measure drift:<br/>drift = our_pos - host_pos"]
        calibrate["EMA update:<br/>offset = α×ideal + (1-α)×offset<br/>(α=0.15, bounds: 100-2000ms)"]
    end

    cider_h --> detect --> broadcast --> topic --> receive --> seek
    heartbeat --> measure --> calibrate -.->|"learned offset"| seek
```

The calibrator starts at 500ms offset and converges to the actual Cider buffer latency (~700ms typical).

### Component Architecture

```mermaid
flowchart TB
    subgraph Swift["SwiftUI App"]
        views["Views<br/>╌╌╌╌╌╌╌╌<br/>HomeView<br/>RoomView<br/>DebugView"]
        appstate["AppState<br/>╌╌╌╌╌╌╌╌<br/>@MainActor<br/>@Published vars"]
    end

    subgraph FFI["UniFFI Boundary"]
        session["Session<br/>╌╌╌╌╌╌╌╌<br/>#[uniffi::Object]<br/>Arc&lt;Mutex&lt;...&gt;&gt;"]
        callback["SessionCallback<br/>╌╌╌╌╌╌╌╌<br/>#[uniffi::export(<br/>callback_interface)]"]
    end

    subgraph Rust["Rust Core (tokio runtime)"]
        handler["Event Handler<br/>╌╌╌╌╌╌╌╌<br/>mpsc channels<br/>async/await"]
        cider["Cider Client<br/>╌╌╌╌╌╌╌╌<br/>reqwest HTTP<br/>localhost:10767"]
        netmgr["NetworkManager<br/>╌╌╌╌╌╌╌╌<br/>Swarm event loop<br/>Room state machine"]
        signal["SignalingClient<br/>╌╌╌╌╌╌╌╌<br/>ntfy.sh polling<br/>Address exchange"]
    end

    subgraph Swarm["libp2p Swarm"]
        behaviour["CiderBehaviour<br/>╌╌╌╌╌╌╌╌<br/>6 composed protocols"]
    end

    views <--> appstate
    appstate <--> session
    callback --> appstate

    session --> handler
    handler --> cider
    handler <--> netmgr
    netmgr --> signal
    netmgr <--> behaviour
```

### Relay Server Architecture

```mermaid
flowchart TB
    subgraph Server["Relay Server Binary"]
        subgraph Behaviour["RelayServerBehaviour"]
            relay_s["relay::Behaviour<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>Accept reservations<br/>Create circuits"]
            identify_s["identify::Behaviour<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>Verify /cider-*<br/>Reject non-Cider"]
            ping_s["ping::Behaviour<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>Keep-alive 15s"]
        end
        metrics["Metrics<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>active_reservations<br/>active_circuits<br/>connected_peers"]
        dashboard["TUI Dashboard<br/>╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌<br/>ratatui terminal UI"]
    end

    subgraph Listen["Listen Addresses"]
        tcp_l["/ip4/0.0.0.0/tcp/4001"]
        quic_l["/ip4/0.0.0.0/udp/4001/quic-v1"]
        tcp6["/ip6/::/tcp/4001"]
        quic6["/ip6/::/udp/4001/quic-v1"]
    end

    subgraph Clients["Clients"]
        c1["Client A<br/>/p2p-circuit reservation"]
        c2["Client B<br/>circuit to A"]
    end

    tcp_l & quic_l & tcp6 & quic6 --> Behaviour
    c1 <-->|"reservation"| relay_s
    c2 <-->|"circuit"| relay_s
    relay_s <-->|"relay traffic"| c1
    Behaviour --> metrics --> dashboard
```

### Key Files

| Layer | File | What it does |
|-------|------|--------------|
| **FFI** | [`ffi/session.rs`](cider-core/src/ffi/session.rs) | `Session` object exported to Swift/C# via UniFFI |
| **FFI** | [`ffi/types.rs`](cider-core/src/ffi/types.rs) | `SessionCallback` trait for Rust→Native async events |
| **Network** | [`network/behaviour.rs`](cider-core/src/network/behaviour.rs) | `CiderBehaviour` struct + 1000-line event loop |
| **Network** | [`network/signaling.rs`](cider-core/src/network/signaling.rs) | ntfy.sh HTTP client for address exchange |
| **Network** | [`network/room_code.rs`](cider-core/src/network/room_code.rs) | 8-char room code generation (Base32 Crockford) |
| **Sync** | [`sync/protocol.rs`](cider-core/src/sync/protocol.rs) | `SyncMessage` enum definitions |
| **Cider** | [`cider/client.rs`](cider-core/src/cider/client.rs) | Cider REST API client (localhost:10767) |
| **Relay** | [`relay-server/src/network.rs`](relay-server/src/network.rs) | Dedicated relay server implementation |
| **macOS** | [`AppState.swift`](apps/macos/CiderTogether/CiderTogether/Models/AppState.swift) | `@MainActor` observable state machine |

</details>
