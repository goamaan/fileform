import SwiftUI
import CoreText

@main
struct FileformApp: App {
    @NSApplicationDelegateAdaptor(FileformAppDelegate.self) private var delegate
    @AppStorage("appearance") private var appearance = "system"
    @State private var workspace = WorkspaceModel()
    init() {
        for name in ["JetBrainsMono-Regular", "JetBrainsMono-Medium"] {
            if let url = Bundle.main.url(forResource: name, withExtension: "ttf", subdirectory: "Fonts") {
                CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
            }
        }
    }
    var body: some Scene {
        Window("Fileform", id: "main") {
            WorkspaceView(model: workspace)
                .background(LaunchWindowSize())
                .frame(minWidth: 740, minHeight: 508) // 560-point minimum including the native titlebar.
                .preferredColorScheme(appearance == "dark" ? .dark : appearance == "light" ? .light : nil)
                .task {
                    delegate.readyForFiles = false; delegate.workspace = workspace
                    await workspace.startPersistence()
                    delegate.readyForFiles = true
                }
                .tint(FileformTheme.accent)
        }
        .defaultSize(width: 1180, height: 720)
        .commands {
            CommandGroup(after: .toolbar) {
                Button("Compact Window") { NSApp.mainWindow?.setContentSize(NSSize(width: 740, height: 560)) }
                Button("Standard Window") { NSApp.mainWindow?.setContentSize(NSSize(width: 1180, height: 720)) }
                Button("Fill Display") {
                    guard let window = NSApp.mainWindow, let screen = window.screen else { return }
                    window.setFrame(screen.visibleFrame, display: true, animate: true)
                }
            }
            CommandGroup(replacing: .newItem) {
                Button("Save Choices as a Setup…") { workspace.presentedSheet = .setups(.saveCurrent) }
                    .keyboardShortcut("s").disabled(workspace.currentSetupRequest == nil)
                Button("Saved Setups…") { workspace.presentedSheet = .setups(.manage) }.keyboardShortcut("s", modifiers: [.command, .shift])
                Divider()
                Button("Clear Workspace History") { workspace.clearWorkspace() }.disabled(workspace.restoringSession || workspace.isRunning || workspace.jobs.isEmpty)
                Button("Add Files…") { workspace.chooseFiles() }.keyboardShortcut("o").disabled(workspace.restoringSession)
                Button("Choose Output Folder…") { workspace.chooseDestination() }.keyboardShortcut("o", modifiers: [.command, .shift])
                Divider()
                Button("Find a Task or Format…") { NotificationCenter.default.post(name: .fileformSearch, object: nil) }.keyboardShortcut("k")
                Button("Show Inspector") { NotificationCenter.default.post(name: .fileformInspector, object: nil) }.keyboardShortcut("i")
                Button("Convert Files") { workspace.chooseTask(.convert) }.keyboardShortcut("1").disabled(workspace.restoringSession)
                Button("Organize PDFs") { workspace.chooseTask(.pages) }.keyboardShortcut("2").disabled(workspace.restoringSession)
                Button("Trim Audio or Video") { workspace.chooseTask(.trim) }.keyboardShortcut("3").disabled(workspace.restoringSession)
                Button("Save from a Link") { workspace.chooseTask(.link) }.keyboardShortcut("4").disabled(workspace.restoringSession)
                Button("Open Link Workspace") { workspace.chooseTask(.link) }.keyboardShortcut("l").disabled(workspace.restoringSession)
                Button("Read Text") { workspace.chooseTask(.text) }.keyboardShortcut("5").disabled(workspace.restoringSession)
                Button("Stop Everything") { workspace.cancelAll() }.keyboardShortcut(".", modifiers: [.command, .option]).disabled(!workspace.isRunning)
                Divider()
                Button(workspace.primaryAction) { workspace.runReadyJobs() }.keyboardShortcut(.return, modifiers: .command).disabled(!workspace.canRun)
            }
        }
        Settings { FileformSettingsView(model: workspace).preferredColorScheme(appearance == "dark" ? .dark : appearance == "light" ? .light : nil) }
    }
}

/// Apply the launch preference to this scene's window once, after attachment.
private struct LaunchWindowSize: NSViewRepresentable {
    func makeNSView(context: Context) -> WindowObserver { WindowObserver() }
    func updateNSView(_ nsView: WindowObserver, context: Context) {}

    final class WindowObserver: NSView {
        private var applied = false
        private var exitObserver: NSObjectProtocol?
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            guard !applied, let window else { return }
            applied = true
            DispatchQueue.main.async { [weak window] in
                guard let window else { return }
                window.deminiaturize(nil)
                let choice = UserDefaults.standard.string(forKey: "launchWindowSize") ?? "expanded"
                if choice != "remember", let screen = window.screen ?? NSScreen.main {
                    if window.styleMask.contains(.fullScreen) {
                        self.exitObserver = NotificationCenter.default.addObserver(forName: NSWindow.didExitFullScreenNotification, object: window, queue: .main) { [weak self, weak window] _ in
                            MainActor.assumeIsolated {
                                if let window, let screen = window.screen ?? NSScreen.main {
                                    window.setFrame(screen.visibleFrame, display: true)
                                }
                                if let observer = self?.exitObserver { NotificationCenter.default.removeObserver(observer) }
                                self?.exitObserver = nil
                            }
                        }
                        window.toggleFullScreen(nil)
                    } else {
                        window.setFrame(screen.visibleFrame, display: true)
                    }
                }
            }
        }
    }
}

@MainActor final class FileformAppDelegate: NSObject, NSApplicationDelegate {
    var workspace: WorkspaceModel? {
        didSet {
            guard readyForFiles, let workspace, !pendingFiles.isEmpty else { return }
            let files = pendingFiles; pendingFiles.removeAll()
            workspace.addFiles(files)
        }
    }
    var readyForFiles = true {
        didSet {
            guard readyForFiles, let workspace, !pendingFiles.isEmpty else { return }
            let files = pendingFiles; pendingFiles.removeAll(); workspace.addFiles(files)
        }
    }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let workspace else { return .terminateNow }
        if workspace.restoringSession {
            Task { @MainActor in
                while workspace.restoringSession { try? await Task.sleep(for: .milliseconds(100)) }
                workspace.saveSession(); sender.reply(toApplicationShouldTerminate: true)
            }
            return .terminateLater
        }
        guard workspace.isRunning else { workspace.saveSession(); return .terminateNow }
        let alert = NSAlert()
        alert.messageText = "Work is still running"
        alert.informativeText = "Wait for completed files, or stop work and quit. Your originals and completed results will be kept."
        alert.addButton(withTitle: "Wait and Quit")
        alert.addButton(withTitle: "Stop and Quit")
        alert.addButton(withTitle: "Cancel")
        let response = alert.runModal()
        if response == .alertThirdButtonReturn { return .terminateCancel }
        if response == .alertSecondButtonReturn { workspace.cancelAll() }
        Task { @MainActor in
            while workspace.isRunning { try? await Task.sleep(for: .milliseconds(100)) }
            workspace.saveSession()
            sender.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
    }
    private var pendingFiles: [URL] = []
    func application(_ sender: NSApplication, open urls: [URL]) {
        if readyForFiles, let workspace { workspace.addFiles(urls) }
        else { pendingFiles.append(contentsOf: urls) }
    }
}
