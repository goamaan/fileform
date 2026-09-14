import SwiftUI

struct ImageCropControls: View {
    let model: WorkspaceModel
    let job: FileJob
    @Environment(\.undoManager) private var undo
    var body: some View {
        HStack(spacing: 6) {
            Button {
                var draft = job.crop; draft.enabled.toggle()
                model.changeCrop(job, to: draft, undo: undo)
            } label: { Label("Crop", systemImage: "crop") }
                .buttonStyle(WorkbenchButtonStyle()).accessibilityIdentifier("preview-crop")
                .accessibilityValue(job.crop.enabled ? "On" : "Off")
            if job.crop.enabled, let size = job.uprightImageSize {
                Menu {
                    ForEach(ImageCropAspect.allCases, id: \.self) { aspect in
                        Button(aspect.title) {
                            var draft = job.crop; draft.aspect = aspect
                            if let ratio = aspect.ratio(in: size) {
                                draft.setRectangle(ImageCropGeometry.fitting(ImageCropGeometry.rectangle(draft, in: size) ?? CGRect(origin: .zero, size: size), ratio: ratio))
                            }
                            model.changeCrop(job, to: draft, undo: undo)
                        }
                    }
                    Divider()
                    Button("Reset to whole image") {
                        var draft = job.crop; draft.aspect = nil
                        draft.setRectangle(CGRect(origin: .zero, size: size))
                        model.changeCrop(job, to: draft, undo: undo)
                    }
                } label: { Text((job.crop.aspect ?? .free).title).font(.system(size: 11)) }
                    .fixedSize().accessibilityLabel("Crop aspect ratio").accessibilityIdentifier("crop-aspect")
            }
        }.disabled(!job.editable)
    }
}

struct ImageCropPreview: View {
    let model: WorkspaceModel
    let job: FileJob
    let image: NSImage
    @State private var dragStart: CGRect?
    @State private var dragged: CGRect?
    @Environment(\.undoManager) private var undo
    private var space: String { "crop-preview-\(job.id)" }

    var body: some View {
        GeometryReader { geometry in
            if let size = job.uprightImageSize {
                let scale = min(max(1, geometry.size.width - 32) / size.width, max(1, geometry.size.height - 32) / size.height)
                let frame = CGRect(x: (geometry.size.width - size.width * scale) / 2,
                                   y: (geometry.size.height - size.height * scale) / 2,
                                   width: size.width * scale, height: size.height * scale)
                Image(nsImage: image).resizable().frame(width: frame.width, height: frame.height)
                    .position(x: frame.midX, y: frame.midY).accessibilityLabel("Original preview")
                if job.crop.enabled, let rect = dragged ?? ImageCropGeometry.rectangle(job.crop, in: size) {
                    let crop = CGRect(x: frame.minX + rect.minX * scale, y: frame.minY + rect.minY * scale,
                                      width: rect.width * scale, height: rect.height * scale)
                    Path { path in path.addRect(frame); path.addRect(crop) }
                        .fill(.black.opacity(0.48), style: FillStyle(eoFill: true)).allowsHitTesting(false)
                    Rectangle().fill(.clear).contentShape(Rectangle())
                        .overlay(Rectangle().stroke(.black.opacity(0.65), lineWidth: 3))
                        .overlay(Rectangle().stroke(FileformTheme.focus, lineWidth: 1.5))
                        .frame(width: crop.width, height: crop.height).position(x: crop.midX, y: crop.midY)
                        .gesture(drag(corner: nil, size: size, scale: scale))
                        .accessibilityLabel("Crop selection").accessibilityValue("\(Int(rect.width)) by \(Int(rect.height)) pixels at \(Int(rect.minX)), \(Int(rect.minY))")
                    if job.editable { ForEach(Array(ImageCropCorner.allCases.enumerated()), id: \.offset) { _, corner in
                        RoundedRectangle(cornerRadius: 2).fill(.white)
                            .frame(width: 8, height: 8).padding(8).contentShape(Rectangle())
                            .position(x: corner.left ? crop.minX : crop.maxX, y: corner.top ? crop.minY : crop.maxY)
                            .gesture(drag(corner: corner, size: size, scale: scale))
                            .accessibilityHidden(true)
                    } }
                    Text("\(Int(rect.width)) × \(Int(rect.height)) px" + (job.editable ? " · drag corners or move selection" : ""))
                        .font(FileformTheme.mono(10)).foregroundStyle(FileformTheme.ink3)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom).padding(.bottom, 2)
                        .allowsHitTesting(false)
                }
            }
        }.coordinateSpace(name: space).allowsHitTesting(job.editable)
            .onChange(of: job.crop) { _, _ in dragStart = nil; dragged = nil }
            .onChange(of: job.state) { _, _ in dragStart = nil; dragged = nil }
    }

    private func drag(corner: ImageCropCorner?, size: CGSize, scale: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: 1, coordinateSpace: .named(space))
            .onChanged { value in
                guard job.editable, job.crop.enabled else { return }
                if dragStart == nil { dragStart = ImageCropGeometry.rectangle(job.crop, in: size) }
                guard let start = dragStart else { return }
                let delta = CGSize(width: value.translation.width / scale, height: value.translation.height / scale)
                if let corner {
                    dragged = ImageCropGeometry.resize(start, corner: corner, by: delta, in: size, ratio: job.crop.aspect?.ratio(in: size))
                } else { dragged = ImageCropGeometry.move(start, by: delta, in: size) }
            }.onEnded { _ in
                if let dragged, job.editable {
                    var draft = job.crop; draft.setRectangle(dragged)
                    model.changeCrop(job, to: draft, undo: undo)
                }
                dragged = nil; dragStart = nil
            }
    }
}

extension ImageCropDraft {
    mutating func setRectangle(_ rect: CGRect) {
        x = String(Int(rect.minX)); y = String(Int(rect.minY))
        width = String(Int(rect.width)); height = String(Int(rect.height))
    }
}
