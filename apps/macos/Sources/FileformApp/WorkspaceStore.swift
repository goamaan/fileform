import Foundation
import FileformDomain

/// Local permission records are deliberately separate from portable engine recipes.
struct WorkspaceFileReference: Codable, Equatable {
    let lastKnownURL: URL
    let bookmark: Data

    init(_ url: URL, readOnly: Bool) throws {
        lastKnownURL = url
        bookmark = try url.bookmarkData(options: readOnly ? [.withSecurityScope, .securityScopeAllowOnlyReadAccess] : [.withSecurityScope],
                                        includingResourceValuesForKeys: nil, relativeTo: nil)
    }
    func resolve() throws -> URL {
        var stale = false
        let url = try URL(resolvingBookmarkData: bookmark, options: [.withSecurityScope, .withoutUI, .withoutMounting],
                          relativeTo: nil, bookmarkDataIsStale: &stale)
        guard url.isFileURL else { throw WorkspaceStoreError.invalid }
        return url // A fresh bookmark is captured on the next save, including stale resolutions.
    }
}

enum WorkspaceStoreError: LocalizedError {
    case invalid, tooLarge
    var errorDescription: String? {
        switch self {
        case .invalid: "The saved workspace could not be read. It has been left unchanged."
        case .tooLarge: "The workspace is too large to save. Remove older items from the list."
        }
    }
}

struct FileJobDraft: Codable {
    var format: OutputFormat
    var outputName: String?
    var goal: ConversionGoal
    var quality: Double
    var minimumQuality: Double
    var sizeLimitMB: String
    var resize: Bool
    var longestEdge: String
    var background: String
    var minimumVideoBitrate: String
    var pdfPage: String
    var pdfCompression: PDFCompressionDraft?
    var crop: ImageCropDraft
    var trimDraft: MediaTrimDraft?
    var collisionPolicy: CollisionPolicy?
    @MainActor init(_ job: FileJob) {
        outputName = job.outputName
        trimDraft = job.trimDraft
        pdfCompression = job.pdfCompression
        format = job.format; goal = job.goal; quality = job.quality; minimumQuality = job.minimumQuality
        sizeLimitMB = job.sizeLimitMB; resize = job.resize; longestEdge = job.longestEdge
        collisionPolicy = job.collisionPolicy
        background = job.background; minimumVideoBitrate = job.minimumVideoBitrate; pdfPage = job.pdfPage; crop = job.crop
    }
    @MainActor func apply(to job: FileJob) {
        job.outputName = outputName ?? ""
        job.trimDraft = trimDraft
        job.pdfCompression = pdfCompression ?? .init()
        job.format = format; job.goal = goal; job.quality = quality; job.minimumQuality = minimumQuality
        job.sizeLimitMB = sizeLimitMB; job.resize = resize; job.longestEdge = longestEdge
        job.collisionPolicy = collisionPolicy ?? .rename
        job.background = background; job.minimumVideoBitrate = minimumVideoBitrate; job.pdfPage = pdfPage; job.crop = crop
    }
}
/// Provenance is metadata, not permission to reopen the parent path.
struct ResultOrigin: Codable, Equatable {
    var rootJobID: UUID
    var parentJobID: UUID
    var parentOutput: URL
}
struct WorkspaceJobRecord: Codable {
    var id: UUID
    var source: WorkspaceFileReference
    var draft: FileJobDraft
    var state: FileJobState
    var outputs: [WorkspaceFileReference]
    var lastTrimDetails: MediaTrimDetails?
    var lastPDFOptimizationDetails: PDFOptimizationDetails?
    var origin: ResultOrigin?
}
struct SavedLinkRecord: Codable {
    var id: UUID
    var date: Date
    var output: WorkspaceFileReference
    var format: OutputFormat
    var receipt: FetchReceipt
}
struct WorkspaceRecord: Codable {
    var schemaVersion = 1
    var task: WorkspaceTask
    var selection: UUID?
    var destination: WorkspaceFileReference?
    var jobs: [WorkspaceJobRecord]
    var pages: [PDFPageDraft]
    var pageSelection: UUID?
    var pdfSourceIDs: [String]
    var pdfOutputName: String
    var pdfInterrupted: Bool
    var pdfOutputs: [WorkspaceFileReference]
    var pdfArtifacts: [CommittedArtifact] = []
    var pdfCollisionPolicy: CollisionPolicy?
    var pdfExportAsDirectory: Bool?
    var pdfWasOpened: Bool?
    var pdfPendingSourceIDs: [UUID]?
    var linkHistory: [SavedLinkRecord]?
    var linkInterrupted: Bool?
    var linkOpenInTrim: Bool?
    var batchSelection: [UUID]?
    var runSelectionOnly: Bool?
    var pdfSelectedPages: [UUID]?
    var pdfPageOutput: PDFPageOutputOptions?
}

