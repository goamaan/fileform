import AppKit
import Foundation
import Observation
import FileformCore
import FileformDomain

enum FileJobState: String, Codable {
    case inspecting, ready, queued, running, completed, unchanged, failed, cancelled, interrupted, missing
    var title: String {
        switch self {
        case .inspecting: "Inspecting"; case .ready: "Ready"; case .queued: "Queued"; case .running: "Converting"
        case .interrupted: "Interrupted — review and retry"; case .missing: "Source unavailable — locate file"
        case .completed: "Done"; case .unchanged: "Original retained"; case .failed: "Needs attention"; case .cancelled: "Cancelled"
        }
    }
}

@MainActor @Observable
final class FileJob: Identifiable {
    let id: UUID
    var input: URL
    var hasInputScope: Bool
    var inspection: Inspection?
    var capabilities: [Capability] = []
    var state: FileJobState = .inspecting
    var phase: JobPhase?
    var outputName = ""
    var approvedPlanKey: String?
    var approvedPlanFolder: URL?
    var format: OutputFormat = .jpeg
    var goal: ConversionGoal = .convert
    var collisionPolicy: CollisionPolicy = .rename
    var quality = 0.82
    var minimumQuality = 0.35
    var sizeLimitMB = "10"
    var resize = false
    var longestEdge = "1920"
    var background = "preserve"
    var minimumVideoBitrate = "150000"
    var pdfPage = ""
    var pdfCompression = PDFCompressionDraft()
    var crop = ImageCropDraft()
    var origin: ResultOrigin?
    var trimDraft: MediaTrimDraft?
    var trimPlan: TransformationPlan?
    var trimError: String?
    var mediaTimeline: MediaTimeline?
    var lastTrimDetails: MediaTrimDetails?
    var lastPDFOptimizationDetails: PDFOptimizationDetails?
    var trimEditable: Bool { inspection?.family == .media && ![.inspecting, .missing, .running, .queued].contains(state) }
    var plan: TransformationPlan?
    var error: String?
    var result: VerifiedResult?
    var originalPreview: Data?
    var resultPreview: Data?
    var outputs: [URL] = []
    var cancelRequested = false
    init(input: URL, id: UUID = UUID(), acquireAccess: Bool = true) {
        self.id = id; self.input = input
        hasInputScope = acquireAccess && input.startAccessingSecurityScopedResource()
    }
    var editable: Bool { [.ready, .failed, .cancelled, .unchanged, .interrupted].contains(state) }
    var isTerminal: Bool { [.completed, .unchanged, .failed, .cancelled].contains(state) }
    var planKey: String {
        [outputName, format.rawValue, goal.rawValue, String(quality), String(minimumQuality), sizeLimitMB,
         String(resize), longestEdge, background, minimumVideoBitrate, pdfPage, pdfCompression.key, crop.key, collisionPolicy.rawValue].joined(separator: "|")
    }
    var availableGoals: [ConversionGoal] {
        let supported = capabilities.filter { $0.format == format }.flatMap(\.goals)
        return [ConversionGoal.convert, .compress, .fit].filter { supported.contains($0) }
    }
    var availableFormats: [OutputFormat] {
        var seen = Set<OutputFormat>()
        return capabilities.map(\.format).filter { seen.insert($0).inserted }
    }
    var optimizesPDF: Bool { inspection?.family == .pdf && format == .pdf && goal != .convert }
    var supportsResize: Bool {
        (inspection?.family == .image && format != .txt) ||
        (inspection?.family == .pdf && format.family == .image) ||
        (inspection?.family == .media && [.mp4, .mov].contains(format))
    }
    var automaticOutputName: String {
        var stem = input.deletingPathExtension().lastPathComponent
        while stem.utf8.count > 220 { stem.removeLast() }
        return stem + "-converted"
    }
    var outputTitle: String { OutputDescription.title(format) }
    var statusText: String {
        guard state == .running, let phase else { return state.title }
        return switch phase {
        case .inspecting: "Inspecting"; case .preparing: "Preparing"; case .encoding: format == .txt ? "Extracting text" : "Converting"
        case .verifying: "Checking result"; case .saving: "Saving"
        }
    }
}

@MainActor @Observable
final class WorkspaceModel {
    var jobs: [FileJob] = []
    var workspaceTask: WorkspaceTask = .convert
    let pdfWorkspace: PDFWorkspaceModel
    let linkWorkspace: LinkWorkspaceModel
    let setups = SetupLibrary()
    var presentedSheet: WorkspaceSheet?
    var selection: UUID? {
        didSet { if !updatingBatchSelection { batchSelection = selection.map { [$0] } ?? [] } }
    }
    var batchSelection = Set<UUID>()
    var runSelectionOnly = false
    @ObservationIgnored private var updatingBatchSelection = false
    let preferences: WorkspacePreferences
    var pendingImportGoal: ConversionGoal?
    var outputFolder: URL?
    private var batchRunning = false
    var isRunning: Bool { batchRunning || pdfWorkspace.isRunning || linkWorkspace.isBusy }
    var alert: String?
    let engine: ConversionEngine
    let mediaPreviews: MediaPreviewService?
    private let nativeWorker: NativePreviewService?
    private var outputScope = false
    private var batchTask: Task<Void, Never>?
    private var activeTask: Task<TransformationResult, Error>?
    private var activeID: UUID?
    private var persistence: WorkspaceStore?
    private var persistenceStarted = false
    private var persistenceEnabled = false
    private var storedSources: [UUID: WorkspaceFileReference] = [:]
    private var storedOutputs: [URL: WorkspaceFileReference] = [:]
    private var pendingPDFImports = Set<UUID>()
    @ObservationIgnored private var reusableOpenPanel: NSOpenPanel?
    @ObservationIgnored private var reusableSavePanel: NSSavePanel?
    var persistenceNotice: String?
    var restoringSession = false

