import AppKit
import XCTest
@testable import Fileform
import FileformCore

@MainActor final class PDFSelectionTests: XCTestCase {
    func testCommandToggleAndAnchoredRangeAcrossSources() {
        let draft = workspace()
        let pages = draft.pages
        draft.selectPage(pages[1].id)
        draft.selectPage(pages[3].id, toggling: true)
        XCTAssertEqual(draft.selectedPageIDs, [pages[1].id, pages[3].id])
        draft.selectPage(pages[5].id, extending: true)
        XCTAssertEqual(draft.selectedPageIDs, Set(pages[3...5].map(\.id)))
        draft.selectPage(pages[2].id, extending: true)
        XCTAssertEqual(draft.selectedPageIDs, Set(pages[2...3].map(\.id)))
        draft.selectPage(pages[3].id, toggling: true)
        XCTAssertEqual(draft.selectedPageIDs, [pages[2].id])
        XCTAssertEqual(draft.selection, pages[2].id)
        draft.selectPage(pages[2].id, toggling: true)
        XCTAssertTrue(draft.selectedPageIDs.isEmpty)
        XCTAssertNil(draft.selection)
        draft.selection = pages[0].id
        XCTAssertEqual(draft.selectedPageIDs, [pages[0].id], "Older callers still set a single primary selection")
    }

    func testMoveDisjointRunsPreservesRelativeOrderAndPageMetadata() {
        let draft = workspace()
        draft.pages[2].splitAfter = true
        let original = draft.pages
        draft.selectPages(Set([original[1].id, original[2].id, original[4].id]))
        draft.moveSelection(-1, undo: nil)
        XCTAssertEqual(draft.pages.map(\.pageIndex), [1, 2, 0, 4, 3, 5])
        XCTAssertEqual(draft.pages[1], original[2])
        draft.moveSelection(1, undo: nil)
        XCTAssertEqual(draft.pages, original)
        draft.selectPages(Set(draft.pages.map(\.id)))
        XCTAssertFalse(draft.canMoveSelection(-1))
        XCTAssertFalse(draft.canMoveSelection(1))
    }

    func testDuplicateAndRemoveGroupUndoRestoresPagesAndSelection() {
        let draft = workspace()
        draft.pages[1].rotation = 90
        draft.pages[1].splitAfter = true
        let original = draft.pages
        let selected = Set([original[1].id, original[3].id])
        draft.selectPages(selected)
        let undo = UndoManager()
        undo.groupsByEvent = false
        undo.beginUndoGrouping()
        draft.duplicateSelection(undo: undo)
        undo.endUndoGrouping()
        XCTAssertEqual(draft.pages.map(\.pageIndex), [0, 1, 2, 3, 1, 3, 4, 5])
        let copies = Array(draft.pages[4...5])
        XCTAssertEqual(draft.selectedPageIDs, Set(copies.map(\.id)))
        XCTAssertTrue(Set(copies.map(\.id)).isDisjoint(with: Set(original.map(\.id))))
        XCTAssertEqual(copies[0].rotation, 90)
        XCTAssertFalse(copies[0].splitAfter, "Duplicating pages must not silently add output boundaries")
        undo.undo()
        XCTAssertEqual(draft.pages, original)
        XCTAssertEqual(draft.selectedPageIDs, selected)
        undo.redo()
        XCTAssertEqual(draft.selectedPageIDs, Set(copies.map(\.id)))
        undo.beginUndoGrouping()
        draft.removeSelection(undo: undo)
        undo.endUndoGrouping()
        XCTAssertEqual(draft.pages, original)
        undo.undo()
        XCTAssertEqual(draft.selectedPageIDs, Set(copies.map(\.id)))
        XCTAssertEqual(Array(draft.pages[4...5]), copies)
    }

    func testRotationAndRunningGuardApplyToWholeSelection() {
        let draft = workspace()
        let ids = Set([draft.pages[1].id, draft.pages[4].id])
        draft.selectPages(ids)
        draft.rotateSelection(undo: nil)
        XCTAssertEqual(draft.pages.map(\.rotation), [0, 90, 0, 0, 90, 0])
        let before = draft.pages
        draft.isRunning = true
        draft.rotateSelection(undo: nil)
        draft.removeSelection(undo: nil)
        draft.duplicateSelection(undo: nil)
        draft.moveSelection(-1, undo: nil)
        draft.selectPages([])
        draft.selectPage(draft.pages[0].id)
        XCTAssertEqual(draft.pages, before)
        XCTAssertEqual(draft.selectedPageIDs, ids)
    }

    func testRestoredSelectionFiltersMissingIDsAndPreservesPrimary() {
        let draft = workspace()
        let pages = draft.pages
        draft.restorePageSelection([pages[1].id, pages[4].id, UUID()], primary: pages[4].id)
        XCTAssertEqual(draft.selectedPageIDs, [pages[1].id, pages[4].id])
        XCTAssertEqual(draft.selection, pages[4].id)
        draft.selectPage(pages[5].id, extending: true)
        XCTAssertEqual(draft.selectedPageIDs, [pages[4].id, pages[5].id], "Restored primary also anchors range selection")
        draft.restorePageSelection([pages[1].id, pages[4].id], primary: UUID())
        XCTAssertEqual(draft.selection, pages[1].id)
        draft.restorePageSelection([], primary: pages[1].id)
        XCTAssertNil(draft.selection)
        draft.isRunning = true
        draft.restorePageSelection([pages[1].id], primary: pages[1].id)
        XCTAssertTrue(draft.selectedPageIDs.isEmpty)
    }

