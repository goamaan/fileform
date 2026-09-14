import AppKit
import CoreGraphics
import PDFKit
import XCTest
@testable import Fileform
import FileformCore
import FileformDomain

@MainActor final class EditingWorkspaceTests: XCTestCase {
    func testBatchImportWaitsForEachFileReadinessBeforeCreatingPagePlan() async throws {
        let directory = try makeDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let first = directory.appendingPathComponent("first.png"), second = directory.appendingPathComponent("second.png")
        try writeImage(to: first); try writeImage(to: second)
        let model = WorkspaceModel()
        model.setDestination(directory)
        model.chooseTask(.pages)
        model.addFiles([first, second])
        let deadline = ContinuousClock.now + .seconds(5)
        while (model.jobs.contains(where: { $0.state == .inspecting }) || model.pdfWorkspace.plan == nil), ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        XCTAssertEqual(model.jobs.map(\.state), [.ready, .ready])
        XCTAssertEqual(model.pdfWorkspace.pages.count, 2)
        XCTAssertTrue(model.pendingPDFSourceJobs.isEmpty)
        XCTAssertNil(model.pdfWorkspace.error)
        XCTAssertTrue(model.pdfWorkspace.canRun, "The batch must become runnable without changing an unrelated setting")
        while model.jobs.contains(where: { $0.originalPreview == nil }), ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
    }

    func testMigrationPreservesExactByteLimitAndConversionOptions() throws {
        let model = WorkspaceModel()
        let job = sampleImageJob()
        job.goal = .fit
        job.sizeLimitMB = "1.000001"
        job.resize = true
        job.longestEdge = "160"
        job.quality = 0.71
        job.minimumQuality = 0.4
        job.background = "white"
        let legacy = try model.request(for: job)
        let request = try model.transformationRequest(for: job)
        guard case .conversion(let parameters) = request.operation else {
            return XCTFail("An ordinary conversion must remain a conversion after migration")
        }
        XCTAssertEqual(parameters.options.maximumBytes, 1_000_001)
        XCTAssertEqual(parameters.options, legacy.options)
        XCTAssertEqual(parameters.goal, legacy.goal)
        XCTAssertEqual(request.assets.map(\.url), [legacy.input])
        XCTAssertEqual(request.output.destination, legacy.destination)
        XCTAssertEqual(request.output.format, legacy.format)
        XCTAssertEqual(request.collisionPolicy, .rename)
        XCTAssertNotEqual(request.output.destination, job.input)
    }

    func testCropUsesTypedPixelsAndRejectsInvalidUserInput() throws {
        let model = WorkspaceModel()
        let job = sampleImageJob()
        job.crop = ImageCropDraft(enabled: true, x: "12", y: "24", width: "100", height: "80")
        job.resize = true
        job.longestEdge = "50"
        let request = try model.transformationRequest(for: job)
        guard case .imageCrop(let rectangle, let conversion) = request.operation else {
            return XCTFail("The enabled crop must survive in the executable request")
        }
        XCTAssertEqual(rectangle, PixelCrop(x: 12, y: 24, width: 100, height: 80))
        XCTAssertEqual(conversion.options.maxDimension, 50)
        XCTAssertEqual(request.assets.first?.url, job.input)
        for invalidX in ["1.5", "twelve", "-1", String(Int.max)] {
            job.crop.x = invalidX
            XCTAssertThrowsError(try model.transformationRequest(for: job), invalidX)
        }
        job.crop.x = "0"
        job.crop.width = "0"
        XCTAssertThrowsError(try model.transformationRequest(for: job))
        job.crop.width = "100"
        job.crop.height = "-10"
        XCTAssertThrowsError(try model.transformationRequest(for: job))
    }

    func testApprovedCropPlanDoesNotReadLaterDraftEdits() throws {
        let model = WorkspaceModel()
        let job = sampleImageJob()
        job.crop = ImageCropDraft(enabled: true, x: "10", y: "20", width: "100", height: "80")
        let approved = TransformationPlan(request: try model.transformationRequest(for: job),
                                           inputs: [.init(id: "source", inspection: try XCTUnwrap(job.inspection))], warnings: [])
        job.plan = approved
        job.crop.x = "200"
        job.crop.width = "10"
        job.format = .png
        job.quality = 0.5
        XCTAssertEqual(approved.request.output.format, .jpeg)
        guard case .imageCrop(let rectangle, let conversion) = approved.request.operation else {
            return XCTFail("Approved crop was lost")
        }
        XCTAssertEqual(rectangle, PixelCrop(x: 10, y: 20, width: 100, height: 80))
        XCTAssertEqual(conversion.options.quality, 0.82)
        XCTAssertNotEqual(approved.request.operation, try model.transformationRequest(for: job).operation)
    }

