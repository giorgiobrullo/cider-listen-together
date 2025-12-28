import SwiftUI

struct DebugView: View {
    @EnvironmentObject var appState: AppState

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Debug Info")
                    .font(.headline)

                GroupBox("Connection") {
                    VStack(alignment: .leading, spacing: 8) {
                        LabeledValue(label: "Role", value: appState.isHost ? "Host" : "Listener")
                        LabeledValue(label: "In Room", value: appState.isInRoom ? "Yes" : "No")
                        LabeledValue(label: "Cider Connected", value: appState.ciderConnected ? "Yes" : "No")

                        if let roomState = appState.roomState {
                            LabeledValue(label: "Room Code", value: roomState.roomCode)
                            LabeledValue(label: "Participants", value: "\(roomState.participants.count)")
                            LabeledValue(label: "Local Peer ID", value: String(roomState.localPeerId.prefix(12)) + "...")
                            LabeledValue(label: "Host Peer ID", value: String(roomState.hostPeerId.prefix(12)) + "...")
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }

                if !appState.isHost {
                    GroupBox("Sync Status") {
                        if let status = appState.syncStatus {
                            VStack(alignment: .leading, spacing: 8) {
                                HStack {
                                    Text("Drift")
                                        .foregroundColor(.secondary)
                                    Spacer()
                                    Text(formatDrift(status.driftMs))
                                        .font(.system(.body, design: .monospaced))
                                        .foregroundColor(driftColor(status.driftMs))
                                }

                                HStack {
                                    Text("Latency")
                                        .foregroundColor(.secondary)
                                    Spacer()
                                    Text("\(status.latencyMs)ms")
                                        .font(.system(.body, design: .monospaced))
                                }

                                HStack {
                                    Text("Last Heartbeat")
                                        .foregroundColor(.secondary)
                                    Spacer()
                                    Text("\(status.elapsedMs)ms ago")
                                        .font(.system(.body, design: .monospaced))
                                }

                                HStack {
                                    Text("Quality")
                                        .foregroundColor(.secondary)
                                    Spacer()
                                    SyncQualityBadge(driftMs: status.driftMs)
                                }
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        } else {
                            Text("Waiting for sync data...")
                                .foregroundColor(.secondary)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                } else {
                    GroupBox("Sync Status") {
                        Text("Sync status is only available for listeners")
                            .foregroundColor(.secondary)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }

                if let track = appState.nowPlaying {
                    GroupBox("Now Playing") {
                        VStack(alignment: .leading, spacing: 8) {
                            LabeledValue(label: "Track", value: track.name)
                            LabeledValue(label: "Artist", value: track.artist)
                            LabeledValue(label: "Song ID", value: track.songId)
                            LabeledValue(label: "Position", value: "\(track.positionMs)ms / \(track.durationMs)ms")
                            LabeledValue(label: "Playing", value: appState.isPlaying ? "Yes" : "No")
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
            .padding()
        }
        .frame(minWidth: 350, minHeight: 400, maxHeight: .infinity)
    }

    private func formatDrift(_ drift: Int64) -> String {
        if drift >= 0 {
            return "+\(drift)ms"
        } else {
            return "\(drift)ms"
        }
    }

    private func driftColor(_ drift: Int64) -> Color {
        let absDrift = abs(drift)
        if absDrift < 200 {
            return .green
        } else if absDrift < 1000 {
            return .orange
        } else {
            return .red
        }
    }
}

struct LabeledValue: View {
    let label: String
    let value: String

    var body: some View {
        HStack {
            Text(label)
                .foregroundColor(.secondary)
            Spacer()
            Text(value)
                .font(.system(.body, design: .monospaced))
                .lineLimit(1)
        }
    }
}

struct SyncQualityBadge: View {
    let driftMs: Int64

    private var quality: (text: String, color: Color) {
        let absDrift = abs(driftMs)
        if absDrift < 200 {
            return ("Excellent", .green)
        } else if absDrift < 500 {
            return ("Good", .green)
        } else if absDrift < 1000 {
            return ("Fair", .orange)
        } else if absDrift < 3000 {
            return ("Poor", .orange)
        } else {
            return ("Bad", .red)
        }
    }

    var body: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(quality.color)
                .frame(width: 8, height: 8)
            Text(quality.text)
                .font(.system(.body, design: .monospaced))
                .foregroundColor(quality.color)
        }
    }
}

#Preview {
    DebugView()
        .environmentObject(AppState())
}
