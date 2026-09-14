import SwiftUI

struct PDFSplitControls: View {
    let draft: PDFWorkspaceModel
    @Environment(\.undoManager) private var undo
    @State private var every = "2"
    @State private var ranges = ""
    @State private var error: String?
    private enum Field: Hashable { case every, ranges }
    @FocusState private var focusedField: Field?

    var body: some View {
        DisclosureGroup("Split tools") {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Text("Every")
                    TextField("2", text: $every).frame(width: 54).focused($focusedField, equals: .every)
                        .accessibilityLabel("Pages per PDF").accessibilityIdentifier("pdf-split-every")
                    Text("pages")
                    Spacer(minLength: 0)
                    Button("Apply") {
                        perform {
                            guard let count = Int(every) else { throw SplitInputError() }
                            try draft.splitEvery(count, undo: undo)
                        }
                    }.accessibilityLabel("Split every specified number of pages").accessibilityIdentifier("pdf-apply-every")
                }
                Divider()
                Text("Page ranges").font(.system(size: 12, weight: .medium))
                TextField("1-3; 4,2; 5", text: $ranges).focused($focusedField, equals: .ranges)
                    .font(FileformTheme.mono(12)).accessibilityLabel("Output PDF page ranges")
                    .accessibilityIdentifier("pdf-split-ranges")
                Text("Numbers refer to the current arrangement. Commas join pages in one PDF; semicolons start another. Order and repeated pages are kept.")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                Button("Apply ranges") { perform { try draft.applyPageRanges(ranges, undo: undo) } }
                    .accessibilityIdentifier("pdf-apply-ranges")
                Text("Apply updates the page preview. Pages omitted from ranges leave this draft; originals stay untouched. Undo restores the previous arrangement.")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                if let error { Text(error).font(.system(size: 12)).foregroundStyle(FileformTheme.danger) }
            }.padding(.top, 8).textFieldStyle(.roundedBorder)
                .disabled(draft.isRunning || draft.pages.isEmpty)
        }.font(.system(size: 13))
    }
    private func perform(_ action: () throws -> Void) {
        focusedField = nil
        do { try action(); error = nil } catch { self.error = error.localizedDescription }
    }
    private struct SplitInputError: LocalizedError {
        var errorDescription: String? { "Enter a positive whole number of pages per PDF." }
    }
}