    func testRealPDFAndImageCompositionPreservesProvenanceAndApprovedSplit() async throws {
        let directory = try makeDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let pdf = directory.appendingPathComponent("two pages.pdf")
        let image = directory.appendingPathComponent("photo.png")
        try writePDF(to: pdf, pages: 2)
        try writeImage(to: image)
        let engine = ConversionEngine()
        let documentJob = FileJob(input: pdf)
        documentJob.inspection = try await engine.inspect(pdf)
        documentJob.state = .ready
        let imageJob = FileJob(input: image)
        imageJob.inspection = try await engine.inspect(image)
        imageJob.state = .ready
        let workspace = PDFWorkspaceModel(engine: engine)
        workspace.include([documentJob, imageJob], destination: directory)
        let originalPages = workspace.pages
        XCTAssertEqual(originalPages.map(\.sourceID), [documentJob.id.uuidString, documentJob.id.uuidString, imageJob.id.uuidString])
        XCTAssertEqual(originalPages.map(\.pageIndex), [0, 1, 0])
        workspace.include([documentJob, imageJob], destination: directory)
        XCTAssertEqual(workspace.pages, originalPages, "Returning to the task must not duplicate included sources")
        workspace.edit("Split", undo: nil) { $0[0].splitAfter = true }
        try await waitForPlan(workspace)
        let approved = try XCTUnwrap(workspace.plan)
        guard case .pdfSplit(let groups) = approved.request.operation else { return XCTFail("A split boundary must create split outputs") }
        XCTAssertEqual(groups.map(\.count), [1, 2])
        XCTAssertEqual(groups[1].map(\.sourceID), [documentJob.id.uuidString, imageJob.id.uuidString])
        XCTAssertEqual(approved.request.output.cardinality, .directory)
        workspace.edit("Reorder", undo: nil) { $0.reverse(); $0[0].rotation = 90 }
        workspace.outputName = "Revised.pdf"
        workspace.refresh()
        try await waitForPlan(workspace)
        XCTAssertEqual(workspace.orderedSourceJobs.map(\.id), [imageJob.id, documentJob.id])
        XCTAssertEqual(groups[0][0], originalPages[0].reference)
        XCTAssertEqual(approved.request.assets.map(\.id), [documentJob.id.uuidString, imageJob.id.uuidString])
        XCTAssertNotEqual(approved.request.operation, workspace.plan?.request.operation)
        XCTAssertTrue(FileManager.default.fileExists(atPath: pdf.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: image.path))
    }

    func testPageEditsUndoRedoRetainIdentityAndSplitBoundaries() {
        let workspace = PDFWorkspaceModel(engine: ConversionEngine())
        let pages = (0..<3).map { PDFPageDraft(sourceID: "source", pageIndex: $0) }
        workspace.pages = pages
        workspace.selection = pages[1].id
        let undo = UndoManager()
        undo.groupsByEvent = false
        func edit(_ name: String, _ change: (inout [PDFPageDraft]) -> Void) {
            undo.beginUndoGrouping()
            workspace.edit(name, undo: undo, change)
            undo.endUndoGrouping()
        }
        edit("Rotate") { $0[1].rotation = 90 }
        edit("Move before") { $0.swapAt(0, 1) }
        edit("Split here") { $0[0].splitAfter = true }
        edit("Remove") { $0.remove(at: 0) }
        XCTAssertEqual(workspace.pages.map(\.id), [pages[0].id, pages[2].id])
        XCTAssertEqual(workspace.selection, pages[0].id)
        undo.undo()
        XCTAssertEqual(workspace.pages[0].id, pages[1].id)
        XCTAssertEqual(workspace.pages[0].rotation, 90)
        XCTAssertTrue(workspace.pages[0].splitAfter)
        XCTAssertEqual(workspace.outputCount, 2)
        undo.undo(); undo.undo(); undo.undo()
        XCTAssertEqual(workspace.pages, pages)
        undo.redo(); undo.redo(); undo.redo(); undo.redo()
        XCTAssertEqual(workspace.pages.map(\.id), [pages[0].id, pages[2].id])
        XCTAssertEqual(workspace.outputCount, 1)
    }

