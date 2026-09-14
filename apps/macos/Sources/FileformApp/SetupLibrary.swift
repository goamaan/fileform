import Foundation
import Observation
import FileformDomain

/// Stores public, path-free recipes. File permissions belong to WorkspaceStore.
@MainActor @Observable final class SetupLibrary {
    private(set) var recipes: [TransformationRecipe] = []
    private(set) var storageError: String?
    private var storageURL: URL?
    private var canWrite = true
    private struct Record: Codable {
        var schemaVersion = 1
        var recipes: [TransformationRecipe]
    }
    func connect(to url: URL) {
        guard storageURL == nil else { return }
        storageURL = url
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        do {
            let data = try Self.read(url, maximumBytes: 4 * 1_048_576)
            let record = try JSONDecoder().decode(Record.self, from: data)
            guard record.schemaVersion == 1, record.recipes.count <= 200,
                  Set(record.recipes.map(\.id)).count == record.recipes.count else {
                throw FileformError(.invalidRequest, "Unsupported or invalid saved setup library.")
            }
            recipes = record.recipes
        } catch {
            canWrite = false
            storageError = "The saved setup library could not be read. It has been left unchanged; saving setups is paused."
        }
    }
    func recipe(_ id: UUID) -> TransformationRecipe? { recipes.first { $0.id == id } }
    @discardableResult
    func save(name: String, request: TransformationRequest, replacing id: UUID? = nil) throws -> TransformationRecipe {
        let previous = id.flatMap(recipe)
        guard id == nil || previous != nil else { throw FileformError(.invalidRequest, "This setup no longer exists.") }
        guard (previous?.revision ?? 0) < Int.max else { throw FileformError(.resourceLimit, "This setup revision cannot be incremented.") }
        let value = try TransformationRecipe(id: id ?? UUID(), revision: (previous?.revision ?? 0) + 1,
            name: name.trimmingCharacters(in: .whitespacesAndNewlines), request: request)
        try put(value)
        return value
    }
    func rename(_ id: UUID, to name: String) throws {
        guard let previous = recipe(id) else { throw FileformError(.invalidRequest, "This setup no longer exists.") }
        let request = try previous.bind(assets: previous.assetSlots.enumerated().map {
            .init(id: $0.element, url: URL(fileURLWithPath: "/fileform-setup/source-\($0.offset)"))
        }, destination: URL(fileURLWithPath: "/fileform-setup/output"))
        try save(name: name, request: request, replacing: id)
    }
    func remove(_ id: UUID) throws {
        try commit(recipes.filter { $0.id != id })
    }
    func importRecipe(_ value: TransformationRecipe, replaceExisting: Bool) throws {
        try value.validate()
        if let current = recipe(value.id) {
            guard replaceExisting, value.revision > current.revision else {
                throw FileformError(.invalidRequest, "Import a newer revision to replace this saved setup. The current revision is retained.")
            }
        }
        try put(value)
    }
    static func readRecipe(at url: URL) throws -> TransformationRecipe {
        let scope = url.startAccessingSecurityScopedResource()
        defer { if scope { url.stopAccessingSecurityScopedResource() } }
        let data = try read(url, maximumBytes: 1_048_576)
        do { return try JSONDecoder().decode(TransformationRecipe.self, from: data) }
        catch let error as FileformError { throw error }
        catch { throw FileformError(.invalidRequest, "This file is not a supported Fileform setup.") }
    }
    static func export(_ recipe: TransformationRecipe, to url: URL) throws {
        try recipe.validate()
        let encoder = JSONEncoder(); encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(recipe)
        guard data.count <= 1_048_576 else { throw FileformError(.resourceLimit, "A portable setup cannot exceed 1 MiB.") }
        let scope = url.startAccessingSecurityScopedResource()
        defer { if scope { url.stopAccessingSecurityScopedResource() } }
        // Exclusive creation protects every existing file, including source aliases.
        // The native chooser may offer replacement; export still requires a new name.
        do { try data.write(to: url, options: .withoutOverwriting) }
        catch { throw FileformError(.ioFailure, "Could not export the setup. Choose a new filename in a writable folder; existing files are never replaced.") }
    }
    private static func read(_ url: URL, maximumBytes: Int) throws -> Data {
        guard try url.resolvingSymlinksInPath().resourceValues(forKeys: [.isRegularFileKey]).isRegularFile == true else {
            throw FileformError(.invalidRequest, "Choose a regular JSON setup file.")
        }
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
        let data = try file.read(upToCount: maximumBytes + 1) ?? Data()
        guard data.count <= maximumBytes else { throw FileformError(.resourceLimit, "The setup file exceeds the supported size.") }
        return data
    }
    private func put(_ value: TransformationRecipe) throws {
        var updated = recipes
        if let index = updated.firstIndex(where: { $0.id == value.id }) { updated[index] = value }
        else { updated.append(value) }
        try commit(updated)
    }
    private func commit(_ values: [TransformationRecipe]) throws {
        guard canWrite else { throw FileformError(.ioFailure, storageError ?? "Saving setups is paused.") }
        guard values.count <= 200 else { throw FileformError(.resourceLimit, "The current library holds up to 200 saved setups.") }
        for recipe in values { try recipe.validate() }
        let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys]
        let data = try encoder.encode(Record(recipes: values))
        guard data.count <= 4 * 1_048_576 else { throw FileformError(.resourceLimit, "The setup library exceeds 4 MiB.") }
        if let storageURL {
            try FileManager.default.createDirectory(at: storageURL.deletingLastPathComponent(), withIntermediateDirectories: true,
                                                    attributes: [.posixPermissions: 0o700])
            try data.write(to: storageURL, options: .atomic)
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: storageURL.path)
        }
        recipes = values
    }
}

