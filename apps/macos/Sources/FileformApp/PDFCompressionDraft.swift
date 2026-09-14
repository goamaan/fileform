import Foundation

struct PDFCompressionDraft: Codable, Equatable {
    var compressImages = false
    var quality = 0.8
    var minimumQuality = 0.5
    var resizeImages = false
    var longestEdge = "1920"
    var key: String { [String(compressImages), String(quality), String(minimumQuality), String(resizeImages), longestEdge].joined(separator: ":") }
    var isValidRecord: Bool {
        quality.isFinite && minimumQuality.isFinite && (0.05...1).contains(quality)
            && (0.05...quality).contains(minimumQuality) && longestEdge.utf8.count <= 32
    }
}
