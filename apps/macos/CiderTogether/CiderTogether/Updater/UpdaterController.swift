import Foundation
import Combine
import Sparkle

/// Controller for managing app updates via Sparkle
final class UpdaterController: ObservableObject {
    let updater: SPUUpdater

    /// True if app was installed via Homebrew (updates managed externally)
    let isHomebrewInstall: Bool

    @Published var canCheckForUpdates = false
    @Published var lastUpdateCheckDate: Date?

    init() {
        // Check if installed via Homebrew before starting updater
        self.isHomebrewInstall = Self.detectHomebrewInstall()

        // Create the updater controller - only start if not Homebrew install
        let updaterController = SPUStandardUpdaterController(
            startingUpdater: !isHomebrewInstall,
            updaterDelegate: nil,
            userDriverDelegate: nil
        )
        self.updater = updaterController.updater

        // Observe canCheckForUpdates
        updater.publisher(for: \.canCheckForUpdates)
            .assign(to: &$canCheckForUpdates)

        updater.publisher(for: \.lastUpdateCheckDate)
            .assign(to: &$lastUpdateCheckDate)
    }

    func checkForUpdates() {
        guard !isHomebrewInstall else { return }
        updater.checkForUpdates()
    }

    /// Detects if the app was installed via Homebrew by checking its location
    private static func detectHomebrewInstall() -> Bool {
        let bundlePath = Bundle.main.bundlePath

        // Resolve symlinks to get the real installation path
        let realPath = (bundlePath as NSString).resolvingSymlinksInPath

        // Homebrew installs to /opt/homebrew/Caskroom (Apple Silicon)
        // or /usr/local/Caskroom (Intel)
        return realPath.contains("/Caskroom/") || realPath.contains("/homebrew/")
    }
}
