import SwiftUI

struct AcknowledgmentsView: View {
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                // Header
                VStack(alignment: .leading, spacing: 8) {
                    Text("Acknowledgments")
                        .font(.title.bold())

                    Text("Cider Together uses the following open source libraries:")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }

                Divider()

                // Libraries
                VStack(alignment: .leading, spacing: 16) {
                    LicenseItem(
                        name: "libp2p",
                        license: "MIT",
                        url: "https://github.com/libp2p/rust-libp2p",
                        licenseUrl: "https://opensource.org/licenses/MIT"
                    )

                    LicenseItem(
                        name: "UniFFI",
                        license: "MPL-2.0",
                        url: "https://github.com/mozilla/uniffi-rs",
                        licenseUrl: "https://opensource.org/licenses/MPL-2.0"
                    )

                    LicenseItem(
                        name: "Tokio",
                        license: "MIT",
                        url: "https://github.com/tokio-rs/tokio",
                        licenseUrl: "https://opensource.org/licenses/MIT"
                    )

                    LicenseItem(
                        name: "Serde",
                        license: "MIT / Apache-2.0",
                        url: "https://github.com/serde-rs/serde",
                        licenseUrl: "https://opensource.org/licenses/MIT"
                    )

                    LicenseItem(
                        name: "Reqwest",
                        license: "MIT / Apache-2.0",
                        url: "https://github.com/seanmonstar/reqwest",
                        licenseUrl: "https://opensource.org/licenses/MIT"
                    )

                    LicenseItem(
                        name: "Sparkle",
                        license: "MIT",
                        url: "https://github.com/sparkle-project/Sparkle",
                        licenseUrl: "https://opensource.org/licenses/MIT"
                    )
                }

                Divider()

                // License notice
                Text("This software is proprietary. All rights reserved.\nSee individual library licenses for their respective terms.")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            .padding(24)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(minWidth: 400, minHeight: 300)
    }
}

struct LicenseItem: View {
    let name: String
    let license: String
    let url: String
    let licenseUrl: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(name)
                    .font(.headline)

                Spacer()

                Link(destination: URL(string: licenseUrl)!) {
                    Text(license)
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 2)
                        .background(Color.secondary.opacity(0.1))
                        .cornerRadius(4)
                }
                .buttonStyle(.plain)
            }

            Link(url, destination: URL(string: url)!)
                .font(.caption)
                .foregroundColor(.accentColor)
        }
    }
}

#Preview {
    AcknowledgmentsView()
        .frame(width: 500, height: 400)
}
