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
                                    Text("Seek Offset")
                                        .foregroundColor(.secondary)
                                    Spacer()
                                    Text("\(status.seekOffsetMs)ms")
                                        .font(.system(.body, design: .monospaced))
                                }

                                // Calibration details
                                if status.calibrationPending {
                                    VStack(alignment: .leading, spacing: 4) {
                                        HStack {
                                            Text("Calibration")
                                                .foregroundColor(.secondary)
                                            Spacer()
                                            Text("PENDING")
                                                .font(.system(.caption, design: .monospaced))
                                                .foregroundColor(.orange)
                                                .padding(.horizontal, 6)
                                                .padding(.vertical, 2)
                                                .background(Color.orange.opacity(0.2))
                                                .cornerRadius(4)
                                        }

                                        if let nextSample = status.nextCalibrationSample {
                                            // Show the calculation
                                            VStack(alignment: .leading, spacing: 2) {
                                                Text("ideal = offset - drift")
                                                    .font(.system(.caption2, design: .monospaced))
                                                    .foregroundColor(.secondary)
                                                Text("\(nextSample) = \(status.seekOffsetMs) - (\(status.driftMs > 0 ? "+" : "")\(status.driftMs))")
                                                    .font(.system(.caption, design: .monospaced))
                                                    .foregroundColor(.orange)
                                            }
                                            .padding(.leading, 8)
                                        } else {
                                            // Outlier - show why
                                            VStack(alignment: .leading, spacing: 2) {
                                                Text("drift |\(status.driftMs)ms| > 1500ms threshold")
                                                    .font(.system(.caption, design: .monospaced))
                                                    .foregroundColor(.orange)
                                                Text("Sample will use damped weight (5%)")
                                                    .font(.system(.caption2, design: .monospaced))
                                                    .foregroundColor(.secondary)
                                            }
                                            .padding(.leading, 8)
                                        }
                                    }
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

                                // Sample history
                                if !status.sampleHistory.isEmpty {
                                    Divider()

                                    VStack(alignment: .leading, spacing: 4) {
                                        Text("Calibration History")
                                            .foregroundColor(.secondary)
                                            .font(.caption)

                                        // Show samples newest first
                                        ForEach(Array(status.sampleHistory.reversed().enumerated()), id: \.offset) { index, sample in
                                            CalibrationSampleRow(sample: sample, isNewest: index == 0)
                                        }
                                    }
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

struct CalibrationSampleRow: View {
    let sample: CalibrationSample
    let isNewest: Bool

    var body: some View {
        HStack(spacing: 6) {
            // Drift value
            Text(sample.driftMs >= 0 ? "+\(sample.driftMs)" : "\(sample.driftMs)")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(sample.rejected ? .red : (sample.driftMs >= 0 ? .orange : .blue))
                .frame(width: 50, alignment: .trailing)

            Image(systemName: "arrow.right")
                .font(.caption2)
                .foregroundColor(.secondary)

            // Resulting offset
            Text("\(sample.newOffsetMs)ms")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(sample.rejected ? .secondary : .primary)

            if sample.rejected {
                Text("DAMPED")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(.orange)
                    .padding(.horizontal, 4)
                    .padding(.vertical, 1)
                    .background(Color.orange.opacity(0.15))
                    .cornerRadius(3)
            }

            Spacer()

            if isNewest {
                Text("latest")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(.secondary)
            }
        }
        .padding(.vertical, 2)
        .padding(.horizontal, 6)
        .background(isNewest ? Color.accentColor.opacity(0.1) : Color.clear)
        .cornerRadius(4)
    }
}

#Preview {
    DebugView()
        .environmentObject(AppState())
}
