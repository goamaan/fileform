import Foundation
import FileformDomain

// Entries represent implemented task families. Additional operations join this
// same rail/search as their adapters and native editors gain acceptance evidence.
enum WorkspaceTask: String, CaseIterable, Identifiable, Codable {
    case convert, pages, trim, link, text
    var id: String { rawValue }
    var title: String {
        switch self { case .convert: "Convert files"; case .pages: "Organize PDFs"; case .trim: "Trim audio or video"; case .link: "Save from a link"; case .text: "Extract text" }
    }
    var symbol: String {
        switch self { case .convert: "arrow.triangle.2.circlepath"; case .pages: "doc.on.doc"; case .trim: "waveform"; case .link: "link"; case .text: "text.viewfinder" }
    }
}
extension Notification.Name {
    static let fileformSearch = Notification.Name("app.fileform.search")
    static let fileformInspector = Notification.Name("app.fileform.inspector")
}

struct ImageCropDraft: Equatable, Codable {
    var enabled = false
    var aspect: ImageCropAspect?
    var x = "0", y = "0", width = "1", height = "1"
    var key: String { [String(enabled), x, y, width, height].joined(separator: ":") }
}

enum SetupIntent {
    case manage, saveCurrent, apply(UUID)
    case saveRequest(TransformationRequest, UUID)
    var key: String { switch self { case .manage: "manage"; case .saveCurrent: "save"; case .saveRequest(_, let id): "save:\(id)"; case .apply(let id): "apply:\(id)" } }
}
enum WorkspaceSheet: Identifiable {
    case search, inspector(FileJob), preview(FileJob), setups(SetupIntent), batch(UUID, [UUID])
    var id: String {
        switch self { case .batch(let source, _): "batch:\(source)"; case .search: "search"; case .inspector(let job): "inspector:\(job.id)"; case .preview(let job): "preview:\(job.id)"; case .setups(let intent): "setups:\(intent.key)" }
    }
    var blocksExecution: Bool { switch self { case .setups, .batch, .preview: true; default: false } }
}
