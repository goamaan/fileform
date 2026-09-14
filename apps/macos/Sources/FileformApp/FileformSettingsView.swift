import SwiftUI
import FileformDomain

private enum SettingsSection: String, CaseIterable {
    case licence = "About", saving = "Saving", formats = "Formats", network = "Network", updates = "Updates"
}

struct FileformSettingsView: View {
    let model: WorkspaceModel
    @State private var section = SettingsSection.saving
    @State private var capabilities: [Capability] = []
    @State private var loadedFormats = false
    @State private var clearHistory = false
    @AppStorage("appearance") private var appearance = "system"
    @AppStorage("launchWindowSize") private var launchWindowSize = "expanded"
    private var version: String { Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "Unknown" }
    private var build: String { Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "Unknown" }
    private var developerIDBuild: Bool { Bundle.main.object(forInfoDictionaryKey: "FileformDistribution") as? String == "developer-id" }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(spacing: 12) {
                Image(nsImage: NSApplication.shared.applicationIconImage)
                    .resizable().frame(width: 44, height: 44).accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 4) {
                    Text("Fileform for Mac").font(FileformTheme.display(18))
                }
                Spacer()
                Text(version).font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink4)
            }
            HStack(spacing: 6) {
                ForEach(SettingsSection.allCases, id: \.self) { value in
                    Button { section = value } label: {
                        Text(value.rawValue).font(.system(size: 12, weight: section == value ? .medium : .regular))
                            .foregroundStyle(section == value ? FileformTheme.accentStrong : FileformTheme.ink3)
                            .padding(.horizontal, 12).frame(height: 28)
                            .background(section == value ? FileformTheme.accentSoft : .clear, in: RoundedRectangle(cornerRadius: 6))
                    }.buttonStyle(.plain).accessibilityAddTraits(section == value ? .isSelected : [])
                        .accessibilityIdentifier("settings-\(value.rawValue.lowercased())")
                }
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    switch section {
                    case .saving: saving
                    case .network: network
                    case .formats: formats
                    case .licence: licence
                    case .updates: updates
                    }
                }.frame(maxWidth: .infinity, alignment: .leading).padding(.bottom, 4)
            }
        }.padding(24).frame(width: 600, height: 470)
            .font(.system(size: 13)).foregroundStyle(FileformTheme.ink).background(FileformTheme.canvas)
            .tint(FileformTheme.accent).disabled(model.restoringSession)
            .task { capabilities = await model.engine.capabilities(); loadedFormats = true }
            .confirmationDialog("Clear workspace history?", isPresented: $clearHistory) {
                Button("Clear History", role: .destructive) { model.clearWorkspace() }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("Remove drafts and recent-result references? Source files, saved outputs and saved setups remain on disk.")
            }
    }

    private var saving: some View {
        @Bindable var preferences = model.preferences
        return VStack(alignment: .leading, spacing: 16) {
            card("SAVING") {
                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Output folder").fontWeight(.medium)
                        Text(model.outputFolder?.path ?? "No folder selected")
                            .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3).textSelection(.enabled)
                    }
                    Spacer(minLength: 12)
                    Button("Change…") { model.chooseDestination() }.buttonStyle(WorkbenchButtonStyle()).disabled(model.isRunning)
                }
                Toggle("Remember the output folder next time", isOn: $preferences.rememberOutputFolder)
                    .accessibilityIdentifier("remember-output-folder")
                Text("When off, choose a folder again after reopening Fileform.")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                Divider()
                Picker("When a name already exists", selection: $preferences.defaultCollision) {
                    Text("Add a number to the new file").tag(CollisionPolicy.rename)
                    Text("Stop and let me choose").tag(CollisionPolicy.fail)
                }.accessibilityIdentifier("default-collision-policy")
                Text("Applies to new conversion files. Existing drafts keep their choices. Existing files are never overwritten.")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            }
            card("WORKSPACE") {
                Picker("Appearance", selection: $appearance) {
                    Text("System").tag("system"); Text("Light").tag("light"); Text("Dark").tag("dark")
                }.pickerStyle(.segmented).accessibilityIdentifier("appearance")
                Picker("Open window", selection: $launchWindowSize) {
                    Text("Fill display").tag("expanded")
                    Text("Remember size").tag("remember")
                }.accessibilityIdentifier("launch-window-size")
                HStack {
                    Text("Drafts and recent results are saved on this Mac.").foregroundStyle(FileformTheme.ink3)
                    Spacer()
                    Button("Clear history…") { clearHistory = true }.buttonStyle(WorkbenchButtonStyle())
                        .disabled(model.isRunning || model.jobs.contains { $0.state == .inspecting })
                }
            }
        }
    }

    private var network: some View {
        @Bindable var preferences = model.preferences
        return VStack(alignment: .leading, spacing: 16) {
            card("LINKS") {
                Toggle("Allow link lookups and downloads", isOn: $preferences.allowLinks).accessibilityIdentifier("allow-link-network")
                Text("Lookup contacts the source and redirects; Save downloads the file. No request starts until you choose an action.")
                    .foregroundStyle(FileformTheme.ink3)
                Text("Turning this off cancels link requests. Local conversions and saved files remain available.")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            }
            card("YOUR FILES") {
                Label("Local conversions stay on this Mac", systemImage: "desktopcomputer").fontWeight(.medium)
                Text("Link URLs are not saved in history. Results retain the source hostname and verification receipt.")
                    .foregroundStyle(FileformTheme.ink3)
                Divider()
                Text("Usage reporting is off.")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            }
        }
    }

    private var formats: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Available on this Mac").font(FileformTheme.display(15))
            Text("Add a file to see which outputs support it.")
                .foregroundStyle(FileformTheme.ink3)
            if !loadedFormats { ProgressView().controlSize(.small) }
            else {
                ForEach([FileFamily.image, .media, .pdf, .table, .text], id: \.self) { family in
                    let values = capabilities.filter { $0.format.family == family }
                    if !values.isEmpty {
                        card(family.rawValue.uppercased()) {
                            ForEach(values) { capability in
                                DisclosureGroup {
                                    Text(capability.limitation ?? "Available for compatible source files.")
                                        .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3).padding(.top, 6)
                                } label: {
                                    HStack {
                                        VStack(alignment: .leading, spacing: 3) {
                                            Text(OutputDescription.title(capability.format))
                                            Text(capability.engine == "imageio" ? "From images" : capability.engine == "documents" ? "From PDFs and documents" : capability.engine == "tables" ? "From tables" : capability.engine == "qpdf" ? "From PDFs" : capability.engine == "ffmpeg" ? "From audio or video" : "Compatible sources")
                                                .font(.system(size: 11)).foregroundStyle(FileformTheme.ink4)
                                        }
                                        Spacer()
                                        Text(capability.available ? "Available" : "Unavailable")
                                            .font(.system(size: 11)).foregroundStyle(capability.available ? FileformTheme.success : FileformTheme.ink4)
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Text("Engines are bundled. Separate pack installation and removal are unavailable.")
                .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            noticesButton
        }
    }

    private var licence: some View {
        VStack(alignment: .leading, spacing: 16) {
            card("FREE AND OPEN SOURCE") {
                Text("Fileform is free. No account or activation required.").font(.system(size: 14, weight: .medium))
                Text("Use it on any of your computers.")
                    .foregroundStyle(FileformTheme.ink3)
            }
            card("FILEFORM AND ITS ENGINE") {
                Text("The app, engine and CLI are open source under Apache-2.0.")
                    .foregroundStyle(FileformTheme.ink3)
                Link("Source and contributors", destination: URL(string: "https://github.com/goamaan/fileform")!)
                noticesButton
            }
        }
    }

    private var updates: some View {
        VStack(alignment: .leading, spacing: 16) {
            card("INSTALLED VERSION") {
                HStack { Text("Fileform \(version)").font(.system(size: 15, weight: .semibold)); Spacer(); Text("Build \(build)").font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink4) }
                Text("Development channel").foregroundStyle(FileformTheme.ink3)
                Text("Automatic updates are unavailable. No update check has run.")
                    .foregroundStyle(FileformTheme.ink3)
            }
            card("DISTRIBUTION") {
                Text(developerIDBuild ? "Developer ID signed." : "Ad-hoc signed; not a notarized customer release.").foregroundStyle(FileformTheme.ink3)
            }
        }
    }

    @ViewBuilder private var noticesButton: some View {
        if let folder = Bundle.main.resourceURL?.appendingPathComponent("Notices"), FileManager.default.fileExists(atPath: folder.path) {
            Button("Show bundled notices") { model.reveal(folder) }.buttonStyle(WorkbenchButtonStyle())
        }
    }
    private func card<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title).font(FileformTheme.mono(10)).foregroundStyle(FileformTheme.ink4)
            content()
        }.frame(maxWidth: .infinity, alignment: .leading).padding(16)
            .background(FileformTheme.elevated, in: RoundedRectangle(cornerRadius: 9))
            .overlay(RoundedRectangle(cornerRadius: 9).stroke(FileformTheme.border, lineWidth: 1))
    }
}
