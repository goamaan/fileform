import XCTest
import AVFoundation
@testable import Fileform
import FileformDomain

@MainActor final class MediaPlaybackTests: XCTestCase {
    func testRationalSelectionBoundsAndReplay() throws {
        let range = MediaInterval(start: .init(ticks: 48_001, timescale: 48_000), end: .init(ticks: 96_013, timescale: 48_000))
        let bounds = try MediaPlaybackBounds(duration: .init(ticks: 3, timescale: 1), selection: range, selectionOnly: true)
        XCTAssertEqual(try bounds.clamped(.init(ticks: 0, timescale: 1)), range.start)
        XCTAssertEqual(try bounds.clamped(.init(ticks: 4, timescale: 1)), range.end)
        XCTAssertEqual(bounds.startForPlayback(at: range.end), range.start)
        XCTAssertThrowsError(try MediaPlaybackBounds(duration: .init(ticks: 2, timescale: 1), selection: range, selectionOnly: true))
    }

    func testRealPlayerStopsAtSelectedEndAndTeardownClearsObservers() async throws {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("fileform-playback-\(UUID().uuidString).wav")
        defer { try? FileManager.default.removeItem(at: url) }
        var data = Data()
        func text(_ value: String) { data.append(contentsOf: value.utf8) }
        func integer<T: FixedWidthInteger>(_ value: T) {
            var little = value.littleEndian; withUnsafeBytes(of: &little) { data.append(contentsOf: $0) }
        }
        text("RIFF"); integer(UInt32(36 + 96_000)); text("WAVEfmt "); integer(UInt32(16))
        integer(UInt16(1)); integer(UInt16(1)); integer(UInt32(48_000)); integer(UInt32(96_000))
        integer(UInt16(2)); integer(UInt16(16)); text("data"); integer(UInt32(96_000))
        data.append(Data(repeating: 0, count: 96_000)); try data.write(to: url)
        let controller = MediaPlaybackController()
        defer { controller.teardown() }
        let range = MediaInterval(start: .init(ticks: 1, timescale: 10), end: .init(ticks: 35, timescale: 100))
        await controller.load(url: url, duration: .init(ticks: 1, timescale: 1), selection: range)
        let deadline = ContinuousClock.now.advanced(by: .seconds(5))
        while (controller.isLoading || controller.isSeeking) && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(20)) }
        XCTAssertNil(controller.error); XCTAssertFalse(controller.isLoading)
        controller.play()
        try await Task.sleep(for: .milliseconds(800))
        let current = try XCTUnwrap(controller.player).currentTime()
        XCTAssertEqual(CMTimeGetSeconds(current), 0.35, accuracy: 0.005)
        XCTAssertFalse(controller.isPlaying)
        XCTAssertEqual(TrimTimeText.seconds(controller.playhead), 0.35, accuracy: 0.005)
        controller.teardown()
        XCTAssertNil(controller.player); XCTAssertNil(controller.duration)
        try await Task.sleep(for: .milliseconds(60))
        XCTAssertEqual(controller.playhead.ticks, 0)
    }
}