    init(engine: ConversionEngine? = nil, preferences: WorkspacePreferences? = nil) {
        let preferences = preferences ?? WorkspacePreferences()
        self.preferences = preferences
        let bundled = Bundle.main.resourceURL?.appendingPathComponent("MediaPack")
        let pack = bundled.flatMap { FileManager.default.fileExists(atPath: $0.appendingPathComponent("manifest.json").path) ? $0 : nil }
        let bundledPDF = Bundle.main.resourceURL?.appendingPathComponent("PDFPack")
        let pdfPack = bundledPDF.flatMap { FileManager.default.fileExists(atPath: $0.appendingPathComponent("manifest.json").path) ? $0 : nil }
        let worker = Bundle.main.bundleURL.appendingPathComponent("Contents/Helpers/fileform-worker")
        let executor = engine ?? ConversionEngine(mediaPack: pack, pdfPack: pdfPack, workerExecutable: worker)
        self.engine = executor
        self.mediaPreviews = pack.map { MediaPreviewService(mediaPack: $0) }
        let previews = FileManager.default.isExecutableFile(atPath: worker.path) ? NativePreviewService(executable: worker) : nil
        self.nativeWorker = previews
        self.pdfWorkspace = PDFWorkspaceModel(engine: executor, previews: previews)
        self.linkWorkspace = LinkWorkspaceModel(engine: executor)
        self.linkWorkspace.networkAllowed = preferences.allowLinks
        preferences.networkAccessChanged = { [weak self] allowed in self?.linkWorkspace.networkAllowed = allowed }
        preferences.destinationRetentionChanged = { [weak self] in self?.saveSession() }
        self.linkWorkspace.onSaved = { [weak self] saved, open in
            guard let self else { return }
            if open { self.openLinkResultInTrim(saved) }
            self.saveSession()
        }
        self.pdfWorkspace.onIdle = { [weak self] in self?.flushPendingPDFImports() }
    }
    var selectedJob: FileJob? { jobs.first { $0.id == selection } }
    var conversionRunJobs: [FileJob] {
        jobs.filter { $0.editable && $0.plan != nil && $0.approvedPlanKey == $0.planKey && $0.approvedPlanFolder == outputFolder && $0.plan?.request.assets.first?.url.standardizedFileURL == $0.input.standardizedFileURL && (!runSelectionOnly || batchSelection.contains($0.id)) }
    }
    var readyCount: Int { conversionRunJobs.count }
    func selectBatch(_ values: Set<UUID>) {
        let valid = values.intersection(jobs.map(\.id))
        let added = valid.subtracting(batchSelection)
        updatingBatchSelection = true
        selection = jobs.last(where: { added.contains($0.id) })?.id
            ?? (selection.flatMap { valid.contains($0) ? $0 : nil })
            ?? jobs.first(where: { valid.contains($0.id) })?.id
        batchSelection = valid
        updatingBatchSelection = false
        if valid.count > 1 { runSelectionOnly = true }
    }
    func reviewBatchChoices() {
        guard !isRunning, !restoringSession, batchSelection.count > 1, let source = selectedJob else { return }
        presentedSheet = .batch(source.id, jobs.filter { batchSelection.contains($0.id) }.map(\.id))
    }

    var completedCount: Int { jobs.reduce(0) { $0 + $1.outputs.count } + pdfWorkspace.completedArtifacts.count + linkWorkspace.history.count }
    var canRun: Bool {
        guard !(presentedSheet?.blocksExecution ?? false), !restoringSession else { return false }
        if workspaceTask == .link { return !batchRunning && !pdfWorkspace.isRunning && linkWorkspace.canRun }
        if workspaceTask == .pages { return !linkWorkspace.isBusy && pdfWorkspace.canRun && pendingPDFImports.isEmpty }
        return !isRunning && outputFolder != nil && (workspaceTask == .trim ? selectedJob?.trimEditable == true && selectedJob?.trimPlan != nil : readyCount > 0)
    }
    var primaryAction: String {
        if workspaceTask == .link { return "Save verified file" }
        if workspaceTask == .pages { return pdfWorkspace.primaryAction }
        if workspaceTask == .trim {
            if let interval = selectedJob?.trimPlan?.mediaTrim?.realized ?? (try? selectedJob?.trimDraft?.interval()) { return "Export \(TrimTimeText.duration(interval)) trim" }
            return "Export trim"
        }
        if readyCount > 1 { return "Create \(readyCount) files" }
        guard let job = conversionRunJobs.first else { return "Create file" }
        if job.goal == .compress { return "Make smaller" }
        if job.goal == .fit { return "Create \(job.format.title) under \(job.sizeLimitMB) MB" }
        if job.format == .txt { return "Extract text" }
        if job.inspection?.videoCodec != nil && [.m4a, .wav, .flac, .mp3].contains(job.format) { return "Extract audio" }
        if job.inspection?.family == .pdf && job.format == .pdf { return "Export page" }
        return "Create \(job.format.title)"
    }