extension WorkspaceRecord {
    func validate() throws {
        guard pdfPageOutput?.isValidRecord ?? true,
              jobs.allSatisfy({ $0.draft.pdfCompression?.isValidRecord ?? true }),
              Set(pdfSelectedPages ?? []).isSubset(of: Set(pages.map(\.id))),
              Set(batchSelection ?? []).isSubset(of: Set(jobs.map(\.id))),
              (linkHistory?.count ?? 0) <= 100, (linkHistory ?? []).allSatisfy({ $0.output.lastKnownURL.isFileURL }),
              self.schemaVersion == 1, self.pdfOutputs.count == self.pdfArtifacts.count, self.jobs.count <= 1000, self.pages.count <= 1000,
              Set(self.jobs.map(\.id)).count == self.jobs.count,
              Set(self.pages.map(\.id)).count == self.pages.count,
              self.jobs.allSatisfy({ $0.source.lastKnownURL.isFileURL && $0.outputs.allSatisfy { $0.lastKnownURL.isFileURL } }),
              Set(self.pages.map(\.sourceID)).isSubset(of: Set(self.pdfSourceIDs)),
              Set(self.pdfSourceIDs).isSubset(of: Set(self.jobs.map { $0.id.uuidString })),
              Set(self.pdfPendingSourceIDs ?? []).isSubset(of: Set(self.jobs.map(\.id))) else { throw WorkspaceStoreError.invalid }
    }
}

/// Small, bounded metadata only: no previews, source contents, diagnostics or credentials.
/// Called on the main actor so snapshots and atomic replacements cannot reorder.
@MainActor final class WorkspaceStore {
    let url: URL
    private var references: [String: WorkspaceFileReference] = [:]
    private var restoredScopes: [URL] = []
    private let maximumBytes = 4 * 1024 * 1024
    init(url: URL) { self.url = url }
    static func applicationStore() -> WorkspaceStore {
        let directory = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Fileform", isDirectory: true)
        return WorkspaceStore(url: directory.appendingPathComponent("workspace-v1.json"))
    }
    func reference(_ url: URL, readOnly: Bool = true) throws -> WorkspaceFileReference {
        let key = "\(readOnly):\(url.absoluteString)"
        if let saved = references[key] { return saved }
        let saved = try WorkspaceFileReference(url, readOnly: readOnly)
        references[key] = saved
        return saved
    }
    func resolve(_ reference: WorkspaceFileReference) -> URL? {
        guard let url = try? reference.resolve() else { return nil }
        let scoped = url.startAccessingSecurityScopedResource()
        guard FileManager.default.fileExists(atPath: url.path) else {
            if scoped { url.stopAccessingSecurityScopedResource() }
            return nil
        }
        if scoped { restoredScopes.append(url) }
        return url
    }
    func releaseReferences() {
        for url in restoredScopes { url.stopAccessingSecurityScopedResource() }
        restoredScopes.removeAll(); references.removeAll()
    }
    func load() throws -> WorkspaceRecord? {
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        guard let size = try url.resourceValues(forKeys: [.fileSizeKey]).fileSize, size <= maximumBytes else { throw WorkspaceStoreError.tooLarge }
        let record: WorkspaceRecord
        do { record = try JSONDecoder().decode(WorkspaceRecord.self, from: Data(contentsOf: url)) }
        catch { throw WorkspaceStoreError.invalid }
        try record.validate()
        return record
    }
    func save(_ record: WorkspaceRecord) throws {
        try record.validate()
        guard record.jobs.count <= 1000, record.pages.count <= 1000 else { throw WorkspaceStoreError.tooLarge }
        let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys]
        let data = try encoder.encode(record)
        guard data.count <= maximumBytes else { throw WorkspaceStoreError.tooLarge }
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true,
                                                attributes: [.posixPermissions: 0o700])
        try data.write(to: url, options: [.atomic])
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
    }
}
