import Foundation
import Observation
import FileformCore
import FileformDomain

struct LinkSavedResult: Identifiable {
    var id = UUID()
    var date = Date()
    var url: URL
    var format: OutputFormat
    var receipt: FetchReceipt
}

@MainActor @Observable final class LinkWorkspaceModel {
    var networkAllowed = true {
        didSet {
            guard networkAllowed != oldValue else { return }
            if !networkAllowed { cancel(); invalidateLookup() }
        }
    }
    var urlText = "" { didSet { if urlText != oldValue { invalidateLookup() } } }
    var maximumMB = "512" { didSet { if maximumMB != oldValue { invalidateLookup() } } }
    var format: OutputFormat = .mp4 { didSet { rebuildPlan() } }
    var outputName = "Saved recording" { didSet { rebuildPlan() } }
    var collision: CollisionPolicy = .rename { didSet { rebuildPlan() } }
    var destination: URL? { didSet { rebuildPlan() } }
    var openInTrim = false
    var history: [LinkSavedResult] = []
    private(set) var source: FetchSourceSnapshot?
    private(set) var plan: TransformationPlan?
    private(set) var isLookingUp = false
    private(set) var isSaving = false
    private(set) var phase: JobPhase?
    private(set) var fraction: Double?
    var error: String?
    var notice: String?
    var onSaved: ((LinkSavedResult, Bool) -> Void)?
    private let engine: ConversionEngine
    private var approvedLookup: TransformationPlan?
    private var lookupTask: Task<Void, Never>?
    private var saveTask: Task<Void, Never>?
    private var lookupID = UUID()
    private var saveID = UUID()
    init(engine: ConversionEngine) { self.engine = engine }
    var isBusy: Bool { isLookingUp || isSaving }
    var canLookup: Bool { networkAllowed && !isBusy && !urlText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    var canRun: Bool { networkAllowed && !isBusy && destination != nil && plan != nil }
    var status: String {
        if isLookingUp { return "Looking up source…" }
        switch phase {
        case .inspecting: return "Checking the source again…"
        case .preparing: return fraction.map { "Downloading · \(Int($0 * 100))%" } ?? "Downloading…"
        case .verifying: return "Checking the complete file…"
        case .saving: return "Saving…"
        default: return ""
        }
    }
    func lookup() {
        guard canLookup else { return }
        invalidateLookup(); error = nil; notice = nil
        let id = lookupID
        do {
            let url = try sourceURL(), maximum = try byteLimit()
            isLookingUp = true
            lookupTask = Task {
                defer { if lookupID == id { isLookingUp = false; lookupTask = nil } }
                do {
                    let metadata = try await DirectHTTPClient().inspect(url,
                        policy: .init(maximumBytes: maximum, allowInsecureHTTP: url.scheme?.lowercased() == "http"))
                    try Task.checkCancellation()
                    guard lookupID == id else { return }
                    guard !["text/html", "application/xhtml+xml"].contains(metadata.contentType ?? "") else {
                        throw FileformError(.unsupported, "This link returns a web page. Use a direct link to an audio or video file; a site downloader is not installed.")
                    }
                    format = Self.suggestedFormat(url: metadata.url, contentType: metadata.contentType)
                    outputName = Self.suggestedName(metadata.url)
                    let value = try await engine.plan(request(url: url, maximum: maximum))
                    try Task.checkCancellation()
                    guard lookupID == id else { return }
                    approvedLookup = value; source = value.fetchSource
                    rebuildPlan()
                } catch is CancellationError { if lookupID == id { notice = "Lookup cancelled." } }
                catch { if lookupID == id { self.error = error.localizedDescription } }
            }
        } catch { self.error = error.localizedDescription }
    }
    func run() {
        guard canRun, let plan else { return }
        let openAfter = openInTrim
        let id = UUID(); saveID = id
        isSaving = true; error = nil; notice = nil; phase = .inspecting; fraction = nil
        saveTask = Task {
            defer { isSaving = false; saveTask = nil; phase = nil; fraction = nil }
            do {
                let result = try await engine.run(plan) { [weak self] event in
                    Task { @MainActor in
                        guard let self, self.isSaving, self.saveID == id else { return }
                        self.phase = event.phase; self.fraction = event.fraction
                    }
                }
                guard let artifact = result.artifacts.first, let receipt = result.fetchReceipt else {
                    throw FileformError(.verificationFailed, "The saved file has no verification receipt.")
                }
                let saved = LinkSavedResult(url: artifact.url, format: artifact.format, receipt: receipt)
                history.insert(saved, at: 0)
                if history.count > 100 { history.removeLast(history.count - 100) }
                notice = "Saved and verified \(artifact.url.lastPathComponent)."
                onSaved?(saved, openAfter)
            } catch is CancellationError { notice = "Download cancelled. Existing files are kept." }
            catch { self.error = error.localizedDescription }
        }
    }
    func cancel() { lookupTask?.cancel(); saveTask?.cancel() }
    func invalidateLookup() {
        lookupTask?.cancel(); lookupTask = nil; lookupID = UUID(); isLookingUp = false
        source = nil; plan = nil; approvedLookup = nil; error = nil; notice = nil
    }
    private func rebuildPlan() {
        plan = nil
        guard let base = approvedLookup, let source = base.fetchSource else { return }
        do {
            let request = try request(url: source.requestedURL, maximum: byteLimit())
            plan = .init(request: request, inputs: [], warnings: base.warnings, fetchSource: source)
            error = nil
        } catch { self.error = error.localizedDescription }
    }
    func sourceURL() throws -> URL {
        guard let url = URL(string: urlText.trimmingCharacters(in: .whitespacesAndNewlines)),
              ["https", "http"].contains(url.scheme?.lowercased() ?? ""), url.host?.isEmpty == false else {
            throw FileformError(.invalidRequest, "Paste a full link beginning with https:// or http://.")
        }
        try HTTPAcquisitionPolicy(maximumBytes: byteLimit(), allowInsecureHTTP: true).validate(url)
        return url
    }
    private func byteLimit() throws -> Int64 {
        guard let value = Int64(maximumMB), (1...8000).contains(value) else {
            throw FileformError(.invalidRequest, "Enter a whole-number download limit from 1 to 8000 MB.")
        }
        return value * 1_000_000
    }
    private func request(url: URL, maximum: Int64) throws -> TransformationRequest {
        let name = outputName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, name.utf8.count <= 240, !name.contains("/"), !name.contains(":"),
              !name.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains), name != ".", name != ".." else {
            throw FileformError(.invalidRequest, "Enter a filename without path separators or control characters.")
        }
        let suffix = "." + format.fileExtension
        let filename = name.lowercased().hasSuffix(suffix) ? name : name + suffix
        return try .init(assets: [], operation: .fetch(url: url, maximumBytes: maximum),
            output: .init(destination: (destination ?? FileManager.default.temporaryDirectory).appendingPathComponent(filename), format: format), collisionPolicy: collision)
    }
    static func suggestedFormat(url: URL, contentType: String?) -> OutputFormat {
        if let format = OutputFormat.allCases.first(where: { $0.family == .media && $0.fileExtension == url.pathExtension.lowercased() }) { return format }
        switch contentType?.lowercased() {
        case "audio/wav", "audio/x-wav", "audio/wave": return .wav
        case "audio/flac", "audio/x-flac": return .flac
        case "audio/mpeg": return .mp3
        case "audio/mp4", "audio/x-m4a": return .m4a
        case "video/quicktime": return .mov
        default: return .mp4
        }
    }
    static func suggestedName(_ url: URL) -> String {
        let text = url.deletingPathExtension().lastPathComponent
        var clean = text.unicodeScalars.filter { !CharacterSet.controlCharacters.contains($0) && $0 != "/" && $0 != ":" }.prefix(100).map(String.init).joined()
        while clean.utf8.count > 180 { clean.removeLast() }
        return clean.isEmpty || clean == "." || clean == ".." ? "Saved recording" : clean
    }
}