    func chooseTask(_ task: WorkspaceTask) {
        guard !restoringSession else { return }
        workspaceTask = task
        pendingImportGoal = nil
        if task == .trim, selectedJob?.inspection?.family != .media { selection = jobs.first { $0.inspection?.family == .media }?.id }
        if task == .text, let job = selectedJob, job.editable,
           job.capabilities.contains(where: { $0.format == .txt }) {
            job.format = .txt; job.goal = .convert
            Task { await refreshPlan(job) }
        }
        if task == .pages, pdfWorkspace.open(jobs, destination: outputFolder) {
            pendingPDFImports.formUnion(jobs.filter { $0.inspection == nil || $0.state == .inspecting }.map(\.id))
        }
    }

    func startConversion(goal: ConversionGoal) {
        guard !restoringSession, !isRunning else { return }
        chooseTask(.convert)
        if let job = selectedJob, job.editable, job.inspection != nil {
            applyConversionGoal(goal, to: job)
            Task { await refreshPlan(job) }
        } else {
            pendingImportGoal = goal
        }
    }

    private func applyConversionGoal(_ goal: ConversionGoal, to job: FileJob) {
        let choices = job.capabilities.filter { $0.goals.contains(goal) }
        guard let capability = choices.first(where: { $0.format == job.format })
            ?? choices.first(where: { $0.format.fileExtension == job.input.pathExtension.lowercased() })
            ?? choices.first else {
            alert = "\(job.input.lastPathComponent) has no available output for \(goal == .fit ? "an upload limit" : "making it smaller"). Choose one of its available formats instead."
            return
        }
        job.format = capability.format
        job.goal = goal
    }

    func pasteLink() {
        guard !restoringSession, !linkWorkspace.isBusy else { return }
        chooseTask(.link)
        if let text = NSPasteboard.general.string(forType: .string) {
            linkWorkspace.urlText = text.trimmingCharacters(in: .whitespacesAndNewlines)
        }
    }

    func chooseFiles() {
        guard !restoringSession else { return }
        guard let panel = makeOpenPanel() else { return }
        panel.title = "Add files to Fileform"; panel.canChooseFiles = true; panel.canChooseDirectories = false
        panel.allowsMultipleSelection = true
        present(panel) { [weak self] response in
            if response == .OK { self?.addFiles(panel.urls) }
        }
    }

    func chooseDestination() {
        guard !restoringSession, !isRunning else { return }
        guard let panel = makeOpenPanel() else { return }
        panel.title = "Choose where Fileform saves your results"; panel.prompt = "Use This Folder"
        panel.canChooseFiles = false; panel.canChooseDirectories = true; panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false
        let completion: (NSApplication.ModalResponse) -> Void = { [weak self] response in
            guard response == .OK, let folder = panel.url, let self else { return }
            self.setDestination(folder)
        }
        present(panel, completion: completion)
    }

    // Reuse the native panels: on macOS 26.2, repeated fresh sandboxed open
    // panels can leave Open disabled even for valid selections. Reset filters
    // for each purpose so a setup import cannot constrain later file imports.
    func makeOpenPanel() -> NSOpenPanel? {
        let panel = reusableOpenPanel ?? NSOpenPanel()
        guard !panel.isVisible else { return nil }
        reusableOpenPanel = panel
        panel.title = "Open"; panel.prompt = "Open"
        panel.canChooseFiles = true; panel.canChooseDirectories = false
        panel.canCreateDirectories = false; panel.allowsMultipleSelection = false
        panel.allowedContentTypes = []; panel.allowsOtherFileTypes = true
        return panel
    }

    func makeSavePanel() -> NSSavePanel? {
        let panel = reusableSavePanel ?? NSSavePanel()
        guard !panel.isVisible else { return nil }
        reusableSavePanel = panel
        return panel
    }

    func present(_ panel: NSSavePanel, completion: @escaping (NSApplication.ModalResponse) -> Void) {
        let owner = NSApp.keyWindow ?? NSApp.mainWindow ?? NSApp.windows.first { $0.isVisible && !($0 is NSPanel) }
        if let owner { panel.beginSheetModal(for: owner, completionHandler: completion) }
        else { panel.begin(completionHandler: completion) }
    }

    func setDestination(_ folder: URL) {
        guard !isRunning else { return }
        if outputScope { outputFolder?.stopAccessingSecurityScopedResource() }
        outputScope = folder.startAccessingSecurityScopedResource()
        outputFolder = folder
        pdfWorkspace.setDestination(folder)
        linkWorkspace.destination = folder
        for job in jobs {
            job.trimPlan = nil
            Task { await self.refreshPlan(job); await self.refreshTrimPlan(job) }
        }
    }

