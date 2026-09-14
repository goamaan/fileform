import AppKit
import Foundation
import Observation
import FileformCore
import FileformDomain

struct PDFPageDraft: Identifiable, Equatable, Sendable, Codable {
    let id: UUID
    let sourceID: String
    let pageIndex: Int
    var rotation: Int
    var splitAfter: Bool
    init(sourceID: String, pageIndex: Int, rotation: Int = 0, splitAfter: Bool = false) {
        id = UUID(); self.sourceID = sourceID; self.pageIndex = pageIndex
        self.rotation = rotation; self.splitAfter = splitAfter
    }
    var reference: PageReference { .init(sourceID: sourceID, pageIndex: pageIndex, clockwiseRotation: rotation) }
}

struct PDFPageDragPayload: Codable, Equatable, Sendable {
    let workspaceID: UUID
    let dragID: UUID
    let pageIDs: [UUID]
}

enum PDFPageInsertionEdge: Equatable, Sendable { case before, after }

@MainActor @Observable
final class PDFWorkspaceModel {
    var pages: [PDFPageDraft] = []
    // Keep the persisted primary selection compatible with older workspace records.
    var selection: UUID? {
        didSet {
            if !updatingSelection {
                selectedPageIDs = selection.map { [$0] } ?? []
                selectionAnchor = selection
            }
            if pageOutput.isImageOutput && pageOutput.selectedOnly { refresh() }
        }
    }
    private(set) var selectedPageIDs = Set<UUID>()
    @ObservationIgnored private var updatingSelection = false
    private var selectionAnchor: UUID?
    @ObservationIgnored private let dragWorkspaceID = UUID()
    private(set) var activePageDrag: PDFPageDragPayload?
    @ObservationIgnored private var draggedPageSnapshot: [PDFPageDraft] = []
    var outputName = "Combined.pdf"
    var collisionPolicy: CollisionPolicy = .rename
    var exportAsDirectory = false
    var pageOutput = PDFPageOutputOptions() {
        didSet {
            guard oldValue != pageOutput else { return }
            refresh()
        }
    }
    private(set) var hasBeenOpened = false
    var plan: TransformationPlan?
    private(set) var isPlanning = false
    var error: String?
    var isRunning = false
    var phase: JobPhase?
    var result: TransformationResult?
    private(set) var completedArtifacts: [CommittedArtifact] = []
    var previews: [String: Data] = [:]
    private(set) var sources: [String: FileJob] = [:]
    private var destination: URL?
    private var revision = 0
    private weak var undoManager: UndoManager?
    private let engine: ConversionEngine
    private let previewService: NativePreviewService?
    private var runTask: Task<Void, Never>?
    private var planTask: Task<Void, Never>?
    @ObservationIgnored var onIdle: (() -> Void)?
    init(engine: ConversionEngine, previews: NativePreviewService? = nil) { self.engine = engine; self.previewService = previews }
    var canRun: Bool {
        !isRunning && !isPlanning && destination != nil && plan != nil && !pages.isEmpty
            && (!pageOutput.isExtraction || outputCount > 0)
    }
    var scopedPages: [PDFPageDraft] {
        pageOutput.isImageOutput && pageOutput.selectedOnly ? pages.filter { selectedPageIDs.contains($0.id) } : pages
    }
    var exportPages: [PDFPageDraft] {
        pageOutput.isExtraction ? scopedPages.filter { sources[$0.sourceID]?.inspection?.family != .image } : scopedPages
    }
    var excludedImagePageCount: Int { pageOutput.isExtraction ? scopedPages.count - exportPages.count : 0 }
    var extractionDetails: PDFImageExtractionDetails? { result?.pdfImageExtraction ?? plan?.pdfImageExtraction }
    var outputCount: Int {
        if pageOutput.isExtraction { return plan?.pdfImageExtraction?.supportedCount ?? 0 }
        return pageOutput.isRaster ? exportPages.count : max(1, 1 + pages.dropLast().filter(\.splitAfter).count)
    }
    var outputSummary: String {
        if pageOutput.isExtraction { return "\(outputCount) embedded image\(outputCount == 1 ? "" : "s")" }
        return pageOutput.isRaster ? "\(outputCount) \(pageOutput.format.title) image\(outputCount == 1 ? "" : "s")" : "\(outputCount) PDF\(outputCount == 1 ? "" : "s")"
    }
    var primaryAction: String {
        if pageOutput.isExtraction { return "Extract \(outputSummary)" }
        if pageOutput.isRaster { return "Export \(outputSummary)" }
        return outputCount == 1 && exportAsDirectory ? "Create PDF folder · \(pages.count) \(pages.count == 1 ? "page" : "pages")" : outputCount == 1 ? "Create PDF · \(pages.count) \(pages.count == 1 ? "page" : "pages")" : "Create \(outputCount) PDFs"
    }
    var orderedSourceJobs: [FileJob] {
        var seen = Set<String>()
        return pages.compactMap { seen.insert($0.sourceID).inserted ? sources[$0.sourceID] : nil }
    }
    var selectedPageCount: Int { selectedPageIDs.count }
    func chooseOutputFormat(_ format: OutputFormat) {
        guard !isRunning else { return }
        if ["Combined.pdf", "Page images", "Embedded images"].contains(outputName) {
            outputName = format == .pdf ? "Combined.pdf" : format == .images ? "Embedded images" : "Page images"
        }
        pageOutput.format = format
    }
    func selectPage(_ id: UUID, extending: Bool = false, toggling: Bool = false) {
        guard !isRunning, let index = pages.firstIndex(where: { $0.id == id }) else { return }
        if extending, let anchor = selectionAnchor,
           let anchorIndex = pages.firstIndex(where: { $0.id == anchor }) {
            let range = Set(pages[min(index, anchorIndex)...max(index, anchorIndex)].map(\.id))
            setSelection(toggling ? selectedPageIDs.union(range) : range, primary: id, anchor: anchor)
        } else if toggling {
            var ids = selectedPageIDs
            if !ids.insert(id).inserted { ids.remove(id) }
            setSelection(ids, primary: ids.contains(id) ? id : pages.first(where: { ids.contains($0.id) })?.id, anchor: id)
        } else { selection = id }
    }
    func selectPages(_ ids: Set<UUID>) {
        guard !isRunning else { return }
        let valid = ids.intersection(Set(pages.map(\.id)))
        let first = pages.first { valid.contains($0.id) }?.id
        setSelection(valid, primary: first, anchor: first)
    }
    func restorePageSelection(_ ids: Set<UUID>, primary: UUID?) {
        guard !isRunning else { return }
        let valid = ids.intersection(Set(pages.map(\.id)))
        let focus = primary.flatMap { valid.contains($0) ? $0 : nil }
            ?? pages.first(where: { valid.contains($0.id) })?.id
        setSelection(valid, primary: focus, anchor: focus)
    }
    func beginPageDrag(_ id: UUID) -> PDFPageDragPayload? {
        guard !isRunning, pages.contains(where: { $0.id == id }) else { return nil }
        if !selectedPageIDs.contains(id) { selectPage(id) }
        let payload = PDFPageDragPayload(workspaceID: dragWorkspaceID, dragID: UUID(),
                                         pageIDs: pages.filter { selectedPageIDs.contains($0.id) }.map(\.id))
        activePageDrag = payload
        draggedPageSnapshot = pages
        return payload
    }
    func isValidPageDrag(_ payload: PDFPageDragPayload) -> Bool {
        !isRunning && payload.workspaceID == dragWorkspaceID && activePageDrag == payload
            && !payload.pageIDs.isEmpty && pages == draggedPageSnapshot
            && Set(payload.pageIDs) == selectedPageIDs
            && Set(payload.pageIDs).count == payload.pageIDs.count
            && Set(payload.pageIDs).isSubset(of: Set(pages.map(\.id)))
    }
    func cancelPageDrag() {
        activePageDrag = nil; draggedPageSnapshot = []
    }
    @discardableResult
    func dropPages(_ payload: PDFPageDragPayload, at target: UUID, edge: PDFPageInsertionEdge, undo: UndoManager?) -> Bool {
        guard isValidPageDrag(payload), pages.contains(where: { $0.id == target }) else { return false }
        cancelPageDrag()
        let ids = Set(payload.pageIDs)
        // Dropping on any page in the dragged selection is deliberately a no-op.
        guard !ids.contains(target) else { return true }
        let moving = pages.filter { ids.contains($0.id) }
        var reordered = pages.filter { !ids.contains($0.id) }
        guard let targetIndex = reordered.firstIndex(where: { $0.id == target }) else { return false }
        reordered.insert(contentsOf: moving, at: targetIndex + (edge == .after ? 1 : 0))
        guard reordered != pages else { return true }
        edit("Move Pages", undo: undo) { $0 = reordered }
        return true
    }
    private func setSelection(_ ids: Set<UUID>, primary: UUID?, anchor: UUID?) {
        updatingSelection = true
        selectedPageIDs = ids; selection = primary; selectionAnchor = anchor
        updatingSelection = false
    }
    func canMoveSelection(_ offset: Int) -> Bool {
        guard !isRunning, !selectedPageIDs.isEmpty, (offset == -1 || offset == 1), pages.count > 1 else { return false }
        return pages.indices.contains { index in
            selectedPageIDs.contains(pages[index].id) && pages.indices.contains(index + offset)
                && !selectedPageIDs.contains(pages[index + offset].id)
        }
    }
    func moveSelection(_ offset: Int, undo: UndoManager?) {
        guard canMoveSelection(offset) else { return }
        let ids = selectedPageIDs
        edit("Move Pages", undo: undo) { pages in
            // Adjacent runs move one position without reversing either selected
            // or unselected pages, including selections spanning source groups.
            let indices = offset < 0 ? Array(pages.indices) : Array(pages.indices.reversed())
            for index in indices where ids.contains(pages[index].id) {
                let neighbor = index + offset
                if pages.indices.contains(neighbor), !ids.contains(pages[neighbor].id) { pages.swapAt(index, neighbor) }
            }
        }
    }
    func splitEvery(_ count: Int, undo: UndoManager?) throws {
        guard !isRunning else { return }
        let groups = try PDFPageSelection.groups(every: count, pageCount: pages.count)
        let boundaries = Set(groups.dropLast().compactMap(\.last))
        edit("Split Every \(count) Pages", undo: undo) { value in
            for index in value.indices { value[index].splitAfter = boundaries.contains(index) }
        }
    }
    func applyPageRanges(_ ranges: String, undo: UndoManager?) throws {
        guard !isRunning else { return }
        let groups = try PDFPageSelection.groups(ranges: ranges, pageCount: pages.count)
        let before = pages
        let arranged = groups.enumerated().flatMap { groupIndex, indexes in
            indexes.enumerated().map { index, sourceIndex in
                let source = before[sourceIndex]
                return PDFPageDraft(sourceID: source.sourceID, pageIndex: source.pageIndex, rotation: source.rotation,
                    splitAfter: groupIndex < groups.count - 1 && index == indexes.count - 1)
            }
        }
        edit("Apply Page Ranges", undo: undo) { $0 = arranged }
    }
    func rotateSelection(undo: UndoManager?) {
        let ids = selectedPageIDs
        edit("Rotate Pages", undo: undo) { pages in
            for index in pages.indices where ids.contains(pages[index].id) { pages[index].rotation = (pages[index].rotation + 90) % 360 }
        }
    }
    func removeSelection(undo: UndoManager?) {
        let ids = selectedPageIDs
        edit("Remove Pages", undo: undo) { $0.removeAll { ids.contains($0.id) } }
    }
    func duplicateSelection(undo: UndoManager?) {
        guard !isRunning, let last = pages.lastIndex(where: { selectedPageIDs.contains($0.id) }) else { return }
        let copies = pages.filter { selectedPageIDs.contains($0.id) }.map {
            PDFPageDraft(sourceID: $0.sourceID, pageIndex: $0.pageIndex, rotation: $0.rotation)
        }
        edit("Duplicate Pages", undo: undo) { $0.insert(contentsOf: copies, at: last + 1) }
        selectPages(Set(copies.map(\.id)))
    }
    @discardableResult
    func open(_ jobs: [FileJob], destination: URL?) -> Bool {
        guard !isRunning, !hasBeenOpened else { return false }
        include(jobs, destination: destination)
        return true
    }
    func include(_ jobs: [FileJob], destination: URL?) {
        guard !isRunning else { return }
        hasBeenOpened = true
        self.destination = destination
        var added = false
        for job in jobs {
            guard let info = job.inspection, job.state != .inspecting,
                  [.pdf, .image].contains(info.family), sources[job.id.uuidString] == nil else { continue }
            let id = job.id.uuidString
            sources[id] = job
            for page in 0..<(info.pageCount ?? 1) { pages.append(.init(sourceID: id, pageIndex: page)) }
            added = true
        }
        if selection == nil { selection = pages.first?.id }
        if added { refresh() }
    }
    func setDestination(_ destination: URL) { guard !isRunning else { return }; self.destination = destination; refresh() }
    func removeSource(_ id: String) {
        guard !isRunning else { return }
        undoManager?.removeAllActions(withTarget: self)
        sources.removeValue(forKey: id); pages.removeAll { $0.sourceID == id }; selection = pages.first?.id; refresh()
    }
    func sourceName(_ page: PDFPageDraft) -> String { sources[page.sourceID]?.input.lastPathComponent ?? "Missing source" }
    func previewKey(_ page: PDFPageDraft) -> String {
        guard let job = sources[page.sourceID] else { return "\(page.sourceID):\(page.pageIndex):missing" }
        let available = job.inspection != nil && ![.missing, .inspecting].contains(job.state)
        return "\(page.sourceID):\(page.pageIndex):\(job.input.absoluteString):\(available)"
    }
    func loadPreview(_ page: PDFPageDraft) async {
        let key = previewKey(page)
        guard previews[key] == nil, let job = sources[page.sourceID], job.inspection != nil,
              ![.missing, .inspecting].contains(job.state) else { return }
        guard let client = previewService else { return }
        if let png = try? await client.preview(job.input, maximumDimension: 240,
                                              pageIndex: job.inspection?.family == .pdf ? page.pageIndex : nil).png,
           previewKey(page) == key { previews[key] = png }
    }
    func edit(_ actionName: String, undo: UndoManager?, _ edit: (inout [PDFPageDraft]) -> Void) {
        guard !isRunning else { return }
        let before = pages
        let beforeIDs = selectedPageIDs, beforePrimary = selection, beforeAnchor = selectionAnchor
        edit(&pages)
        guard pages != before else { return }
        cancelPageDrag()
        undoManager = undo
        // Native window undo is synchronous on the main actor. Older SDKs do
        // not annotate that callback; defer-to-Task would break redo grouping.
        undo?.registerUndo(withTarget: self) { target in
            MainActor.assumeIsolated { target.restore(before, selected: beforeIDs, primary: beforePrimary, anchor: beforeAnchor, name: actionName) }
        }
        undo?.setActionName(actionName)
        let remaining = selectedPageIDs.intersection(Set(pages.map(\.id)))
        if remaining.isEmpty { selection = pages.first?.id }
        else {
            let primary = selection.flatMap { remaining.contains($0) ? $0 : nil }
                ?? pages.first(where: { remaining.contains($0.id) })?.id
            setSelection(remaining, primary: primary, anchor: selectionAnchor)
        }
        refresh()
    }
    private func restore(_ value: [PDFPageDraft], selected: Set<UUID>, primary: UUID?, anchor: UUID?, name: String) {
        guard !isRunning else { return }
        let before = pages
        let beforeIDs = selectedPageIDs, beforePrimary = selection, beforeAnchor = selectionAnchor
        undoManager?.registerUndo(withTarget: self) { target in
            MainActor.assumeIsolated { target.restore(before, selected: beforeIDs, primary: beforePrimary, anchor: beforeAnchor, name: name) }
        }
        undoManager?.setActionName(name)
        cancelPageDrag()
        pages = value; setSelection(selected, primary: primary, anchor: anchor); refresh()
    }
    func refresh() {
        guard !isRunning else { return }
        revision += 1; let version = revision
        planTask?.cancel(); plan = nil; isPlanning = false; error = nil; result = nil
        guard !pages.isEmpty else { return }
        let chosenPages = exportPages
        guard !chosenPages.isEmpty else {
            error = pageOutput.isExtraction ? "Choose at least one PDF page to search for embedded images. Standalone images are already image files." : "Select at least one page, or choose All pages."
            return
        }
        guard chosenPages.allSatisfy({ page in
            guard let job = sources[page.sourceID] else { return false }
            return job.inspection != nil && job.state != .missing && job.state != .inspecting
        }) else { error = "A required source is unavailable or is still being inspected. Locate it before exporting."; return }
        do {
            let sourceIDs = Set(chosenPages.map(\.sourceID))
            let assets = orderedSourceJobs.filter { sourceIDs.contains($0.id.uuidString) }.map { AssetReference(id: $0.id.uuidString, url: $0.input) }
            guard let folder = destination ?? orderedSourceJobs.first?.input.deletingLastPathComponent() else { return }
            var groups: [[PageReference]] = [[]]
            for (index, page) in pages.enumerated() {
                groups[groups.count - 1].append(page.reference)
                if page.splitAfter && index < pages.count - 1 { groups.append([]) }
            }
            let split = pageOutput.isImageOutput || groups.count > 1 || exportAsDirectory
            let output = try Self.outputURL(name: outputName, folder: folder, split: split, pageImages: pageOutput.isImageOutput)
            let operation: TransformationOperation
            if pageOutput.isExtraction {
                operation = .pdfExtractImages(pages: chosenPages.map { .init(sourceID: $0.sourceID, pageIndex: $0.pageIndex) })
            } else if pageOutput.isRaster {
                guard let dpi = Int(pageOutput.dpi), (36...600).contains(dpi) else {
                    throw FileformError(.invalidRequest, "Enter a whole-number resolution from 36 to 600 DPI.")
                }
                operation = .pdfRasterize(pages: chosenPages.map(\.reference), dpi: dpi, quality: pageOutput.quality)
            } else { operation = split ? .pdfSplit(groups: groups) : .pdfComposition(pages: groups[0]) }
            let request = try TransformationRequest(assets: assets,
                operation: operation,
                output: .init(destination: output, format: pageOutput.format, cardinality: split ? .directory : .file), collisionPolicy: collisionPolicy)
            isPlanning = true
            planTask = Task {
                defer { if version == revision { isPlanning = false } }
                do {
                    let value = try await engine.plan(request)
                    guard !Task.isCancelled, version == revision, !isRunning else { return }
                    plan = value
                } catch {
                    guard !Task.isCancelled, version == revision, !isRunning else { return }
                    self.error = error.localizedDescription
                }
            }
        } catch { self.error = error.localizedDescription }
    }
    func run() {
        guard canRun, let approved = plan else { return }
        cancelPageDrag()
        isRunning = true; error = nil; result = nil
        runTask = Task {
            do {
                let value = try await engine.run(approved) { event in Task { @MainActor in self.phase = event.phase } }
                result = value
                completedArtifacts.append(contentsOf: value.artifacts)
            } catch is CancellationError { error = "Stopped. Originals and completed files are unchanged." }
            catch { self.error = error.localizedDescription }
            phase = nil; isRunning = false; runTask = nil; onIdle?()
        }
    }
    func restoreSession(pages: [PDFPageDraft], selection: UUID?, sources: [FileJob], outputName: String, collisionPolicy: CollisionPolicy = .rename, exportAsDirectory: Bool = false, hasBeenOpened: Bool? = nil,
                        pageOutput: PDFPageOutputOptions = .init(),
                        artifacts: [CommittedArtifact], interrupted: Bool, destination: URL?) {
        guard !isRunning else { return }
        cancelPageDrag()
        self.pages = pages; self.selection = selection; self.outputName = outputName; self.collisionPolicy = collisionPolicy; self.exportAsDirectory = exportAsDirectory
        self.pageOutput = pageOutput
        self.sources = Dictionary(uniqueKeysWithValues: sources.map { ($0.id.uuidString, $0) })
        self.destination = destination
        self.hasBeenOpened = hasBeenOpened ?? (!pages.isEmpty || !sources.isEmpty || !artifacts.isEmpty)
        completedArtifacts = artifacts
        refresh()
        if interrupted { error = "The previous PDF export was interrupted. Review the pages and create the output again when ready." }
    }
    static func outputURL(name: String, folder: URL, split: Bool, pageImages: Bool = false) throws -> URL {
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, name != ".", name != "..", name.utf8.count <= 240,
              !name.contains("/"), !name.contains(":"), !name.contains("\\"),
              !name.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains) else {
            throw FileformError(.invalidRequest, "Use a filename without separators or control characters.")
        }
        if pageImages { return folder.appendingPathComponent(name, isDirectory: true) }
        let base = (name as NSString).deletingPathExtension
        return folder.appendingPathComponent(split ? base + " pages" : (name.lowercased().hasSuffix(".pdf") ? name : name + ".pdf"))
    }
    func applySetupDraft(pages: [PDFPageDraft], sources: [FileJob], outputName: String, destination: URL?, plan: TransformationPlan) {
        guard !isRunning else { return }
        invalidatePlan(); undoManager?.removeAllActions(withTarget: self)
        hasBeenOpened = true
        cancelPageDrag()
        self.pages = pages; self.selection = pages.first?.id; self.outputName = outputName
        self.sources = Dictionary(uniqueKeysWithValues: sources.map { ($0.id.uuidString, $0) })
        self.destination = destination; collisionPolicy = plan.request.collisionPolicy
        exportAsDirectory = plan.request.output.cardinality == .directory
        if case .pdfRasterize(_, let dpi, let quality) = plan.request.operation {
            pageOutput = .init(format: plan.request.output.format, dpi: String(dpi), quality: quality, selectedOnly: false)
        } else if case .pdfExtractImages = plan.request.operation {
            pageOutput = .init(format: .images)
        } else { pageOutput = .init() }
        // Setting output options may have scheduled a fresh plan. The reviewed
        // setup already has an approved plan; cancel that transient task first.
        invalidatePlan()
        self.plan = plan; error = nil; result = nil
    }
    func invalidatePlan() { planTask?.cancel(); revision += 1; plan = nil; isPlanning = false }
    func clear() {
        guard !isRunning else { return }
        planTask?.cancel(); revision += 1; isPlanning = false
        cancelPageDrag()
        pages = []; selection = nil; sources = [:]; previews = [:]; completedArtifacts = []
        plan = nil; error = nil; result = nil; outputName = "Combined.pdf"; collisionPolicy = .rename; exportAsDirectory = false; hasBeenOpened = false
        pageOutput = .init()
        undoManager?.removeAllActions(withTarget: self)
    }
    func cancel() { runTask?.cancel() }
}
