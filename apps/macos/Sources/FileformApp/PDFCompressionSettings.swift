import SwiftUI
import FileformDomain

struct PDFCompressionSettings: View {
    @Bindable var job: FileJob

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Picker("PDF compression", selection: $job.pdfCompression.compressImages) {
                Text("Lossless").tag(false)
                Text("Compress images").tag(true)
            }.pickerStyle(.segmented).labelsHidden().accessibilityIdentifier("pdf-compression-mode")
            if job.pdfCompression.compressImages {
                Text("Supported images become JPEG and may lose detail. Pages, text and vectors stay; unsupported images remain unchanged.")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                HStack {
                    Text("Image quality")
                    Spacer()
                    Text(job.pdfCompression.quality.formatted(.percent.precision(.fractionLength(0))))
                        .font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink3)
                }
                Slider(value: $job.pdfCompression.quality, in: 0.05...1, step: 0.01)
                    .accessibilityLabel("PDF image quality").accessibilityIdentifier("pdf-image-quality")
                    .onChange(of: job.pdfCompression.quality) { _, value in
                        if job.pdfCompression.minimumQuality > value { job.pdfCompression.minimumQuality = value }
                    }
                if job.goal == .fit {
                    HStack {
                        Text("Minimum quality")
                        Spacer()
                        Text(job.pdfCompression.minimumQuality.formatted(.percent.precision(.fractionLength(0))))
                            .font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink3)
                    }
                    Slider(value: $job.pdfCompression.minimumQuality, in: 0.05...max(0.051, job.pdfCompression.quality), step: 0.01)
                        .accessibilityLabel("Minimum PDF image quality").accessibilityIdentifier("pdf-image-quality-floor")
                        .disabled(job.pdfCompression.quality <= 0.05)
                    Text("Tries lower quality down to this floor. If the complete PDF cannot fit, nothing is saved.")
                        .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                }
                Toggle("Resize embedded images", isOn: $job.pdfCompression.resizeImages)
                    .accessibilityIdentifier("pdf-resize-images")
                if job.pdfCompression.resizeImages {
                    HStack {
                        Text("Longest edge")
                        Spacer()
                        TextField("1920", text: $job.pdfCompression.longestEdge).frame(width: 78)
                            .font(FileformTheme.mono(12)).accessibilityLabel("Maximum embedded image edge in pixels")
                            .accessibilityIdentifier("pdf-image-longest-edge")
                        Text("px").foregroundStyle(FileformTheme.ink3)
                    }
                    Text("Larger images shrink to this limit. Smaller images are not enlarged.")
                        .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                }
            } else {
                Text("Keeps every page and image quality. Some PDFs cannot be reduced further.")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            }
        }.fixedSize(horizontal: false, vertical: true)
    }
}


struct PDFCompressionReport: View {
    let details: PDFOptimizationDetails
    let title: String

    private var eligibleCount: Int { details.candidates.filter { $0.skipReason == nil }.count }
    private var unchangedCount: Int { details.candidates.count - eligibleCount }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title.uppercased()).font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink4)
            if let quality = details.usedQuality {
                Text("JPEG quality: \(quality.formatted(.percent.precision(.fractionLength(0)))) · \(details.optimizedImageCount) recompressed · \(details.resampledImageCount) resized")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            } else {
                Text("\(eligibleCount) eligible for recompression · \(unchangedCount) kept unchanged")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            }
            if !details.attemptedQualities.isEmpty {
                Text("\(details.attemptedQualities.count) quality attempt\(details.attemptedQualities.count == 1 ? "" : "s")")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            }
            if !details.candidates.isEmpty {
                DisclosureGroup("Image changes") {
                    VStack(alignment: .leading, spacing: 10) {
                        ForEach(Array(details.candidates.enumerated()), id: \.offset) { index, image in
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Image \(index + 1)").font(.system(size: 12, weight: .medium))
                                if let width = image.originalWidth, let height = image.originalHeight {
                                    Text("\(width) × \(height) pixels" + outputDimensions(image))
                                        .font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink3)
                                }
                                if let reason = image.skipReason {
                                    Text("Kept unchanged: " + reason).font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                                }
                            }.accessibilityElement(children: .combine)
                        }
                    }.padding(.top, 8)
                }.font(.system(size: 12))
            }
        }.fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("pdf-compression-report")
    }
    private func outputDimensions(_ image: PDFOptimizationImageCandidate) -> String {
        guard image.skipReason == nil, let width = image.outputWidth, let height = image.outputHeight,
              width != image.originalWidth || height != image.originalHeight else { return "" }
        return " → \(width) × \(height)"
    }
}
