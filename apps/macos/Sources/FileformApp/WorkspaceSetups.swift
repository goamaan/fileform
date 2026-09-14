import Foundation
import FileformDomain

struct SetupSourceSnapshot: Equatable {
    let id: UUID
    let input: URL
    let key: String
    let state: FileJobState
    let identity: FileIdentity?
    @MainActor init(_ job: FileJob) { id = job.id; input = job.input; key = job.planKey + "|" + (job.trimDraft?.key ?? ""); state = job.state; identity = job.inspection?.identity }
}
struct SetupReviewIssue: Identifiable {
    let id: UUID
    let name: String
    let message: String
}
struct PreparedSetupChange {
    let job: FileJob
    let source: SetupSourceSnapshot
    let draft: FileJobDraft
    let plan: TransformationPlan
}
struct PDFSetupSnapshot: Equatable {
    let pages: [PDFPageDraft]
    let sourceIDs: [String]
    let outputName: String
    let collisionPolicy: CollisionPolicy
    let exportAsDirectory: Bool
    let pageOutput: PDFPageOutputOptions
    let exportedSelection: Set<UUID>
    @MainActor init(_ draft: PDFWorkspaceModel) {
        pages = draft.pages; sourceIDs = draft.sources.keys.sorted(); outputName = draft.outputName; collisionPolicy = draft.collisionPolicy; exportAsDirectory = draft.exportAsDirectory
        pageOutput = draft.pageOutput
        exportedSelection = draft.pageOutput.isImageOutput && draft.pageOutput.selectedOnly ? draft.selectedPageIDs : []
    }
}
struct PreparedPDFSetup {
    let jobs: [FileJob]
    let sources: [SetupSourceSnapshot]
    let priorDraft: PDFSetupSnapshot
    let pages: [PDFPageDraft]
    let outputName: String
    let plan: TransformationPlan
}
struct SetupApplicationReview {
    let recipe: TransformationRecipe
    let destinationFolder: URL?
    let changes: [PreparedSetupChange]
    let issues: [SetupReviewIssue]
    let pdf: PreparedPDFSetup?
    var applicableCount: Int { pdf?.jobs.count ?? changes.count }
    var warnings: [String] {
        var seen = Set<String>()
        return (pdf.map { [$0.plan] } ?? changes.map(\.plan)).flatMap(\.warnings).filter { seen.insert($0).inserted }
    }
}

extension FileJobDraft {
    @MainActor init(recipe: TransformationRecipe, job: FileJob) throws {
        self.init(job)
        pdfCompression = .init()
        let parameters: ConversionParameters
        switch recipe.operation {
        case .mediaTrim(let interval, let mode, let audio, let muted):
            guard recipe.fidelity == .allowDeclaredLosses else {
                throw FileformError(.unsupported, "This editor cannot promise this setup's strict lossless policy.")
            }
            trimDraft = .init(interval: interval, format: recipe.format,
                outputName: job.input.deletingPathExtension().lastPathComponent + "-trimmed",
                mode: mode, audioStream: audio, muteAudio: muted, collisionPolicy: recipe.collisionPolicy)
            return
        case .pdfOptimize(let value):
            guard job.inspection?.family == .pdf, recipe.format == .pdf, recipe.fidelity == .allowDeclaredLosses else {
                throw FileformError(.unsupported, "This image-compression setup needs a PDF source and explicit permission for image quality changes.")
            }
            format = .pdf; goal = value.goal; collisionPolicy = recipe.collisionPolicy; crop.enabled = false
            sizeLimitMB = value.maximumBytes.map { NSDecimalNumber(decimal: Decimal($0) / 1_000_000).stringValue } ?? "10"
            pdfCompression = .init(compressImages: true, quality: value.quality, minimumQuality: value.minimumQuality,
                resizeImages: value.maximumImageDimension != nil, longestEdge: String(value.maximumImageDimension ?? 1920))
            return
        case .conversion(let value): parameters = value; crop.enabled = false
        case .imageCrop(let rectangle, let value):
            parameters = value
            crop = .init(enabled: true, x: String(rectangle.x), y: String(rectangle.y), width: String(rectangle.width), height: String(rectangle.height))
        default: throw FileformError(.unsupported, "This setup needs an editor that is not available here yet.")
        }
        let structuralPDF = job.inspection?.family == .pdf && recipe.format == .pdf && parameters.goal != .convert &&
            recipe.fidelity == .requireLossless && parameters.color == .preserve && parameters.metadata == .preserve
        guard structuralPDF || (recipe.fidelity == .allowDeclaredLosses && parameters.color == .convertToSRGB && parameters.metadata == .removeDescriptive) else {
            throw FileformError(.unsupported, "This setup requests preservation policies that this editor cannot apply yet. Its choices have not been changed.")
        }
        format = recipe.format; goal = parameters.goal; collisionPolicy = recipe.collisionPolicy
        quality = parameters.options.quality; minimumQuality = parameters.options.minimumQuality
        sizeLimitMB = parameters.options.maximumBytes.map { NSDecimalNumber(decimal: Decimal($0) / 1_000_000).stringValue } ?? "10"
        resize = parameters.options.maxDimension != nil; longestEdge = String(parameters.options.maxDimension ?? 1920)
        background = parameters.options.background?.rawValue ?? "preserve"
        minimumVideoBitrate = String(parameters.options.minimumVideoBitrate)
        pdfPage = parameters.options.pageNumber.map(String.init) ?? ""
    }
}