    func testSelectedPageImagesIgnoreSplitMarkersAndRefreshWhenSelectionChanges() async throws {
        let directory = try makeDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let pdf = directory.appendingPathComponent("document.pdf")
        let image = directory.appendingPathComponent("photo.png")
        try writePDF(to: pdf, pages: 2); try writeImage(to: image)
        let worker = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("../../.build/debug/fileform-worker").standardizedFileURL
        let engine = ConversionEngine(workerExecutable: worker)
        let documentJob = FileJob(input: pdf), imageJob = FileJob(input: image)
        documentJob.inspection = try await engine.inspect(pdf); documentJob.state = .ready
        imageJob.inspection = try await engine.inspect(image); imageJob.state = .ready
        let workspace = PDFWorkspaceModel(engine: engine)
        workspace.include([documentJob, imageJob], destination: directory)
        workspace.pageOutput = .init(format: .png, dpi: "144", quality: 0.85, selectedOnly: true)
        workspace.edit("Arrange", undo: nil) { $0[0].splitAfter = true; $0[2].rotation = 90 }
        workspace.selectPages([workspace.pages[0].id, workspace.pages[2].id])
        try await waitForPlan(workspace)
        let request = try XCTUnwrap(workspace.plan?.request)
        guard case .pdfRasterize(let selected, let dpi, _) = request.operation else { return XCTFail("Expected page images") }
        XCTAssertEqual(dpi, 144)
        XCTAssertEqual(selected, [workspace.pages[0].reference, workspace.pages[2].reference])
        XCTAssertEqual(request.output.format, .png)
        XCTAssertEqual(request.output.cardinality, .directory)
        XCTAssertEqual(workspace.outputCount, 2)
        workspace.selectPages([workspace.pages[1].id])
        XCTAssertNil(workspace.plan, "Selection changes must invalidate the approved page set synchronously")
        try await waitForPlan(workspace)
        XCTAssertEqual(workspace.plan?.request.assets.map(\.id), [documentJob.id.uuidString])
        guard case .pdfRasterize(let revised, _, _) = workspace.plan?.request.operation else { return XCTFail("Expected revised page images") }
        XCTAssertEqual(revised, [workspace.pages[1].reference])
        workspace.selectPages([])
        XCTAssertFalse(workspace.canRun)
        XCTAssertNotNil(workspace.error)
    }

    func testEmbeddedImageScopeExcludesStandaloneImagesAndEmptyPlanCannotRun() async throws {
        let directory = try makeDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let core = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("../..").standardizedFileURL
        let pack = core.appendingPathComponent("Artifacts/PDFPack")
        guard FileManager.default.fileExists(atPath: pack.path) else { throw XCTSkip("Requires the verified PDF engine pack") }
        let engine = ConversionEngine(pdfPack: pack, workerExecutable: core.appendingPathComponent(".build/debug/fileform-worker"))
        let pdf = directory.appendingPathComponent("vector-only.pdf"), image = directory.appendingPathComponent("photo.png")
        try writePDF(to: pdf, pages: 2); try writeImage(to: image)
        let pdfJob = FileJob(input: pdf), imageJob = FileJob(input: image)
        pdfJob.inspection = try await engine.inspect(pdf); pdfJob.state = .ready
        imageJob.inspection = try await engine.inspect(image); imageJob.state = .ready
        let workspace = PDFWorkspaceModel(engine: engine)
        workspace.include([pdfJob, imageJob], destination: directory)
        workspace.chooseOutputFormat(.images)
        workspace.pageOutput.selectedOnly = true
        workspace.edit("Arrange", undo: nil) { $0[0].rotation = 90; $0[0].splitAfter = true }
        workspace.selectPages([workspace.pages[0].id, workspace.pages[2].id])
        try await waitForPlan(workspace)
        let request = try XCTUnwrap(workspace.plan?.request)
        guard case .pdfExtractImages(let pages) = request.operation else { return XCTFail("Expected embedded image extraction") }
        XCTAssertEqual(pages, [.init(sourceID: pdfJob.id.uuidString, pageIndex: 0)])
        XCTAssertEqual(request.assets.map(\.id), [pdfJob.id.uuidString])
        XCTAssertEqual(request.output.format, .images)
        XCTAssertEqual(request.output.cardinality, .directory)
        XCTAssertEqual(request.output.destination.lastPathComponent, "Embedded images")
        XCTAssertEqual(workspace.excludedImagePageCount, 1)
        XCTAssertEqual(workspace.extractionDetails?.discoveredCount, 0)
        XCTAssertFalse(workspace.canRun, "A successful inspection with no embedded images must not enable extraction")
        workspace.selectPages([workspace.pages[1].id])
        XCTAssertNil(workspace.plan, "Changing selected PDF pages invalidates the approved search synchronously")
        try await waitForPlan(workspace)
        guard case .pdfExtractImages(let revised) = workspace.plan?.request.operation else { return XCTFail("Expected revised search") }
        XCTAssertEqual(revised, [.init(sourceID: pdfJob.id.uuidString, pageIndex: 1)])
        workspace.selectPages([workspace.pages[2].id])
        XCTAssertNil(workspace.plan)
        XCTAssertFalse(workspace.canRun)
        XCTAssertTrue(workspace.error?.contains("PDF page") == true)
        pdfJob.inspection = nil; pdfJob.state = .missing
        workspace.selectPages([workspace.pages[0].id])
        XCTAssertNil(workspace.plan)
        XCTAssertFalse(workspace.canRun)
        XCTAssertEqual(workspace.excludedImagePageCount, 0, "An unavailable source must not be treated as a standalone image")
        XCTAssertTrue(workspace.error?.contains("unavailable") == true)
        let encoded = try JSONEncoder().encode(workspace.pageOutput)
        XCTAssertEqual(try JSONDecoder().decode(PDFPageOutputOptions.self, from: encoded), workspace.pageOutput)
        XCTAssertTrue(workspace.pageOutput.isValidRecord)
    }

