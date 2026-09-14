// SPDX-License-Identifier: Apache-2.0
import Foundation

/// Explicitly lossy embedded-image optimization. Never rasterizes a PDF page.
public struct PDFOptimizationParameters: Codable, Equatable, Sendable {
    public let goal: ConversionGoal
    public let quality: Double
    public let minimumQuality: Double
    /// Maximum intrinsic image edge in pixels; never interpreted as DPI.
    public let maximumImageDimension: Int?
    public let maximumBytes: Int64?
    public init(goal: ConversionGoal = .compress, quality: Double = 0.8, minimumQuality: Double = 0.5,
                maximumImageDimension: Int? = nil, maximumBytes: Int64? = nil) {
        self.goal = goal; self.quality = quality; self.minimumQuality = minimumQuality
        self.maximumImageDimension = maximumImageDimension; self.maximumBytes = maximumBytes
    }
    public func validate() throws {
        guard [.compress, .fit].contains(goal), quality.isFinite, minimumQuality.isFinite,
              (0.05...1).contains(quality), (0.05...quality).contains(minimumQuality),
              maximumImageDimension.map({ (1...16384).contains($0) }) ?? true,
              goal == .fit ? (maximumBytes.map({ $0 > 0 }) ?? false) : maximumBytes == nil else {
            throw FileformError(.invalidRequest, "PDF optimization requires compress or fit, quality 0.05–1, a floor no higher than quality, positive pixel dimensions up to 16384, and a byte limit only for fit.")
        }
    }
    public var qualitySchedule: [Double] {
        guard goal == .fit, quality > minimumQuality else { return [quality] }
        let step = (quality - minimumQuality) / 5.0
        return (0..<6).map { (index: Int) -> Double in index == 5 ? minimumQuality : quality - step * Double(index) }
    }
}
public struct PDFOptimizationImageCandidate: Codable, Equatable, Sendable {
    public let objectNumber: Int
    public let generation: Int
    public let originalWidth: Int?
    public let originalHeight: Int?
    public let outputWidth: Int?
    public let outputHeight: Int?
    public let skipReason: String?
    public init(objectNumber: Int, generation: Int, originalWidth: Int?, originalHeight: Int?, outputWidth: Int?, outputHeight: Int?, skipReason: String?) {
        self.objectNumber = objectNumber; self.generation = generation
        self.originalWidth = originalWidth; self.originalHeight = originalHeight
        self.outputWidth = outputWidth; self.outputHeight = outputHeight; self.skipReason = skipReason
    }
}
public struct PDFOptimizationDetails: Codable, Equatable, Sendable {
    public let candidates: [PDFOptimizationImageCandidate]
    public let attemptedQualities: [Double]
    public let optimizedImageCount: Int
    public let resampledImageCount: Int
    public let usedQuality: Double?
    public init(candidates: [PDFOptimizationImageCandidate], attemptedQualities: [Double] = [], usedQuality: Double? = nil) {
        self.candidates = candidates; self.attemptedQualities = attemptedQualities; self.usedQuality = usedQuality
        optimizedImageCount = candidates.filter { $0.skipReason == nil }.count
        resampledImageCount = candidates.filter { $0.skipReason == nil && ($0.originalWidth != $0.outputWidth || $0.originalHeight != $0.outputHeight) }.count
    }
}