    func addFiles(_ urls: [URL]) {
        let importGoal = pendingImportGoal
        if urls.contains(where: \.isFileURL) { pendingImportGoal = nil }
        for url in urls where url.isFileURL {
            let url = url.standardizedFileURL
            let addToPDF = workspaceTask == .pages && !(presentedSheet?.blocksExecution ?? false)
            if let existing = jobs.first(where: { $0.input == url }) {
                if addToPDF {
                    if existing.inspection != nil && existing.state != .inspecting { pdfWorkspace.include([existing], destination: outputFolder) }
                    else { pendingPDFImports.insert(existing.id) }
                }
                continue
            }
            let job = FileJob(input: url)
            job.collisionPolicy = preferences.defaultCollision
            if addToPDF { pendingPDFImports.insert(job.id) }
            jobs.append(job)
            if selection == nil { selection = job.id }
            let task = workspaceTask
            Task { await inspect(job, task: task, importGoal: importGoal) }
        }
    }

    private func inspect(_ job: FileJob, task: WorkspaceTask, importGoal: ConversionGoal? = nil, savedDraft: FileJobDraft? = nil, savedState: FileJobState? = nil) async {
        do {
            let info = try await engine.inspect(job.input)
            guard jobs.contains(where: { $0 === job }) else { return }
            job.inspection = info
            if !restoringSession, workspaceTask == .trim, info.family == .media, selectedJob?.inspection?.family != .media { selection = job.id }
            job.capabilities = await engine.capabilities(for: info).filter(\.available)
            if let savedDraft { savedDraft.apply(to: job) }
            else {
                switch info.family {
                case .image: job.format = info.hasAlpha == true ? .png : .jpeg
                case .media: job.format = info.videoCodec == nil ? .m4a : .mp4
                case .pdf: job.format = .txt
                case .table: job.format = info.detectedType == "json" ? .csv : .json
                case .text: job.format = .txt
                }
                if task == .text, job.capabilities.contains(where: { $0.format == .txt }) { job.format = .txt }
                if info.family == .image {
                    let sideways = (5...8).contains(info.orientation ?? 1)
                    job.crop.width = String((sideways ? info.height : info.width) ?? 1)
                    job.crop.height = String((sideways ? info.width : info.height) ?? 1)
                }
            }
            if savedDraft == nil, let importGoal { applyConversionGoal(importGoal, to: job) }
            job.state = savedState ?? .ready
            flushPendingPDFImports()
            await refreshPlan(job)
            if [.image, .pdf].contains(info.family), let nativeWorker {
                job.originalPreview = try? await nativeWorker.preview(job.input, maximumDimension: 1024).png
            } else if info.family == .image { job.originalPreview = try? await engine.thumbnail(for: job.input) }
            if job.state == .completed, let output = job.outputs.last, job.format.family == .image, let nativeWorker {
                job.resultPreview = try? await nativeWorker.preview(output).png
            }
        } catch {
            job.state = .failed; job.error = error.localizedDescription
        }
    }

    func request(for job: FileJob) throws -> ConversionRequest {
        let maximum: Int64?
        if job.goal == .fit {
            let enteredSize = job.sizeLimitMB.trimmingCharacters(in: .whitespacesAndNewlines)
            guard enteredSize.range(of: #"^(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)$"#, options: .regularExpression) != nil,
                  let value = Decimal(string: enteredSize, locale: Locale(identifier: "en_US_POSIX")), value > 0, value <= 1_000_000 else {
                throw FileformError(.invalidRequest, "Enter a positive size in MB, using a decimal point if needed.")
            }
            let bytes = value * 1_000_000
            let integer = NSDecimalNumber(decimal: bytes).int64Value
            guard Decimal(integer) == bytes, integer > 0 else {
                throw FileformError(.invalidRequest, "The size limit must resolve to a positive whole number of bytes.")
            }
            maximum = integer
        } else { maximum = nil }
        let dimension: Int?
        if job.resize && job.supportsResize {
            guard let value = Int(job.longestEdge), value > 0 else { throw FileformError(.invalidRequest, "Enter a positive longest edge in pixels.") }
            dimension = value
        } else { dimension = nil }
        guard let minimumVideoBitrate = Int(job.minimumVideoBitrate) else { throw FileformError(.invalidRequest, "Enter a whole-number minimum video bitrate.") }
        let page: Int?
        if job.inspection?.family == .pdf && !job.optimizesPDF && !job.pdfPage.isEmpty {
            guard let value = Int(job.pdfPage), value > 0 else { throw FileformError(.invalidRequest, "Enter a positive PDF page number.") }
            page = value
        } else { page = nil }
        let folder = outputFolder ?? job.input.deletingLastPathComponent()
        let entered = job.outputName.trimmingCharacters(in: .whitespacesAndNewlines)
        let name = entered.isEmpty ? job.automaticOutputName : entered
        guard name.utf8.count <= 240, name != ".", name != "..", !name.contains("/"), !name.contains(":"),
              !name.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains) else {
            throw FileformError(.invalidRequest, "Enter a filename without path separators or control characters.")
        }
        let suffix = "." + job.format.fileExtension
        let filename = name.lowercased().hasSuffix(suffix) ? name : name + suffix
        return .init(input: job.input,
                     destination: folder.appendingPathComponent(filename),
                     format: job.format, goal: job.goal,
                     options: .init(quality: job.quality, minimumQuality: job.minimumQuality,
                                    maxDimension: dimension, maximumBytes: maximum,
                                    background: job.format.family == .image && !job.format.supportsAlpha ? AlphaBackground(rawValue: job.background) : nil,
                                    minimumVideoBitrate: minimumVideoBitrate, pageNumber: page),
                     collisionPolicy: job.collisionPolicy)
    }

