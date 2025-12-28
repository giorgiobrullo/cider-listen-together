import SwiftUI

struct ContentView: View {
    @EnvironmentObject var appState: AppState
    @State private var currentArtworkUrl: String?

    var body: some View {
        ZStack {
            // Background layer - extends under title bar
            backgroundView
                .ignoresSafeArea()

            // Content with glass effects - respects safe areas
            GlassEffectContainer {
                ScrollView(showsIndicators: false) {
                    mainContent
                        .frame(maxWidth: .infinity)
                        .padding(.horizontal, 20)
                        .padding(.vertical, 20)
                }
                .safeAreaBar(edge: .top) {
                    headerView
                        .padding(.horizontal, 20)
                        .padding(.vertical, 12)
                }
            }
            .padding(.top, 28) // Account for title bar with traffic light buttons
        }
        .ignoresSafeArea(edges: .top)
        .frame(minWidth: 360, maxWidth: 440, minHeight: 480, maxHeight: 600)
        .preferredColorScheme(.dark) // Force dark mode - blurred artwork bg needs light text
        .alert("Error", isPresented: .constant(appState.errorMessage != nil)) {
            Button("OK") {
                appState.errorMessage = nil
            }
        } message: {
            Text(appState.errorMessage ?? "")
        }
        .task {
            appState.onAppear()
        }
    }

    // MARK: - Background

    @ViewBuilder
    private var backgroundView: some View {
        GeometryReader { geometry in
            ZStack {
                // Base background color
                Color(nsColor: .windowBackgroundColor)

                // Animated artwork background
                if let artwork = currentArtworkUrl,
                   !artwork.isEmpty,
                   let url = URL(string: artwork) {
                    AsyncImage(url: url) { phase in
                        switch phase {
                        case .success(let image):
                            image
                                .resizable()
                                .aspectRatio(contentMode: .fill)
                                .frame(width: geometry.size.width, height: geometry.size.height)
                                .blur(radius: 60)
                                .overlay(Color.black.opacity(0.35))
                                .transition(.opacity)
                        case .failure, .empty:
                            EmptyView()
                        @unknown default:
                            EmptyView()
                        }
                    }
                }
            }
        }
        .animation(.easeInOut(duration: 0.5), value: currentArtworkUrl)
        .onChange(of: appState.nowPlaying?.artworkUrl) { _, newArtwork in
            currentArtworkUrl = newArtwork
        }
        .onAppear {
            currentArtworkUrl = appState.nowPlaying?.artworkUrl
        }
    }

    // MARK: - Header

    private var headerView: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("Cider Together")
                    .font(.headline)
                    .foregroundColor(.primary)

                // Only show status when connected
                if appState.ciderConnected {
                    HStack(spacing: 4) {
                        Circle()
                            .fill(Color.green)
                            .frame(width: 6, height: 6)
                        Text("Connected")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                }
            }

            Spacer()

            if appState.isInRoom {
                Button {
                    Task { @MainActor in
                        appState.moveToMenuBar()
                    }
                } label: {
                    Image(systemName: "menubar.arrow.up.rectangle")
                        .font(.body)
                }
                .buttonStyle(.plain)
                .foregroundColor(.secondary)
                .help("Move to Menu Bar")

                Button(action: { appState.leaveRoom() }) {
                    Image(systemName: "xmark.circle.fill")
                        .font(.body)
                }
                .buttonStyle(.plain)
                .foregroundColor(.secondary)
                .help("Leave Room")
            }
        }
    }

    // MARK: - Main Content

    @ViewBuilder
    private var mainContent: some View {
        switch appState.viewState {
        case .home:
            HomeView()
        case .creating:
            VStack(spacing: 12) {
                ProgressView()
                Text("Creating room...")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .joining(let progress):
            JoiningView(progress: progress)
        case .inRoom:
            RoomView()
        }
    }
}

// MARK: - Joining View

struct JoiningView: View {
    @EnvironmentObject var appState: AppState
    let progress: AppState.JoiningProgress

    var body: some View {
        VStack(spacing: 20) {
            switch progress {
            case .searching:
                searchingView
            case .connecting:
                connectingView
            case .timeout:
                timeoutView
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var searchingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .scaleEffect(1.2)

            VStack(spacing: 6) {
                Text("Looking for room...")
                    .font(.headline)

                if let code = appState.joiningRoomCode {
                    Text("Room code: \(formatRoomCode(code))")
                        .font(.subheadline.monospaced())
                        .foregroundColor(.secondary)
                }

                Text("Connecting to peers...")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Button("Cancel") {
                appState.cancelJoin()
            }
            .buttonStyle(.glass)
            .controlSize(.regular)
        }
    }

    private var connectingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .scaleEffect(1.2)

            VStack(spacing: 6) {
                Text("Connecting...")
                    .font(.headline)

                Text("Found host, syncing room state")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
        }
    }

    private var timeoutView: some View {
        VStack(spacing: 20) {
            Image(systemName: "person.slash")
                .font(.system(size: 40))
                .foregroundColor(.orange)

            VStack(spacing: 6) {
                Text("Room Not Found")
                    .font(.headline)

                if let code = appState.joiningRoomCode {
                    Text("Could not find room \(formatRoomCode(code))")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }

                Text("The room code may be incorrect, or the host is no longer available.")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 260)
            }

            HStack(spacing: 12) {
                Button("Go Back") {
                    appState.cancelJoin()
                }
                .buttonStyle(.glass)
                .controlSize(.regular)

                Button("Retry") {
                    appState.retryJoin()
                }
                .buttonStyle(.glassProminent)
                .controlSize(.regular)
            }
        }
    }

    private func formatRoomCode(_ code: String) -> String {
        if code.count == 6 {
            return "\(code.prefix(3))-\(code.suffix(3))"
        }
        return code
    }
}

#Preview {
    ContentView()
        .environmentObject(AppState())
}
