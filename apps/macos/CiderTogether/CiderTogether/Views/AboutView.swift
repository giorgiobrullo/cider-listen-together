import SwiftUI

struct AboutView: View {
    @Environment(\.dismiss) private var dismiss

    private var appVersion: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0"
    }

    private var buildNumber: String {
        Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "1"
    }

    var body: some View {
        VStack(spacing: 20) {
            // App Icon
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .frame(width: 96, height: 96)
                .cornerRadius(20)
                .shadow(color: .black.opacity(0.2), radius: 8, y: 4)

            // App Name & Version
            VStack(spacing: 4) {
                Text("Cider Together")
                    .font(.title.bold())

                Text("Version \(appVersion) (\(buildNumber))")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }

            Divider()
                .frame(width: 200)

            // Credits
            VStack(spacing: 16) {
                VStack(spacing: 4) {
                    Text("Created by")
                        .font(.caption)
                        .foregroundColor(.secondary)

                    Text("Giorgio Brullo")
                        .font(.headline)

                    Link("@giorgiobrullo", destination: URL(string: "https://github.com/giorgiobrullo")!)
                        .font(.subheadline)
                        .foregroundColor(.accentColor)
                }

                VStack(spacing: 4) {
                    Text("Built on")
                        .font(.caption)
                        .foregroundColor(.secondary)

                    Link("Cider", destination: URL(string: "https://cider.sh")!)
                        .font(.subheadline.bold())
                        .foregroundColor(.accentColor)

                    Text("by the Cider Collective")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }

            // Description
            Text("Listen to music together with friends.")
                .font(.caption)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 240)

            Divider()
                .frame(width: 200)

            // Links
            Link(destination: URL(string: "https://github.com/giorgiobrullo/cider-listen-together")!) {
                Label("GitHub", systemImage: "link")
                    .font(.caption)
            }

            // Copyright
            Text("© 2025 Giorgio Brullo")
                .font(.caption2)
                .foregroundStyle(.secondary.opacity(0.7))
        }
        .padding(32)
        .frame(width: 320)
    }
}

#Preview {
    AboutView()
}
