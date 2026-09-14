import XCTest
import AppKit
@testable import Fileform
import FileformDomain

@MainActor final class MediaTrimWorkspaceTests: XCTestCase {
    private func job() -> FileJob {
        let job = FileJob(input: URL(fileURLWithPath: "/tmp/recording.wav"), acquireAccess: false)
        job.inspection = .init(input: job.input,
            identity: .init(device: 0, inode: 0, bytes: 100, modifiedSeconds: 0, modifiedNanoseconds: 0),
            family: .media, detectedType: "wav")
        job.state = .ready; job.format = .m4a
        job.trimDraft = .init(interval: .init(start: .init(ticks: 0, timescale: 1), end: .init(ticks: 8008, timescale: 3000)), format: .wav, outputName: "clip")
        return job
    }
    func testDraftPersistenceAndInvalidEditInvalidatesExportAndUndoRestoresExactRange() throws {
        let model = WorkspaceModel(), job = job()
        model.jobs = [job]; model.selection = job.id; model.workspaceTask = .trim
        model.outputFolder = URL(fileURLWithPath: "/tmp/output")
        let original = try XCTUnwrap(job.trimDraft)
        XCTAssertEqual(Set((0..<100).map { _ in original.key }).count, 1, "Stable observation keys must not restart planning on redraw")
        let plan = TransformationPlan(request: try model.trimRequest(for: job), inputs: [], warnings: [])
        job.trimPlan = plan
        XCTAssertTrue(model.canRun)
        let saved = try JSONDecoder().decode(FileJobDraft.self, from: JSONEncoder().encode(FileJobDraft(job)))
        let restored = self.job(); saved.apply(to: restored)
        XCTAssertEqual(restored.trimDraft, original)
        XCTAssertEqual(restored.format, .m4a)
        let undo = UndoManager(); undo.groupsByEvent = false; undo.beginUndoGrouping()
        var invalid = original; invalid.from = "not a time"
        model.changeTrim(job, to: invalid, undo: undo); undo.endUndoGrouping()
        XCTAssertFalse(model.canRun); XCTAssertNil(job.trimPlan)
        XCTAssertThrowsError(try model.trimRequest(for: job))
        undo.undo()
        XCTAssertEqual(job.trimDraft, original)
        XCTAssertEqual(try job.trimDraft?.interval().end, original.originalInterval.end)
        XCTAssertFalse(model.canRun, "Undo requires fresh engine planning before export")
    }
    func testTrimSetupKeepsConversionChoicesAndDetectsStaleTrimEdits() throws {
        let model = WorkspaceModel(), job = job()
        model.jobs = [job]; model.selection = job.id; model.workspaceTask = .trim
        let request = try XCTUnwrap(model.currentSetupRequest)
        XCTAssertEqual(request.operation.id, .mediaTrim)
        let before = SetupSourceSnapshot(job)
        var changed = try XCTUnwrap(job.trimDraft); changed.to = "1.5"
        model.changeTrim(job, to: changed)
        XCTAssertNotEqual(SetupSourceSnapshot(job), before)
        XCTAssertEqual(job.format, .m4a)
        model.workspaceTask = .convert
        XCTAssertEqual(try XCTUnwrap(model.currentSetupRequest).operation.id, .conversion)
    }
}
