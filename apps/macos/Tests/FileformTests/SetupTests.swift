import AppKit
import CoreGraphics
import XCTest
@testable import Fileform
import FileformCore
import FileformDomain

@MainActor final class SetupTests: XCTestCase {
    func testPageImageSetupRetainsOrderedDuplicatesResolutionAndQuality() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let image = directory.appendingPathComponent("source.png")
        try writeImage(to: image)
        let worker = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("../../.build/debug/fileform-worker").standardizedFileURL
        let engine = ConversionEngine(workerExecutable: worker)
        let source = try await job(image, engine: engine)
        let model = WorkspaceModel(engine: engine); model.jobs = [source]; model.selection = source.id; model.outputFolder = directory
        let pages = [PageReference(sourceID: "source", pageIndex: 0, clockwiseRotation: 90),
                     PageReference(sourceID: "source", pageIndex: 0)]
        let request = try TransformationRequest(assets: [.init(id: "source", url: image)],
            operation: .pdfRasterize(pages: pages, dpi: 300, quality: 0.72),
            output: .init(destination: directory.appendingPathComponent("Images"), format: .jpeg, cardinality: .directory), collisionPolicy: .rename)
        let recipe = try TransformationRecipe(name: "JPEG page images", request: request)
        XCTAssertTrue(recipe.isCompositionSetup)
        let review = try await model.reviewSetup(recipe, sourceIDs: [source.id], outputName: "Images")
        try model.applySetup(review)
        XCTAssertEqual(model.workspaceTask, .pages)
        XCTAssertEqual(model.pdfWorkspace.pageOutput, .init(format: .jpeg, dpi: "300", quality: 0.72, selectedOnly: false))
        XCTAssertEqual(model.pdfWorkspace.pages.map(\.pageIndex), [0, 0])
        XCTAssertEqual(model.pdfWorkspace.pages.map(\.rotation), [90, 0])
        XCTAssertEqual(model.pdfWorkspace.outputCount, 2)
        XCTAssertEqual(model.pdfWorkspace.plan?.request.output.format, .jpeg)
        XCTAssertEqual(model.pdfWorkspace.plan?.request.output.cardinality, .directory)
        XCTAssertEqual(model.pdfWorkspace.plan?.request.output.destination.lastPathComponent, "Images")
    }

    func testEmbeddedImageSetupAppliesAndExtractsOriginalSizePixels() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let core = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("../..").standardizedFileURL
        let pack = core.appendingPathComponent("Artifacts/PDFPack")
        guard FileManager.default.fileExists(atPath: pack.path) else { throw XCTSkip("Requires the verified PDF engine pack") }
        let engine = ConversionEngine(pdfPack: pack, workerExecutable: core.appendingPathComponent(".build/debug/fileform-worker"))
        let pdf = directory.appendingPathComponent("source.pdf")
        try writeEmbeddedRGBPDF(to: pdf)
        let original = try Data(contentsOf: pdf)
        let source = try await job(pdf, engine: engine)
        let model = WorkspaceModel(engine: engine)
        let store = WorkspaceStore(url: directory.appendingPathComponent("session/workspace.json"))
        await model.startPersistence(store: store)
        model.jobs = [source]; model.selection = source.id; model.outputFolder = directory
        let request = try TransformationRequest(assets: [.init(id: "source", url: pdf)],
            operation: .pdfExtractImages(pages: [.init(sourceID: "source", pageIndex: 0)]),
            output: .init(destination: directory.appendingPathComponent("Extracted"), format: .images, cardinality: .directory), collisionPolicy: .rename)
        let recipe = try TransformationRecipe(name: "Embedded images", request: request)
        let review = try await model.reviewSetup(recipe, sourceIDs: [source.id], outputName: "Extracted")
        try model.applySetup(review)
        XCTAssertEqual(model.workspaceTask, .pages)
        XCTAssertEqual(model.pdfWorkspace.pageOutput.format, .images)
        XCTAssertEqual(model.pdfWorkspace.outputCount, 1)
        XCTAssertTrue(model.pdfWorkspace.canRun)
        model.pdfWorkspace.run()
        let deadline = ContinuousClock.now.advanced(by: .seconds(15))
        while model.pdfWorkspace.isRunning && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(20)) }
        XCTAssertFalse(model.pdfWorkspace.isRunning)
        XCTAssertNil(model.pdfWorkspace.error)
        let artifact = try XCTUnwrap(model.pdfWorkspace.completedArtifacts.first)
        let extracted = try XCTUnwrap(NSBitmapImageRep(data: Data(contentsOf: artifact.url)))
        XCTAssertEqual(extracted.pixelsWide, 32); XCTAssertEqual(extracted.pixelsHigh, 24)
        XCTAssertEqual(artifact.pdfEmbeddedImage?.encodingOutcome, .reconstructedPixels)
        XCTAssertEqual(artifact.pdfEmbeddedImage?.resourcePages.map(\.pageIndex), [0])
        XCTAssertEqual(artifact.pdfEmbeddedImage?.sourceID, source.id.uuidString, "Restored provenance must identify the workspace source, not a portable setup slot")
        XCTAssertEqual(try Data(contentsOf: pdf), original)
        model.saveSession()
        let restored = WorkspaceModel(engine: engine)
        await restored.startPersistence(store: WorkspaceStore(url: store.url))
        XCTAssertEqual(restored.pdfWorkspace.pageOutput.format, .images)
        XCTAssertEqual(restored.pdfWorkspace.completedArtifacts.first?.pdfEmbeddedImage, artifact.pdfEmbeddedImage)
        XCTAssertEqual(restored.pdfWorkspace.completedArtifacts.first?.url, artifact.url)
        XCTAssertFalse(restored.pdfWorkspace.isRunning)
    }

    func testPDFImageCompressionSetupPreservesParametersAndResultAcrossRestart() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let core = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("../..").standardizedFileURL
        let pack = core.appendingPathComponent("Artifacts/PDFPack")
        guard FileManager.default.fileExists(atPath: pack.path) else { throw XCTSkip("Requires the verified PDF engine pack") }
        let engine = ConversionEngine(pdfPack: pack, workerExecutable: core.appendingPathComponent(".build/debug/fileform-worker"))
        let pdf = directory.appendingPathComponent("source.pdf")
        try writeEmbeddedRGBPDF(to: pdf)
        let original = try Data(contentsOf: pdf), source = try await job(pdf, engine: engine)
        let model = WorkspaceModel(engine: engine), store = WorkspaceStore(url: directory.appendingPathComponent("session/workspace.json"))
        await model.startPersistence(store: store)
        model.jobs = [source]; model.selection = source.id; model.outputFolder = directory
        let parameters = PDFOptimizationParameters(goal: .fit, quality: 0.63, minimumQuality: 0.42,
            maximumImageDimension: 16, maximumBytes: 100_000)
        let request = try TransformationRequest(assets: [.init(id: "pdf", url: pdf)], operation: .pdfOptimize(parameters: parameters),
            output: .init(destination: directory.appendingPathComponent("smaller.pdf"), format: .pdf), collisionPolicy: .rename)
        let recipe = try TransformationRecipe(name: "Smaller PDF photos", request: request)
        let review = try await model.reviewSetup(recipe, sourceIDs: [source.id], outputName: "Unused")
        try model.applySetup(review)
        XCTAssertEqual(model.workspaceTask, .convert)
        XCTAssertTrue(source.pdfCompression.compressImages)
        XCTAssertEqual(source.pdfCompression.quality, 0.63)
        XCTAssertEqual(source.pdfCompression.minimumQuality, 0.42)
        XCTAssertEqual(source.pdfCompression.longestEdge, "16")
        XCTAssertEqual(source.sizeLimitMB, "0.1")
        XCTAssertEqual(try model.transformationRequest(for: source).operation, request.operation)
        XCTAssertTrue(model.canRun)
        source.pdfCompression.minimumQuality = 0.4
        XCTAssertFalse(model.canRun, "An edited quality floor cannot execute the stale approved plan")
        source.pdfCompression.minimumQuality = 0.42
        model.runReadyJobs()
        let deadline = ContinuousClock.now.advanced(by: .seconds(20))
        while model.isRunning && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(20)) }
        XCTAssertFalse(model.isRunning)
        XCTAssertEqual(source.state, .completed, source.error ?? "Expected a complete optimized PDF")
        let output = try XCTUnwrap(source.outputs.first)
        XCTAssertLessThanOrEqual(try Data(contentsOf: output).count, 100_000)
        let report = try XCTUnwrap(source.lastPDFOptimizationDetails)
        XCTAssertEqual(report.optimizedImageCount, 1)
        XCTAssertEqual(report.resampledImageCount, 1)
        XCTAssertGreaterThanOrEqual(try XCTUnwrap(report.usedQuality), 0.42)
        XCTAssertEqual(report.candidates.first?.outputWidth, 16)
        XCTAssertEqual(report.candidates.first?.outputHeight, 12)
        XCTAssertEqual(try Data(contentsOf: pdf), original)
        model.saveSession()
        let restored = WorkspaceModel(engine: engine)
        await restored.startPersistence(store: WorkspaceStore(url: store.url))
        let reopened = try XCTUnwrap(restored.jobs.first)
        XCTAssertEqual(reopened.pdfCompression, source.pdfCompression)
        XCTAssertEqual(reopened.lastPDFOptimizationDetails, report)
        XCTAssertEqual(reopened.outputs, [output])
        XCTAssertFalse(restored.isRunning)
        let reapplied = try await restored.reviewSetup(recipe, sourceIDs: [reopened.id], outputName: "Unused")
        try restored.applySetup(reapplied)
        XCTAssertEqual(reopened.state, .ready)
        XCTAssertTrue(restored.canRun, "Applying to a completed source should prepare another output")
        XCTAssertEqual(reopened.outputs, [output])
        reopened.sizeLimitMB = "0.000001"; reopened.outputName = "impossible"
        await restored.refreshPlan(reopened)
        restored.runReadyJobs()
        let missDeadline = ContinuousClock.now.advanced(by: .seconds(20))
        while restored.isRunning && ContinuousClock.now < missDeadline { try await Task.sleep(for: .milliseconds(20)) }
        XCTAssertFalse(restored.isRunning)
        XCTAssertEqual(reopened.state, .failed)
        XCTAssertEqual(reopened.lastPDFOptimizationDetails?.attemptedQualities.count, 6)
        XCTAssertEqual(reopened.lastPDFOptimizationDetails?.attemptedQualities.last, 0.42)
        XCTAssertNil(reopened.lastPDFOptimizationDetails?.usedQuality)
        XCTAssertEqual(reopened.outputs, [output])
        XCTAssertFalse(FileManager.default.fileExists(atPath: directory.appendingPathComponent("impossible.pdf").path))
        XCTAssertEqual(try Data(contentsOf: pdf), original)
    }

    func testLibraryRevisionsPortableRoundTripAndExclusiveExportProtectSources() throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let input = directory.appendingPathComponent("private source.png")
        try Data("original bytes".utf8).write(to: input)
        let request = try request(input: input, directory: directory)
        let library = SetupLibrary(); let store = directory.appendingPathComponent("library/setups.json")
        library.connect(to: store)
        let original = try library.save(name: "Screen image", request: request)
        try library.rename(original.id, to: "Email image")
        let renamed = try XCTUnwrap(library.recipe(original.id))
        XCTAssertEqual(renamed.revision, 2)
        XCTAssertEqual(renamed.operation, original.operation)
        let updated = try library.save(name: renamed.name, request: request, replacing: original.id)
        XCTAssertEqual(updated.revision, 3)
        let reloaded = SetupLibrary(); reloaded.connect(to: store)
        XCTAssertNil(reloaded.storageError)
        XCTAssertEqual(reloaded.recipes.first?.name, "Email image")
        let output = directory.appendingPathComponent("portable.json")
        try SetupLibrary.export(updated, to: output)
        let encoded = try String(contentsOf: output, encoding: .utf8)
        XCTAssertFalse(encoded.contains(input.lastPathComponent))
        XCTAssertFalse(encoded.contains(directory.path))
        XCTAssertFalse(encoded.contains("bookmark"))
        let portable = try SetupLibrary.readRecipe(at: output)
        XCTAssertEqual(portable.id, original.id)
        XCTAssertEqual(portable.operation, request.operation)
        XCTAssertEqual(portable.collisionPolicy, .rename)
        let alias = directory.appendingPathComponent("source alias.json")
        try FileManager.default.linkItem(at: input, to: alias)
        XCTAssertThrowsError(try SetupLibrary.export(updated, to: input))
        XCTAssertThrowsError(try SetupLibrary.export(updated, to: alias))
        XCTAssertEqual(try Data(contentsOf: input), Data("original bytes".utf8))
        let savedBytes = try Data(contentsOf: store)
        XCTAssertThrowsError(try library.importRecipe(original, replaceExisting: true))
        XCTAssertEqual(try Data(contentsOf: store), savedBytes)
        try library.remove(original.id)
        XCTAssertTrue(library.recipes.isEmpty)
        XCTAssertTrue(FileManager.default.fileExists(atPath: input.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: output.path))
    }

    func testCorruptFutureAndOversizedSetupDataIsRejectedWithoutReplacement() throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = directory.appendingPathComponent("setups.json")
        let bytes = Data("{\"schemaVersion\":999,\"recipes\":[]}".utf8)
        try bytes.write(to: store)
        let library = SetupLibrary(); library.connect(to: store)
        XCTAssertNotNil(library.storageError)
        XCTAssertThrowsError(try library.save(name: "New", request: request(input: directory.appendingPathComponent("input.png"), directory: directory)))
        XCTAssertEqual(try Data(contentsOf: store), bytes)
        let oversized = directory.appendingPathComponent("large.json")
        try Data(repeating: 32, count: 1_048_577).write(to: oversized)
        XCTAssertThrowsError(try SetupLibrary.readRecipe(at: oversized)) { error in
            XCTAssertEqual((error as? FileformError)?.code, .resourceLimit)
        }
    }

    func testMixedBatchReviewPreservesExactChoicesAndDoesNotExecuteOrMutateIncompatibleFiles() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let image = directory.appendingPathComponent("new image.png")
        try writeImage(to: image)
        let table = directory.appendingPathComponent("table.csv")
        try Data("a,b\n1,2\n".utf8).write(to: table)
        let engine = ConversionEngine()
        let imageJob = try await job(image, engine: engine)
        let tableJob = try await job(table, engine: engine)
        let model = WorkspaceModel(engine: engine); model.jobs = [imageJob, tableJob]; model.selection = imageJob.id; model.outputFolder = directory
        let operation = TransformationOperation.imageCrop(rectangle: .init(x: 2, y: 3, width: 8, height: 6),
            conversion: .init(goal: .fit, options: .init(quality: 0.73, minimumQuality: 0.25, maxDimension: 4, maximumBytes: 1_000_001)))
        let recipe = try TransformationRecipe(name: "Exact choices", request: .init(assets: [.init(id: "picture", url: image)], operation: operation,
            output: .init(destination: directory.appendingPathComponent("template.png"), format: .png), collisionPolicy: .rename))
        let originalImage = SetupSourceSnapshot(imageJob); let originalTable = SetupSourceSnapshot(tableJob)
        let review = try await model.reviewSetup(recipe, sourceIDs: [imageJob.id, tableJob.id], outputName: "Unused.pdf")
        XCTAssertEqual(review.applicableCount, 1)
        XCTAssertEqual(review.issues.map(\.id), [tableJob.id])
        XCTAssertEqual(SetupSourceSnapshot(imageJob), originalImage)
        XCTAssertEqual(SetupSourceSnapshot(tableJob), originalTable)
        try model.applySetup(review)
        XCTAssertEqual(imageJob.sizeLimitMB, "1.000001")
        XCTAssertEqual(imageJob.quality, 0.73)
        XCTAssertEqual(imageJob.minimumQuality, 0.25)
        XCTAssertEqual(try model.transformationRequest(for: imageJob).operation, operation)
        XCTAssertEqual(SetupSourceSnapshot(tableJob), originalTable)
        XCTAssertFalse(model.isRunning)
        let destination = directory.appendingPathComponent("new image-converted.png")
        XCTAssertFalse(FileManager.default.fileExists(atPath: destination.path))
        let result = try await engine.run(XCTUnwrap(imageJob.plan))
        let produced = try XCTUnwrap(result.artifacts.first?.url)
        let info = try await engine.inspect(produced)
        XCTAssertEqual(info.width, 4); XCTAssertEqual(info.height, 3)
    }

    func testChangedDraftRejectsPreviouslyReviewedApplyWithoutPartialMutation() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let a = directory.appendingPathComponent("a.png"), b = directory.appendingPathComponent("b.png")
        try writeImage(to: a); try writeImage(to: b)
        let engine = ConversionEngine()
        let first = try await job(a, engine: engine), second = try await job(b, engine: engine)
        let model = WorkspaceModel(engine: engine); model.jobs = [first, second]; model.outputFolder = directory
        let recipe = try TransformationRecipe(name: "Resize", request: request(input: a, directory: directory))
        let review = try await model.reviewSetup(recipe, sourceIDs: [first.id, second.id], outputName: "Unused.pdf")
        XCTAssertEqual(review.applicableCount, 2)
        let before = SetupSourceSnapshot(first)
        second.quality = 0.5
        XCTAssertThrowsError(try model.applySetup(review))
        XCTAssertEqual(SetupSourceSnapshot(first), before)
        XCTAssertEqual(second.quality, 0.5)
    }

    func testPDFSetupRebindsSourcesAndPreservesSingleDirectoryAndCollisionPolicy() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let a = directory.appendingPathComponent("a.png"), b = directory.appendingPathComponent("b.png")
        try writeImage(to: a); try writeImage(to: b)
        let engine = ConversionEngine()
        let first = try await job(a, engine: engine), second = try await job(b, engine: engine)
        let model = WorkspaceModel(engine: engine); model.jobs = [first, second]; model.outputFolder = directory
        let recipe = try TransformationRecipe(name: "Rotated pair in a folder", request: .init(
            assets: [.init(id: "first", url: a), .init(id: "second", url: b)],
            operation: .pdfSplit(groups: [[.init(sourceID: "second", pageIndex: 0, clockwiseRotation: 90), .init(sourceID: "first", pageIndex: 0)]]),
            output: .init(destination: directory.appendingPathComponent("template folder"), format: .pdf, cardinality: .directory), collisionPolicy: .fail))
        let review = try await model.reviewSetup(recipe, sourceIDs: [second.id, first.id], outputName: "Bound.pdf")
        try model.applySetup(review)
        XCTAssertEqual(model.pdfWorkspace.pages.map(\.sourceID), [first.id.uuidString, second.id.uuidString])
        XCTAssertEqual(model.pdfWorkspace.pages.first?.rotation, 90)
        let arranged = model.pdfWorkspace.pages
        model.chooseTask(.convert); model.chooseTask(.pages)
        XCTAssertEqual(model.pdfWorkspace.pages, arranged)
        XCTAssertTrue(model.pdfWorkspace.exportAsDirectory)
        XCTAssertEqual(model.pdfWorkspace.collisionPolicy, .fail)
        model.pdfWorkspace.refresh()
        try await waitForPDFPlan(model.pdfWorkspace)
        let rebound = try XCTUnwrap(model.pdfWorkspace.plan)
        XCTAssertEqual(rebound.request.output.cardinality, .directory)
        XCTAssertEqual(rebound.request.collisionPolicy, .fail)
        guard case .pdfSplit(let groups) = rebound.request.operation else { return XCTFail("A one-group directory setup must remain a split directory") }
        XCTAssertEqual(groups.count, 1)
        let result = try await engine.run(rebound)
        XCTAssertEqual(result.artifacts.count, 1)
        XCTAssertEqual(result.artifacts.first?.url.deletingLastPathComponent().lastPathComponent, "Bound pages")
    }

    func testPDFDraftKeepsItsSourceSelectionAcrossTasksAndExplicitImports() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let first = directory.appendingPathComponent("first.png"), second = directory.appendingPathComponent("second.png")
        try writeImage(to: first); try writeImage(to: second)
        let model = WorkspaceModel(engine: ConversionEngine())
        model.addFiles([first])
        model.chooseTask(.pages) // First PDF opening includes the still-inspecting source.
        var deadline = ContinuousClock.now.advanced(by: .seconds(5))
        while model.pdfWorkspace.pages.isEmpty && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
        XCTAssertEqual(model.pdfWorkspace.pages.count, 1)
        let originalPages = model.pdfWorkspace.pages
        model.chooseTask(.convert)
        model.addFiles([second])
        deadline = ContinuousClock.now.advanced(by: .seconds(5))
        while model.jobs.last?.state == .inspecting && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
        model.chooseTask(.pages)
        XCTAssertEqual(model.pdfWorkspace.pages, originalPages, "Returning to Pages must retain the draft, not add unrelated conversion files")
        model.addFiles([second]) // Explicit native import can reuse a file already in the queue.
        XCTAssertEqual(model.pdfWorkspace.pages.count, 2)
        model.chooseTask(.convert); model.chooseTask(.pages)
        XCTAssertEqual(model.pdfWorkspace.pages.count, 2)
    }

    func testFailedRequestedPDFSourceBlocksUntilExplicitSkip() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let good = directory.appendingPathComponent("good.png"), bad = directory.appendingPathComponent("bad.png")
        try writeImage(to: good); try Data("not an image".utf8).write(to: bad)
        let model = WorkspaceModel(engine: ConversionEngine()); model.outputFolder = directory
        model.chooseTask(.pages); model.addFiles([good, bad])
        let deadline = ContinuousClock.now.advanced(by: .seconds(5))
        while (model.jobs.contains { $0.state == .inspecting } || model.pdfWorkspace.plan == nil) && ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        XCTAssertEqual(model.pdfWorkspace.pages.count, 1)
        XCTAssertNotNil(model.pdfWorkspace.plan)
        XCTAssertFalse(model.canRun)
        let failed = try XCTUnwrap(model.jobs.first { $0.input == bad })
        XCTAssertEqual(model.pendingPDFSourceJobs.map(\.id), [failed.id])
        model.skipPendingPDFImport(failed.id)
        XCTAssertTrue(model.canRun)
        XCTAssertEqual(failed.state, .failed)
        XCTAssertEqual(try Data(contentsOf: bad), Data("not an image".utf8))
    }

    func testOversizedPDFReviewFailsBeforeReplacingDraft() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = directory.appendingPathComponent("image.png"); try writeImage(to: source)
        let engine = ConversionEngine(); let input = try await job(source, engine: engine)
        let model = WorkspaceModel(engine: engine); model.jobs = [input]; model.outputFolder = directory
        let recipe = try TransformationRecipe(name: "Too many pages", request: .init(assets: [.init(id: "source", url: source)],
            operation: .pdfComposition(pages: Array(repeating: .init(sourceID: "source", pageIndex: 0), count: 1001)),
            output: .init(destination: directory.appendingPathComponent("many.pdf"), format: .pdf)))
        let before = PDFSetupSnapshot(model.pdfWorkspace)
        do { _ = try await model.reviewSetup(recipe, sourceIDs: [input.id], outputName: "Many.pdf"); XCTFail("The shared plan must reject more than 1000 pages") }
        catch { XCTAssertEqual(PDFSetupSnapshot(model.pdfWorkspace), before) }
    }

    func testInspectorSaveRequestUsesTheInspectedFileNotSelection() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let a = directory.appendingPathComponent("a.png"), b = directory.appendingPathComponent("b.png")
        try writeImage(to: a); try writeImage(to: b)
        let engine = ConversionEngine(); let first = try await job(a, engine: engine), second = try await job(b, engine: engine)
        let model = WorkspaceModel(engine: engine); model.jobs = [first, second]; model.selection = first.id
        first.format = .jpeg; second.format = .png; second.resize = true; second.longestEdge = "12"
        let request = try XCTUnwrap(model.setupRequest(for: second))
        XCTAssertEqual(request.assets.first?.url, second.input)
        XCTAssertEqual(request.output.format, .png)
        XCTAssertNotEqual(request.output.format, model.currentSetupRequest?.output.format)
    }

    func testSelectedExecutionUsesFreshNamesAndCapturesItsFiles() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let engine = ConversionEngine(), model = WorkspaceModel()
        var files: [FileJob] = []
        for name in ["a", "b", "c"] {
            let input = directory.appendingPathComponent(name + ".png")
            try writeImage(to: input)
            files.append(try await job(input, engine: engine))
        }
        model.jobs = files; model.outputFolder = directory
        for file in files { await model.refreshPlan(file) }
        model.selectBatch([files[0].id, files[1].id])
        XCTAssertTrue(model.runSelectionOnly); XCTAssertEqual(model.readyCount, 2)
        files[0].outputName = "Chosen output"
        XCTAssertEqual(model.readyCount, 1, "A changed name cannot execute a stale plan")
        await model.refreshPlan(files[0])
        XCTAssertEqual(model.readyCount, 2)
        model.runReadyJobs()
        model.selectBatch([files[2].id])
        let deadline = ContinuousClock.now.advanced(by: .seconds(5))
        while model.isRunning && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
        XCTAssertFalse(model.isRunning)
        XCTAssertEqual(files[0].outputs.last?.lastPathComponent, "Chosen output.jpg")
        XCTAssertEqual(files[1].outputs.last?.lastPathComponent, "b-converted.jpg")
        XCTAssertTrue(files[2].outputs.isEmpty)
        XCTAssertFalse(FileManager.default.fileExists(atPath: directory.appendingPathComponent("c-converted.jpg").path))
    }
    func testSharedChoicesPreserveEachFilesCustomName() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let engine = ConversionEngine(), model = WorkspaceModel()
        var files: [FileJob] = []
        for name in ["a", "b"] {
            let input = directory.appendingPathComponent(name + ".png")
            try writeImage(to: input)
            let file = try await job(input, engine: engine)
            file.outputName = "custom-" + name
            files.append(file)
        }
        model.jobs = files; model.outputFolder = directory
        let recipe = try TransformationRecipe(name: "Shared", request: request(input: files[0].input, directory: directory))
        let review = try await model.reviewSetup(recipe, sourceIDs: files.map(\.id), outputName: "")
        XCTAssertEqual(review.changes.map { $0.plan.request.output.destination.lastPathComponent }, ["custom-a.png", "custom-b.png"])
        try model.applySetup(review)
        XCTAssertEqual(files.map(\.outputName), ["custom-a", "custom-b"])
        XCTAssertEqual(model.readyCount, 2)
    }

    func testCancelledViewPlanningDoesNotTurnAFileIntoFailure() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let input = directory.appendingPathComponent("focus.png")
        try writeImage(to: input)
        let engine = ConversionEngine(), model = WorkspaceModel()
        let file = try await job(input, engine: engine)
        model.jobs = [file]
        let task = Task { withUnsafeCurrentTask { $0?.cancel() }; await model.refreshPlan(file) }
        await task.value
        XCTAssertEqual(file.state, .ready)
        XCTAssertNil(file.error)
        await model.ensurePlan(file)
        XCTAssertNotNil(file.plan)
        XCTAssertEqual(file.state, .ready)
    }

    private func request(input: URL, directory: URL) throws -> TransformationRequest {
        try .init(assets: [.init(id: "source", url: input)], operation: .conversion(.init(options: .init(maxDimension: 12))),
            output: .init(destination: directory.appendingPathComponent("template.png"), format: .png), collisionPolicy: .rename)
    }
    func testSizeShortcutSurvivesImportAndDoesNotAffectLaterImports() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let first = directory.appendingPathComponent("first.png")
        let second = directory.appendingPathComponent("second.png")
        try writeImage(to: first); try writeImage(to: second)
        let model = WorkspaceModel(engine: ConversionEngine())
        model.startConversion(goal: .fit)
        XCTAssertEqual(model.pendingImportGoal, .fit)
        model.addFiles([first])
        XCTAssertNil(model.pendingImportGoal)
        let deadline = ContinuousClock.now + .seconds(10)
        while model.jobs.first?.state == .inspecting && ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        let file = try XCTUnwrap(model.jobs.first)
        XCTAssertEqual(file.goal, .fit)
        XCTAssertTrue(file.availableGoals.contains(.fit))
        model.addFiles([second])
        while model.jobs.last?.state == .inspecting && ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        XCTAssertEqual(model.jobs.last?.goal, .convert)
        model.selection = file.id
        model.startConversion(goal: .compress)
        XCTAssertEqual(file.goal, .compress)
        XCTAssertNil(model.pendingImportGoal)
    }

    private func job(_ url: URL, engine: ConversionEngine) async throws -> FileJob {
        let job = FileJob(input: url); job.inspection = try await engine.inspect(url)
        job.capabilities = await engine.capabilities(for: job.inspection).filter(\.available)
        job.state = .ready
        return job
    }
    private func writeImage(to url: URL) throws {
        let bitmap = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 32, pixelsHigh: 24, bitsPerSample: 8,
            samplesPerPixel: 3, hasAlpha: false, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
        let color = try XCTUnwrap(NSColor.systemTeal.usingColorSpace(.deviceRGB))
        for y in 0..<24 { for x in 0..<32 { bitmap.setColor(color, atX: x, y: y) } }
        try XCTUnwrap(bitmap.representation(using: .png, properties: [:])).write(to: url)
    }
    private func writeEmbeddedRGBPDF(to url: URL) throws {
        // Explicit DeviceRGB avoids CoreGraphics introducing an ICC profile,
        // which is a separate, unsupported extraction contract.
        var samples = Data()
        for index in 0..<768 {
            samples.append(UInt8(index % 256))
            samples.append(UInt8((index / 32) * 10))
            samples.append(160)
        }
        let content = Data("q 128 0 0 96 30 30 cm /Image Do Q".utf8)
        func stream(_ header: String, bytes: Data) -> Data {
            var value = Data((header + "\nstream\n").utf8)
            value.append(bytes); value.append(Data("\nendstream".utf8))
            return value
        }
        let objects: [Data] = [
            Data("<< /Type /Catalog /Pages 2 0 R >>".utf8),
            Data("<< /Type /Pages /Count 1 /Kids [3 0 R] >>".utf8),
            Data("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Resources << /XObject << /Image 5 0 R >> >> /Contents 4 0 R >>".utf8),
            stream("<< /Length \(content.count) >>", bytes: content),
            stream("<< /Type /XObject /Subtype /Image /Width 32 /Height 24 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length \(samples.count) >>", bytes: samples)
        ]
        var pdf = Data("%PDF-1.4\n".utf8), offsets: [Int] = []
        for (index, object) in objects.enumerated() {
            offsets.append(pdf.count)
            pdf += Data("\(index + 1) 0 obj\n".utf8) + object + Data("\nendobj\n".utf8)
        }
        let xref = pdf.count
        pdf += Data("xref\n0 6\n0000000000 65535 f \n".utf8)
        for offset in offsets { pdf += Data(String(format: "%010d 00000 n \n", offset).utf8) }
        pdf += Data("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n\(xref)\n%%EOF\n".utf8)
        try pdf.write(to: url)
    }
    private func temporaryDirectory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("fileform-setup-tests-\(UUID())")
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }
    private func waitForPDFPlan(_ model: PDFWorkspaceModel) async throws {
        let deadline = ContinuousClock.now.advanced(by: .seconds(5))
        while model.plan == nil && model.error == nil && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
        XCTAssertNil(model.error); XCTAssertNotNil(model.plan)
    }
}