    func transformationRequest(for job: FileJob) throws -> TransformationRequest {
        let legacy = try request(for: job)
        if job.optimizesPDF {
            if job.pdfCompression.compressImages {
                let draft = job.pdfCompression
                let dimension: Int?
                if draft.resizeImages {
                    guard let pixels = Int(draft.longestEdge), (1...16384).contains(pixels) else {
                        throw FileformError(.invalidRequest, "Enter a maximum embedded image edge from 1 to 16384 pixels.")
                    }
                    dimension = pixels
                } else { dimension = nil }
                return try .init(assets: [.init(id: "source", url: job.input)],
                    operation: .pdfOptimize(parameters: .init(goal: job.goal, quality: draft.quality,
                        minimumQuality: draft.minimumQuality, maximumImageDimension: dimension, maximumBytes: legacy.options.maximumBytes)),
                    output: .init(destination: legacy.destination, format: .pdf), fidelity: .allowDeclaredLosses,
                    collisionPolicy: legacy.collisionPolicy)
            }
            return try .init(assets: [.init(id: "source", url: job.input)],
                operation: .conversion(.init(goal: job.goal, options: legacy.options, color: .preserve, metadata: .preserve)),
                output: .init(destination: legacy.destination, format: .pdf), fidelity: .requireLossless,
                collisionPolicy: legacy.collisionPolicy)
        }
        guard job.crop.enabled, job.inspection?.family == .image, job.format.family == .image else { return try .init(legacy: legacy) }
        guard let x = Int(job.crop.x), let y = Int(job.crop.y), let width = Int(job.crop.width), let height = Int(job.crop.height) else {
            throw FileformError(.invalidRequest, "Crop coordinates and dimensions must be whole pixels.")
        }
        return try .init(assets: [.init(id: "source", url: job.input)],
                         operation: .imageCrop(rectangle: .init(x: x, y: y, width: width, height: height),
                                               conversion: .init(goal: job.goal, options: legacy.options)),
                         output: .init(destination: legacy.destination, format: job.format), collisionPolicy: legacy.collisionPolicy)
    }

    func ensurePlan(_ job: FileJob) async {
        if job.plan != nil, job.approvedPlanKey == job.planKey, job.approvedPlanFolder == outputFolder,
           job.plan?.request.assets.first?.url.standardizedFileURL == job.input.standardizedFileURL { return }
        await refreshPlan(job)
    }

    func refreshPlan(_ job: FileJob) async {
        guard job.editable, job.inspection != nil else { return }
        let key = job.planKey; let folder = outputFolder; let input = job.input
        job.plan = nil; job.error = nil
        do {
            let plan = try await engine.plan(transformationRequest(for: job))
            guard key == job.planKey, folder == outputFolder, input == job.input, job.editable else { return }
            job.plan = plan; job.approvedPlanKey = key; job.approvedPlanFolder = folder; job.error = nil
            if ![.interrupted, .cancelled].contains(job.state) { job.state = .ready }
        } catch is CancellationError { return }
        catch {
            guard key == job.planKey, folder == outputFolder, input == job.input, job.editable else { return }
            job.error = error.localizedDescription; job.state = .failed
        }
    }

    func runReadyJobs() {
        guard canRun else { return }
        if workspaceTask == .link { linkWorkspace.run(); saveSession(); return }
        if workspaceTask == .pages { pdfWorkspace.run(); saveSession(); return }
        let trimming = workspaceTask == .trim
        let batch = trimming ? selectedJob.map { [$0] } ?? [] : conversionRunJobs
        let approvedPlans = Dictionary(uniqueKeysWithValues: batch.compactMap { job in (trimming ? job.trimPlan : job.plan).map { (job.id, $0) } })
        batchRunning = true
        for job in batch { job.state = .queued }
        saveSession()
        batchTask = Task {
            for job in batch {
                if Task.isCancelled || job.state == .cancelled { job.state = .cancelled; continue }
                job.state = .running; job.error = nil; job.cancelRequested = false; activeID = job.id
                job.lastPDFOptimizationDetails = nil
                if trimming { job.trimError = nil }
                do {
                    guard let plan = approvedPlans[job.id] else { throw FileformError(.invalidRequest, "The approved plan is missing.") }
                    if job.cancelRequested { throw CancellationError() }
                    let task = Task { try await engine.run(plan) { event in
                        Task { @MainActor in if job.state == .running { job.phase = event.phase } }
                    } }
                    activeTask = task
                    if Task.isCancelled { task.cancel() }
                    let result = try await task.value
                    let first = result.artifacts.first
                    job.result = .init(status: result.status, input: job.input, output: first?.url,
                                       inputBytes: plan.inputs.first?.inspection.identity.bytes ?? 0, outputBytes: first?.bytes,
                                       format: plan.request.output.format, warnings: result.warnings, attempts: result.attempts)
                    job.lastTrimDetails = result.mediaTrim
                    job.lastPDFOptimizationDetails = result.pdfOptimization
                    job.outputs.append(contentsOf: result.artifacts.map(\.url))
                    job.state = result.status == .succeeded ? .completed : .unchanged
                    if let output = result.artifacts.first?.url, plan.request.output.format.family == .image {
                        if let nativeWorker { job.resultPreview = try? await nativeWorker.preview(output).png }
                        else { job.resultPreview = try? await engine.thumbnail(for: output) }
                    }
                } catch is CancellationError { job.state = .cancelled }
                catch {
                    job.state = .failed
                    job.lastPDFOptimizationDetails = (error as? FileformError)?.pdfOptimization
                    if trimming { job.trimError = error.localizedDescription }
                    else { job.error = error.localizedDescription }
                }
                activeTask = nil; activeID = nil; job.phase = nil
            }
            batchRunning = false; batchTask = nil
        }
    }

