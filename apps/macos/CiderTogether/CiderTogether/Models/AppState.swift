import SwiftUI
import Combine

/// Main application state
@MainActor
class AppState: ObservableObject {
    // MARK: - Published State

    @Published var viewState: ViewState = .home
    @Published var ciderConnected: Bool = false
    @Published var isCheckingConnection: Bool = false
    @Published var roomState: RoomState? = nil
    @Published var nowPlaying: TrackInfo? = nil
    @Published var playback: PlaybackState? = nil
    @Published var errorMessage: String? = nil  // For alerts (room errors, etc.)
    @Published var connectionError: String? = nil  // For inline display
    @Published var ciderDisconnected: Bool = false  // True if Cider closed while we were connected
    @Published var isPlaying: Bool = false  // Current playback state
    @Published var isHost: Bool = false  // Cached to avoid synchronous FFI calls
    @Published var isInRoom: Bool = false  // Cached to avoid synchronous FFI calls
    @Published var isInMenuBarMode: Bool = false  // Whether app is minimized to menu bar
    @Published var joiningRoomCode: String? = nil  // Room code we're trying to join (for retries)

    // MARK: - Persisted State

    @AppStorage("displayName") var displayName: String = "Listener"
    @AppStorage("ciderApiToken") var apiToken: String = ""

    // MARK: - Private

    private var session: Session
    private var pollingTask: Task<Void, Never>?
    private var consecutiveFailures: Int = 0
    private let maxConsecutiveFailures: Int = 3
    private var hasAppeared: Bool = false

    // MARK: - View State Enum

    enum ViewState: Equatable {
        case home
        case creating
        case joining(JoiningProgress)
        case inRoom
    }

    enum JoiningProgress: Equatable {
        case searching      // Looking for room host via mDNS
        case connecting     // Found host, establishing connection
        case timeout        // No host found after timeout
    }

    // MARK: - Initialization

    init() {
        session = Session()
        session.setCallback(callback: SessionCallbackImpl(appState: self))

        // Apply saved token
        if !apiToken.isEmpty {
            session.setCiderToken(token: apiToken)
        }
    }

    /// Call this after the view has appeared to avoid state updates during view construction
    func onAppear() {
        guard !hasAppeared else { return }
        hasAppeared = true
        Task {
            await checkCiderConnection(showError: false)
        }
    }

    // MARK: - Settings

    func updateApiToken(_ token: String) {
        apiToken = token
        session.setCiderToken(token: token.isEmpty ? nil : token)
        connectionError = nil  // Clear error when token changes
    }

    // MARK: - Cider Connection

    func checkCiderConnection(showError: Bool = true) async {
        isCheckingConnection = true
        connectionError = nil
        let startTime = Date()

        // Run blocking FFI call off the main thread
        let result: Result<Void, CoreError> = await Task.detached { [session] in
            do {
                try session.checkCiderConnection()
                return .success(())
            } catch let error as CoreError {
                return .failure(error)
            } catch {
                return .failure(.CiderNotReachable)
            }
        }.value

        // Ensure loading is visible for at least 200ms
        let elapsed = Date().timeIntervalSince(startTime)
        if elapsed < 0.2 {
            try? await Task.sleep(for: .milliseconds(Int((0.2 - elapsed) * 1000)))
        }

        switch result {
        case .success:
            self.ciderConnected = true
            self.connectionError = nil
            self.ciderDisconnected = false
            self.consecutiveFailures = 0
            _ = await fetchNowPlaying()
            startPolling()
        case .failure(let error):
            self.ciderConnected = false
            if showError {
                self.connectionError = connectionErrorMessage(for: error)
            }
            stopPolling()
        }

        isCheckingConnection = false
    }

    private func connectionErrorMessage(for error: CoreError) -> String {
        switch error {
        case .CiderNotReachable:
            return "Cider is not running or not reachable"
        case .CiderApiError(let msg):
            return msg
        case .NetworkError(let msg):
            return "Network error: \(msg)"
        default:
            return error.localizedDescription
        }
    }

