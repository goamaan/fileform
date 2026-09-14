import XCTest
@testable import Fileform
import FileformDomain

@MainActor final class ResultContinuationTests: XCTestCase {
    func testTrimResultStartsNewSourceAndRetainsRootAcrossChain() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let first = directory.appendingPathComponent("first.wav"), second = directory.appendingPathComponent("second.wav")
        try Data("saved first".utf8).write(to: first); try Data("saved second".utf8).write(to: second)
        let model = WorkspaceModel()
        let parent = FileJob(input: directory.appendingPathComponent("source.mp4"), acquireAccess: false)
        parent.state = .completed; parent.outputs = [first]
        parent.trimDraft = .init(interval: .init(start: .init(ticks: 10, timescale: 1), end: .init(ticks: 12, timescale: 1)), format: .wav, outputName: "first")
        model.jobs = [parent]
        model.trimResult(first, from: parent)
        let child = try XCTUnwrap(model.selectedJob)
        XCTAssertNotEqual(child.id, parent.id); XCTAssertEqual(child.input, first)
        XCTAssertNil(child.trimDraft); XCTAssertNil(child.trimPlan)
        XCTAssertEqual(child.origin?.rootJobID, parent.id); XCTAssertEqual(child.origin?.parentJobID, parent.id)
        XCTAssertEqual(parent.outputs, [first]); XCTAssertEqual(parent.trimDraft?.from, "0:10")
        child.outputs = [second]; child.state = .completed
        model.trimResult(second, from: child)
        let grandchild = try XCTUnwrap(model.selectedJob)
        XCTAssertEqual(grandchild.origin?.rootJobID, parent.id); XCTAssertEqual(grandchild.origin?.parentJobID, child.id)
        XCTAssertEqual(grandchild.input, second); XCTAssertEqual(model.workspaceTask, .trim)
        let bytes = try JSONEncoder().encode(grandchild.origin)
        XCTAssertEqual(try JSONDecoder().decode(ResultOrigin?.self, from: bytes), grandchild.origin)
        let count = model.jobs.count
        model.trimResult(first, from: parent)
        XCTAssertEqual(model.selectedJob?.id, child.id); XCTAssertEqual(model.jobs.count, count)
        XCTAssertEqual(try Data(contentsOf: first), Data("saved first".utf8))
        XCTAssertEqual(try Data(contentsOf: second), Data("saved second".utf8))
    }
    func testMissingOrUnownedResultCannotCreateContinuation() throws {
        let model = WorkspaceModel(), parent = FileJob(input: URL(fileURLWithPath: "/tmp/source.wav"), acquireAccess: false)
        let missing = URL(fileURLWithPath: "/tmp/never-created-\(UUID().uuidString).wav")
        model.jobs = [parent]; parent.outputs = [missing]
        model.trimResult(missing, from: parent)
        XCTAssertEqual(model.jobs.count, 1); XCTAssertNotNil(model.alert)
        model.trimResult(URL(fileURLWithPath: "/tmp/unrelated.wav"), from: parent)
        XCTAssertEqual(model.jobs.count, 1)
    }
}
