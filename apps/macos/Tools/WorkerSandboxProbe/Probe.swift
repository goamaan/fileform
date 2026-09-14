import SwiftUI
import AppKit
import CryptoKit
import Darwin
import FileformCore

@main
struct WorkerSandboxProbe: App {
    var body: some Scene {
        WindowGroup("Fileform worker sandbox probe") { ProbeView() }
            .defaultSize(width: 640, height: 460)
    }
}
struct ProbeView: View {
    @State private var report = "Choose the synthetic source.png fixture. The probe will inspect and preview it through inherited handles, then verify that an unselected sibling is denied."
    @State private var running = false
    @State private var preview: NSImage?
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Native worker access test").font(.title2)
            Text(report).textSelection(.enabled).fixedSize(horizontal: false, vertical: true).frame(maxWidth: .infinity, alignment: .leading)
                .accessibilityIdentifier("probe.report")
            if let preview { Image(nsImage: preview).resizable().scaledToFit().frame(height: 120) }
            HStack {
                Button("Choose source fixture…", action: choose).disabled(running).keyboardShortcut("o")
                if running { ProgressView().controlSize(.small) }
            }
            Spacer()
        }.padding(24).frame(minWidth: 560, minHeight: 400)
    }
    private func choose() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true; panel.canChooseDirectories = false; panel.allowsMultipleSelection = false
        panel.prompt = "Inspect fixture"
        guard panel.runModal() == .OK, let source = panel.url else { report = "Picker cancelled; no worker started."; return }
        running = true; report = "Running sandboxed worker…"
        let scoped = source.startAccessingSecurityScopedResource()
        Task {
            defer { if scoped { source.stopAccessingSecurityScopedResource() }; running = false }
            do {
                let before = try Data(contentsOf: source)
                let sibling = source.deletingLastPathComponent().appendingPathComponent("unselected.png")
                let fd = open(sibling.path, O_RDONLY)
                let denied = fd < 0 && errno == EPERM
                if fd >= 0 { close(fd) }
                let worker = Bundle.main.bundleURL.appendingPathComponent("Contents/Helpers/fileform-worker")
                let client = NativeWorkerClient(executable: worker)
                let inspection = try await client.inspect(source)
                let artifact = try await client.preview(source, maximumDimension: 240)
                let unchanged = SHA256.hash(data: before) == SHA256.hash(data: try Data(contentsOf: source))
                preview = NSImage(data: artifact.png)
                report = "\(denied && unchanged && preview != nil ? "PASS" : "FAIL")\nSelected file: \(inspection.family.rawValue)\nWorker preview: \(artifact.width) × \(artifact.height), \(artifact.png.count) bytes\nOriginal unchanged: \(unchanged)\nUnselected sibling denied: \(denied)\nWorker process completed and staging cleaned."
            } catch { report = "FAIL\n\(error.localizedDescription)" }
        }
    }
}