    private func fetchNowPlaying() async -> Bool {
        // Run blocking FFI call off the main thread (single call fetches both concurrently)
        let result: Result<CurrentPlayback, CoreError> = await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async { [session] in
                do {
                    let playback = try session.getPlaybackState()
                    continuation.resume(returning: .success(playback))
                } catch let error as CoreError {
                    continuation.resume(returning: .failure(error))
                } catch {
                    continuation.resume(returning: .failure(.CiderNotReachable))
                }
            }
        }

        switch result {
        case .success(let playback):
            self.nowPlaying = playback.track
            self.isPlaying = playback.isPlaying
            self.consecutiveFailures = 0
            if ciderDisconnected {
                ciderDisconnected = false
            }
            // Note: Host broadcast is handled by the Rust core's broadcast loop
            return true
        case .failure:
            self.consecutiveFailures += 1
            if consecutiveFailures >= 2 {
                self.ciderDisconnected = true
            }
            if consecutiveFailures >= maxConsecutiveFailures {
                self.nowPlaying = nil
                self.isPlaying = false
            }
            return false
        }
    }

    private func startPolling() {
        stopPolling()
        pollingTask = Task { [weak self] in
            while !Task.isCancelled {
                guard let self = self else { break }

                let success = await self.fetchNowPlaying()

                // Backoff: wait longer after failures (1.5s normal, 3s after failure)
                let delay: UInt64 = success ? 1_500_000_000 : 3_000_000_000
                try? await Task.sleep(nanoseconds: delay)
            }
        }
    }

    private func stopPolling() {
        pollingTask?.cancel()
        pollingTask = nil
    }

    // MARK: - Room Management

    func createRoom() {
        viewState = .creating
        let name = displayName

        Task {
            let result: Result<String, Error> = await Task.detached { [session] in
                do {
                    let code = try session.createRoom(displayName: name)
                    return .success(code)
                } catch {
                    return .failure(error)
                }
            }.value

            switch result {
            case .success:
                viewState = .inRoom
                isInRoom = true
                isHost = true
            case .failure(let error):
                errorMessage = "Failed to create room: \(error.localizedDescription)"
                viewState = .home
            }
        }
    }

    func joinRoom(code: String) {
        viewState = .joining(.searching)
        joiningRoomCode = code
        let name = displayName

        Task {
            let error: Error? = await Task.detached { [session] in
                do {
                    try session.joinRoom(roomCode: code, displayName: name)
                    return nil
                } catch {
                    return error
                }
            }.value

            if let error {
                errorMessage = "Failed to join room: \(error.localizedDescription)"
                viewState = .home
                joiningRoomCode = nil
            }
            // Note: Success is handled by the onConnected/onRoomStateChanged callbacks
            // The viewState stays as .joining until we get a callback or timeout
        }
    }

    func cancelJoin() {
        // Set state first to prevent any pending callbacks from showing errors
        viewState = .home
        joiningRoomCode = nil
        errorMessage = nil  // Clear any pending errors

        // Leave the room (which clears the joining state on the Rust side)
        Task {
            _ = await Task.detached { [session] in
                try? session.leaveRoom()
            }.value
        }
    }

    func retryJoin() {
        guard let code = joiningRoomCode else { return }
        joinRoom(code: code)
    }

    func leaveRoom() {
        Task {
            let error: Error? = await Task.detached { [session] in
                do {
                    try session.leaveRoom()
                    return nil
                } catch {
                    return error
                }
            }.value

            if let error {
                errorMessage = "Failed to leave room: \(error.localizedDescription)"
            } else {
                viewState = .home
                roomState = nil
                isInRoom = false
                isHost = false
            }
        }
    }

    func transferHost(to peerId: String) {
        Task {
            let error: Error? = await Task.detached { [session] in
                do {
                    try session.transferHost(peerId: peerId)
                    return nil
                } catch {
                    return error
                }
            }.value

            if let error {
                errorMessage = "Failed to transfer host: \(error.localizedDescription)"
            }
        }
    }

    // MARK: - Playback Controls

    func play() {
        Task.detached { [session] in
            try? session.syncPlay()
        }
    }

    func pause() {
        Task.detached { [session] in
            try? session.syncPause()
        }
    }

    func next() {
        Task.detached { [session] in
            try? session.syncNext()
        }
    }

    func previous() {
        Task.detached { [session] in
            try? session.syncPrevious()
        }
    }

    // MARK: - Menu Bar Mode

    func moveToMenuBar() {
        isInMenuBarMode = true
        // Defer window operations to avoid SwiftUI view update conflicts
        DispatchQueue.main.async {
            // Hide from Dock
            NSApp.setActivationPolicy(.accessory)
            // Hide all windows
            for window in NSApp.windows where window.isVisible && window.className != "NSStatusBarWindow" {
                window.orderOut(nil)
            }
        }
    }

    func showMainWindow() {
        isInMenuBarMode = false
        // Defer window operations to avoid SwiftUI view update conflicts
        DispatchQueue.main.async {
            // Show in Dock again
            NSApp.setActivationPolicy(.regular)

            // Small delay to let activation policy take effect
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
                // Find and show the main window
                for window in NSApp.windows {
                    // Skip status bar windows and About window
                    if window.className == "NSStatusBarWindow" || window.title == "About Cider Together" {
                        continue
                    }
                    // This is our main window - make it visible
                    window.deminiaturize(nil)
                    window.setIsVisible(true)
                    window.makeKeyAndOrderFront(nil)
                    NSApp.activate(ignoringOtherApps: true)
                    return
                }
            }
        }
    }

}