    func testDragPlacesDisjointSelectionBeforeAndAfterWithOneUndo() throws {
        let draft = workspace()
        draft.pages[1].rotation = 270
        draft.pages[4].splitAfter = true
        let original = draft.pages
        let ids = Set([original[1].id, original[4].id])
        draft.restorePageSelection(ids, primary: original[4].id)
        let undo = UndoManager()
        undo.groupsByEvent = false
        let beforePayload = try XCTUnwrap(draft.beginPageDrag(original[1].id))
        XCTAssertEqual(beforePayload.pageIDs, [original[1].id, original[4].id])
        undo.beginUndoGrouping()
        XCTAssertTrue(draft.dropPages(beforePayload, at: original[0].id, edge: .before, undo: undo))
        undo.endUndoGrouping()
        XCTAssertEqual(draft.pages, [original[1], original[4], original[0], original[2], original[3], original[5]])
        XCTAssertEqual(draft.selectedPageIDs, ids)
        XCTAssertEqual(draft.selection, original[4].id)
        XCTAssertFalse(draft.isValidPageDrag(beforePayload), "A consumed payload cannot be replayed")
        undo.undo()
        XCTAssertEqual(draft.pages, original)
        XCTAssertFalse(undo.canUndo, "The entire drop is one native undo operation")
        undo.redo()
        XCTAssertEqual(Array(draft.pages.prefix(2)), [original[1], original[4]])
        undo.undo()
        let afterPayload = try XCTUnwrap(draft.beginPageDrag(original[4].id))
        XCTAssertTrue(draft.dropPages(afterPayload, at: original[5].id, edge: .after, undo: nil))
        XCTAssertEqual(draft.pages, [original[0], original[2], original[3], original[5], original[1], original[4]])
    }

    func testDragRejectsForeignStaleAlteredAndRunningPayloads() throws {
        let draft = workspace()
        let original = draft.pages
        let first = try XCTUnwrap(draft.beginPageDrag(original[1].id))
        let replacement = try XCTUnwrap(draft.beginPageDrag(original[2].id))
        XCTAssertFalse(draft.dropPages(first, at: original[5].id, edge: .after, undo: nil))
        let foreign = workspace()
        foreign.pages = original
        _ = foreign.beginPageDrag(original[2].id)
        XCTAssertFalse(foreign.dropPages(replacement, at: original[5].id, edge: .after, undo: nil))
        let altered = PDFPageDragPayload(workspaceID: replacement.workspaceID, dragID: replacement.dragID,
                                         pageIDs: [original[4].id])
        XCTAssertFalse(draft.dropPages(altered, at: original[5].id, edge: .after, undo: nil))
        draft.pages[0].rotation = 90
        XCTAssertFalse(draft.dropPages(replacement, at: original[5].id, edge: .after, undo: nil))
        let changedSelection = try XCTUnwrap(draft.beginPageDrag(original[2].id))
        draft.selectPage(original[3].id)
        XCTAssertFalse(draft.dropPages(changedSelection, at: original[5].id, edge: .after, undo: nil))
        let current = try XCTUnwrap(draft.beginPageDrag(original[2].id))
        draft.isRunning = true
        XCTAssertNil(draft.beginPageDrag(original[3].id))
        XCTAssertFalse(draft.dropPages(current, at: original[5].id, edge: .after, undo: nil))
        XCTAssertEqual(draft.pages.map(\.id), original.map(\.id))
        draft.isRunning = false
        draft.pages.remove(at: 2)
        XCTAssertFalse(draft.dropPages(current, at: original[5].id, edge: .after, undo: nil))
    }

    func testDragSelfAndUnchangedPlacementDoNotRegisterUndo() throws {
        let draft = workspace()
        let original = draft.pages
        draft.selectPages([original[1].id, original[2].id])
        let undo = UndoManager()
        undo.groupsByEvent = false
        let selfPayload = try XCTUnwrap(draft.beginPageDrag(original[1].id))
        XCTAssertTrue(draft.dropPages(selfPayload, at: original[2].id, edge: .after, undo: undo))
        XCTAssertEqual(draft.pages, original)
        XCTAssertFalse(undo.canUndo)
        let adjacent = try XCTUnwrap(draft.beginPageDrag(original[2].id))
        XCTAssertTrue(draft.dropPages(adjacent, at: original[3].id, edge: .before, undo: undo))
        XCTAssertEqual(draft.pages, original)
        XCTAssertFalse(undo.canUndo)
        let unselected = try XCTUnwrap(draft.beginPageDrag(original[4].id))
        XCTAssertEqual(unselected.pageIDs, [original[4].id])
        XCTAssertEqual(draft.selectedPageIDs, [original[4].id])
    }

    private func workspace() -> PDFWorkspaceModel {
        let draft = PDFWorkspaceModel(engine: ConversionEngine())
        draft.pages = (0..<6).map { PDFPageDraft(sourceID: $0 < 3 ? "first" : "second", pageIndex: $0) }
        draft.selection = draft.pages.first?.id
        return draft
    }
}