    func canTrimResult(_ output: URL, from job: FileJob) -> Bool {
        job.outputs.contains(output) && OutputFormat.allCases.contains {
            $0.family == .media && $0.fileExtension == output.pathExtension.lowercased()
        }
    }

    func trimResult(_ output: URL, from parent: FileJob) {
        guard !isRunning, !restoringSession, jobs.contains(where: { $0 === parent }),
              canTrimResult(output, from: parent) else { return }
        guard FileManager.default.fileExists(atPath: output.path) else {
            alert = "This result has moved or was removed. Locate the saved file before trimming it."
            return
        }
        let input = output.standardizedFileURL
        if let existing = jobs.first(where: { $0.input.standardizedFileURL == input }) {
            selection = existing.id; workspaceTask = .trim
            return
        }
        let child = FileJob(input: input)
        child.collisionPolicy = preferences.defaultCollision
        child.origin = .init(rootJobID: parent.origin?.rootJobID ?? parent.id,
                             parentJobID: parent.id, parentOutput: output)
        jobs.append(child); selection = child.id; workspaceTask = .trim
        // The result is a new source with a new full-range draft. Parent times
        // belong to the parent's clock and must never be inherited here.
        Task { await inspect(child, task: .trim); saveSession() }
        saveSession()
    }

    func openLinkResultInTrim(_ saved: LinkSavedResult) {
        guard FileManager.default.fileExists(atPath: saved.url.path) else { alert = "This saved file has moved or was removed."; return }
        workspaceTask = .trim
        addFiles([saved.url])
        if let job = jobs.first(where: { $0.input.standardizedFileURL == saved.url.standardizedFileURL }) {
            if job.state == .inspecting { job.format = saved.format }
            selection = job.id
        }
    }

    func cancelAll() { batchTask?.cancel(); activeTask?.cancel(); pdfWorkspace.cancel(); linkWorkspace.cancel() }
    func makeAnotherVersion(_ job: FileJob) {
        guard !isRunning, job.state != .missing else { return }
        job.result = nil; job.resultPreview = nil; job.state = .ready
        Task { await refreshPlan(job) }
    }
    func cancel(_ job: FileJob) {
        if job.state == .running {
            job.cancelRequested = true
            if job.id == activeID { activeTask?.cancel() }
        }
        else if job.state == .queued { job.state = .cancelled }
    }
    func remove(_ job: FileJob) {
        guard !restoringSession, !pdfWorkspace.isRunning, job.state != .running && job.state != .queued && job.state != .inspecting else { return }
        if job.hasInputScope { job.input.stopAccessingSecurityScopedResource() }
        pendingPDFImports.remove(job.id)
        pdfWorkspace.removeSource(job.id.uuidString)
        jobs.removeAll { $0.id == job.id }
        let retained = batchSelection.subtracting([job.id])
        if !retained.isEmpty { selectBatch(retained) }
        else if selection == job.id { selection = jobs.first?.id }
    }
    func reveal(_ url: URL) {
        guard FileManager.default.fileExists(atPath: url.path) else { alert = "This result has moved or was removed."; return }
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }
    func open(_ url: URL) {
        guard FileManager.default.fileExists(atPath: url.path) else { alert = "This result has moved or was removed."; return }
        if !NSWorkspace.shared.open(url) { alert = "No application could open this result. You can reveal it in Finder instead." }
    }
}

