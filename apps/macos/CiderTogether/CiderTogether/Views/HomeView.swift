import SwiftUI

struct HomeView: View {
    @EnvironmentObject var appState: AppState
    @State private var roomCode: String = ""
    @State private var showJoinSheet: Bool = false
    @State private var showSettings: Bool = false
    @State private var tokenInput: String = ""

    var body: some View {
        VStack(spacing: 24) {
            if appState.ciderConnected {
                connectedView
            } else {
                disconnectedView
            }
        }
        .sheet(isPresented: $showJoinSheet) {
            JoinRoomSheet(roomCode: $roomCode, isPresented: $showJoinSheet)
                .environmentObject(appState)
        }
        .sheet(isPresented: $showSettings) {
            SettingsSheet(isPresented: $showSettings)
                .environmentObject(appState)
        }
        .onReceive(NotificationCenter.default.publisher(for: .createRoom)) { _ in
            if appState.ciderConnected && !appState.isInRoom {
                appState.createRoom()
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .joinRoom)) { _ in
            if appState.ciderConnected && !appState.isInRoom {
                showJoinSheet = true
            }
        }
    }

    // MARK: - Connected View

    private var connectedView: some View {
        VStack(spacing: 24) {
            // Cider connection warning
            if appState.ciderDisconnected {
                CiderWarningBanner()
            }

            // Now Playing Preview
            NowPlayingCard()

            // Action Buttons
            VStack(spacing: 10) {
                Button(action: { appState.createRoom() }) {
                    HStack {
                        Image(systemName: "plus.circle.fill")
                        Text("Create Room")
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(.glassProminent)
                .controlSize(.large)

                Button(action: { showJoinSheet = true }) {
                    HStack {
                        Image(systemName: "person.2.fill")
                        Text("Join Room")
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(.glass)
                .controlSize(.large)
            }
            .frame(maxWidth: 280)

            // Display Name & Settings
            VStack(alignment: .leading, spacing: 4) {
                Text("Your Name")
                    .font(.caption)
                    .foregroundColor(.secondary)

                HStack {
                    TextField("Display Name", text: $appState.displayName)
                        .textFieldStyle(.roundedBorder)
                        .textContentType(.none)
                        .autocorrectionDisabled()
                        .frame(width: 140)

                    Spacer()

                    Button(action: { showSettings = true }) {
                        Image(systemName: "gear")
                            .font(.title3)
                    }
                    .buttonStyle(.plain)
                    .foregroundColor(.secondary)
                }
            }
            .frame(maxWidth: 280)
        }
        .frame(maxWidth: .infinity)
    }

    // MARK: - Disconnected View

    private var disconnectedView: some View {
        VStack(spacing: 20) {
            if appState.isCheckingConnection {
                ProgressView()
                    .scaleEffect(1.5)
                    .frame(height: 48)

                Text("Connecting to Cider...")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            } else {
                Image(systemName: "music.note.tv")
                    .font(.system(size: 40))
                    .foregroundColor(.secondary)

                VStack(spacing: 6) {
                    Text(appState.ciderDisconnected ? "Cider Disconnected" : "Connect to Cider")
                        .font(.headline)

                    Text(appState.ciderDisconnected
                         ? "Cider was closed or stopped responding. Restart it and reconnect."
                         : "Make sure Cider is running with API access enabled.")
                        .font(.caption)
                        .foregroundColor(appState.ciderDisconnected ? .orange : .secondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 260)
                }

                // Setup fields
                VStack(alignment: .leading, spacing: 12) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("API Token")
                            .font(.caption)
                            .foregroundColor(.secondary)
                        SecureField("Paste your Cider API token", text: $tokenInput)
                            .textFieldStyle(.roundedBorder)
                            .onAppear { tokenInput = appState.apiToken }
                            .onChange(of: tokenInput) { _, newValue in
                                appState.updateApiToken(newValue)
                            }
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text("Your Name")
                            .font(.caption)
                            .foregroundColor(.secondary)
                        TextField("Display name", text: $appState.displayName)
                            .textFieldStyle(.roundedBorder)
                            .textContentType(.none)
                            .autocorrectionDisabled()
                    }
                }
                .frame(maxWidth: 260)

                Button {
                    Task {
                        await appState.checkCiderConnection()
                    }
                } label: {
                    Text("Connect")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.glassProminent)
                .controlSize(.large)
                .frame(maxWidth: 260)

                // Connection error
                if let error = appState.connectionError {
                    Text(error)
                        .font(.caption)
                        .foregroundColor(.red)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 260)
                }
            }
        }
        .padding(.vertical, 20)
    }
}

// MARK: - Join Room Sheet

struct JoinRoomSheet: View {
    @EnvironmentObject var appState: AppState
    @Binding var roomCode: String
    @Binding var isPresented: Bool
    @FocusState private var isFocused: Bool