extension WorkspaceModel {
    func reviewSetup(_ recipe: TransformationRecipe, sourceIDs: [UUID], outputName: String) async throws -> SetupApplicationReview {
        try recipe.validate()
        guard !isRunning, !restoringSession, !sourceIDs.isEmpty,
              Set(sourceIDs).count == sourceIDs.count else { throw FileformError(.invalidRequest, "Select available files while no work is running.") }
        let selected = try sourceIDs.map { id in
            guard let job = jobs.first(where: { $0.id == id }) else { throw FileformError(.invalidRequest, "A selected file was removed. Choose files again.") }
            return job
        }
        let folder = outputFolder
        if recipe.isCompositionSetup {
            guard selected.count == recipe.assetSlots.count else {
                throw FileformError(.invalidRequest, "This setup needs \(recipe.assetSlots.count) distinct source files in the displayed slot order.")
            }
            guard recipe.fidelity == .allowDeclaredLosses else {
                throw FileformError(.unsupported, "The PDF editor cannot guarantee this setup's strict lossless policy.")
            }
            let snapshots = selected.map(SetupSourceSnapshot.init)
            let priorDraft = PDFSetupSnapshot(pdfWorkspace)
            for job in selected { try requireSetupSource(job) }
            let destination = try PDFWorkspaceModel.outputURL(name: outputName,
                folder: folder ?? selected[0].input.deletingLastPathComponent(), split: recipe.operation.cardinality == .directory,
                pageImages: [.pdfRasterize, .pdfExtractImages].contains(recipe.operation.id))
            let assets = zip(recipe.assetSlots, selected).map { AssetReference(id: $0.0, url: $0.1.input) }
            let bound = try recipe.bind(assets: assets, destination: destination)
            let bindings = Dictionary(uniqueKeysWithValues: zip(recipe.assetSlots, selected.map { $0.id.uuidString }))
            func rebound(_ pages: [PageReference]) -> [PageReference] {
                pages.map { .init(sourceID: bindings[$0.sourceID]!, pageIndex: $0.pageIndex, clockwiseRotation: $0.clockwiseRotation) }
            }
            let operation: TransformationOperation
            switch bound.operation {
            case .pdfComposition(let pages): operation = .pdfComposition(pages: rebound(pages))
            case .pdfSplit(let groups): operation = .pdfSplit(groups: groups.map(rebound))
            case .pdfRasterize(let pages, let dpi, let quality): operation = .pdfRasterize(pages: rebound(pages), dpi: dpi, quality: quality)
            case .pdfExtractImages(let pages): operation = .pdfExtractImages(pages: rebound(pages))
            default: throw FileformError(.unsupported, "This setup is not a PDF arrangement.")
            }
            let request = try TransformationRequest(assets: selected.map { .init(id: $0.id.uuidString, url: $0.input) },
                operation: operation, output: bound.output, fidelity: bound.fidelity, collisionPolicy: bound.collisionPolicy)
            let plan = try await planSetupRequest(request)
            try Task.checkCancellation()
            for (job, snapshot) in zip(selected, snapshots) { try requireUnchangedSetupSource(job, snapshot: snapshot, plan: plan) }
            let groups: [[PageReference]]
            switch recipe.operation {
            case .pdfComposition(let pages): groups = [pages]
            case .pdfSplit(let values): groups = values
            case .pdfRasterize(let pages, _, _), .pdfExtractImages(let pages): groups = [pages]
            default: throw FileformError(.unsupported, "This setup is not a PDF arrangement.")
            }
            var pages: [PDFPageDraft] = []
            for (groupIndex, group) in groups.enumerated() {
                for (pageIndex, page) in group.enumerated() {
                    guard let sourceID = bindings[page.sourceID] else { throw FileformError(.invalidRequest, "A PDF source slot is missing.") }
                    pages.append(.init(sourceID: sourceID, pageIndex: page.pageIndex, rotation: page.clockwiseRotation,
                        splitAfter: groupIndex < groups.count - 1 && pageIndex == group.count - 1))
                }
            }
            return .init(recipe: recipe, destinationFolder: folder, changes: [], issues: [],
                pdf: .init(jobs: selected, sources: snapshots, priorDraft: priorDraft, pages: pages, outputName: outputName, plan: plan))
        }
        var changes: [PreparedSetupChange] = []
        var issues: [SetupReviewIssue] = []
        for job in selected {
            try Task.checkCancellation()
            do {
                try requireSetupSource(job)
                let snapshot = SetupSourceSnapshot(job)
                let draft = try FileJobDraft(recipe: recipe, job: job)
                let candidate = FileJob(input: job.input, acquireAccess: false)
                candidate.inspection = job.inspection; candidate.capabilities = job.capabilities
                draft.apply(to: candidate)
                let roundTrip = try recipe.operation.id == .mediaTrim ? trimRequest(for: candidate) : transformationRequest(for: candidate)
                guard roundTrip.operation == recipe.operation, roundTrip.output.format == recipe.format,
                      roundTrip.fidelity == recipe.fidelity, roundTrip.collisionPolicy == recipe.collisionPolicy else {
                    throw FileformError(.unsupported, "Some options in this setup cannot be represented by this editor yet. The saved choices are unchanged.")
                }
                let request = try recipe.bind(assets: [.init(id: recipe.assetSlots[0], url: job.input)], destination: roundTrip.output.destination)
                let plan = try await planSetupRequest(request)
                try requireUnchangedSetupSource(job, snapshot: snapshot, plan: plan)
                changes.append(.init(job: job, source: snapshot, draft: draft, plan: plan))
            } catch is CancellationError { throw CancellationError() }
            catch { issues.append(.init(id: job.id, name: job.input.lastPathComponent, message: error.localizedDescription)) }
        }
        return .init(recipe: recipe, destinationFolder: folder, changes: changes, issues: issues, pdf: nil)
    }
    func applySetup(_ review: SetupApplicationReview) throws {
        guard !isRunning, !restoringSession, outputFolder == review.destinationFolder, review.applicableCount > 0 else {
            throw FileformError(.invalidRequest, "The workspace changed. Review this setup again before applying it.")
        }
        if let pdf = review.pdf {
            guard PDFSetupSnapshot(pdfWorkspace) == pdf.priorDraft else {
                throw FileformError(.invalidRequest, "The page arrangement changed. Review the replacement again.")
            }
            for (job, snapshot) in zip(pdf.jobs, pdf.sources) { try requireUnchangedSetupSource(job, snapshot: snapshot, plan: pdf.plan) }
            discardPendingPDFImports()
            pdfWorkspace.applySetupDraft(pages: pdf.pages, sources: pdf.jobs, outputName: pdf.outputName,
                                        destination: outputFolder, plan: pdf.plan)
            workspaceTask = .pages
        } else {
            // Validate the whole approved change set before mutating any file's choices.
            for change in review.changes { try requireUnchangedSetupSource(change.job, snapshot: change.source, plan: change.plan) }
            for change in review.changes {
                change.draft.apply(to: change.job)
                change.job.result = nil; change.job.resultPreview = nil; change.job.error = nil
                change.job.state = .ready
                if review.recipe.operation.id == .mediaTrim { change.job.trimPlan = change.plan }
                else {
                    change.job.plan = change.plan
                    change.job.approvedPlanKey = change.job.planKey
                    change.job.approvedPlanFolder = outputFolder
                }
            }
            selection = review.changes.first?.job.id
            workspaceTask = review.recipe.operation.id == .mediaTrim ? .trim : review.recipe.format == .txt ? .text : .convert
        }
        saveSession()
    }
    private func requireSetupSource(_ job: FileJob) throws {
        guard jobs.contains(where: { $0 === job }), job.inspection != nil,
              ![.missing, .inspecting, .running, .queued].contains(job.state) else {
            throw FileformError(.invalidRequest, "This source is unavailable, still being inspected, or busy. Its choices will stay unchanged.")
        }
    }
    private func requireUnchangedSetupSource(_ job: FileJob, snapshot: SetupSourceSnapshot, plan: TransformationPlan) throws {
        try requireSetupSource(job)
        guard SetupSourceSnapshot(job) == snapshot,
              let input = plan.inputs.first(where: { $0.inspection.input.standardizedFileURL == job.input.standardizedFileURL }),
              input.inspection.identity == job.inspection?.identity else {
            throw FileformError(.inputChanged, "This file or its choices changed. Reinspect the source and review the setup again.")
        }
    }
}