extension WorkspaceModel {
    func startPersistence(store: WorkspaceStore? = nil) async {
        guard !persistenceStarted else { return }
        persistenceStarted = true
        restoringSession = true
        defer { restoringSession = false }
        let store = store ?? WorkspaceStore.applicationStore()
        persistence = store
        setups.connect(to: store.url.deletingLastPathComponent().appendingPathComponent("setups-v1.json"))
        do {
            if let record = try store.load() {
                workspaceTask = record.task
                pendingPDFImports = Set(record.pdfPendingSourceIDs ?? [])
                if preferences.rememberOutputFolder, let reference = record.destination {
                    if let folder = store.resolve(reference) { setDestination(folder) }
                    else { persistenceNotice = "The output folder is unavailable. Choose it again before creating files." }
                }
                var restored: [(FileJob, WorkspaceJobRecord)] = []
                for saved in record.jobs {
                    let url = store.resolve(saved.source)
                    let job = FileJob(input: url ?? saved.source.lastKnownURL, id: saved.id, acquireAccess: url != nil)
                    saved.draft.apply(to: job)
                    job.lastTrimDetails = saved.lastTrimDetails; job.lastPDFOptimizationDetails = saved.lastPDFOptimizationDetails
                    job.origin = saved.origin
                    storedSources[job.id] = saved.source
                    job.outputs = saved.outputs.map { reference in
                        let result = store.resolve(reference) ?? reference.lastKnownURL
                        storedOutputs[result] = reference
                        return result
                    }
                    if url == nil {
                        job.state = .missing; job.error = "This source is unavailable. Locate it to continue; saved results are kept."
                    } else {
                        job.state = [.queued, .running, .inspecting].contains(saved.state) ? .interrupted : saved.state
                        if job.state == .missing { job.state = .ready }
                        restored.append((job, saved))
                    }
                    jobs.append(job)
                }
                selection = jobs.contains { $0.id == record.selection } ? record.selection : jobs.first?.id
                if let selected = record.batchSelection {
                    let focus = selection
                    selectBatch(Set(selected))
                    if let focus, batchSelection.contains(focus) { updatingBatchSelection = true; selection = focus; updatingBatchSelection = false }
                }
                runSelectionOnly = record.runSelectionOnly ?? false
                let pdfOutputs = record.pdfOutputs.map { reference in
                    let result = store.resolve(reference) ?? reference.lastKnownURL
                    storedOutputs[result] = reference
                    return result
                }
                pdfWorkspace.restoreSession(pages: record.pages, selection: record.pageSelection,
                    sources: jobs.filter { record.pdfSourceIDs.contains($0.id.uuidString) }, outputName: record.pdfOutputName,
                    collisionPolicy: record.pdfCollisionPolicy ?? .rename, exportAsDirectory: record.pdfExportAsDirectory ?? false, hasBeenOpened: record.pdfWasOpened,
                    pageOutput: record.pdfPageOutput ?? .init(),
                    artifacts: zip(record.pdfArtifacts, pdfOutputs).map { artifact, url in
                        .init(url: url, format: artifact.format, bytes: artifact.bytes, sourceIDs: artifact.sourceIDs, sourcePages: artifact.sourcePages, pdfEmbeddedImage: artifact.pdfEmbeddedImage)
                    }, interrupted: record.pdfInterrupted, destination: outputFolder)
                if let selected = record.pdfSelectedPages {
                    pdfWorkspace.restorePageSelection(Set(selected), primary: record.pageSelection)
                }
                for (job, saved) in restored {
                    await inspect(job, task: .convert, savedDraft: saved.draft, savedState: job.state)
                }
                linkWorkspace.history = (record.linkHistory ?? []).map { item in
                    LinkSavedResult(id: item.id, date: item.date, url: store.resolve(item.output) ?? item.output.lastKnownURL,
                                    format: item.format, receipt: item.receipt)
                }
                linkWorkspace.openInTrim = record.linkOpenInTrim ?? false
                if record.linkInterrupted == true { linkWorkspace.notice = "The previous link request was interrupted. Look Up again to retry." }
                pdfWorkspace.refresh()
                if record.pdfInterrupted { persistenceNotice = "The previous PDF export was interrupted. Review the pages before retrying." }
            }
            persistenceEnabled = true
            observeSession()
        } catch {
            persistenceNotice = error.localizedDescription + " Session saving is paused to preserve the existing record."
        }
    }

