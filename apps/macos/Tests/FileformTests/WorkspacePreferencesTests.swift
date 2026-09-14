import XCTest
@testable import Fileform
import FileformCore
import FileformDomain

@MainActor final class WorkspacePreferencesTests: XCTestCase {
    func testPreferencesPersistAndUnknownCollisionFallsBackSafely() throws {
        let name = "fileform-preferences-test-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        let preferences = WorkspacePreferences(defaults: defaults)
        XCTAssertTrue(preferences.allowLinks); XCTAssertTrue(preferences.rememberOutputFolder)
        XCTAssertEqual(preferences.defaultCollision, .rename)
        preferences.allowLinks = false; preferences.rememberOutputFolder = false; preferences.defaultCollision = .fail
        let restored = WorkspacePreferences(defaults: defaults)
        XCTAssertFalse(restored.allowLinks); XCTAssertFalse(restored.rememberOutputFolder)
        XCTAssertEqual(restored.defaultCollision, .fail)
        defaults.set("overwrite", forKey: "saving.defaultCollision")
        XCTAssertEqual(WorkspacePreferences(defaults: defaults).defaultCollision, .rename)
    }

    func testNetworkPreferenceGuardsLookupAndSavingWithoutARequest() throws {
        let name = "fileform-network-test-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        let preferences = WorkspacePreferences(defaults: defaults)
        let model = WorkspaceModel(engine: ConversionEngine(), preferences: preferences)
        model.linkWorkspace.urlText = "https://example.invalid/recording.wav"
        XCTAssertTrue(model.linkWorkspace.canLookup)
        preferences.allowLinks = false
        XCTAssertFalse(model.linkWorkspace.canLookup)
        model.linkWorkspace.lookup(); model.linkWorkspace.run()
        XCTAssertFalse(model.linkWorkspace.isBusy)
        XCTAssertNil(model.linkWorkspace.source); XCTAssertNil(model.linkWorkspace.plan)
        preferences.allowLinks = true
        XCTAssertTrue(model.linkWorkspace.canLookup)
    }

    func testSavingPreferencesApplyToNewJobsAndForgetOnlyPersistedDestination() async throws {
        let name = "fileform-saving-test-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(name)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { defaults.removePersistentDomain(forName: name); try? FileManager.default.removeItem(at: folder) }
        let input = folder.appendingPathComponent("table.csv")
        try "name,value\nexample,3\n".write(to: input, atomically: true, encoding: .utf8)
        let preferences = WorkspacePreferences(defaults: defaults)
        preferences.defaultCollision = .fail
        let model = WorkspaceModel(engine: ConversionEngine(), preferences: preferences)
        let store = WorkspaceStore(url: folder.appendingPathComponent("workspace.json"))
        await model.startPersistence(store: store)
        model.setDestination(folder); model.addFiles([input])
        let deadline = ContinuousClock.now + .seconds(10)
        while model.jobs.contains(where: { $0.state == .inspecting }) && ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        XCTAssertEqual(model.jobs.first?.collisionPolicy, .fail)
        preferences.defaultCollision = .rename
        XCTAssertEqual(model.jobs.first?.collisionPolicy, .fail, "New defaults must not overwrite existing drafts")
        model.saveSession(); XCTAssertNotNil(try store.load()?.destination)
        preferences.rememberOutputFolder = false
        XCTAssertEqual(model.outputFolder, folder, "The active session retains its selected folder")
        XCTAssertNil(try store.load()?.destination)
        let restored = WorkspaceModel(engine: ConversionEngine(), preferences: WorkspacePreferences(defaults: defaults))
        await restored.startPersistence(store: WorkspaceStore(url: store.url))
        XCTAssertNil(restored.outputFolder)
        XCTAssertEqual(restored.jobs.count, 1)
        XCTAssertEqual(restored.jobs.first?.collisionPolicy, .fail)
    }
}
