import XCTest
@testable import Fileform
import FileformDomain

final class MediaTrimDraftTests: XCTestCase {
    func testTypedClockAndFractionsRetainExactTimes() throws {
        XCTAssertEqual(try TrimTimeText.parse("1:02.000001"), .init(ticks: 62_000_001, timescale: 1_000_000))
        XCTAssertEqual(try TrimTimeText.parse("1:02:03.5"), .init(ticks: 37_235, timescale: 10))
        let source = MediaTime(ticks: 8008, timescale: 30_000)
        let formatted = TrimTimeText.edit(source)
        let restored = try TrimTimeText.parse(formatted)
        XCTAssertFalse(try source.isBefore(restored)); XCTAssertFalse(try restored.isBefore(source))
        for invalid in ["1:60", "1:60:00", "-1", "1/0", "1.2.3", "NaN", "0.0000000001", "99999999999999999999999"] {
            XCTAssertThrowsError(try TrimTimeText.parse(invalid), invalid)
        }
    }
    func testDraftPreservesImportedRationalRepresentationAndInvalidEditsBlockRequest() throws {
        let range = MediaInterval(start: .init(ticks: 48_000, timescale: 48_000), end: .init(ticks: 80_080, timescale: 30_000))
        var draft = MediaTrimDraft(interval: range, format: .m4a, outputName: "Recording-trim")
        XCTAssertEqual(try draft.interval(), range)
        let restored = try JSONDecoder().decode(MediaTrimDraft.self, from: JSONEncoder().encode(draft))
        XCTAssertEqual(try restored.interval(), range)
        draft.from = "bad input"
        XCTAssertThrowsError(try draft.request(input: URL(fileURLWithPath: "/tmp/source.m4a"), folder: URL(fileURLWithPath: "/tmp")))
        draft.from = "0:00.25"
        XCTAssertEqual(try draft.interval().start, .init(ticks: 25, timescale: 100))
        XCTAssertEqual(try draft.interval().end, range.end)
    }
    func testDurationAndSourceAliasGuard() throws {
        let range = MediaInterval(start: .init(ticks: 125, timescale: 100), end: .init(ticks: 2750, timescale: 1000))
        XCTAssertEqual(TrimTimeText.duration(range), "0:01.5")
        let draft = MediaTrimDraft(interval: range, format: .mp4, outputName: "source.mp4", muteAudio: true)
        XCTAssertThrowsError(try draft.request(input: URL(fileURLWithPath: "/tmp/source.mp4"), folder: URL(fileURLWithPath: "/tmp")))
        XCTAssertThrowsError(try draft.interval(duration: .init(ticks: 2, timescale: 1)))
    }
}
