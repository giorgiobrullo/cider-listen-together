import SwiftUI

struct MenuBarView: View {
    @EnvironmentObject var appState: AppState
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 12) {
            // Header with close button
            HStack {
                if let roomState = appState.roomState {
                    Text(formattedCode(roomState.roomCode))
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(.secondary)

                    Spacer()

                    HStack(spacing: 4) {
                        Image(systemName: "person.2.fill")
                            .font(.caption2)
                        Text("\(roomState.participants.count)")
                            .font(.caption)
                    }
                    .foregroundColor(.secondary)
                } else {
                    Spacer()
                }

                Button {
                    dismiss()
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                .buttonStyle(.plain)
            }

            // Now Playing
            if let track = appState.nowPlaying {
                HStack(spacing: 10) {
                    // Album Art
                    if !track.artworkUrl.isEmpty,
                       let url = URL(string: track.artworkUrl) {
                        AsyncImage(url: url) { phase in
                            switch phase {
                            case .success(let image):
                                image
                                    .resizable()
                                    .aspectRatio(contentMode: .fill)
                            default:
                                placeholderImage
                            }
                        }
                        .frame(width: 48, height: 48)
                        .clipShape(RoundedRectangle(cornerRadius: 6))
                    } else {
                        placeholderImage
                            .frame(width: 48, height: 48)
                            .clipShape(RoundedRectangle(cornerRadius: 6))
                    }

                    // Track Info
                    VStack(alignment: .leading, spacing: 2) {
                        Text(track.name)
                            .font(.subheadline.bold())
                            .lineLimit(1)

                        Text(track.artist)
                            .font(.caption)
                            .foregroundColor(.secondary)
                            .lineLimit(1)
                    }

                    Spacer()
                }

                // Progress bar
                MiniProgressBar(positionMs: track.positionMs, durationMs: track.durationMs)

                // Playback controls (host only)
                if appState.isHost {
                    HStack(spacing: 20) {
                        Button { appState.previous() } label: {
                            Image(systemName: "backward.fill")
                                .font(.caption)
                        }
                        .buttonStyle(.plain)

                        Button {
                            if appState.isPlaying {
                                appState.pause()
                            } else {
                                appState.play()
                            }
                        } label: {
                            Image(systemName: appState.isPlaying ? "pause.fill" : "play.fill")
                                .font(.subheadline)
                        }
                        .buttonStyle(.plain)

                        Button { appState.next() } label: {
                            Image(systemName: "forward.fill")
                                .font(.caption)
                        }
                        .buttonStyle(.plain)
                    }
                    .foregroundColor(.primary)
                }
            } else {
                HStack(spacing: 10) {
                    placeholderImage
                        .frame(width: 48, height: 48)
                        .clipShape(RoundedRectangle(cornerRadius: 6))

                    Text("Nothing playing")
                        .font(.subheadline)
                        .foregroundColor(.secondary)

                    Spacer()
                }
            }

            Divider()

            // Actions
            VStack(spacing: 4) {
                Button {
                    dismiss()
                    appState.showMainWindow()
                } label: {
                    HStack {
                        Image(systemName: "arrow.up.left.and.arrow.down.right")
                        Text("Open Window")
                        Spacer()
                    }
                }
                .buttonStyle(.plain)

                Button {
                    dismiss()
                    appState.leaveRoom()
                } label: {
                    HStack {
                        Image(systemName: "xmark.circle")
                        Text("Leave Room")
                        Spacer()
                    }
                    .foregroundColor(.red)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(12)
        .frame(width: 240)
    }

    private var placeholderImage: some View {
        ZStack {
            Color.gray.opacity(0.2)
            Image(systemName: "music.note")
                .foregroundColor(.secondary)
        }
    }

    private func formattedCode(_ code: String) -> String {
        if code.count == 6 {
            let index = code.index(code.startIndex, offsetBy: 3)
            return "\(code[..<index])-\(code[index...])"
        }
        return code
    }
}

// MARK: - Mini Progress Bar

struct MiniProgressBar: View {
    let positionMs: UInt64
    let durationMs: UInt64

    private var progress: Double {
        guard durationMs > 0 else { return 0 }
        return Double(positionMs) / Double(durationMs)
    }

    private func formatTime(ms: UInt64) -> String {
        let totalSeconds = ms / 1000
        let minutes = totalSeconds / 60
        let seconds = totalSeconds % 60
        return String(format: "%d:%02d", minutes, seconds)
    }

    var body: some View {
        VStack(spacing: 2) {
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 1.5)
                        .fill(Color.primary.opacity(0.2))
                        .frame(height: 3)

                    RoundedRectangle(cornerRadius: 1.5)
                        .fill(Color.primary.opacity(0.6))
                        .frame(width: geometry.size.width * progress, height: 3)
                }
            }
            .frame(height: 3)

            HStack {
                Text(formatTime(ms: positionMs))
                Spacer()
                Text(formatTime(ms: durationMs))
            }
            .font(.system(size: 9, design: .monospaced))
            .foregroundColor(.secondary)
        }
    }
}

#Preview {
    MenuBarView()
        .environmentObject(AppState())
}
