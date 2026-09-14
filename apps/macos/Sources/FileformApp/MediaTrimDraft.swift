import Foundation
import FileformDomain

/// Text remains editable (including temporarily invalid input); original
/// rational values are retained until an endpoint is actually changed.
struct MediaTrimDraft: Codable, Equatable {
    var originalInterval: MediaInterval
    var from: String
    var to: String
    var mode: TrimMode = .exact
    var audioStream: Int?
    var muteAudio = false
    var format: OutputFormat
    var outputName: String
    var collisionPolicy: CollisionPolicy = .rename

    init(interval: MediaInterval, format: OutputFormat, outputName: String,
         mode: TrimMode = .exact, audioStream: Int? = nil, muteAudio: Bool = false,
         collisionPolicy: CollisionPolicy = .rename) {
        originalInterval = interval
        from = TrimTimeText.edit(interval.start); to = TrimTimeText.edit(interval.end)
        self.format = format; self.outputName = outputName; self.mode = mode
        self.audioStream = audioStream; self.muteAudio = muteAudio; self.collisionPolicy = collisionPolicy
    }
    var key: String {
        let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys]
        return (try? encoder.encode(self).base64EncodedString()) ?? ""
    }
    func interval(duration: MediaTime? = nil) throws -> MediaInterval {
        let start = from == TrimTimeText.edit(originalInterval.start) ? originalInterval.start : try TrimTimeText.parse(from)
        let end = to == TrimTimeText.edit(originalInterval.end) ? originalInterval.end : try TrimTimeText.parse(to)
        let value = MediaInterval(start: start, end: end)
        try value.validate(duration: duration)
        return value
    }
    func request(input: URL, folder: URL, duration: MediaTime? = nil) throws -> TransformationRequest {
        let name = outputName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, name.utf8.count <= 240, !name.contains("/"), !name.contains(":"),
              !name.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains),
              name != ".", name != ".." else {
            throw FileformError(.invalidRequest, "Enter a filename without path separators or control characters.")
        }
        let suffix = "." + format.fileExtension
        let filename = name.lowercased().hasSuffix(suffix) ? name : name + suffix
        return try .init(assets: [.init(id: "source", url: input)],
            operation: .mediaTrim(interval: interval(duration: duration), mode: mode, audioStream: audioStream, muteAudio: muteAudio),
            output: .init(destination: folder.appendingPathComponent(filename), format: format), collisionPolicy: collisionPolicy)
    }
}

enum TrimTimeText {
    static func parse(_ value: String) throws -> MediaTime {
        let text = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, text.count <= 80 else { throw invalid() }
        if text.contains("/") {
            let parts = text.split(separator: "/", omittingEmptySubsequences: false)
            guard parts.count == 2, digits(parts[0]), digits(parts[1]),
                  let ticks = Int64(parts[0]), let scale = Int32(parts[1]), scale > 0 else { throw invalid() }
            return .init(ticks: ticks, timescale: scale)
        }
        let parts = text.split(separator: ":", omittingEmptySubsequences: false)
        guard (1...3).contains(parts.count), let last = parts.last else { throw invalid() }
        let seconds = last.split(separator: ".", omittingEmptySubsequences: false)
        guard (1...2).contains(seconds.count), digits(seconds[0]), let whole = Int64(seconds[0]),
              parts.count == 1 || whole < 60 else { throw invalid() }
        var leading: Int64 = 0
        for (index, part) in parts.dropLast().enumerated() {
            guard digits(part), let number = Int64(part), number <= 21_600,
                  index == 0 || number < 60 else { throw invalid() }
            leading = leading * 60 + number
        }
        let fraction = seconds.count == 2 ? seconds[1] : ""
        guard seconds.count == 1 || (digits(fraction) && fraction.count <= 9) else { throw invalid() }
        let scale = (0..<fraction.count).reduce(Int64(1)) { value, _ in value * 10 }
        let total = leading.multipliedReportingOverflow(by: 60)
        let sum = total.partialValue.addingReportingOverflow(whole)
        let product = sum.partialValue.multipliedReportingOverflow(by: scale)
        let ticks = product.partialValue.addingReportingOverflow(Int64(fraction) ?? 0)
        guard !total.overflow, !sum.overflow, !product.overflow, !ticks.overflow else { throw invalid() }
        return .init(ticks: ticks.partialValue, timescale: Int32(scale))
    }
    static func edit(_ time: MediaTime) -> String {
        guard time.ticks >= 0, time.timescale > 0 else { return "" }
        return exact(ticks: time.ticks, scale: Int64(time.timescale))
    }
    static func duration(_ interval: MediaInterval) -> String {
        let start = interval.start, end = interval.end
        guard (try? interval.validate()) != nil else { return "—" }
        let common = gcd(Int64(start.timescale), Int64(end.timescale))
        let a = Int64(end.timescale) / common, b = Int64(start.timescale) / common
        let left = end.ticks.multipliedReportingOverflow(by: b)
        let right = start.ticks.multipliedReportingOverflow(by: a)
        guard !left.overflow, !right.overflow else { return "\(edit(end)) − \(edit(start))" }
        return exact(ticks: left.partialValue - right.partialValue, scale: Int64(start.timescale) * a)
    }
    static func seconds(_ time: MediaTime) -> Double { Double(time.ticks) / Double(time.timescale) }
    static func fromSeconds(_ seconds: Double) -> MediaTime {
        .init(ticks: Int64((min(21_600, max(0, seconds.isFinite ? seconds : 0)) * 1_000_000).rounded()), timescale: 1_000_000)
    }
    private static func exact(ticks: Int64, scale: Int64) -> String {
        let factor = gcd(ticks, scale), numerator = ticks / factor, denominator = scale / factor
        var decimalScale: Int64 = 1, places = 0
        while decimalScale % denominator != 0 && places < 9 { decimalScale *= 10; places += 1 }
        guard decimalScale % denominator == 0 else { return "\(numerator)/\(denominator)" }
        let whole = numerator / denominator
        let remainder = numerator % denominator
        let prefix = whole >= 3600 ? String(format: "%lld:%02lld:%02lld", whole / 3600, (whole / 60) % 60, whole % 60)
            : String(format: "%lld:%02lld", whole / 60, whole % 60)
        guard remainder > 0 else { return prefix }
        var fraction = String(remainder * (decimalScale / denominator))
        fraction = String(repeating: "0", count: places - fraction.count) + fraction
        while fraction.last == "0" { fraction.removeLast() }
        return prefix + "." + fraction
    }
    private static func gcd(_ a: Int64, _ b: Int64) -> Int64 {
        var a = a, b = b
        while b != 0 { let next = a % b; a = b; b = next }
        return max(1, a)
    }
    private static func digits(_ value: Substring) -> Bool { !value.isEmpty && value.allSatisfy { $0.isASCII && $0.isNumber } }
    private static func invalid() -> FileformError { .init(.invalidRequest, "Enter seconds, minutes:seconds, hours:minutes:seconds, or exact ticks/timescale.") }
}
