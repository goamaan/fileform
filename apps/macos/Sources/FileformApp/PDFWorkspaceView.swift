import SwiftUI
import FileformDomain

struct PDFWorkspaceView: View {
    let model: WorkspaceModel
    var compact = false
    var inspectorVisible = true
    @Environment(\.undoManager) private var undo
    @State private var showingSettings = false
    @State private var pageFrames: [UUID: CGRect] = [:]
    @State private var dragPointer: CGPoint?
    @State private var dragTarget: PDFPageDropTarget?
    private let pageCoordinateSpace = "pdf-page-grid"
    private var draft: PDFWorkspaceModel { model.pdfWorkspace }
    private var inlineInspector: Bool { !compact && inspectorVisible }

    var body: some View {
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 0) {
                toolbar
                if !model.pendingPDFSourceJobs.isEmpty { pendingSources.padding(16) }
                if draft.pages.isEmpty {
                    ContentUnavailableView("Bring PDFs or photos", systemImage: "doc.on.doc",
                        description: Text("Arrange pages, then save as PDF or images."))
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    controls.padding(.horizontal, 16).padding(.vertical, 10)
                    ScrollView {
                        VStack(alignment: .leading, spacing: 16) {
                            ForEach(sourceRuns) { run in
                                VStack(alignment: .leading, spacing: 12) {
                                    sourceHeading(run)
                                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 88, maximum: 120), spacing: 12)], spacing: 12) {
                                        ForEach(run.indices.map { draft.pages[$0] }) { page in
                                            if let index = draft.pages.firstIndex(where: { $0.id == page.id }) { pageCard(page, index: index) }
                                        }
                                    }
                                }
                            }
                            Button { model.chooseFiles() } label: {
                                Label("Add PDFs or images…", systemImage: "plus").font(.system(size: 13))
                                    .frame(maxWidth: .infinity).padding(.vertical, 12)
                                    .foregroundStyle(FileformTheme.ink3)
                                    .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(FileformTheme.fieldLine, style: StrokeStyle(lineWidth: 1, dash: [4])))
                            }.buttonStyle(.plain).disabled(draft.isRunning)
                            if let error = draft.error { WorkbenchNotice(text: error, tone: FileformTheme.danger) }
                        }.padding(16)
                    }
                    .coordinateSpace(name: pageCoordinateSpace)
                    .onPreferenceChange(PDFPageFramesKey.self) { [framesBinding = $pageFrames] frames in
                        framesBinding.wrappedValue = frames
                    }
                    .overlay(alignment: .topLeading) {
                        if let point = dragPointer, let payload = draft.activePageDrag {
                            Text("\(payload.pageIDs.count) \(payload.pageIDs.count == 1 ? "page" : "pages")")
                                .font(.system(size: 12, weight: .medium)).padding(.horizontal, 12).padding(.vertical, 8)
                                .background(FileformTheme.elevated, in: RoundedRectangle(cornerRadius: 6))
                                .overlay(RoundedRectangle(cornerRadius: 6).stroke(FileformTheme.border, lineWidth: 1))
                                .fixedSize().position(x: point.x + 46, y: max(18, point.y - 24))
                                .allowsHitTesting(false).accessibilityHidden(true)
                        }
                    }
                    .onExitCommand { draft.cancelPageDrag(); dragPointer = nil; dragTarget = nil }
                    .onChange(of: draft.activePageDrag) { _, value in
                        if value == nil { dragPointer = nil; dragTarget = nil }
                    }
                }
                if !draft.completedArtifacts.isEmpty {
                    DisclosureGroup("Saved files · \(draft.completedArtifacts.count)") { savedFiles.padding(.top, 8) }
                        .font(.system(size: 13)).padding(.horizontal, 16).padding(.vertical, 10)
                }
            }.frame(maxWidth: .infinity, maxHeight: .infinity)
            if inlineInspector { WorkbenchInspector { PDFInspectorView(model: model) } }
        }.background(FileformTheme.canvas)
            .onDisappear { draft.cancelPageDrag() }
            .onReceive(NotificationCenter.default.publisher(for: .fileformInspector)) { _ in if compact { showingSettings = true } }
            .sheet(isPresented: $showingSettings) {
                VStack(spacing: 0) {
                    HStack {
                        Text("PDF Settings").font(.system(size: 17, weight: .semibold))
                        Spacer()
                        Button("Done") { showingSettings = false }.keyboardShortcut(.defaultAction)
                    }.padding(16)
                    Divider()
                    PDFInspectorView(model: model)
                }.frame(width: 370, height: 490).background(FileformTheme.elevated)
            }
    }

    private var toolbar: some View {
        HStack(spacing: 10) {
            Text("Organize pages").font(.system(size: 13, weight: .semibold)).foregroundStyle(FileformTheme.ink)
            Text("\(draft.pages.count) page\(draft.pages.count == 1 ? "" : "s") · \(draft.outputSummary)")
                .font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink3).lineLimit(1)
            Spacer(minLength: 0)
            if !inlineInspector {
                Button { showingSettings = true } label: { Image(systemName: "slider.horizontal.3") }
                    .accessibilityLabel("PDF Settings").help("PDF Settings")
            }
            Menu {
                Button("Save as Setup…") { model.presentedSheet = .setups(.saveCurrent) }.disabled(model.currentSetupRequest == nil)
                Button("Saved Setups…") { model.presentedSheet = .setups(.manage) }
            } label: { Image(systemName: "ellipsis") }.menuIndicator(.hidden).accessibilityLabel("PDF choices")
            Button("Add files…") { model.chooseFiles() }.disabled(draft.isRunning)
        }.buttonStyle(WorkbenchButtonStyle()).padding(.horizontal, 16).padding(.vertical, 12)
            .overlay(alignment: .bottom) { Rectangle().fill(FileformTheme.border).frame(height: 1) }
    }

    private var pendingSources: some View {
        HStack {
            Text("\(model.pendingPDFSourceJobs.count) source\(model.pendingPDFSourceJobs.count == 1 ? " is" : "s are") waiting to join the draft.")
                .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            Spacer()
            Menu("Pending sources") {
                Button("Review Files") { model.chooseTask(.convert) }
                ForEach(model.pendingPDFSourceJobs) { job in
                    Button("Skip \(job.input.lastPathComponent) · \(job.state.title)") { model.skipPendingPDFImport(job.id) }
                }
            }
        }
    }

    private var controls: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) { selectionLabel; pageActions; Spacer(minLength: 0) }
            VStack(alignment: .leading, spacing: 8) { selectionLabel; HStack(spacing: 8) { pageActions; Spacer(minLength: 0) } }
        }.buttonStyle(WorkbenchButtonStyle()).controlSize(.small)
    }
    private var selectionLabel: some View {
        Text(draft.selectedPageCount > 1 ? "\(draft.selectedPageCount) pages selected" : selectedIndex.map { "Page \($0 + 1) selected" } ?? "Select pages")
            .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3).fixedSize()
            .accessibilityIdentifier("pdf-selection-count")
    }
    @ViewBuilder private var pageActions: some View {
        Button { draft.moveSelection(-1, undo: undo) } label: { Image(systemName: "arrow.left") }
            .disabled(!draft.canMoveSelection(-1)).accessibilityLabel("Move selected pages before").help("Move selected pages before")
            .accessibilityIdentifier("pdf-move-before")
        Button { draft.moveSelection(1, undo: undo) } label: { Image(systemName: "arrow.right") }
            .disabled(!draft.canMoveSelection(1)).accessibilityLabel("Move selected pages after").help("Move selected pages after")
            .accessibilityIdentifier("pdf-move-after")
        Button("Rotate") { draft.rotateSelection(undo: undo) }.disabled(draft.isRunning || draft.selectedPageCount == 0)
            .accessibilityLabel("Rotate selected pages").accessibilityIdentifier("pdf-rotate-selection")
        Button("Remove") { draft.removeSelection(undo: undo) }.disabled(draft.isRunning || draft.selectedPageCount == 0)
            .accessibilityLabel("Remove selected pages").accessibilityIdentifier("pdf-remove-selection")
        Menu("More") {
            Button("Duplicate Selected Pages") { draft.duplicateSelection(undo: undo) }.disabled(draft.selectedPageCount == 0)
            Button(selectedIndex.map { draft.pages[$0].splitAfter ? "Remove Split After Page" : "Split After Page" } ?? "Split After Page") {
                change("Split PDF") { $0.splitAfter.toggle() }
            }.disabled(draft.selectedPageCount != 1 || selectedIndex == draft.pages.count - 1)
            Divider()
            Button("Select All Pages") { draft.selectPages(Set(draft.pages.map(\.id))) }
            Button("Deselect All Pages") { draft.selectPages([]) }.disabled(draft.selectedPageCount == 0)
        }.disabled(draft.isRunning)
    }
    private var savedFiles: some View {
        VStack(alignment: .leading, spacing: 6) {
            Label("Saved files · \(draft.completedArtifacts.count)", systemImage: "checkmark.circle.fill")
                .font(.system(size: 13)).foregroundStyle(FileformTheme.success)
            ScrollView {
                VStack(spacing: 6) {
                    ForEach(draft.completedArtifacts, id: \.url) { file in
                        HStack {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(file.url.lastPathComponent).font(FileformTheme.mono(11)).lineLimit(1).help(file.url.path)
                                if let image = file.pdfEmbeddedImage {
                                    Text(savedImageSummary(image)).font(.system(size: 11)).foregroundStyle(FileformTheme.ink3)
                                    Text(savedImageProvenance(image)).font(.system(size: 11)).foregroundStyle(FileformTheme.ink3)
                                        .lineLimit(2).help(savedImageProvenance(image))
                                }
                            }.accessibilityElement(children: .combine)
                            Spacer(); Button("Open") { model.open(file.url) }; Button("Reveal") { model.reveal(file.url) }
                        }.draggable(file.url)
                    }
                }
            }.frame(maxHeight: min(144, CGFloat(draft.completedArtifacts.reduce(0) { $0 + ($1.pdfEmbeddedImage == nil ? 28 : 64) })))
        }.padding(12).background(FileformTheme.elevated, in: RoundedRectangle(cornerRadius: 9))
    }
    private func savedImageSummary(_ image: PDFEmbeddedImageCandidate) -> String {
        let size = image.width.flatMap { width in image.height.map { "\(width) × \($0) · " } } ?? ""
        return size + (image.encodingOutcome == .preservedEncodedBytes ? "Original JPEG" : "Reconstructed PNG")
    }
    private func savedImageProvenance(_ image: PDFEmbeddedImageCandidate) -> String {
        let source = draft.sources[image.sourceID]?.input.lastPathComponent ?? "Source PDF"
        let pages = Array(Set(image.resourcePages.map { $0.pageIndex + 1 })).sorted().map(String.init).joined(separator: ", ")
        return "\(source) · image resources on pages \(pages)"
    }
    private var selectedIndex: Int? { draft.pages.firstIndex { $0.id == draft.selection } }
    private func change(_ name: String, _ action: (inout PDFPageDraft) -> Void) {
        guard let index = selectedIndex else { return }
        draft.edit(name, undo: undo) { action(&$0[index]) }
    }
    private struct SourceRun: Identifiable {
        let id: UUID
        let sourceID: String
        var indices: [Int]
    }
    private var sourceRuns: [SourceRun] {
        var runs: [SourceRun] = []
        for (index, page) in draft.pages.enumerated() {
            if runs.last?.sourceID == page.sourceID { runs[runs.count - 1].indices.append(index) }
            else { runs.append(SourceRun(id: page.id, sourceID: page.sourceID, indices: [index])) }
        }
        return runs
    }
    private func sourceHeading(_ run: SourceRun) -> some View {
        HStack(spacing: 8) {
            RoundedRectangle(cornerRadius: 2).fill(pdfSourceColor(run.sourceID, draft: draft)).frame(width: 8, height: 8)
            Text(draft.sources[run.sourceID]?.input.lastPathComponent ?? "Missing source")
                .font(.system(size: 13)).foregroundStyle(FileformTheme.ink2).lineLimit(1).truncationMode(.middle)
            Text("\(run.indices.count) \(run.indices.count == 1 ? "page" : "pages")").font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink4).fixedSize()
            Rectangle().fill(FileformTheme.border).frame(height: 1)
            Button("Select group") { draft.selectPages(Set(run.indices.map { draft.pages[$0].id })) }
                .buttonStyle(.plain).font(.system(size: 11)).foregroundStyle(FileformTheme.accent).fixedSize()
                .disabled(draft.isRunning)
                .accessibilityLabel("Select group of \(run.indices.count) pages from \(draft.sources[run.sourceID]?.input.lastPathComponent ?? "Missing source")")
        }
    }
    private func pageCard(_ page: PDFPageDraft, index: Int) -> some View {
        VStack(spacing: 6) {
                GeometryReader { geometry in
                    ZStack {
                        RoundedRectangle(cornerRadius: 4).fill(FileformTheme.field)
                        if let data = draft.previews[draft.previewKey(page)], let image = NSImage(data: data) {
                            let quarterTurn = page.rotation % 180 != 0
                            Image(nsImage: image).resizable().scaledToFit()
                                .frame(width: max(1, (quarterTurn ? geometry.size.height : geometry.size.width) - 14),
                                       height: max(1, (quarterTurn ? geometry.size.width : geometry.size.height) - 14))
                                .rotationEffect(.degrees(Double(page.rotation)))
                                .frame(width: geometry.size.width, height: geometry.size.height)
                        } else { Image(systemName: "doc").foregroundStyle(FileformTheme.ink4) }
                    }.clipShape(RoundedRectangle(cornerRadius: 4))
                        .overlay(alignment: .top) { Rectangle().fill(pdfSourceColor(page.sourceID, draft: draft)).frame(height: 3).clipShape(UnevenRoundedRectangle(topLeadingRadius: 4, topTrailingRadius: 4)) }
                        .overlay(RoundedRectangle(cornerRadius: 4).stroke(draft.selectedPageIDs.contains(page.id) ? FileformTheme.accent : FileformTheme.border, lineWidth: draft.selectedPageIDs.contains(page.id) ? 2 : 1))
                }.aspectRatio(0.77, contentMode: .fit)
                HStack(spacing: 4) {
                    Text("\(index + 1)").foregroundStyle(draft.selectedPageIDs.contains(page.id) ? FileformTheme.accent : FileformTheme.ink3)
                    if page.rotation != 0 { Text("\(page.rotation)°").foregroundStyle(FileformTheme.warning) }
                }.font(FileformTheme.mono(11))
                if page.splitAfter && index < draft.pages.count - 1 {
                    Label("Split here", systemImage: "scissors").font(.system(size: 12)).foregroundStyle(FileformTheme.accent)
                        .frame(maxWidth: .infinity).padding(.vertical, 4).background(FileformTheme.accentSoft, in: RoundedRectangle(cornerRadius: 4))
                }
        }.contentShape(Rectangle())
            .onTapGesture {
                let modifiers = NSApp.currentEvent?.modifierFlags ?? []
                draft.selectPage(page.id, extending: modifiers.contains(.shift), toggling: modifiers.contains(.command))
            }
            .focusable()
            .onKeyPress(keys: [.space, .return]) { key in
                guard key.modifiers.intersection([.command, .control, .option]).isEmpty else { return .ignored }
                draft.selectPage(page.id, extending: key.modifiers.contains(.shift))
                return .handled
            }
            .accessibilityElement(children: .ignore)
            .accessibilityAddTraits(.isButton)
            .accessibilityAction { draft.selectPage(page.id) }
            .disabled(draft.isRunning)
            .help("\(draft.sourceName(page)) · source page \(page.pageIndex + 1). Command-click to toggle; Shift-click to select a range. Drag selected pages before or after another page.")
            .accessibilityIdentifier("pdf-page-\(index + 1)")
            .accessibilityLabel("Output page \(index + 1), \(draft.sourceName(page)), source page \(page.pageIndex + 1), rotated \(page.rotation) degrees\(page.splitAfter && index < draft.pages.count - 1 ? ", split after this page" : "")")
            .accessibilityAddTraits(draft.selectedPageIDs.contains(page.id) ? .isSelected : [])
            .modifier(PDFPageDragModifier(draft: draft, pageID: page.id, undo: undo, frames: pageFrames,
                                          coordinateSpace: pageCoordinateSpace, pointer: $dragPointer, target: $dragTarget))
            .task(id: draft.previewKey(page)) { await draft.loadPreview(page) }
    }
}

@MainActor
func pdfSourceColor(_ sourceID: String, draft: PDFWorkspaceModel) -> Color {
    // Stable provenance colors even when page edits move a source ahead of another.
    let colors = [FileformTheme.accent, FileformTheme.success, FileformTheme.warning, FileformTheme.media]
    let hash = sourceID.utf8.reduce(UInt64(14695981039346656037)) { ($0 ^ UInt64($1)) &* 1099511628211 }
    return colors[Int(hash % UInt64(colors.count))]
}