extension TransformationRecipe {
    var setupSummary: String {
        switch operation {
        case .conversion(let parameters): conversionSummary(parameters)
        case .imageCrop(let crop, let parameters): "Crop \(crop.width)×\(crop.height) · " + conversionSummary(parameters)
        case .pdfComposition(let pages): "Combine \(assetSlots.count) \(assetSlots.count == 1 ? "source" : "sources") · \(pages.count) \(pages.count == 1 ? "page" : "pages")"
        case .pdfSplit(let groups): "\(assetSlots.count) \(assetSlots.count == 1 ? "source" : "sources") · \(groups.count) PDF\(groups.count == 1 ? "" : "s")"
        case .pdfRasterize(let pages, let dpi, _): "\(pages.count) \(format.title) images · \(dpi) DPI"
        case .pdfExtractImages(let pages): "Embedded images from \(pages.count) PDF page\(pages.count == 1 ? "" : "s")"
        case .pdfOptimize(let value): "PDF image compression · \(value.quality.formatted(.percent.precision(.fractionLength(0)))) quality" + (value.maximumImageDimension.map { " · \($0)px maximum" } ?? "")
        case .mediaTrim: "Trim recording · \(format.title)"
        case .fetch: "Link acquisition"
        }
    }
    private func conversionSummary(_ parameters: ConversionParameters) -> String {
        var pieces = [format.title]
        if parameters.goal == .compress { pieces.append("make smaller") }
        if let bytes = parameters.options.maximumBytes, parameters.goal == .fit {
            pieces.append("under \(NSDecimalNumber(decimal: Decimal(bytes) / 1_000_000).stringValue) MB")
        }
        if let dimension = parameters.options.maxDimension { pieces.append("\(dimension)px maximum") }
        if let page = parameters.options.pageNumber { pieces.append("page \(page)") }
        return pieces.joined(separator: " · ")
    }
    var isCompositionSetup: Bool {
        switch operation { case .pdfComposition, .pdfSplit, .pdfRasterize, .pdfExtractImages: true; default: false }
    }
}