    func testPDFCompressionIsLosslessUntilImageChangesAreExplicitlyEnabled() throws {
        let model = WorkspaceModel(), job = FileJob(input: URL(fileURLWithPath: "/tmp/fileform-pdf-compression.pdf"))
        job.inspection = .init(input: job.input, identity: .init(device: 0, inode: 0, bytes: 1000, modifiedSeconds: 0, modifiedNanoseconds: 0),
            family: .pdf, detectedType: "com.adobe.pdf", pageCount: 2)
        job.state = .ready; job.format = .pdf; job.goal = .fit; job.sizeLimitMB = "1.234567"
        let original = try model.transformationRequest(for: job)
        XCTAssertEqual(original.fidelity, .requireLossless)
        guard case .conversion(let lossless) = original.operation else { return XCTFail("Expected lossless default") }
        XCTAssertEqual(lossless.color, .preserve); XCTAssertEqual(lossless.metadata, .preserve)
        job.pdfCompression = .init(compressImages: true, quality: 0.73, minimumQuality: 0.41, resizeImages: true, longestEdge: "1200")
        let explicit = try model.transformationRequest(for: job)
        guard case .pdfOptimize(let parameters) = explicit.operation else { return XCTFail("Expected explicit image compression") }
        XCTAssertEqual(explicit.fidelity, .allowDeclaredLosses)
        XCTAssertEqual(parameters.maximumBytes, 1_234_567)
        XCTAssertEqual(parameters.quality, 0.73); XCTAssertEqual(parameters.minimumQuality, 0.41)
        XCTAssertEqual(parameters.maximumImageDimension, 1200)
        job.pdfCompression.longestEdge = "0"
        XCTAssertThrowsError(try model.transformationRequest(for: job))
        job.pdfCompression.compressImages = false
        XCTAssertEqual(try model.transformationRequest(for: job).operation, original.operation)
        let draft = FileJobDraft(job)
        var record = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(draft)) as? [String: Any])
        record.removeValue(forKey: "pdfCompression")
        let legacy = try JSONDecoder().decode(FileJobDraft.self, from: JSONSerialization.data(withJSONObject: record))
        let restored = FileJob(input: job.input); legacy.apply(to: restored)
        XCTAssertEqual(restored.pdfCompression, .init(), "Older sessions must keep lossless PDF compression")
    }

    func testRangeArrangementAndEverySplitPreserveSourcesRotationsAndUndo() async throws {
        let directory = try makeDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let input = directory.appendingPathComponent("five.pdf")
        try writePDF(to: input, pages: 5)
        let engine = ConversionEngine(), job = FileJob(input: input)
        job.inspection = try await engine.inspect(input); job.state = .ready
        let workspace = PDFWorkspaceModel(engine: engine)
        workspace.include([job], destination: directory)
        workspace.edit("Rotate", undo: nil) { $0[1].rotation = 90 }
        let original = workspace.pages
        let undo = UndoManager(); undo.groupsByEvent = false
        undo.beginUndoGrouping(); try workspace.splitEvery(2, undo: undo); undo.endUndoGrouping()
        XCTAssertEqual(workspace.pages.map(\.splitAfter), [false,true,false,true,false])
        XCTAssertEqual(workspace.pages.map(\.id), original.map(\.id))
        undo.undo(); XCTAssertEqual(workspace.pages, original)
        undo.beginUndoGrouping(); try workspace.applyPageRanges("2-3; 5,2,5", undo: undo); undo.endUndoGrouping()
        XCTAssertEqual(workspace.pages.map(\.pageIndex), [1,2,4,1,4])
        XCTAssertEqual(workspace.pages.map(\.rotation), [90,0,0,90,0])
        XCTAssertEqual(workspace.pages.map(\.splitAfter), [false,true,false,false,false])
        XCTAssertEqual(Set(workspace.pages.map(\.id)).count, 5)
        try await waitForPlan(workspace)
        guard case .pdfSplit(let groups) = workspace.plan?.request.operation else { return XCTFail("Expected shared PDF split operation") }
        XCTAssertEqual(groups.map { $0.map(\.pageIndex) }, [[1,2],[4,1,4]])
        let arranged = workspace.pages
        XCTAssertThrowsError(try workspace.applyPageRanges("1;99", undo: undo))
        XCTAssertEqual(workspace.pages, arranged, "Invalid ranges must not partially mutate the draft")
        undo.undo(); XCTAssertEqual(workspace.pages, original)
        undo.redo(); XCTAssertEqual(workspace.pages, arranged)
    }

    func testRemovingSourceInvalidatesUndoSnapshotsAndMissingSourceCannotPlan() {
        let workspace = PDFWorkspaceModel(engine: ConversionEngine())
        let job = sampleImageJob(); job.state = .ready
        workspace.include([job], destination: nil)
        let undo = UndoManager(); undo.groupsByEvent = false
        undo.beginUndoGrouping()
        workspace.edit("Rotate", undo: undo) { $0[0].rotation = 90 }
        undo.endUndoGrouping()
        XCTAssertTrue(undo.canUndo)
        workspace.removeSource(job.id.uuidString)
        XCTAssertFalse(undo.canUndo)
        XCTAssertTrue(workspace.pages.isEmpty)
        job.state = .missing
        workspace.include([job], destination: URL(fileURLWithPath: "/tmp"))
        XCTAssertNil(workspace.plan)
        XCTAssertFalse(workspace.canRun)
        XCTAssertNotNil(workspace.error)
    }

    private func sampleImageJob() -> FileJob {
        let job = FileJob(input: URL(fileURLWithPath: "/tmp/fileform-editing-source.png"))
        job.inspection = .init(input: job.input,
                               identity: .init(device: 0, inode: 0, bytes: 100, modifiedSeconds: 0, modifiedNanoseconds: 0),
                               family: .image, detectedType: "public.png", width: 320, height: 240)
        return job
    }
    private func makeDirectory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("fileform-editing-tests-\(UUID())")
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }
    private func writePDF(to url: URL, pages: Int) throws {
        var box = CGRect(x: 0, y: 0, width: 200, height: 300)
        let context = try XCTUnwrap(CGContext(url as CFURL, mediaBox: &box, nil))
        for index in 0..<pages {
            context.beginPDFPage(nil)
            context.setFillColor(CGColor(gray: CGFloat(index + 1) / CGFloat(pages + 1), alpha: 1))
            context.fill(CGRect(x: 20, y: 20, width: 100, height: 120))
            context.endPDFPage()
        }
        context.closePDF()
    }
    private func writeImage(to url: URL) throws {
        let bitmap = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 32, pixelsHigh: 24,
                                                   bitsPerSample: 8, samplesPerPixel: 3, hasAlpha: false,
                                                   isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
        let color = try XCTUnwrap(NSColor.systemTeal.usingColorSpace(.deviceRGB))
        for y in 0..<24 { for x in 0..<32 { bitmap.setColor(color, atX: x, y: y) } }
        try XCTUnwrap(bitmap.representation(using: .png, properties: [:])).write(to: url)
    }
    private func waitForPlan(_ workspace: PDFWorkspaceModel) async throws {
        let deadline = ContinuousClock.now.advanced(by: .seconds(5))
        while workspace.plan == nil && workspace.error == nil && ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        XCTAssertNil(workspace.error)
        XCTAssertNotNil(workspace.plan, "The real inspected composition should produce a plan")
    }
}
