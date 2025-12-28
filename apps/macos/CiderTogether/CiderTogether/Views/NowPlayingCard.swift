import SwiftUI
import Combine

struct NowPlayingCard: View {
    @EnvironmentObject var appState: AppState

    /// The track to display - uses host's track when listener, local when host or not in room
    private var displayTrack: TrackInfo? {
        // If in room as listener, prefer room state's track from host
        if appState.isInRoom && !appState.isHost {
            return appState.roomState?.currentTrack
        }
        // Otherwise use local Cider playback
        return appState.nowPlaying
    }

    var body: some View {
        VStack(spacing: 12) {
            // Album Artwork
            artworkView
                .frame(width: 180, height: 180)
                .cornerRadius(8)
                .shadow(color: .black.opacity(0.2), radius: 12, x: 0, y: 4)

            // Track Info
            if let track = displayTrack {
                VStack(spacing: 6) {
                    VStack(spacing: 2) {
                        Text(track.name)
                            .font(.system(.body, weight: .medium))
                            .foregroundColor(.primary)
                            .lineLimit(1)
                            .truncationMode(.tail)

                        Text(track.artist)
                            .font(.subheadline)
                            .foregroundColor(.secondary)
                            .lineLimit(1)
                            .truncationMode(.tail)
                    }

                    // Progress bar - use room playback state for listeners
                    if appState.isInRoom && !appState.isHost, let playback = appState.roomState?.playback {
                        ProgressBarView(
                            positionMs: playback.positionMs,
                            durationMs: track.durationMs,
                            isPlaying: playback.isPlaying
                        )
                    } else {
                        ProgressBarView(
                            positionMs: track.positionMs,
                            durationMs: track.durationMs,
                            isPlaying: appState.isPlaying
                        )
                    }
                }
                .frame(maxWidth: 280)
            } else {
                VStack(spacing: 2) {
                    Text("Not Playing")
                        .font(.system(.body, weight: .medium))
                        .foregroundColor(.secondary)

                    Text(appState.isInRoom && !appState.isHost ? "Waiting for host..." : "Play something in Cider")
                        .font(.caption)
                        .foregroundColor(.secondary.opacity(0.7))
                }
            }
        }
    }

    @ViewBuilder
    private var artworkView: some View {
        if let artwork = displayTrack?.artworkUrl, let url = URL(string: artwork) {
            AsyncImage(url: url) { phase in
                switch phase {
                case .success(let image):
                    image
                        .resizable()
                        .aspectRatio(contentMode: .fill)
                case .failure(_):
                    placeholderArtwork
                case .empty:
                    placeholderArtwork
                        .overlay(ProgressView())
                @unknown default:
                    placeholderArtwork
                }
            }
        } else {
            placeholderArtwork
        }
    }

    private var placeholderArtwork: some View {
        ZStack {
            Image(systemName: "music.note")
                .font(.system(size: 40))
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .glassEffect(.regular, in: .rect(cornerRadius: 8))
    }
}

struct ProgressBarView: View {
    let positionMs: UInt64
    let durationMs: UInt64
    var isPlaying: Bool = true

    @State private var interpolatedPosition: UInt64 = 0
    @State private var lastUpdateTime: Date = Date()

    private var displayPosition: UInt64 {
        min(interpolatedPosition, durationMs)
    }

    private var progress: Double {
        guard durationMs > 0 else { return 0 }
        return Double(displayPosition) / Double(durationMs)
    }

    private var positionText: String {
        formatTime(ms: displayPosition)
    }

    private var durationText: String {
        formatTime(ms: durationMs)
    }

    private func formatTime(ms: UInt64) -> String {
        let totalSeconds = ms / 1000
        let minutes = totalSeconds / 60
        let seconds = totalSeconds % 60
        return String(format: "%d:%02d", minutes, seconds)
    }

    var body: some View {
        VStack(spacing: 4) {
            // Progress bar
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    // Background track
                    RoundedRectangle(cornerRadius: 2)
                        .fill(Color.primary.opacity(0.2))
                        .frame(height: 4)

                    // Progress fill
                    RoundedRectangle(cornerRadius: 2)
                        .fill(Color.primary.opacity(0.8))
                        .frame(width: geometry.size.width * progress, height: 4)
                }
            }
            .frame(height: 4)

            // Time labels
            HStack {
                Text(positionText)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.secondary)

                Spacer()

                Text(durationText)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.secondary)
            }
        }
        .onAppear {
            interpolatedPosition = positionMs
            lastUpdateTime = Date()
        }
        .onChange(of: positionMs) { _, newValue in
            interpolatedPosition = newValue
            lastUpdateTime = Date()
        }
        .onReceive(Timer.publish(every: 0.25, on: .main, in: .common).autoconnect()) { _ in
            guard isPlaying else { return }
            let elapsed = Date().timeIntervalSince(lastUpdateTime)
            interpolatedPosition = positionMs + UInt64(elapsed * 1000)
        }
    }
}

#Preview {
    NowPlayingCard()
        .environmentObject(AppState())
        .frame(width: 400, height: 350)
        .padding()
}
