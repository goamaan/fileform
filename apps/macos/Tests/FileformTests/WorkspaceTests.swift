import XCTest
@testable import Fileform
import FileformDomain

@MainActor final class WorkspaceTests: XCTestCase {
    private func imageJob() -> FileJob {
        let job = FileJob(input: URL(fileURLWithPath: "/tmp/fileform-original.png"))
        job.inspection = .init(input: job.input,
                               identity: .init(device: 0, inode: 0, bytes: 100, modifiedSeconds: 0, modifiedNanoseconds: 0),
                               family: .image, detectedType: "public.png", width: 320, height: 240)
        return job
    }
    func testExactDecimalSizeAndDistinctDestination() throws {
        let model = WorkspaceModel()
        let job = imageJob()
        job.goal = .fit; job.sizeLimitMB = "0.000001"; job.format = .png
        let request = try model.request(for: job)
        XCTAssertEqual(request.options.maximumBytes, 1)
        XCTAssertNotEqual(request.input, request.destination)
        XCTAssertEqual(request.collisionPolicy, .rename)
        job.sizeLimitMB = "0.0000001"
        XCTAssertThrowsError(try model.request(for: job))
    }
    func testInvalidSizeAndResizeAreNotSilentlyIgnored() {
        let model = WorkspaceModel()
        let job = imageJob()
        job.goal = .fit; job.sizeLimitMB = "-1"
        XCTAssertThrowsError(try model.request(for: job))
        job.sizeLimitMB = "12MB"
        XCTAssertThrowsError(try model.request(for: job))
        job.sizeLimitMB = "1.2.3"
        XCTAssertThrowsError(try model.request(for: job))
        job.goal = .convert; job.resize = true; job.longestEdge = "not a number"
        XCTAssertThrowsError(try model.request(for: job))
    }
    func testRunRequiresExplicitOutputFolderAndReadyPlan() {
        let model = WorkspaceModel()
        XCTAssertFalse(model.canRun)
        model.runReadyJobs()
        XCTAssertFalse(model.isRunning)
    }
    func testPDFCompressionKeepsAllPagesAndPreservationPolicy() throws {
        let model = WorkspaceModel()
        let job = FileJob(input: URL(fileURLWithPath: "/tmp/fileform-pages.pdf"), acquireAccess: false)
        job.inspection = .init(input: job.input,
            identity: .init(device: 0, inode: 0, bytes: 1000, modifiedSeconds: 0, modifiedNanoseconds: 0),
            family: .pdf, detectedType: "com.adobe.pdf", pageCount: 3)
        job.capabilities = [
            .init(format: .pdf, goals: [.convert], engine: "documents", available: true),
            .init(format: .pdf, goals: [.compress, .fit], engine: "qpdf", available: true)]
        job.format = .pdf; job.goal = .fit; job.sizeLimitMB = "0.004"; job.pdfPage = "2"
        XCTAssertEqual(job.availableFormats, [.pdf])
        XCTAssertEqual(job.availableGoals, [.convert, .compress, .fit])
        let request = try model.transformationRequest(for: job)
        XCTAssertEqual(request.fidelity, .requireLossless)
        guard case .conversion(let parameters) = request.operation else { return XCTFail("Expected PDF optimization") }
        XCTAssertEqual(parameters.color, .preserve)
        XCTAssertEqual(parameters.metadata, .preserve)
        XCTAssertEqual(parameters.options.maximumBytes, 4000)
        XCTAssertNil(parameters.options.pageNumber, "An earlier page-export choice must not reduce the document during compression")
        let recipe = try TransformationRecipe(name: "Small complete PDF", request: request)
        let draft = try FileJobDraft(recipe: recipe, job: job)
        draft.apply(to: job)
        XCTAssertEqual(try model.transformationRequest(for: job).operation, request.operation)
        XCTAssertEqual(try model.transformationRequest(for: job).fidelity, .requireLossless)
        job.goal = .convert
        job.pdfPage = "2"
        XCTAssertEqual(try model.request(for: job).options.pageNumber, 2)
    }
    func testRestorationPreventsTaskSwitchAndClearingTheLoadingWorkspace() {
        let model = WorkspaceModel(); let job = imageJob(); job.state = .completed
        model.jobs = [job]; model.restoringSession = true
        model.chooseTask(.pages); model.clearWorkspace(); model.remove(job)
        XCTAssertEqual(model.workspaceTask, .convert)
        XCTAssertEqual(model.jobs.map(\.id), [job.id])
        XCTAssertFalse(model.canRun)
    }
    func testOpenFilesBuffersTheWholeBatchUntilWindowIsReady() {
        let delegate = FileformAppDelegate()
        let inputs = [URL(fileURLWithPath: "/tmp/fileform-first.csv"), URL(fileURLWithPath: "/tmp/fileform-second.pdf")]
        delegate.application(NSApplication.shared, open: inputs)
        let model = WorkspaceModel()
        delegate.workspace = model
        XCTAssertEqual(model.jobs.map(\.input), inputs)
        delegate.workspace = model
        XCTAssertEqual(model.jobs.count, 2)
    }
}
