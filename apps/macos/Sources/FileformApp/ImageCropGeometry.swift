import Foundation

enum ImageCropAspect: String, Codable, CaseIterable {
    case free, original, square, landscape, widescreen, portrait
    var title: String {
        switch self { case .free: "Free"; case .original: "Original"; case .square: "1:1"; case .landscape: "4:3"; case .widescreen: "16:9"; case .portrait: "4:5" }
    }
    func ratio(in size: CGSize) -> CGFloat? {
        switch self { case .free: nil; case .original: size.width / size.height; case .square: 1; case .landscape: 4 / 3; case .widescreen: 16 / 9; case .portrait: 4 / 5 }
    }
}

enum ImageCropCorner: CaseIterable {
    case topLeft, topRight, bottomLeft, bottomRight
    var left: Bool { self == .topLeft || self == .bottomLeft }
    var top: Bool { self == .topLeft || self == .topRight }
}

enum ImageCropGeometry {
    static func rectangle(_ draft: ImageCropDraft, in size: CGSize) -> CGRect? {
        guard let x = Int(draft.x), let y = Int(draft.y), let width = Int(draft.width), let height = Int(draft.height),
              x >= 0, y >= 0, width > 0, height > 0,
              CGFloat(x) + CGFloat(width) <= size.width, CGFloat(y) + CGFloat(height) <= size.height else { return nil }
        return CGRect(x: x, y: y, width: width, height: height)
    }

    static func move(_ rect: CGRect, by delta: CGSize, in size: CGSize) -> CGRect {
        CGRect(x: min(max(0, (rect.minX + delta.width).rounded()), size.width - rect.width),
               y: min(max(0, (rect.minY + delta.height).rounded()), size.height - rect.height),
               width: rect.width, height: rect.height)
    }

    static func resize(_ rect: CGRect, corner: ImageCropCorner, by delta: CGSize, in size: CGSize, ratio: CGFloat?) -> CGRect {
        let anchor = CGPoint(x: corner.left ? rect.maxX : rect.minX, y: corner.top ? rect.maxY : rect.minY)
        let room = CGSize(width: corner.left ? anchor.x : size.width - anchor.x, height: corner.top ? anchor.y : size.height - anchor.y)
        var width = min(room.width, max(1, rect.width + (corner.left ? -delta.width : delta.width)))
        var height = min(room.height, max(1, rect.height + (corner.top ? -delta.height : delta.height)))
        if let ratio {
            if abs(delta.width) / rect.width >= abs(delta.height) / rect.height { height = width / ratio }
            else { width = height * ratio }
            let factor = min(1, room.width / width, room.height / height)
            width *= factor; height *= factor
        }
        width = max(1, min(room.width, width.rounded()))
        height = max(1, min(room.height, height.rounded()))
        return CGRect(x: corner.left ? anchor.x - width : anchor.x, y: corner.top ? anchor.y - height : anchor.y, width: width, height: height)
    }

    static func fitting(_ rect: CGRect, ratio: CGFloat) -> CGRect {
        let width = min(rect.width, rect.height * ratio).rounded(.down)
        let height = min(rect.height, rect.width / ratio).rounded(.down)
        return CGRect(x: (rect.midX - max(1, width) / 2).rounded(.down), y: (rect.midY - max(1, height) / 2).rounded(.down),
                      width: max(1, width), height: max(1, height))
    }
}

extension FileJob {
    var uprightImageSize: CGSize? {
        guard let info = inspection, info.family == .image, let width = info.width, let height = info.height, width > 0, height > 0 else { return nil }
        let sideways = (5...8).contains(info.orientation ?? 1)
        return CGSize(width: sideways ? height : width, height: sideways ? width : height)
    }
}

extension WorkspaceModel {
    func changeCrop(_ job: FileJob, to draft: ImageCropDraft, undo: UndoManager?) {
        guard jobs.contains(where: { $0 === job }), job.editable, job.crop != draft else { return }
        let prior = job.crop
        // Older SDKs do not annotate the synchronous UI undo callback as MainActor.
        undo?.registerUndo(withTarget: self) { model in
            MainActor.assumeIsolated { model.changeCrop(job, to: prior, undo: undo) }
        }
        undo?.setActionName("Adjust crop")
        job.crop = draft
        Task { await refreshPlan(job) }
        saveSession()
    }
}
