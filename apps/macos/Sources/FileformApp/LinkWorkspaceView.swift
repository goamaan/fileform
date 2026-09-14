import SwiftUI
import FileformDomain

struct LinkWorkspaceView: View {
    let model: WorkspaceModel
    let compact: Bool
    @Bindable var link: LinkWorkspaceModel
    var body: some View {
        HStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    VStack(alignment: .leading, spacing: 5) {
                        Text("Save from a link").font(.system(size: 17, weight: .semibold))
                        Text("Paste a direct audio or video URL. Video-site webpages are unsupported.")
                            .font(.system(size: 13)).foregroundStyle(FileformTheme.ink3)
                    }
                    HStack(spacing: 8) {
                        TextField("https://…", text: $link.urlText).textFieldStyle(.roundedBorder)
                            .font(FileformTheme.mono(12)).accessibilityIdentifier("link-url")
                            .onSubmit { link.lookup() }
                        Button("Look Up") { link.lookup() }.disabled(!link.canLookup)
                            .accessibilityIdentifier("lookup-link")
                    }.disabled(link.isBusy)
                    if !link.networkAllowed {
                        WorkbenchNotice(text: "Link access is off. Enable it in Settings → Network.")
                        SettingsLink { Text("Open Settings") }.buttonStyle(WorkbenchButtonStyle())
                    } else {
                        WorkbenchNotice(text: "Lookup and saving contact the source and redirects. Local files are not uploaded.")
                    }
                    if link.isBusy {
                        HStack { ProgressView().controlSize(.small); Text(link.status).font(.system(size: 13)) }
                        if let fraction = link.fraction { ProgressView(value: fraction) }
                    }
                    if let source = link.source {
                        VStack(alignment: .leading, spacing: 14) {
                            HStack(spacing: 12) {
                                Image(systemName: "waveform").font(.system(size: 25)).foregroundStyle(FileformTheme.media)
                                    .frame(width: 72, height: 48).background(FileformTheme.field, in: RoundedRectangle(cornerRadius: 4))
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(link.outputName).font(.system(size: 13, weight: .semibold)).lineLimit(2)
                                    Text(source.resolvedURL.host ?? "Source").font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink3)
                                    Text(source.expectedBytes.map { ByteCountFormatter.string(fromByteCount: $0, countStyle: .file) } ?? "Size unknown")
                                        .font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink4)
                                }
                            }
                            Divider()
                            Text("Original file").font(.system(size: 13, weight: .semibold))
                            Text("Saved without conversion. File type is verified first.")
                                .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                            WorkbenchChoice(title: "Source file type", selection: $link.format, options: [.mp4, .mov, .m4a, .wav, .flac, .mp3]) { OutputDescription.title($0) }
                            TextField("Output name", text: $link.outputName).textFieldStyle(.roundedBorder).accessibilityIdentifier("link-output-name")
                            Toggle("Open in Trim after saving", isOn: $link.openInTrim)
                            if let plan = link.plan { DisclosureGroup("What to expect") { ForEach(plan.warnings, id: \.self) { Text($0).font(.system(size: 12)) } } }
                        }.padding(14).background(FileformTheme.inset, in: RoundedRectangle(cornerRadius: 9))
                            .overlay(RoundedRectangle(cornerRadius: 9).stroke(FileformTheme.border, lineWidth: 1))
                            .disabled(link.isBusy)
                    }
                    HStack {
                        Text("Maximum download").font(.system(size: 12))
                        TextField("512", text: $link.maximumMB).frame(width: 70).textFieldStyle(.roundedBorder).accessibilityLabel("Maximum download in MB")
                        Text("MB").font(FileformTheme.mono(12))
                        Spacer()
                    }.disabled(link.isBusy)
                    if let error = link.error { WorkbenchNotice(text: error, tone: FileformTheme.danger) }
                    if let notice = link.notice { WorkbenchNotice(text: notice, tone: FileformTheme.success) }
                    if compact { recentSaves }
                }.padding(.horizontal, 24).padding(.vertical, 20)
            }.frame(maxWidth: .infinity, maxHeight: .infinity)
            if !compact { WorkbenchInspector { ScrollView { recentSaves.padding(.horizontal, 15).padding(.vertical, 17) } } }
        }.buttonStyle(WorkbenchButtonStyle())
    }
    private var recentSaves: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Recent saves").font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink4)
            if link.history.isEmpty { Text("No saved links yet.").font(.system(size: 12)).foregroundStyle(FileformTheme.ink3) }
            ForEach(link.history) { saved in
                VStack(alignment: .leading, spacing: 5) {
                    Text(saved.url.lastPathComponent).font(.system(size: 13, weight: .medium)).lineLimit(2)
                    Text(saved.receipt.sourceHost).font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink3).lineLimit(1)
                    Text(saved.date.formatted(date: .abbreviated, time: .omitted) + " · " + ByteCountFormatter.string(fromByteCount: saved.receipt.bytes, countStyle: .file))
                        .font(.system(size: 12)).foregroundStyle(FileformTheme.ink4)
                    HStack {
                        Button("Reveal") { model.reveal(saved.url) }
                        Button("Trim") { model.openLinkResultInTrim(saved) }.disabled(model.isRunning)
                    }
                }.draggable(saved.url)
            }
            Divider()
            Text("Save only files you have permission to download.").font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
}