    /// Clean code without formatting (alphanumeric only, uppercase)
    private var cleanCode: String {
        roomCode
            .replacingOccurrences(of: "-", with: "")
            .filter { $0.isLetter || $0.isNumber }
            .uppercased()
    }

    /// Whether the room code is valid (exactly 6 alphanumeric characters)
    private var isValidCode: Bool {
        cleanCode.count == 6
    }

    var body: some View {
        VStack(spacing: 20) {
            Text("Join a Room")
                .font(.title3.bold())

            VStack(spacing: 6) {
                Text("Enter the room code")
                    .font(.subheadline)
                    .foregroundColor(.secondary)

                TextField("XXX-XXX", text: $roomCode)
                    .textFieldStyle(.roundedBorder)
                    .font(.title3.monospaced())
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 160)
                    .focused($isFocused)
                    .onChange(of: roomCode) { _, newValue in
                        formatRoomCode(newValue)
                    }
                    .onSubmit {
                        joinRoom()
                    }

                // Validation hint
                if !roomCode.isEmpty && !isValidCode {
                    Text("Enter 6 characters (letters and numbers)")
                        .font(.caption2)
                        .foregroundColor(.orange)
                }
            }

            HStack(spacing: 12) {
                Button("Cancel") {
                    isPresented = false
                }
                .buttonStyle(.glass)
                .keyboardShortcut(.cancelAction)

                Button("Join") {
                    joinRoom()
                }
                .buttonStyle(.glassProminent)
                .keyboardShortcut(.defaultAction)
                .disabled(!isValidCode)
            }
        }
        .padding(24)
        .frame(width: 280)
        .onAppear {
            isFocused = true
        }
    }

    /// Format the room code as XXX-XXX while user types
    private func formatRoomCode(_ input: String) {
        // Extract only alphanumeric characters and uppercase
        let cleaned = input
            .replacingOccurrences(of: "-", with: "")
            .filter { $0.isLetter || $0.isNumber }
            .uppercased()
            .prefix(6)

        // Format as XXX-XXX
        if cleaned.count > 3 {
            let formatted = "\(cleaned.prefix(3))-\(cleaned.suffix(cleaned.count - 3))"
            if roomCode != formatted {
                roomCode = formatted
            }
        } else if String(cleaned) != roomCode.replacingOccurrences(of: "-", with: "") {
            roomCode = String(cleaned)
        }
    }

    private func joinRoom() {
        if isValidCode {
            appState.joinRoom(code: cleanCode)
            isPresented = false
        }
    }
}

// MARK: - Settings Sheet

struct SettingsSheet: View {
    @EnvironmentObject var appState: AppState
    @Binding var isPresented: Bool
    @State private var tokenInput: String = ""

    var body: some View {
        VStack(spacing: 20) {
            Text("Settings")
                .font(.title3.bold())

            VStack(alignment: .leading, spacing: 12) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Cider API Token")
                        .font(.subheadline.bold())

                    Text("Find this in Cider: Settings > Connectivity > Manage External Application Access")
                        .font(.caption)
                        .foregroundColor(.secondary)

                    SecureField("API Token (optional)", text: $tokenInput)
                        .textFieldStyle(.roundedBorder)
                }

                VStack(alignment: .leading, spacing: 6) {
                    Text("Display Name")
                        .font(.subheadline.bold())

                    TextField("Your name", text: $appState.displayName)
                        .textFieldStyle(.roundedBorder)
                        .textContentType(.none)
                        .autocorrectionDisabled()
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            HStack(spacing: 12) {
                Button("Cancel") {
                    isPresented = false
                }
                .buttonStyle(.glass)
                .keyboardShortcut(.cancelAction)

                Button("Save") {
                    appState.updateApiToken(tokenInput)
                    isPresented = false
                }
                .buttonStyle(.glassProminent)
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(width: 340)
        .onAppear {
            tokenInput = appState.apiToken
        }
    }
}

struct CiderWarningBanner: View {
    @EnvironmentObject var appState: AppState

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundColor(.orange)

            VStack(alignment: .leading, spacing: 1) {
                Text("Connection issue")
                    .font(.caption)
                    .foregroundColor(.primary)

                Text("Retrying automatically...")
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }

            Spacer()

            Button("Retry") {
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
        .frame(maxWidth: 280)
    }
}

#Preview {
    HomeView()
        .environmentObject(AppState())
        .frame(width: 400, height: 500)
}
