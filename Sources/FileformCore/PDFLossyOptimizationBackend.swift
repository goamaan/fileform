// SPDX-License-Identifier: Apache-2.0
import Foundation
import FileformDomain

struct PDFLossyOptimizationBackend {
    let pack: PDFPack
    let worker: NativeWorkerClient
    static let warnings = [
        "Lossy JPEG recompression changes supported embedded image pixels; optional longest-edge limits resample intrinsic pixels, never whole PDF pages or inferred DPI. Text, vectors, page order, boxes, rotation and document metadata are preserved.",
        "Only unmasked 8-bit DeviceRGB/DeviceGray images with supported filters are rewritten. Masks, complex color spaces, embedded JPEG ICC profiles and unsupported images remain unchanged with explicit reasons. Embedded JPEG descriptive metadata is not retained in rewritten images.",
        "All reachable document objects and streams are checked against the exact expected replacements; page text, content, geometry and Info/XMP metadata are independently checked. Signed, encrypted, interactive, annotated, tagged and outlined PDFs remain unsupported. The PDF version may increase to 1.5.",
        "Fit tries at most six qualities from the requested value through the exact floor, each from original image samples. Every page remains; an unattainable byte target fails without publishing. Compression may report not smaller."
    ]
    private struct Discovery {
        let plan: TransformationPlan
        let images: [PDFImageGraph.Image]
        let graph: PDFImageGraph
        let fingerprint: String
        let sourceHash: String
    }
    private func inventory(_ input: URL, at destination: URL, proof: Bool) async throws -> Data {
        var arguments = [input.path, "--json", "--json-key=qpdf"]
        if proof { arguments += ["--json-stream-data=inline", "--decode-level=generalized"] }
        else { arguments += ["--json-key=pages", "--json-key=encrypt"] }
        let response = try await ProcessRunner.run(executable: pack.qpdf, arguments: arguments, timeout: 120,
            stdoutFile: destination, maximumStdoutBytes: Int64(proof ? 128 : 32) * 1024 * 1024)
        guard response.status == 0 else { throw FileformError(.unsupported, "PDF object inventory could not be read cleanly.") }
        return try Data(contentsOf: destination)
    }
    private func discover(_ request: TransformationRequest, scratch: URL, previous: [InspectedAsset]? = nil) async throws -> Discovery {
        try request.validate()
        guard case .pdfOptimize(let parameters) = request.operation else { throw FileformError(.invalidRequest, "Expected PDF image optimization.") }
        let asset = request.assets[0]
        if let previous {
            guard previous.count == 1, previous[0].id == asset.id, previous[0].inspection.input.standardizedFileURL == asset.url.standardizedFileURL else { throw FileformError(.invalidRequest, "PDF source bindings changed.") }
            try FileSafety.verifyUnchanged(previous[0].inspection)
        }
        guard try FileSafety.identity(asset.url).bytes <= 512 * 1024 * 1024 else { throw FileformError(.resourceLimit, "PDF source exceeds 512 MiB.") }
        let sourceHash = try PDFPack.hash(asset.url), info = try await worker.inspect(asset.url)
        guard info.family == .pdf else { throw FileformError(.unsupported, "PDF optimization accepts PDF sources only.") }
        try FileSafety.rejectSourceAliases(destination: request.output.destination, inputs: [info])
        let fingerprint = try await worker.pdfContentFingerprint(asset.url)
        let graph = try PDFImageGraph(data: await inventory(asset.url, at: scratch.appendingPathComponent("inventory.json"), proof: false))
        let pages = (0..<graph.pageObjects.count).map { PageReference(sourceID: asset.id, pageIndex: $0) }
        let images = try graph.images(sourceID: asset.id, pages: pages)
        var maskReferences = Set<String>()
        for object in graph.objects.values {
            guard let stream = (object as? [String: Any])?["stream"] as? [String: Any], let dictionary = stream["dict"] as? [String: Any] else { continue }
            for key in ["/Mask", "/SMask"] { if let reference = dictionary[key] as? String, PDFImageGraph.reference(reference) != nil { maskReferences.insert(reference) } }
        }
        var candidates: [PDFOptimizationImageCandidate] = [], totalSamples: Int64 = 0
        for (index, image) in images.enumerated() {
            try Task.checkCancellation()
            let original = image.candidate, dictionary = try graph.dict(image.reference)
            var reason = original.skipReason
            if reason == nil, image.softMask != nil || (dictionary["/SMask"] != nil && dictionary["/SMask"] as? String != "/None") || maskReferences.contains(image.reference) {
                reason = "Masked images and images used as masks remain unchanged."
            }
            if reason == nil, dictionary["/SMaskInData"] != nil { reason = "Images with embedded mask semantics remain unchanged." }
            var width = original.width, height = original.height
            if reason == nil, let sourceWidth = width, let sourceHeight = height {
                let samples = Int64(sourceWidth) * Int64(sourceHeight) * 4
                guard samples <= 512 * 1024 * 1024 - totalSamples else { throw FileformError(.resourceLimit, "Cumulative decoded image samples exceed 512 MiB.") }
                totalSamples += samples
                if let maximum = parameters.maximumImageDimension, max(sourceWidth, sourceHeight) > maximum {
                    let scale = Double(maximum) / Double(max(sourceWidth, sourceHeight))
                    width = max(1, Int(floor(Double(sourceWidth) * scale))); height = max(1, Int(floor(Double(sourceHeight) * scale)))
                }
                // Validate source encoding before advertising a rewrite in a plan.
                // Planning may encode private scratch, but never publishes a file.
                let input = scratch.appendingPathComponent("preflight-\(index).samples"), encoded = scratch.appendingPathComponent("preflight-\(index).jpg")
                try await extract(asset.url, image: image, destination: input)
                do {
                    _ = try await worker.optimizePDFImage(input, destination: encoded, width: sourceWidth, height: sourceHeight, channels: image.channels,
                        encodedJPEG: original.filters == ["/DCTDecode"], outputWidth: width!, outputHeight: height!, quality: parameters.quality)
                } catch let error as FileformError where error.code == .unsupported {
                    reason = "JPEG profile, orientation or sample interpretation is unsupported; original image retained."
                }
                try? FileManager.default.removeItem(at: input); try? FileManager.default.removeItem(at: encoded)
            }
            if reason != nil { width = original.width; height = original.height }
            candidates.append(.init(objectNumber: original.objectNumber, generation: original.generation, originalWidth: original.width,
                originalHeight: original.height, outputWidth: width, outputHeight: height, skipReason: reason))
        }
        try FileSafety.verifyUnchanged(info)
        guard try PDFPack.hash(asset.url) == sourceHash else { throw FileformError(.inputChanged, "PDF changed during planning.") }
        if let previous, previous[0].inspection.identity != info.identity { throw FileformError(.inputChanged, "PDF changed since planning.") }
        var warnings = Self.warnings
        let skipped = candidates.filter { $0.skipReason != nil }.count
        if skipped > 0 { warnings.append("\(skipped) embedded images remain unchanged; review their skip reasons.") }
        if candidates.isEmpty { warnings.append("No resource image objects were found; only structural optimization can reduce this PDF. Inline images remain unchanged.") }
        return .init(plan: .init(request: request, inputs: [.init(id: asset.id, inspection: info)], warnings: warnings,
            pdfOptimization: .init(candidates: candidates)), images: images, graph: graph, fingerprint: fingerprint, sourceHash: sourceHash)
    }
    func plan(_ request: TransformationRequest) async throws -> TransformationPlan {
        let scratch = FileManager.default.temporaryDirectory.appendingPathComponent("fileform-pdf-optimize-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
        defer { try? FileManager.default.removeItem(at: scratch) }
        let discovery = try await discover(request, scratch: scratch)
        // Prove the input is within the complete-object verification budget now,
        // so the displayed plan is executable under the same resource policy.
        _ = try PDFOptimizationGraphProof(data: await inventory(request.assets[0].url, at: scratch.appendingPathComponent("proof.json"), proof: true)).digest()
        return discovery.plan
    }
    func execute(_ supplied: TransformationPlan, progress: @Sendable (ProgressEvent) -> Void) async throws -> TransformationResult {
        progress(.init(.preparing))
        let transaction = try OutputTransaction(destination: supplied.request.output.destination, inputs: supplied.request.assets.map(\.url), collisionPolicy: supplied.request.collisionPolicy, directoryOutput: false)
        defer { transaction.cleanup() }
        let discovery = try await discover(supplied.request, scratch: transaction.directory, previous: supplied.inputs)
        let plan = discovery.plan, input = plan.request.assets[0].url
        guard case .pdfOptimize(let parameters) = plan.request.operation, let details = plan.pdfOptimization else { throw FileformError(.invalidRequest, "Invalid PDF optimization plan.") }
        let original = try PDFOptimizationGraphProof(data: await inventory(input, at: transaction.directory.appendingPathComponent("original-proof.json"), proof: true))
        let check = try await ProcessRunner.run(executable: pack.qpdf, arguments: ["--check", input.path], timeout: 60)
        guard check.status == 0 else { throw FileformError(.unsupported, "PDF failed strict structural validation.") }
        var qualities: [Double] = []
        let schedule = details.optimizedImageCount == 0 ? [parameters.quality] : parameters.qualitySchedule
        for (attempt, quality) in schedule.enumerated() {
            try Task.checkCancellation(); progress(.init(.encoding, fraction: Double(attempt) / Double(schedule.count)))
            let directory = transaction.directory.appendingPathComponent("attempt-\(attempt)")
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
            var expected = original, updates: [String: Any] = [:], encodedBytes: Int64 = 0
            for (index, image) in discovery.images.enumerated() {
                let candidate = details.candidates[index]
                guard candidate.skipReason == nil else { continue }
                try Task.checkCancellation()
                let samples = directory.appendingPathComponent("source-\(index)"), jpeg = directory.appendingPathComponent("image-\(index).jpg")
                // Always read original source streams, never a previous attempt.
                try await extract(input, image: image, destination: samples)
                let artifact = try await worker.optimizePDFImage(samples, destination: jpeg, width: candidate.originalWidth!, height: candidate.originalHeight!,
                    channels: image.channels, encodedJPEG: image.candidate.filters == ["/DCTDecode"], outputWidth: candidate.outputWidth!, outputHeight: candidate.outputHeight!, quality: quality)
                let bytes = try FileSafety.identity(jpeg).bytes
                guard artifact.bytes == bytes, bytes <= 128 * 1024 * 1024 - encodedBytes else { throw FileformError(.resourceLimit, "Replacement JPEG streams exceed the 128 MiB proof budget.") }
                encodedBytes += bytes
                var dictionary = try discovery.graph.dict(image.reference)
                dictionary.removeValue(forKey: "/Length"); dictionary.removeValue(forKey: "/DecodeParms")
                dictionary["/Width"] = candidate.outputWidth!; dictionary["/Height"] = candidate.outputHeight!; dictionary["/Filter"] = "/DCTDecode"
                updates["obj:" + image.reference] = ["stream": ["dict": dictionary, "datafile": jpeg.path]]
                expected.replace(reference: image.reference, dictionary: dictionary, jpeg: try Data(contentsOf: jpeg))
                try FileManager.default.removeItem(at: samples)
            }
            let update = directory.appendingPathComponent("update.json")
            let updateJSON: [String: Any] = ["qpdf": [["jsonversion": 2], updates]]
            try JSONSerialization.data(withJSONObject: updateJSON, options: [.sortedKeys]).write(to: update, options: .withoutOverwriting)
            let candidate = transaction.candidate(attempt, format: .pdf)
            var arguments = ["--compress-streams=y", "--decode-level=generalized", "--recompress-flate", "--compression-level=9", "--object-streams=generate"]
            if !updates.isEmpty { arguments.append("--update-from-json=" + update.path) }
            arguments += [input.path, candidate.path]
            let result = try await ProcessRunner.run(executable: pack.qpdf, arguments: arguments, timeout: 120, monitoredOutput: candidate)
            guard result.status == 0 else { throw FileformError(.engineFailed, "PDF image optimization failed; no output saved.") }
            progress(.init(.verifying))
            let check = try await ProcessRunner.run(executable: pack.qpdf, arguments: ["--check", candidate.path], timeout: 60)
            guard check.status == 0 else { throw FileformError(.verificationFailed, "Optimized PDF failed strict validation.") }
            let actual = try PDFOptimizationGraphProof(data: await inventory(candidate, at: directory.appendingPathComponent("output-proof.json"), proof: true))
            guard try actual.digest() == expected.digest() else {
                throw FileformError(.verificationFailed, "PDF did not preserve the exact expected document objects and streams.")
            }
            guard try await worker.pdfContentFingerprint(candidate) == discovery.fingerprint else {
                throw FileformError(.verificationFailed, "PDF did not preserve text, content, metadata and page geometry.")
            }
            try FileSafety.verifyUnchanged(plan.inputs[0].inspection)
            guard try PDFPack.hash(input) == discovery.sourceHash else { throw FileformError(.inputChanged, "PDF changed during image optimization.") }
            qualities.append(quality)
            let bytes = try FileSafety.identity(candidate).bytes
            let resultDetails = PDFOptimizationDetails(candidates: details.candidates, attemptedQualities: qualities, usedQuality: quality)
            try FileManager.default.removeItem(at: directory)
            if parameters.goal == .fit, bytes > parameters.maximumBytes! {
                try FileManager.default.removeItem(at: candidate); continue
            }
            if parameters.goal == .compress, bytes >= plan.inputs[0].inspection.identity.bytes {
                return .init(operationID: .pdfOptimize, status: .notSmaller, artifacts: [], warnings: plan.warnings + ["The candidate was not smaller; no output was published."], attempts: qualities.count, pdfOptimization: resultDetails)
            }
            try Task.checkCancellation(); progress(.init(.saving))
            let output = try transaction.commit(candidate)
            return .init(operationID: .pdfOptimize, status: .succeeded,
                artifacts: [.init(url: output, format: .pdf, bytes: bytes, sourceIDs: plan.request.assets.map(\.id))],
                warnings: plan.warnings, attempts: qualities.count, pdfOptimization: resultDetails)
        }
        throw FileformError(.targetUnmet, "All pages were retained, but the PDF could not meet the byte limit in \(qualities.count) verified attempts at qualities \(qualities.map { String(format: "%.4f", $0) }.joined(separator: ", ")). The requested quality floor was respected. Increase the limit or revise the explicit pixel bound.", pdfOptimization: .init(candidates: details.candidates, attemptedQualities: qualities))
    }
    private func extract(_ input: URL, image: PDFImageGraph.Image, destination: URL) async throws {
        let jpeg = image.candidate.filters == ["/DCTDecode"]
        let bound = jpeg ? Int64(256 * 1024 * 1024) : Int64(image.candidate.width! * image.candidate.height! * image.channels)
        let result = try await ProcessRunner.run(executable: pack.qpdf,
            arguments: [input.path, "--show-object=\(image.candidate.objectNumber),\(image.candidate.generation)", jpeg ? "--raw-stream-data" : "--filtered-stream-data"],
            timeout: 60, stdoutFile: destination, maximumStdoutBytes: max(1, bound))
        guard result.status == 0, try jpeg || FileSafety.identity(destination).bytes == bound else { throw FileformError(.verificationFailed, "Image stream could not be decoded completely.") }
    }
}