// MARK: - Session Callback Implementation

class SessionCallbackImpl: SessionCallback {
    private weak var appState: AppState?

    init(appState: AppState) {
        self.appState = appState
    }

    func onRoomStateChanged(state: RoomState) {
        DispatchQueue.main.async { [weak self] in
            guard let appState = self?.appState else { return }
            appState.roomState = state
            appState.isInRoom = true
            appState.isHost = state.localPeerId == state.hostPeerId

            // If we were joining, transition to connecting (we got a room state from host)
            if case .joining(.searching) = appState.viewState {
                appState.viewState = .joining(.connecting)
            }
        }
    }

    func onTrackChanged(track: TrackInfo?) {
        DispatchQueue.main.async { [weak self] in
            guard let appState = self?.appState else { return }
            appState.nowPlaying = track
            // Also update roomState.currentTrack for listeners
            if appState.roomState != nil {
                appState.roomState?.currentTrack = track
            }
        }
    }

    func onPlaybackChanged(playback: PlaybackState) {
        DispatchQueue.main.async { [weak self] in
            guard let appState = self?.appState else { return }
            appState.playback = playback
            // Also update roomState.playback for listeners
            if appState.roomState != nil {
                appState.roomState?.playback = playback
            }
        }
    }

    func onParticipantJoined(participant: Participant) {
        // Room state will be updated via onRoomStateChanged
    }

    func onParticipantLeft(peerId: String) {
        // Room state will be updated via onRoomStateChanged
    }

    func onRoomEnded(reason: String) {
        DispatchQueue.main.async { [weak self] in
            guard let appState = self?.appState else { return }
            appState.errorMessage = reason
            appState.viewState = .home
            appState.roomState = nil
            appState.isInRoom = false
            appState.isHost = false
            appState.joiningRoomCode = nil
        }
    }

    func onError(message: String) {
        DispatchQueue.main.async { [weak self] in
            guard let appState = self?.appState else { return }

            // If we were joining and got an error, show timeout state
            if case .joining = appState.viewState {
                if message.contains("not found") {
                    appState.viewState = .joining(.timeout)
                    return  // Don't show alert, the UI will display the timeout view
                }
            }

            appState.errorMessage = message
        }
    }

    func onConnected() {
        DispatchQueue.main.async { [weak self] in
            guard let appState = self?.appState else { return }

            // If we were joining, transition to inRoom
            if case .joining = appState.viewState {
                appState.viewState = .inRoom
                appState.joiningRoomCode = nil
            }
        }
    }

    func onDisconnected() {
        DispatchQueue.main.async { [weak self] in
            guard let appState = self?.appState else { return }
            appState.viewState = .home
            appState.roomState = nil
            appState.isInRoom = false
            appState.isHost = false
            appState.joiningRoomCode = nil
        }
    }
}
