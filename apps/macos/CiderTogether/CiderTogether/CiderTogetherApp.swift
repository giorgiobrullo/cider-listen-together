import SwiftUI
import Sparkle

// Notification names for menu commands
extension Notification.Name {
    static let createRoom = Notification.Name("createRoom")
    static let joinRoom = Notification.Name("joinRoom")
}

@main
struct CiderTogetherApp: App {
    @StateObject private var appState = AppState()
    @State private var showMenuBarExtra = false
    private let updaterController = UpdaterController()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appState)
                .frame(minWidth: 400, minHeight: 500)
                .onReceive(appState.$isInRoom) { newValue in
                    // Defer the state change to avoid view update conflicts
                    DispatchQueue.main.async {
                        showMenuBarExtra = newValue
                    }
                }
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)
        .commands {
            CommandGroup(replacing: .appInfo) {
                Button("About Cider Together") {
                    NSApp.activate(ignoringOtherApps: true)
                    openAboutWindow()
                }
            }
            CommandGroup(after: .appInfo) {
                Button("Check for Updates...") {
                    updaterController.checkForUpdates()
                }
                .disabled(!updaterController.canCheckForUpdates)
            }
            // Room menu
            CommandMenu("Room") {
                Button("Create Room") {
                    NotificationCenter.default.post(name: .createRoom, object: nil)
                }
                .keyboardShortcut("n", modifiers: [.command])
                .disabled(appState.isInRoom)

                Button("Join Room...") {
                    NotificationCenter.default.post(name: .joinRoom, object: nil)
                }
                .keyboardShortcut("j", modifiers: [.command])
                .disabled(appState.isInRoom)

                Divider()

                Button("Copy Room Code") {
                    if let code = appState.roomState?.roomCode {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(code, forType: .string)
                    }
                }
                .keyboardShortcut("c", modifiers: [.command, .shift])
                .disabled(!appState.isInRoom)

                Divider()

                Button("Leave Room") {
                    appState.leaveRoom()
                }
                .keyboardShortcut("w", modifiers: [.command])
                .disabled(!appState.isInRoom)
            }
            // Replace default Help menu with a link to GitHub
            CommandGroup(replacing: .help) {
                Button("Cider Together Help") {
                    if let url = URL(string: "https://github.com/giorgiobrullo/cider-listen-together") {
                        NSWorkspace.shared.open(url)
                    }
                }

                Divider()

                Button("Acknowledgments") {
                    NSApp.activate(ignoringOtherApps: true)
                    openLicensesWindow()
                }
            }
        }

        // Menu bar extra - only visible when in a room
        MenuBarExtra(isInserted: $showMenuBarExtra) {
            MenuBarView()
                .environmentObject(appState)
        } label: {
            Image(systemName: "music.note.house.fill")
        }
        .menuBarExtraStyle(.window)
    }

    private func openAboutWindow() {
        let aboutView = AboutView()
        let hostingController = NSHostingController(rootView: aboutView)
        let window = NSWindow(contentViewController: hostingController)
        window.title = "About Cider Together"
        window.styleMask = [.titled, .closable]
        window.center()
        window.makeKeyAndOrderFront(nil)

        // Keep window reference alive
        WindowController.shared.aboutWindow = window
    }

    private func openLicensesWindow() {
        let licensesView = AcknowledgmentsView()
        let hostingController = NSHostingController(rootView: licensesView)
        let window = NSWindow(contentViewController: hostingController)
        window.title = "Acknowledgments"
        window.styleMask = [.titled, .closable, .resizable]
        window.setContentSize(NSSize(width: 500, height: 400))
        window.center()
        window.makeKeyAndOrderFront(nil)

        // Keep window reference alive
        WindowController.shared.licensesWindow = window
    }
}

// Helper to keep windows alive
class WindowController {
    static let shared = WindowController()
    var aboutWindow: NSWindow?
    var licensesWindow: NSWindow?
}
