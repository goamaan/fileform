import SwiftUI
import FileformDomain

struct TrimRangeTimeline: View {
    let model: WorkspaceModel
    @Bindable var job: FileJob
    let timeline: MediaTimeline
    let waveform: MediaWaveform?
    let playback: MediaPlaybackController
    @State private var zoom = 1.0
    @State private var zoomRequest = UUID()
    @State private var dragPrior: MediaTrimDraft?
    @Environment(\.undoManager) private var undoManager
    private var trackHeight: CGFloat { timeline.video == nil ? 132 : 72 }
    private var seconds: Double { TrimTimeText.seconds(timeline.duration) }

    var body: some View {
        ScrollViewReader { proxy in
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Selection").font(.headline)
                Spacer()
                Button("Zoom to selection") {
                    guard let range = try? job.trimDraft?.interval() else { return }
                    let length = TrimTimeText.seconds(range.end) - TrimTimeText.seconds(range.start)
                    zoom = min(8, max(1, seconds / length * 0.85))
                    zoomRequest = UUID()
                }.font(.caption)
                Text("Zoom")
                Slider(value: $zoom, in: 1...8).frame(width: 120).accessibilityLabel("Timeline zoom")
            }
            GeometryReader { geometry in
                let width = max(1, geometry.size.width - 28) * zoom
                ScrollView(.horizontal) {
                    ZStack(alignment: .topLeading) {
                        if let waveform { TrimWaveformView(waveform: waveform, selection: try? job.trimDraft?.interval()).frame(width: width, height: trackHeight) }
                        else { Rectangle().fill(FileformTheme.field).frame(width: width, height: trackHeight) }
                        TrimTimelinePlayhead(playback: playback, duration: seconds, width: width, height: trackHeight)
                        if let interval = try? job.trimDraft?.interval(duration: timeline.duration) {
                            let start = TrimTimeText.seconds(interval.start) / seconds * width
                            let end = TrimTimeText.seconds(interval.end) / seconds * width
                            Rectangle().fill(FileformTheme.focus.opacity(0.12))
                                .frame(width: max(1, end - start), height: trackHeight)
                                .overlay(Rectangle().stroke(FileformTheme.accent, lineWidth: 1)).offset(x: start)
                            HStack(spacing: 0) {
                                Color.clear.frame(width: start)
                                Color.clear.frame(width: 1, height: trackHeight).id("trim-start-position")
                                Spacer(minLength: 0)
                            }.allowsHitTesting(false)
                            handle(start: true, at: start, width: width)
                            handle(start: false, at: end, width: width)
                        }
                    }.frame(width: width, height: trackHeight + 10)
                        .contentShape(Rectangle())
                        .onTapGesture { location in
                            playback.seek(to: TrimTimeText.fromSeconds(min(seconds, max(0, location.x / width * seconds))))
                        }.padding(.horizontal, 14)
                }
                .task(id: zoomRequest) {
                    await Task.yield()
                    guard !Task.isCancelled else { return }
                    proxy.scrollTo("trim-start-position", anchor: .leading)
                }
            }.frame(height: trackHeight + 14)
            HStack {
                ForEach(0..<5) { index in
                    if index > 0 { Spacer(minLength: 0) }
                    Text(TrimTimeText.edit(TrimTimeText.fromSeconds(seconds * Double(index) / 4)))
                }
            }.font(FileformTheme.mono(10)).foregroundStyle(FileformTheme.ink4)
            Divider()
            TrimTransport(playback: playback, validSelection: (try? job.trimDraft?.interval(duration: timeline.duration)) != nil)
        }.padding(14).background(FileformTheme.field, in: RoundedRectangle(cornerRadius: 9))
            .overlay(RoundedRectangle(cornerRadius: 9).stroke(FileformTheme.border, lineWidth: 1))
        }
    }
    private func handle(start: Bool, at position: Double, width: Double) -> some View {
        RoundedRectangle(cornerRadius: 4).fill(FileformTheme.focus)
            .overlay(Image(systemName: "line.3.horizontal").rotationEffect(.degrees(90)).font(.system(size: 9)).foregroundStyle(.white))
            .frame(width: 9, height: trackHeight).frame(width: 24).contentShape(Rectangle())
            .offset(x: position - 12)
            .focusable()
            .accessibilityLabel(start ? "Trim start" : "Trim end")
            .accessibilityValue(start ? job.trimDraft?.from ?? "" : job.trimDraft?.to ?? "")
            .accessibilityAdjustableAction { direction in nudge(start: start, forward: direction == .increment) }
            .onMoveCommand { direction in
                if direction == .left || direction == .right { nudge(start: start, forward: direction == .right) }
            }
            .gesture(DragGesture(minimumDistance: 0).onChanged { value in
                if dragPrior == nil { dragPrior = job.trimDraft }
                guard let prior = dragPrior, let interval = try? prior.interval() else { return }
                let original = TrimTimeText.seconds(start ? interval.start : interval.end)
                update(start: start, seconds: original + value.translation.width / width * seconds, undo: nil, persist: false)
            }.onEnded { _ in
                if let prior = dragPrior { model.finishTrimGesture(job, prior: prior, undo: undoManager) }
                dragPrior = nil
            })
    }
    private func nudge(start: Bool, forward: Bool) {
        guard let interval = try? job.trimDraft?.interval() else { return }
        let step = NSEvent.modifierFlags.contains(.option) ? 1.0 : 0.1
        update(start: start, seconds: TrimTimeText.seconds(start ? interval.start : interval.end) + (forward ? step : -step), undo: undoManager, persist: true)
    }
    private func update(start: Bool, seconds proposed: Double, undo: UndoManager?, persist: Bool) {
        guard var draft = job.trimDraft, let interval = try? draft.interval() else { return }
        let startSeconds = TrimTimeText.seconds(interval.start), endSeconds = TrimTimeText.seconds(interval.end)
        let bounded = start ? min(endSeconds - 0.000001, max(0, proposed)) : min(seconds, max(startSeconds + 0.000001, proposed))
        let time = bounded == seconds ? timeline.duration : TrimTimeText.fromSeconds(bounded)
        if start { draft.from = TrimTimeText.edit(time) } else { draft.to = TrimTimeText.edit(time) }
        model.changeTrim(job, to: draft, undo: undo, persist: persist)
    }
}

private struct TrimTimelinePlayhead: View {
    let playback: MediaPlaybackController
    let duration: Double
    let width: Double
    let height: CGFloat
    var body: some View {
        Rectangle().fill(FileformTheme.ink).frame(width: 1.5, height: height + 8)
            .offset(x: min(width, max(0, TrimTimeText.seconds(playback.playhead) / duration * width)), y: -4)
            .allowsHitTesting(false).accessibilityHidden(true)
    }
}
