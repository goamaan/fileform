// SPDX-License-Identifier: Apache-2.0
import Foundation
import CoreGraphics
import PDFKit
import Testing
import FileformDomain
@testable import FileformCore

@Test func pdfSplitRetainsOffsetPageGeometryAndRenderedContent() async throws {
    let fixture = try Fixture(); defer { fixture.cleanup() }
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let generated = try await ProcessRunner.run(executable: URL(fileURLWithPath: "/usr/bin/env"), arguments: ["python3", root.appendingPathComponent("Tools/generate-pdf-optimization-fixtures.py").path, fixture.url("source").path, "64"])
    #expect(generated.status == 0)
    let input = fixture.url("source/landscape.pdf"), original = try Data(contentsOf: input)
    let refs = [[PageReference(sourceID: "pdf", pageIndex: 2, clockwiseRotation: 90)],
                [.init(sourceID: "pdf", pageIndex: 0), .init(sourceID: "pdf", pageIndex: 1), .init(sourceID: "pdf", pageIndex: 0)]]
    let request = try TransformationRequest(assets: [.init(id: "pdf", url: input)], operation: .pdfSplit(groups: refs),
        output: .init(destination: fixture.url("split"), format: .pdf, cardinality: .directory))
    let engine = ConversionEngine(), result = try await engine.run(engine.plan(request))
    #expect(result.artifacts.count == 2)
    let source = try #require(PDFDocument(url: input))
    for (artifact, pages) in zip(result.artifacts, refs) {
        let output = try #require(PDFDocument(url: artifact.url))
        #expect(output.pageCount == pages.count)
        for (index, reference) in pages.enumerated() {
            let before = try #require(source.page(at: reference.pageIndex)), after = try #require(output.page(at: index))
            #expect(before.string == after.string)
            #expect((before.rotation + reference.clockwiseRotation) % 360 == after.rotation % 360)
            let a = before.bounds(for: .mediaBox), b = after.bounds(for: .mediaBox)
            for box in [PDFDisplayBox.mediaBox, .cropBox, .bleedBox, .trimBox, .artBox] {
                #expect(before.bounds(for: box).offsetBy(dx: -a.minX, dy: -a.minY) == after.bounds(for: box).offsetBy(dx: -b.minX, dy: -b.minY))
            }
            let first = try splitTestPixels(before, rotation: reference.clockwiseRotation), second = try splitTestPixels(after, rotation: 0)
            #expect(first.elementsEqual(second), "With interpolation disabled, page placement and image samples must render identically")
        }
    }
    #expect(try Data(contentsOf: input) == original)
}

private func splitTestPixels(_ page: PDFPage, rotation: Int) throws -> Data {
    let ref = try #require(page.pageRef)
    let context = try #require(CGContext(data: nil, width: 120, height: 120, bitsPerComponent: 8, bytesPerRow: 480,
        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    // PDFKit writes image interpolation hints; isolate placement/sample preservation.
    context.interpolationQuality = .none
    context.setFillColor(CGColor(gray: 1, alpha: 1)); context.fill(CGRect(x: 0, y: 0, width: 120, height: 120))
    context.concatenate(ref.getDrawingTransform(.mediaBox, rect: CGRect(x: 0, y: 0, width: 120, height: 120), rotate: Int32(rotation), preserveAspectRatio: true))
    context.drawPDFPage(ref)
    return Data(bytes: try #require(context.data), count: 120 * 480)
}
