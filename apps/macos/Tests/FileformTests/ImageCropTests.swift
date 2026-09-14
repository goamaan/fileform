import XCTest
@testable import Fileform

final class ImageCropTests: XCTestCase {
    func testMoveClampsWithoutChangingPixelDimensions() {
        let size = CGSize(width: 1200, height: 800)
        let rect = CGRect(x: 100, y: 200, width: 400, height: 300)
        XCTAssertEqual(ImageCropGeometry.move(rect, by: CGSize(width: -500, height: 1000), in: size), CGRect(x: 0, y: 500, width: 400, height: 300))
    }

    func testEveryCornerStaysInsideImageAndKeepsOppositeAnchor() {
        let size = CGSize(width: 1200, height: 800)
        let rect = CGRect(x: 100, y: 200, width: 400, height: 300)
        for corner in ImageCropCorner.allCases {
            for delta in [CGSize(width: -2000, height: -2000), CGSize(width: 2000, height: 2000)] {
                let result = ImageCropGeometry.resize(rect, corner: corner, by: delta, in: size, ratio: nil)
                XCTAssertTrue(CGRect(origin: .zero, size: size).contains(result))
                XCTAssertGreaterThanOrEqual(result.width, 1); XCTAssertGreaterThanOrEqual(result.height, 1)
                XCTAssertEqual(corner.left ? result.maxX : result.minX, corner.left ? rect.maxX : rect.minX)
                XCTAssertEqual(corner.top ? result.maxY : result.minY, corner.top ? rect.maxY : rect.minY)
            }
        }
    }

    func testAspectPresetsAndResizeRespectPixelRounding() throws {
        let size = CGSize(width: 1200, height: 800)
        let ratio = try XCTUnwrap(ImageCropAspect.widescreen.ratio(in: size))
        XCTAssertEqual(ratio, 16.0 / 9.0)
        let rect = ImageCropGeometry.fitting(CGRect(origin: .zero, size: size), ratio: ratio)
        let result = ImageCropGeometry.resize(rect, corner: .bottomRight, by: CGSize(width: -450, height: -20), in: size, ratio: ratio)
        XCTAssertEqual(result.width / result.height, ratio, accuracy: 0.005)
        XCTAssertEqual(result.origin, rect.origin)
        var draft = ImageCropDraft(); draft.setRectangle(result)
        XCTAssertEqual(ImageCropGeometry.rectangle(draft, in: size), result)
        let legacy = Data(#"{"enabled":true,"x":"0","y":"0","width":"100","height":"100"}"#.utf8)
        XCTAssertNil(try JSONDecoder().decode(ImageCropDraft.self, from: legacy).aspect)
    }

    @MainActor func testCropGestureIsOneUndoableDraftEditAndCannotChangeRunningJob() {
        let model = WorkspaceModel()
        let file = FileJob(input: URL(fileURLWithPath: "/tmp/unused-crop-test.png"), acquireAccess: false)
        file.state = .ready; model.jobs = [file]
        let original = file.crop; let undo = UndoManager()
        var crop = original; crop.enabled = true; crop.setRectangle(CGRect(x: 1, y: 2, width: 300, height: 200))
        undo.beginUndoGrouping(); model.changeCrop(file, to: crop, undo: undo); undo.endUndoGrouping()
        XCTAssertEqual(file.crop, crop)
        undo.undo(); XCTAssertEqual(file.crop, original)
        undo.redo(); XCTAssertEqual(file.crop, crop)
        file.state = .running
        model.changeCrop(file, to: original, undo: undo)
        XCTAssertEqual(file.crop, crop)
    }
}
