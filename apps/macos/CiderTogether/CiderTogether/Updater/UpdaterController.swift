import Foundation
import Combine
import Sparkle

/// Controller for managing app updates via Sparkle
final class UpdaterController: ObservableObject {
    let updater: SPUUpdater

    @Published var canCheckForUpdates = false
    @Published var lastUpdateCheckDate: Date?

    init() {
        // Create the updater controller with the main bundle
        let updaterController = SPUStandardUpdaterController(
            startingUpdater: true,
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
        updater.checkForUpdates()
    }
}
