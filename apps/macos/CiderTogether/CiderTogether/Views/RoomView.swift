import SwiftUI

struct RoomView: View {
    @EnvironmentObject var appState: AppState

    var body: some View {
        VStack(spacing: 20) {
            // Cider disconnected banner
            if appState.ciderDisconnected {
                CiderDisconnectedBanner()
            }

            // Room Code
            if let roomState = appState.roomState {
                RoomCodeView(code: roomState.roomCode)
            }

            // Now Playing
            NowPlayingCard()

            // Playback Controls (host only)
            if appState.isHost {
                PlaybackControlsView()
            }

            // Participants
            if let roomState = appState.roomState {
                ParticipantsView(participants: roomState.participants)
            }
        }
        .frame(maxWidth: .infinity)
    }
}

struct CiderDisconnectedBanner: View {
    @EnvironmentObject var appState: AppState

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundColor(.orange)

            VStack(alignment: .leading, spacing: 1) {
                Text("Cider disconnected")
                    .font(.caption)
                    .foregroundColor(.primary)

                Text("Retrying...")
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }

            Spacer()

            Button("Retry Now") {
                Task {
                    await appState.checkCiderConnection()
                }
            }
            .font(.caption)
            .buttonStyle(.glass)
            .controlSize(.small)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
    }
}

struct RoomCodeView: View {
    let code: String
    @State private var copied = false

    var formattedCode: String {
        if code.count == 6 {
            let index = code.index(code.startIndex, offsetBy: 3)
            return "\(code[..<index])-\(code[index...])"
        }
        return code
    }

    var body: some View {
        Button(action: copyCode) {
            HStack(spacing: 6) {
                Text(formattedCode)
                    .font(.system(.title3, design: .monospaced, weight: .semibold))
                    .foregroundColor(.primary)

                Image(systemName: copied ? "checkmark" : "doc.on.doc")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .glassEffect(.regular, in: .capsule)
        }
        .buttonStyle(.plain)
    }

    private func copyCode() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(formattedCode, forType: .string)

        withAnimation {
            copied = true
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            withAnimation {
                copied = false
            }
        }
    }
}

struct PlaybackControlsView: View {
    @EnvironmentObject var appState: AppState

    var body: some View {
        HStack(spacing: 24) {
            Button(action: { appState.previous() }) {
                Image(systemName: "backward.fill")
                    .font(.title3)
            }
            .buttonStyle(.plain)
            .foregroundColor(.primary)

            Button(action: togglePlayPause) {
                Image(systemName: isPlaying ? "pause.fill" : "play.fill")
                    .font(.title)
            }
            .buttonStyle(.plain)
            .foregroundColor(.primary)

            Button(action: { appState.next() }) {
                Image(systemName: "forward.fill")
                    .font(.title3)
            }
            .buttonStyle(.plain)
            .foregroundColor(.primary)
        }
        .padding(.vertical, 8)
    }

    private var isPlaying: Bool {
        appState.isPlaying
    }

    private func togglePlayPause() {
        if isPlaying {
            appState.pause()
        } else {
            appState.play()
        }
    }
}

struct ParticipantsView: View {
    let participants: [Participant]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Listening (\(participants.count))")
                .font(.caption)
                .foregroundColor(.secondary)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(participants, id: \.peerId) { participant in
                        ParticipantBadge(participant: participant)
                    }
                }
            }
        }
    }
}

struct ParticipantBadge: View {
    let participant: Participant

    var body: some View {
        HStack(spacing: 5) {
            // Avatar
            Circle()
                .fill(avatarColor)
                .frame(width: 22, height: 22)
                .overlay(
                    Text(initials)
                        .font(.system(size: 9, weight: .semibold))
                        .foregroundColor(.white)
                )

            // Name
            Text(participant.displayName)
                .font(.caption)
                .foregroundColor(.primary)
                .lineLimit(1)

            // Host badge
            if participant.isHost {
                Image(systemName: "star.fill")
                    .font(.system(size: 8))
                    .foregroundColor(.orange)
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
        .glassEffect(.regular, in: .capsule)
    }

    private var initials: String {
        let words = participant.displayName.split(separator: " ")
        if words.count >= 2 {
            return "\(words[0].prefix(1))\(words[1].prefix(1))".uppercased()
        }
        return String(participant.displayName.prefix(2)).uppercased()
    }

    private var avatarColor: Color {
        // Generate consistent color from name
        let hash = participant.displayName.hashValue
        let hue = Double(abs(hash) % 360) / 360.0
        return Color(hue: hue, saturation: 0.5, brightness: 0.6)
    }
}

#Preview {
    RoomView()
        .environmentObject(AppState())
        .frame(width: 400, height: 500)
        .padding()
}
