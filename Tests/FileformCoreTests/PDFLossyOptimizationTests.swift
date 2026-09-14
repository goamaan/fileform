// SPDX-License-Identifier: Apache-2.0
import Foundation
import Testing
import PDFKit
import FileformDomain
@testable import FileformCore

private let lossyRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func lossyEngine() -> ConversionEngine {
    .init(pdfPack: lossyRoot.appendingPathComponent("Artifacts/PDFPack"), workerExecutable: lossyRoot.appendingPathComponent(".build/debug/fileform-worker"))
}
private func lossyFixture(_ fixture: Fixture) async throws -> URL {
    let generated = try await ProcessRunner.run(executable: URL(fileURLWithPath: "/usr/bin/env"), arguments: ["python3", lossyRoot.appendingPathComponent("Tools/generate-pdf-optimization-fixtures.py").path, fixture.url("inputs").path, "256"], timeout: 60)
    #expect(generated.status == 0)
    return fixture.url("inputs/landscape.pdf")
}
private func lossyRequest(_ input: URL, _ destination: URL, _ parameters: PDFOptimizationParameters = .init(maximumImageDimension: 128), collision: CollisionPolicy = .fail) throws -> TransformationRequest {
    try .init(assets: [.init(id: "pdf", url: input)], operation: .pdfOptimize(parameters: parameters), output: .init(destination: destination, format: .pdf), collisionPolicy: collision)
}
@Test func pdfLossyOptimizationPreservesDocumentAndResamplesSupportedImages() async throws {
    let fixture = try Fixture(); defer { fixture.cleanup() }
    let input = try await lossyFixture(fixture), engine = lossyEngine(), sourceBytes = try Data(contentsOf: input)
    let request = try lossyRequest(input, fixture.url("optimized.pdf"))
    let plan = try await engine.plan(request), detail = try #require(plan.pdfOptimization)
    #expect(detail.candidates.count == 3 && detail.optimizedImageCount == 2 && detail.resampledImageCount == 1)
    #expect(detail.candidates.first { $0.objectNumber == 10 }?.outputWidth == 128)
    #expect(detail.candidates.first { $0.objectNumber == 16 }?.skipReason?.contains("color space") == true)
    let result = try await engine.run(plan), output = try #require(result.artifacts.first)
    #expect(result.status == .succeeded && result.attempts == 1 && output.bytes < sourceBytes.count)
    #expect(result.pdfOptimization?.usedQuality == 0.8 && result.pdfOptimization?.attemptedQualities == [0.8])
    let before = try #require(PDFDocument(url: input)), after = try #require(PDFDocument(url: output.url))
    #expect(before.pageCount == 3 && after.pageCount == 3)
    #expect(before.documentAttributes?[PDFDocumentAttribute.titleAttribute] as? String == after.documentAttributes?[PDFDocumentAttribute.titleAttribute] as? String)
    for index in 0..<3 {
        let a = try #require(before.page(at: index)), b = try #require(after.page(at: index))
        #expect(a.string == b.string && b.string?.contains("Vector Form text retained") == true)
        #expect(a.rotation == b.rotation)
        for box: PDFDisplayBox in [.mediaBox, .cropBox, .trimBox, .bleedBox, .artBox] { #expect(a.bounds(for: box) == b.bounds(for: box)) }
    }
    let metadata = fixture.url("output.json")
    let qpdf = try await ProcessRunner.run(executable: lossyRoot.appendingPathComponent("Artifacts/PDFPack/bin/qpdf"), arguments: [output.url.path, "--json", "--json-key=qpdf", "--json-key=pages", "--json-key=encrypt"], stdoutFile: metadata)
    #expect(qpdf.status == 0)
    let graph = try PDFImageGraph(data: Data(contentsOf: metadata))
    let images = try graph.images(sourceID: "pdf", pages: (0..<3).map { .init(sourceID: "pdf", pageIndex: $0) })
    #expect(images.filter { $0.candidate.filters == ["/DCTDecode"] }.count == 2)
    #expect(images.contains { $0.candidate.width == 128 && $0.candidate.height == 85 })
    #expect(try Data(contentsOf: input) == sourceBytes)
    let recipe = try TransformationRecipe(name: "PDF email", request: request)
    let rebound = try JSONDecoder().decode(TransformationRecipe.self, from: JSONEncoder().encode(recipe)).bind(assets: request.assets, destination: fixture.url("rebound.pdf"))
    #expect(rebound.operation == request.operation)
    let restored = try JSONDecoder().decode(TransformationResult.self, from: JSONEncoder().encode(result))
    #expect(restored.pdfOptimization == result.pdfOptimization)
}
@Test func pdfLossyOptimizationQualityFloorAndFitUseOriginals() async throws {
    let fixture = try Fixture(); defer { fixture.cleanup() }
    let input = try await lossyFixture(fixture), engine = lossyEngine()
    let floor = 0.35, high = 0.95
    let floorResult = try await engine.run(engine.plan(lossyRequest(input, fixture.url("floor.pdf"), .init(quality: floor, minimumQuality: floor))))
    let floorBytes = try #require(floorResult.artifacts.first?.bytes)
    let fit = try await engine.run(engine.plan(lossyRequest(input, fixture.url("fit.pdf"), .init(goal: .fit, quality: high, minimumQuality: floor, maximumBytes: floorBytes))))
    #expect(fit.artifacts.first?.bytes == floorBytes && fit.attempts > 1 && fit.attempts <= 6)
    #expect(fit.pdfOptimization?.usedQuality == floor && fit.pdfOptimization?.attemptedQualities.last == floor)
    #expect(fit.pdfOptimization?.attemptedQualities.allSatisfy { $0 >= floor && $0 <= high } == true)
    // Reaching the exact independently encoded floor byte count proves the fit
    // route did not compound JPEG loss through its prior attempts.
    let impossible = try await engine.plan(lossyRequest(input, fixture.url("impossible.pdf"), .init(goal: .fit, quality: high, minimumQuality: floor, maximumBytes: 1)))
    do { _ = try await engine.run(impossible); Issue.record("Expected target miss") }
    catch let error as FileformError { #expect(error.code == .targetUnmet); #expect(error.message.contains("6 verified attempts")); #expect(error.pdfOptimization?.attemptedQualities.last == floor && error.pdfOptimization?.usedQuality == nil) }
    #expect(!FileManager.default.fileExists(atPath: fixture.url("impossible.pdf").path))
    #expect(try FileManager.default.contentsOfDirectory(atPath: fixture.directory.path).allSatisfy { !$0.hasPrefix(".fileform-") })
}
@Test func pdfLossyOptimizationSafetyCancellationAndPolicyValidation() async throws {
    let fixture = try Fixture(); defer { fixture.cleanup() }
    let input = try await lossyFixture(fixture), engine = lossyEngine(), destination = fixture.url("output.pdf")
    for parameters in [PDFOptimizationParameters(goal: .convert), .init(quality: .nan), .init(quality: 0.4, minimumQuality: 0.5), .init(maximumImageDimension: 16385), .init(goal: .fit), .init(maximumBytes: 100)] {
        #expect(throws: FileformError.self) { try lossyRequest(input, destination, parameters) }
    }
    #expect(throws: FileformError.self) { try TransformationRequest(assets: [.init(id: "p", url: input)], operation: .pdfOptimize(parameters: .init()), output: .init(destination: destination, format: .pdf), fidelity: .requireLossless) }
    let plan = try await engine.plan(lossyRequest(input, destination))
    try Data("existing".utf8).write(to: destination)
    await #expect(throws: FileformError.self) { try await engine.run(plan) }
    let renamed = try await engine.run(engine.plan(lossyRequest(input, destination, collision: .rename)))
    #expect(renamed.artifacts.first?.url.lastPathComponent == "output-1.pdf")
    #expect(try String(contentsOf: destination, encoding: .utf8) == "existing")
    let alias = fixture.url("alias.pdf"); try FileManager.default.linkItem(at: input, to: alias)
    await #expect(throws: FileformError.self) { try await engine.plan(lossyRequest(input, alias)) }
    let cancellation = try await engine.plan(lossyRequest(input, fixture.url("cancelled.pdf")))
    let (events, continuation) = AsyncStream<ProgressEvent>.makeStream()
    let task = Task { defer { continuation.finish() }; return try await engine.run(cancellation) { continuation.yield($0) } }
    for await event in events { if event.phase == .encoding { task.cancel(); break } }
    do { _ = try await task.value; Issue.record("Expected cancellation") } catch is CancellationError {}
    #expect(!FileManager.default.fileExists(atPath: fixture.url("cancelled.pdf").path))
    let changed = try await engine.plan(lossyRequest(input, fixture.url("changed.pdf")))
    try Data("changed".utf8).write(to: input)
    do { _ = try await engine.run(changed); Issue.record("Expected input changed") } catch let error as FileformError { #expect(error.code == .inputChanged) }
}
@Test func pdfLossyGraphProofRejectsFontVectorResourceAndImageTampering() throws {
    func proof(_ objects: [String: Any]) throws -> String {
        try PDFOptimizationGraphProof(data: JSONSerialization.data(withJSONObject: ["qpdf": [["jsonversion": 2], objects]])).digest()
    }
    let objects: [String: Any] = [
        "trailer": ["value": ["/Root": "1 0 R", "/Size": 5]],
        "obj:1 0 R": ["value": ["/Resources": "2 0 R"]],
        "obj:2 0 R": ["value": ["/Font": ["/F1": "3 0 R"], "/XObject": ["/Form": "4 0 R"]]],
        "obj:3 0 R": ["value": ["/BaseFont": "/Helvetica"]],
        "obj:4 0 R": ["stream": ["dict": ["/Subtype": "/Form", "/Length": 3], "data": "YWJj"]]
    ]
    let baseline = try proof(objects)
    var font = objects; font["obj:3 0 R"] = ["value": ["/BaseFont": "/Courier"]]
    #expect(try proof(font) != baseline)
    var vector = objects; vector["obj:4 0 R"] = ["stream": ["dict": ["/Subtype": "/Form", "/Length": 3], "data": "ZGVm"]]
    #expect(try proof(vector) != baseline)
    var resource = objects; resource["obj:2 0 R"] = ["value": ["/Font": ["/F2": "3 0 R"], "/XObject": ["/Form": "4 0 R"]]]
    #expect(try proof(resource) != baseline)
    var transport = objects; transport["trailer"] = ["value": ["/Root": "1 0 R", "/Size": 99, "/Type": "/XRef", "/W": [1, 2, 1], "/Filter": "/FlateDecode", "/Length": 200, "/ID": ["b:aa", "b:bb"]]]
    #expect(try proof(transport) == baseline)
    var semantic = objects; semantic["obj:4 0 R"] = ["stream": ["dict": ["/Subtype": "/Form", "/Filter": "/DCTDecode", "/Length": 3], "data": "YWJj"]]
    #expect(try proof(semantic) != baseline)
}
@Test func pdfLossyOptimizationRetainsMasksProfilesAndUnsupportedImageBytes() async throws {
    let fixture = try Fixture(); defer { fixture.cleanup() }
    let generated = try await ProcessRunner.run(executable: URL(fileURLWithPath: "/usr/bin/env"), arguments: ["python3", lossyRoot.appendingPathComponent("Tools/generate-pdf-image-fixtures.py").path, fixture.url("inputs").path], timeout: 60)
    #expect(generated.status == 0)
    let engine = lossyEngine(), source = fixture.url("inputs/primary-mask.pdf")
    let plan = try await engine.plan(lossyRequest(source, fixture.url("masks.pdf"), .init(goal: .fit, maximumImageDimension: 1, maximumBytes: 100_000)))
    let details = try #require(plan.pdfOptimization)
    #expect(details.candidates.count == 3 && details.optimizedImageCount == 1)
    #expect(details.candidates.first { $0.objectNumber == 8 }?.skipReason?.contains("masks") == true)
    #expect(details.candidates.first { $0.objectNumber == 7 }?.skipReason?.contains("Masked") == true)
    #expect(try await engine.run(plan).status == .succeeded)
    // Re-encode a proper ICC-tagged JPEG: the PDF's DeviceRGB declaration
    // cannot authorize discarding its embedded profile.
    let jpeg = fixture.url("icc.jpg")
    let program = """
    import Foundation
    import CoreGraphics
    import ImageIO
    let space = CGColorSpace(name: CGColorSpace.displayP3)!
    let bytes = Data([255,0,0, 0,255,0, 0,0,255, 255,255,255])
    let image = CGImage(width:2,height:2,bitsPerComponent:8,bitsPerPixel:24,bytesPerRow:6,space:space,bitmapInfo:CGBitmapInfo(rawValue:0),provider:CGDataProvider(data:bytes as CFData)!,decode:nil,shouldInterpolate:false,intent:.defaultIntent)!
    let dst = CGImageDestinationCreateWithURL(URL(fileURLWithPath:CommandLine.arguments[1]) as CFURL,"public.jpeg" as CFString,1,nil)!
    CGImageDestinationAddImage(dst,image,[kCGImageDestinationLossyCompressionQuality:1] as CFDictionary)
    assert(CGImageDestinationFinalize(dst))
    """
    let script = fixture.url("icc.swift"); try program.write(to: script, atomically: true, encoding: .utf8)
    let encoded = try await ProcessRunner.run(executable: URL(fileURLWithPath: "/usr/bin/swift"), arguments: [script.path, jpeg.path], timeout: 60)
    #expect(encoded.status == 0)
    #expect(try Data(contentsOf: jpeg).range(of: Data("ICC_PROFILE\0".utf8)) != nil)
    let worker = NativeWorkerClient(executable: lossyRoot.appendingPathComponent(".build/debug/fileform-worker"))
    do {
        _ = try await worker.optimizePDFImage(jpeg, destination: fixture.url("discarded.jpg"), width: 2, height: 2, channels: 3, encodedJPEG: true, outputWidth: 1, outputHeight: 1, quality: 0.5)
        Issue.record("Expected embedded ICC rejection")
    } catch let error as FileformError { #expect(error.code == .unsupported) }
    #expect(!FileManager.default.fileExists(atPath: fixture.url("discarded.jpg").path))
}
