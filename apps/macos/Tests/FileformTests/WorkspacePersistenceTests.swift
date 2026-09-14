import AppKit
import CoreGraphics
import XCTest
@testable import Fileform
import FileformDomain

@MainActor final class WorkspacePersistenceTests: XCTestCase {
    func testRestartRecoversInterruptedDraftWithoutExecutingAndClearKeepsFiles() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let input = directory.appendingPathComponent("café 日本語.csv")
        let output = directory.appendingPathComponent("completed.json")
        try Data("name,value\nexample,1\n".utf8).write(to: input)
        try Data("[{\"name\":\"example\",\"value\":\"1\"}]".utf8).write(to: output)
        let store = WorkspaceStore(url: directory.appendingPathComponent("state/workspace.json"))
        let original = FileJob(input: input)
        original.format = .json; original.sizeLimitMB = "1.234567"
        original.longestEdge = "777"; original.crop = .init(enabled: true, x: "9", y: "8", width: "70", height: "60")
        let saved = WorkspaceJobRecord(id: original.id, source: try store.reference(input), draft: .init(original),
                                       state: .running, outputs: [try store.reference(output)])
        try store.save(record(jobs: [saved], destination: try store.reference(directory, readOnly: false)))
        let before = try FileManager.default.contentsOfDirectory(atPath: directory.path).sorted()
        let model = WorkspaceModel()
        await model.startPersistence(store: WorkspaceStore(url: store.url))
        let restored = try XCTUnwrap(model.jobs.first)
        XCTAssertEqual(restored.id, original.id)
        XCTAssertEqual(restored.state, .interrupted)
        XCTAssertEqual(restored.format, .json)
        XCTAssertEqual(restored.crop, original.crop)
        XCTAssertEqual(restored.sizeLimitMB, "1.234567")
        XCTAssertEqual(restored.outputs, [output])
        XCTAssertNotNil(restored.plan)
        XCTAssertFalse(model.isRunning)
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: directory.path).sorted(), before)
        restored.sizeLimitMB = "9.876543"
        model.saveSession()
        XCTAssertEqual(try XCTUnwrap(store.load()).jobs.first?.draft.sizeLimitMB, "9.876543")
        model.clearWorkspace()
        XCTAssertTrue(try XCTUnwrap(store.load()).jobs.isEmpty)
        XCTAssertTrue(FileManager.default.fileExists(atPath: input.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: output.path))
    }

    func testCancelledStateSurvivesReinspectionAndPlanning() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let input = directory.appendingPathComponent("cancelled.csv")
        try Data("a,b\n1,2\n".utf8).write(to: input)
        let store = WorkspaceStore(url: directory.appendingPathComponent("state.json"))
        let job = FileJob(input: input); job.format = .json
        let saved = WorkspaceJobRecord(id: job.id, source: try store.reference(input), draft: .init(job), state: .cancelled, outputs: [])
        try store.save(record(jobs: [saved]))
        let model = WorkspaceModel()
        await model.startPersistence(store: store)
        let restored = try XCTUnwrap(model.jobs.first)
        XCTAssertEqual(restored.state, .cancelled)
        XCTAssertNotNil(restored.plan)
        await model.refreshPlan(restored)
        XCTAssertEqual(restored.state, .cancelled)
    }

    func testUnavailableBookmarkBlocksSourceUntilExplicitRelink() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let input = directory.appendingPathComponent("missing.csv")
        try Data("a,b\n1,2\n".utf8).write(to: input)
        let store = WorkspaceStore(url: directory.appendingPathComponent("state.json"))
        let original = FileJob(input: input); original.format = .json
        let saved = WorkspaceJobRecord(id: original.id, source: try store.reference(input), draft: .init(original), state: .ready, outputs: [])
        try store.save(record(jobs: [saved]))
        try FileManager.default.removeItem(at: input)
        let model = WorkspaceModel()
        await model.startPersistence(store: WorkspaceStore(url: store.url))
        let restored = try XCTUnwrap(model.jobs.first)
        XCTAssertEqual(restored.state, .missing)
        XCTAssertNil(restored.plan)
        XCTAssertFalse(model.canRun)
        model.saveSession()
        XCTAssertEqual(try XCTUnwrap(store.load()).jobs.first?.source, saved.source)
        let replacement = directory.appendingPathComponent("relocated.csv")
        try Data("a,b\n3,4\n".utf8).write(to: replacement)
        model.relink(restored, to: replacement)
        let deadline = ContinuousClock.now.advanced(by: .seconds(5))
        while restored.state == .inspecting && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
        XCTAssertEqual(restored.state, .ready)
        XCTAssertEqual(restored.input, replacement)
        XCTAssertEqual(restored.id, original.id)
        model.saveSession()
        XCTAssertEqual(try XCTUnwrap(store.load()).jobs.first?.source.lastKnownURL, replacement)
    }

    func testUnsupportedOrCorruptSessionIsNotOverwrittenOnLaunch() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = WorkspaceStore(url: directory.appendingPathComponent("state.json"))
        var future = record(jobs: []); future.schemaVersion = 99
        let encoded = try JSONEncoder().encode(future)
        for bytes in [Data("{unfinished".utf8), encoded] {
            try bytes.write(to: store.url)
            let model = WorkspaceModel()
            await model.startPersistence(store: store)
            XCTAssertNotNil(model.persistenceNotice)
            model.saveSession()
            XCTAssertEqual(try Data(contentsOf: store.url), bytes)
        }
    }

    func testPDFDraftKeepsIdentityRotationOrderAndSplitAcrossStore() throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let input = directory.appendingPathComponent("source.pdf")
        try Data("fixture reference only".utf8).write(to: input)
        let store = WorkspaceStore(url: directory.appendingPathComponent("state.json"))
        let job = FileJob(input: input)
        let saved = WorkspaceJobRecord(id: job.id, source: try store.reference(input), draft: .init(job), state: .ready, outputs: [])
        var value = record(jobs: [saved])
        value.task = .pages; value.pdfSourceIDs = [job.id.uuidString]
        value.pages = [.init(sourceID: job.id.uuidString, pageIndex: 2, rotation: 90, splitAfter: true),
                       .init(sourceID: job.id.uuidString, pageIndex: 0)]
        value.pageSelection = value.pages[0].id; value.pdfOutputName = "Review copy.pdf"; value.pdfInterrupted = true
        try store.save(value)
        let restored = try XCTUnwrap(store.load())
        XCTAssertEqual(restored.pages, value.pages)
        XCTAssertEqual(restored.pageSelection, value.pageSelection)
        XCTAssertEqual(restored.pdfOutputName, value.pdfOutputName)
        XCTAssertTrue(restored.pdfInterrupted)
        var bad = value; bad.pdfSourceIDs = []
        XCTAssertThrowsError(try store.save(bad), "Orphaned page references must not replace the last valid snapshot")
        XCTAssertEqual(try XCTUnwrap(store.load()).pages, value.pages)
    }

    func testPDFMultiSelectionRestoresFocusAndSavesChangesWithoutMovingFocus() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let input = directory.appendingPathComponent("three pages.pdf")
        var box = CGRect(x: 0, y: 0, width: 100, height: 120)
        let context = try XCTUnwrap(CGContext(input as CFURL, mediaBox: &box, nil))
        for _ in 0..<3 { context.beginPDFPage(nil); context.fill(box); context.endPDFPage() }
        context.closePDF()
        let store = WorkspaceStore(url: directory.appendingPathComponent("workspace.json"))
        let job = FileJob(input: input); job.format = .pdf
        let source = WorkspaceJobRecord(id: job.id, source: try store.reference(input), draft: .init(job), state: .ready, outputs: [])
        var saved = record(jobs: [source]); saved.task = .pages; saved.pdfSourceIDs = [job.id.uuidString]
        saved.pages = (0..<3).map { PDFPageDraft(sourceID: job.id.uuidString, pageIndex: $0) }
        let first = saved.pages[0].id, last = saved.pages[2].id
        saved.pageSelection = last; saved.pdfSelectedPages = [first, last]
        saved.pdfPageOutput = .init(format: .jpeg, dpi: "300", quality: 0.72, selectedOnly: true)
        try store.save(saved)
        let model = WorkspaceModel(); await model.startPersistence(store: store)
        XCTAssertEqual(model.pdfWorkspace.selectedPageIDs, [first, last])
        XCTAssertEqual(model.pdfWorkspace.selection, last)
        XCTAssertEqual(model.pdfWorkspace.pageOutput, saved.pdfPageOutput)
        model.pdfWorkspace.restorePageSelection([last], primary: last)
        let deadline = ContinuousClock.now + .seconds(3)
        while try Set(store.load()?.pdfSelectedPages ?? []) != [last], ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        XCTAssertEqual(Set(try XCTUnwrap(store.load()?.pdfSelectedPages)), [last])
        var invalid = saved; invalid.pdfSelectedPages = [UUID()]
        XCTAssertThrowsError(try store.save(invalid))
        XCTAssertEqual(Set(try XCTUnwrap(store.load()?.pdfSelectedPages)), [last])
        saved.pdfSelectedPages = nil
        saved.pdfPageOutput = nil
        try store.save(saved)
        let legacy = WorkspaceModel(); await legacy.startPersistence(store: WorkspaceStore(url: store.url))
        XCTAssertEqual(legacy.pdfWorkspace.selectedPageIDs, [last], "Legacy records restore their primary selection")
        XCTAssertEqual(legacy.pdfWorkspace.pageOutput, .init(), "Legacy PDF drafts retain PDF output")
    }

    private func record(jobs: [WorkspaceJobRecord], destination: WorkspaceFileReference? = nil) -> WorkspaceRecord {
        .init(task: .convert, selection: jobs.first?.id, destination: destination, jobs: jobs, pages: [], pageSelection: nil,
              pdfSourceIDs: [], pdfOutputName: "Combined.pdf", pdfInterrupted: false, pdfOutputs: [])
    }
    private func temporaryDirectory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("fileform-session-tests-\(UUID())")
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }
}
