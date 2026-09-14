import SwiftUI

struct PDFPageDropTarget: Equatable {
    let pageID: UUID
    let edge: PDFPageInsertionEdge
}

struct PDFPageFramesKey: PreferenceKey {
    static let defaultValue: [UUID: CGRect] = [:]
    static func reduce(value: inout [UUID: CGRect], nextValue: () -> [UUID: CGRect]) {
        value.merge(nextValue(), uniquingKeysWith: { _, newest in newest })
    }
}

/// Page moves stay within the document. Geometry and pointer state are view
/// concerns; the model validates and commits the ordered, undoable draft edit.
struct PDFPageDragModifier: ViewModifier {
    let draft: PDFWorkspaceModel
    let pageID: UUID
    let undo: UndoManager?
    let frames: [UUID: CGRect]
    let coordinateSpace: String
    @Binding var pointer: CGPoint?
    @Binding var target: PDFPageDropTarget?
    @State private var payload: PDFPageDragPayload?

    func body(content: Content) -> some View {
        content
            .background {
                GeometryReader { geometry in
                    Color.clear.preference(key: PDFPageFramesKey.self,
                        value: [pageID: geometry.frame(in: .named(coordinateSpace))])
                }
            }
            .highPriorityGesture(DragGesture(minimumDistance: 4, coordinateSpace: .named(coordinateSpace))
                .onChanged { value in
                    if payload == nil { payload = draft.beginPageDrag(pageID) }
                    guard let payload, draft.isValidPageDrag(payload) else { pointer = nil; target = nil; return }
                    pointer = value.location
                    target = dropTarget(at: value.location)
                }
                .onEnded { value in
                    if let payload, let destination = dropTarget(at: value.location) {
                        draft.dropPages(payload, at: destination.pageID, edge: destination.edge, undo: undo)
                    }
                    draft.cancelPageDrag(); payload = nil; pointer = nil; target = nil
                })
            .overlay(alignment: .leading) {
                if !draft.selectedPageIDs.contains(pageID) && target?.pageID == pageID && target?.edge == .before { insertionMark.offset(x: -7) }
            }
            .overlay(alignment: .trailing) {
                if !draft.selectedPageIDs.contains(pageID) && target?.pageID == pageID && target?.edge == .after { insertionMark.offset(x: 7) }
            }
            .accessibilityHint("Drag selected pages to the left or right half of another page to move them before or after it. You can also use the Move buttons.")
    }

    private func dropTarget(at location: CGPoint) -> PDFPageDropTarget? {
        guard let page = draft.pages.first(where: { frames[$0.id]?.contains(location) == true }),
              let frame = frames[page.id] else { return nil }
        return .init(pageID: page.id, edge: location.x < frame.midX ? .before : .after)
    }

    private var insertionMark: some View {
        VStack(spacing: 0) {
            Circle().frame(width: 7, height: 7)
            Rectangle().frame(width: 3)
            Circle().frame(width: 7, height: 7)
        }.foregroundStyle(FileformTheme.accent).allowsHitTesting(false).accessibilityHidden(true)
    }
}
