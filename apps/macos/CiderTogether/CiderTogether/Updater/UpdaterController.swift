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

    /// Detects if the app was installed via Homebrew by checking if Homebrew's symlink points to us
    private static func detectHomebrewInstall() -> Bool {
        let fileManager = FileManager.default
        let appPath = Bundle.main.bundlePath

        // Homebrew keeps a symlink in Caskroom pointing to the installed app
        // Apple Silicon: /opt/homebrew/Caskroom/cider-together/VERSION/CiderTogether.app -> /Applications/...
        // Intel: /usr/local/Caskroom/cider-together/VERSION/CiderTogether.app -> /Applications/...
        let caskroomBases = [
            "/opt/homebrew/Caskroom/cider-together",
            "/usr/local/Caskroom/cider-together"
        ]

        for base in caskroomBases {
            guard fileManager.fileExists(atPath: base),
                  let versions = try? fileManager.contentsOfDirectory(atPath: base) else {
                continue
            }

            // Check each version directory for a symlink pointing to our app
            for version in versions where !version.hasPrefix(".") {
                let symlinkPath = "\(base)/\(version)/CiderTogether.app"
                if let target = try? fileManager.destinationOfSymbolicLink(atPath: symlinkPath),
                   target == appPath {
                    return true
                }
            }
        }

        return false
    }
}