    private func sessionRecord() throws -> WorkspaceRecord? {
        guard let store = persistence, persistenceEnabled else { return nil }
        let records = try jobs.map { job -> WorkspaceJobRecord in
            // Missing files retain their permission records without implicitly granting access.
            let source: WorkspaceFileReference
            if job.state == .missing, let stored = storedSources[job.id] { source = stored }
            else { source = try store.reference(job.input) }
            storedSources[job.id] = source
            return .init(id: job.id, source: source, draft: FileJobDraft(job), state: job.state,
                         outputs: try job.outputs.map { try outputReference($0, store: store) }, lastTrimDetails: job.lastTrimDetails, lastPDFOptimizationDetails: job.lastPDFOptimizationDetails, origin: job.origin)
        }
        return WorkspaceRecord(task: workspaceTask, selection: selection,
            destination: try (preferences.rememberOutputFolder ? outputFolder : nil).map { try store.reference($0, readOnly: false) }, jobs: records,
            pages: pdfWorkspace.pages, pageSelection: pdfWorkspace.selection,
            pdfSourceIDs: pdfWorkspace.sources.keys.sorted(), pdfOutputName: pdfWorkspace.outputName,
            pdfInterrupted: pdfWorkspace.isRunning,
            pdfOutputs: try pdfWorkspace.completedArtifacts.map { try outputReference($0.url, store: store) },
            pdfArtifacts: pdfWorkspace.completedArtifacts, pdfCollisionPolicy: pdfWorkspace.collisionPolicy, pdfExportAsDirectory: pdfWorkspace.exportAsDirectory, pdfWasOpened: pdfWorkspace.hasBeenOpened, pdfPendingSourceIDs: pendingPDFImports.sorted { $0.uuidString < $1.uuidString },
            linkHistory: try linkWorkspace.history.map { .init(id: $0.id, date: $0.date, output: try outputReference($0.url, store: store), format: $0.format, receipt: $0.receipt) },
            linkInterrupted: linkWorkspace.isBusy, linkOpenInTrim: linkWorkspace.openInTrim,
            batchSelection: batchSelection.sorted { $0.uuidString < $1.uuidString }, runSelectionOnly: runSelectionOnly,
            pdfSelectedPages: pdfWorkspace.selectedPageIDs.sorted { $0.uuidString < $1.uuidString }, pdfPageOutput: pdfWorkspace.pageOutput)
    }
    private func outputReference(_ url: URL, store: WorkspaceStore) throws -> WorkspaceFileReference {
        if !FileManager.default.fileExists(atPath: url.path), let stored = storedOutputs[url] { return stored }
        let reference = try store.reference(url)
        storedOutputs[url] = reference
        return reference
    }
    func saveSession() {
        do { if let record = try sessionRecord() { try persistence?.save(record) } }
        catch { persistenceNotice = "Could not save the workspace: " + error.localizedDescription }
    }
    private func observeSession() {
        withObservationTracking {
            // Read every persisted field even if capturing a bookmark fails midway.
            _ = workspaceTask; _ = selection; _ = batchSelection; _ = runSelectionOnly; _ = outputFolder; _ = pendingPDFImports; _ = pdfWorkspace.hasBeenOpened
            for job in jobs { _ = FileJobDraft(job); _ = job.state; _ = job.input; _ = job.outputs }
            _ = pdfWorkspace.pages; _ = pdfWorkspace.selection; _ = pdfWorkspace.selectedPageIDs; _ = pdfWorkspace.sources
            _ = pdfWorkspace.outputName; _ = pdfWorkspace.collisionPolicy; _ = pdfWorkspace.exportAsDirectory; _ = pdfWorkspace.isRunning; _ = pdfWorkspace.completedArtifacts
            _ = pdfWorkspace.pageOutput
        } onChange: { [weak self] in
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.saveSession(); self.observeSession()
            }
        }
    }
    func clearWorkspace() {
        guard !restoringSession, !isRunning, !jobs.contains(where: { $0.state == .inspecting }) else { return }
        for job in jobs where job.hasInputScope { job.input.stopAccessingSecurityScopedResource() }
        jobs.removeAll(); selection = nil; pendingPDFImports.removeAll(); pdfWorkspace.clear()
        linkWorkspace.history = []; linkWorkspace.urlText = ""
        batchSelection = []; runSelectionOnly = false; pendingImportGoal = nil
        storedSources.removeAll(); storedOutputs.removeAll()
        persistence?.releaseReferences()
        saveSession()
    }
    func locateSource(_ job: FileJob) {
        guard !isRunning, job.state == .missing else { return }
        guard let panel = makeOpenPanel() else { return }
        panel.title = "Locate \(job.input.lastPathComponent)"; panel.prompt = "Use This File"
        panel.canChooseFiles = true; panel.canChooseDirectories = false; panel.allowsMultipleSelection = false
        present(panel) { [weak self] response in
            guard let self, response == .OK, let url = panel.url else { return }
            self.relink(job, to: url)
        }
    }
    func relink(_ job: FileJob, to url: URL) {
        guard !isRunning, job.state == .missing, url.isFileURL else { return }
        if job.hasInputScope { job.input.stopAccessingSecurityScopedResource() }
        pdfWorkspace.invalidatePlan()
        job.mediaTimeline = nil; job.trimPlan = nil; job.trimError = nil
        job.input = url; job.hasInputScope = url.startAccessingSecurityScopedResource()
        storedSources.removeValue(forKey: job.id)
        let draft = FileJobDraft(job)
        job.state = .inspecting; job.error = nil
        Task {
            await inspect(job, task: .convert, savedDraft: draft, savedState: .ready)
            pdfWorkspace.refresh()
        }
    }
}

extension WorkspaceModel {
    var pendingPDFSourceJobs: [FileJob] { jobs.filter { pendingPDFImports.contains($0.id) } }
    func skipPendingPDFImport(_ id: UUID) { pendingPDFImports.remove(id); saveSession() }
    private func flushPendingPDFImports() {
        guard !pdfWorkspace.isRunning else { return }
        let inspected = pendingPDFSourceJobs.filter { $0.inspection != nil && $0.state != .inspecting }
        let usable = inspected.filter { job in job.inspection.map { [.image, .pdf].contains($0.family) } ?? false }
        if !usable.isEmpty { pdfWorkspace.include(usable, destination: outputFolder) }
        let unsupported = inspected.filter { job in !usable.contains(where: { $0.id == job.id }) }
        if !unsupported.isEmpty {
            alert = "These files cannot join a PDF arrangement: " + unsupported.prefix(5).map { $0.input.lastPathComponent }.joined(separator: ", ") + ". They remain available in Convert Files."
        }
        pendingPDFImports.subtract(inspected.map(\.id))
    }
    func discardPendingPDFImports() { pendingPDFImports.removeAll() }
    func planSetupRequest(_ request: TransformationRequest) async throws -> TransformationPlan { try await engine.plan(request) }
    func setupRequest(for job: FileJob) -> TransformationRequest? {
        guard !isRunning, !restoringSession, job.inspection != nil, ![.missing, .inspecting].contains(job.state) else { return nil }
        return try? transformationRequest(for: job)
    }
    var currentSetupRequest: TransformationRequest? {
        guard !isRunning, !restoringSession else { return nil }
        if workspaceTask == .link { return nil }
        if workspaceTask == .pages { return pdfWorkspace.plan?.request }
        if workspaceTask == .trim { return selectedJob.flatMap { try? trimRequest(for: $0) } }
        guard let job = selectedJob else { return nil }
        return setupRequest(for: job)
    }
}
