import SwiftUI
import FileformDomain

struct WelcomeWorkspaceView: View {
    let model: WorkspaceModel

    private var title: String {
        if model.workspaceTask == .text { return "Extract text" }
        switch model.pendingImportGoal {
        case .fit: return "Fit a size limit"
        case .compress: return "Make a file smaller"
        default: return "Drop files here"
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            VStack(spacing: 22) {
                paperCards.accessibilityHidden(true)
                VStack(spacing: 8) {
                    Text(title).font(FileformTheme.display(21)).multilineTextAlignment(.center)
                    Text(model.pendingImportGoal == nil
                         ? "Images, PDFs, audio, video and tables."
                         : "Choose an output and size after adding files.")
                        .font(.system(size: 13)).foregroundStyle(FileformTheme.ink3)
                        .multilineTextAlignment(.center).lineSpacing(3).frame(maxWidth: 390)
                }
                HStack(spacing: 10) {
                    Button { model.chooseFiles() } label: {
                        HStack(spacing: 9) { Text("Choose Files…"); Text("⌘O").font(FileformTheme.mono(10)).opacity(0.75) }
                    }.buttonStyle(PrimaryButtonStyle()).accessibilityIdentifier("choose-files")
                    Button { model.pasteLink() } label: { Text("Paste a link").padding(.vertical, 2) }
                        .buttonStyle(WorkbenchButtonStyle()).accessibilityIdentifier("welcome-paste-link")
                }
            }.padding(30).frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(FileformTheme.inset, in: RoundedRectangle(cornerRadius: 10))
                .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(FileformTheme.fieldLine, style: StrokeStyle(lineWidth: 1, dash: [5, 4])))
                .padding(20)
            HStack(alignment: .top, spacing: 10) {
                shortcut("Combine PDFs") { model.chooseTask(.pages) }
                shortcut("Trim a recording") { model.chooseTask(.trim) }
                shortcut("Fit a size limit") { model.startConversion(goal: .fit) }
            }.padding(.horizontal, 20).padding(.bottom, 20)
        }
    }

    private var paperCards: some View {
        HStack(alignment: .bottom, spacing: -12) {
            paper("PDF").rotationEffect(.degrees(-7))
            paper("HEIC").zIndex(1)
            paper("MOV").rotationEffect(.degrees(7))
        }.frame(height: 74)
    }

    private func paper(_ title: String) -> some View {
        RoundedRectangle(cornerRadius: 4).fill(FileformTheme.elevated)
            .overlay(RoundedRectangle(cornerRadius: 4).stroke(FileformTheme.fieldLine, lineWidth: 1))
            .overlay(alignment: .bottomTrailing) { Text(title).font(FileformTheme.mono(8)).foregroundStyle(FileformTheme.ink4).padding(6) }
            .frame(width: 52, height: 66)
    }

    private func shortcut(_ title: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(.system(size: 12, weight: .medium)).foregroundStyle(FileformTheme.ink)
            }.frame(maxWidth: .infinity, minHeight: 38, alignment: .topLeading).padding(12)
                .background(FileformTheme.chrome, in: RoundedRectangle(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(FileformTheme.border, lineWidth: 1))
        }.buttonStyle(.plain)
    }
}
