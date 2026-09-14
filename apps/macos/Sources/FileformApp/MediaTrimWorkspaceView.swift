import SwiftUI
import AVKit
import FileformCore
import FileformDomain

struct MediaTrimWorkspaceView: View {
    @Bindable var model: WorkspaceModel
    var compact: Bool
    var inspectorVisible: Bool
    var body: some View {
        Group {
            if let job = model.selectedJob, job.inspection?.family == .media {
                MediaTrimEditor(model: model, job: job, compact: compact, inspectorVisible: inspectorVisible).id(job.id)
            } else if model.selectedJob?.state == .inspecting {
                ProgressView("Reading recording…")
            } else {
                ContentUnavailableView {
                    Label("Add audio or video", systemImage: "waveform")
                } description: { Text("Select a range, then save the clip.") }
                actions: {
                    Button("Add Files") { model.chooseFiles() }
                    if let first = model.jobs.first(where: { $0.inspection?.family == .media }) {
                        Button("Open \(first.input.lastPathComponent)") { model.selection = first.id }
                    }
                }
            }
        }.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

private struct MediaTrimEditor: View {
    let model: WorkspaceModel
    @Bindable var job: FileJob
    let compact: Bool
    let inspectorVisible: Bool
    @State private var settingsSheet: TrimSettingsPresentation?
    @State private var playback = MediaPlaybackController()
    @State private var lease: MediaPlaybackPreview?
    @State private var waveform: MediaWaveform?
    @State private var previewError: String?
    @State private var preparing = false
    @State private var previewID = UUID()
    @Environment(\.undoManager) private var undoManager

    private var previewKey: String {
        "\(job.mediaTimeline?.identity.bytes ?? 0):\(job.trimDraft?.audioStream ?? -1):\(job.trimDraft?.muteAudio ?? false)"
    }
    var body: some View {
        HStack(alignment: .top, spacing: 0) {
            VStack(spacing: 12) {
                if compact {
                    HStack {
                        Text(job.trimDraft?.format.title ?? "Output settings")
                        Spacer()
                        Button("Settings…") { settingsSheet = .init() }
                    }
                }
                editorContent
            }.padding(.leading, 22).padding(.trailing, 12).padding(.top, 18).frame(maxWidth: .infinity, maxHeight: .infinity)
            if !compact && inspectorVisible {
                WorkbenchInspector { MediaTrimSettings(model: model, job: job).padding(.horizontal, 15).padding(.vertical, 17) }
            }
        }
        .padding(.bottom, 10)
        .onReceive(NotificationCenter.default.publisher(for: .fileformInspector)) { _ in
            if compact { settingsSheet = .init() }
        }
        .sheet(item: $settingsSheet) { _ in MediaTrimSettingsSheet(model: model, job: job) }
        .task(id: "\(job.input.absoluteString)|\(job.state == .inspecting)") { await model.prepareTrim(job) }
        .task(id: job.trimDraft?.key) {
            if let interval = try? job.trimDraft?.interval(duration: job.mediaTimeline?.duration) {
                _ = playback.updateSelection(interval)
            } else { playback.pause() }
            await model.refreshTrimPlan(job)
        }
        .task(id: previewKey) { await loadPreview() }
        .onDisappear { previewID = UUID(); playback.teardown(); lease?.discard(); lease = nil }
    }
    private var editorContent: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                if let timeline = job.mediaTimeline, let draft = job.trimDraft {
                    WorkbenchChoice(title: "Source", selection: Binding(get: { model.selection }, set: { model.selection = $0 }),
                        options: model.jobs.filter { $0.inspection?.family == .media }.map { Optional($0.id) }, prominent: true) { id in
                            model.jobs.first { $0.id == id }?.input.lastPathComponent ?? "Choose a recording"
                        }.disabled(model.isRunning)
                    if let origin = job.origin {
                        Text("From: \(origin.parentOutput.lastPathComponent)").font(.caption).foregroundStyle(FileformTheme.ink3)
                    }
                    Text("\(TrimTimeText.edit(timeline.duration)) · \(ByteCountFormatter.string(fromByteCount: timeline.identity.bytes, countStyle: .file)) · \((timeline.video?.codec ?? timeline.audioTracks.first?.codec ?? "Media").uppercased())")
                        .foregroundStyle(FileformTheme.ink2)
                    if timeline.video != nil, let player = playback.player {
                        TrimVideoSurface(player: player).frame(height: compact ? 90 : 110)
                    }
                    TrimRangeTimeline(model: model, job: job, timeline: timeline, waveform: waveform, playback: playback)
                    if preparing { ProgressView("Preparing verified playback…") }
                    if let previewError { Text(previewError).foregroundStyle(.secondary) }
                    HStack(alignment: .top, spacing: 10) {
                        timeField("FROM", key: \.from, identifier: "trim-from")
                        timeField("TO", key: \.to, identifier: "trim-to")
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Duration").font(FileformTheme.mono(10))
                            Text((try? draft.interval(duration: timeline.duration)).map(TrimTimeText.duration) ?? "—")
                                .font(FileformTheme.mono(17)).accessibilityLabel("Selection duration").accessibilityValue((try? draft.interval(duration: timeline.duration)).map(TrimTimeText.duration) ?? "Invalid range")
                        }.frame(maxWidth: .infinity, minHeight: 58, alignment: .topLeading)
                            .padding(12).background(FileformTheme.focus.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))
                    }
                    if let details = job.trimPlan?.mediaTrim {
                        Text("Actual cut: \(TrimTimeText.edit(details.realized.start))–\(TrimTimeText.edit(details.realized.end)) · \(TrimTimeText.duration(details.realized))")
                            .font(.callout).monospacedDigit()
                        if details.copiedStreams {
                            Text("Duration tolerance: \(TrimTimeText.edit(details.durationTolerance))").font(.caption)
                        }
                    }

                } else if job.trimError == nil { ProgressView("Reading media timeline…") }
                if job.state == .cancelled {
                    Label("Cancelled.", systemImage: "stop.circle").foregroundStyle(FileformTheme.ink3)
                }
                if let error = job.trimError { Text(error).foregroundStyle(.red).textSelection(.enabled) }
                if let output = job.outputs.last {
                    Text("Last saved").font(.headline)
                    HStack { Label(output.lastPathComponent, systemImage: "checkmark.circle"); Button("Show in Finder") { model.reveal(output) } }
                    Button("Trim") { model.trimResult(output, from: job) }.disabled(model.isRunning)
                    if let details = job.lastTrimDetails, let duration = details.outputDuration {
                        Text("Verified duration: \(TrimTimeText.edit(duration)) · \(details.copiedStreams ? "Copied streams" : "Exact cut")").font(.callout).monospacedDigit()
                    }
                }
            }.disabled(!job.trimEditable)
        }
    }
    private func timeField(_ label: String, key: WritableKeyPath<MediaTrimDraft, String>, identifier: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(label).font(FileformTheme.mono(10)).foregroundStyle(FileformTheme.ink3)
            TextField(label, text: binding(key)).font(FileformTheme.mono(17))
                .textFieldStyle(.plain).accessibilityIdentifier(identifier)
            Button("Use playhead") {
                guard var draft = job.trimDraft else { return }
                draft[keyPath: key] = TrimTimeText.edit(playback.playhead)
                model.changeTrim(job, to: draft, undo: undoManager)
            }.buttonStyle(.plain).font(.caption).foregroundStyle(FileformTheme.accent)
        }.frame(maxWidth: .infinity, minHeight: 58, alignment: .topLeading)
            .padding(12).background(FileformTheme.elevated, in: RoundedRectangle(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).stroke(FileformTheme.border, lineWidth: 1))
    }
    private func binding<Value>(_ key: WritableKeyPath<MediaTrimDraft, Value>) -> Binding<Value> {
        Binding(get: { job.trimDraft![keyPath: key] }, set: { value in
            guard var draft = job.trimDraft else { return }
            draft[keyPath: key] = value; model.changeTrim(job, to: draft, undo: undoManager)
        })
    }
    private func loadPreview() async {
        let id = UUID(); previewID = id
        playback.teardown(); lease?.discard(); lease = nil; waveform = nil; previewError = nil
        guard let timeline = job.mediaTimeline, let draft = job.trimDraft, let service = model.mediaPreviews else { return }
        preparing = true
        defer { if previewID == id { preparing = false } }
        do {
            if !draft.muteAudio && !timeline.audioTracks.isEmpty {
                let measured = try await service.waveform(for: timeline, audioStream: draft.audioStream)
                try Task.checkCancellation()
                guard previewID == id else { return }
                waveform = measured
            }
            try Task.checkCancellation()
            let generated = try await service.playbackPreview(for: timeline, audioStream: draft.audioStream, muteAudio: draft.muteAudio)
            guard !Task.isCancelled, previewID == id else { generated.discard(); return }
            lease = generated
            let interval = (try? job.trimDraft?.interval(duration: timeline.duration))
                ?? MediaInterval(start: .init(ticks: 0, timescale: 1), end: timeline.duration)
            await playback.load(url: generated.url, duration: generated.duration, selection: interval)
        } catch is CancellationError { }
        catch { if previewID == id { previewError = error.localizedDescription } }
    }
}

struct TrimTransport: View {
    let playback: MediaPlaybackController
    let validSelection: Bool
    var body: some View {
        VStack {
            HStack {
                Button { playback.togglePlayback() } label: {
                    Image(systemName: playback.isPlaying ? "pause.fill" : "play.fill").font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(FileformTheme.onAccent).frame(width: 30, height: 30).background(FileformTheme.accent, in: Circle())
                }.buttonStyle(.plain).accessibilityLabel(playback.isPlaying ? "Pause" : "Play")
                    .disabled(!validSelection || playback.player == nil || playback.isLoading)
                Text(TrimTimeText.edit(playback.playhead)).monospacedDigit()
                Spacer()
                Toggle("Play selection only", isOn: Binding(get: { playback.selectionOnly }, set: { playback.setSelectionOnly($0) }))
            }
            if let error = playback.error { Text(error).foregroundStyle(.secondary) }
        }
    }
}

struct TrimWaveformView: View {
    let waveform: MediaWaveform
    var selection: MediaInterval? = nil
    var body: some View {
        Canvas { context, size in
            let duration = TrimTimeText.seconds(waveform.duration)
            for channel in 0..<waveform.channels {
                let height = size.height / Double(waveform.channels)
                let center = height * (Double(channel) + 0.5)
                var active = Path(), inactive = Path()
                for bucket in waveform.buckets {
                    let x = TrimTimeText.seconds(bucket.interval.start) / duration * size.width
                    var segment = Path()
                    let top = center - Double(bucket.maximum[channel]) * height * 0.45
                    let bottom = center - Double(bucket.minimum[channel]) * height * 0.45
                    let bucketWidth = (TrimTimeText.seconds(bucket.interval.end) - TrimTimeText.seconds(bucket.interval.start)) / duration * size.width
                    segment.addRect(CGRect(x: x, y: min(top, bottom), width: max(1, bucketWidth * 0.75), height: max(1, abs(bottom - top))))
                    let seconds = TrimTimeText.seconds(bucket.interval.start)
                    if let selection, seconds >= TrimTimeText.seconds(selection.start), seconds < TrimTimeText.seconds(selection.end) { active.addPath(segment) }
                    else { inactive.addPath(segment) }
                }
                context.fill(inactive, with: .color(FileformTheme.ink4.opacity(0.24)))
                context.fill(active, with: .color(FileformTheme.accent))
            }
        }.accessibilityLabel("Measured waveform, \(waveform.channels) channels")
    }
}

private struct TrimSettingsPresentation: Identifiable { let id = UUID() }
private struct MediaTrimSettingsSheet: View {
    let model: WorkspaceModel
    let job: FileJob
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack { Text("Trim settings").font(.headline); Spacer(); Button("Done") { dismiss() } }
            MediaTrimSettings(model: model, job: job, showsSaveButton: false)
        }.padding(24).frame(width: 380, height: 520)
    }
}
private struct MediaTrimSettings: View {
    let model: WorkspaceModel
    @Bindable var job: FileJob
    var showsSaveButton = true
    @Environment(\.undoManager) private var undoManager
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if let timeline = job.mediaTimeline, let draft = job.trimDraft {
                    WorkbenchChoice(title: "Turns into", selection: outputBinding,
                        options: timeline.video == nil ? [.wav, .flac, .m4a] : [.mp4, .mov, .m4a, .wav, .flac]) { format in
                            OutputDescription.title(format) + (timeline.video != nil && ![.mp4, .mov].contains(format) ? " · audio only" : "")
                        }
                    Text("Cut mode").font(FileformTheme.mono(10)).foregroundStyle(FileformTheme.ink3)
                    cutChoice(.exact, title: "Exact", detail: "Re-encodes the selection.")
                    cutChoice(.copy, title: "Fast", detail: "Copies media without re-encoding. Boundaries may move; review the actual cut.")
                    if timeline.audioTracks.count > 1 {
                        WorkbenchChoice(title: "Audio track", selection: binding(\.audioStream), options: timeline.audioTracks.map { Optional($0.ordinal) }) { ordinal in
                            guard let ordinal, let track = timeline.audioTracks.first(where: { $0.ordinal == ordinal }) else { return "No audio" }
                            return "Track \(ordinal + 1) · \(track.channels) ch · \(track.sampleRate / 1000) kHz"
                        }.disabled(draft.muteAudio)
                    }
                    if timeline.video != nil && [.mp4, .mov].contains(draft.format) {
                        Toggle("Remove audio", isOn: Binding(get: { draft.muteAudio }, set: { value in
                            var next = draft; next.muteAudio = value
                            next.audioStream = value ? nil : timeline.audioTracks.first?.ordinal
                            model.changeTrim(job, to: next, undo: undoManager)
                        }))
                    }
                    Text("NAME").font(FileformTheme.mono(10)).foregroundStyle(FileformTheme.ink3)
                    TextField("Output filename", text: binding(\.outputName)).textFieldStyle(.roundedBorder)

                    DisclosureGroup("Saving options") {
                        WorkbenchChoice(title: "If the name exists", selection: binding(\.collisionPolicy), options: [.rename, .fail]) {
                            $0 == .rename ? "Keep both files" : "Stop and ask"
                        }.padding(.top, 6)
                    }

                    if showsSaveButton {
                        Button("Save as Setup…") { model.presentedSheet = .setups(.saveCurrent) }
                            .disabled(model.currentSetupRequest == nil)
                    }
                    if let warnings = job.trimPlan?.warnings, !warnings.isEmpty {
                        DisclosureGroup("What changes") { ForEach(warnings, id: \.self) { Text($0).font(.system(size: 12)).foregroundStyle(FileformTheme.ink3) } }
                    }
                }
            }.disabled(!job.trimEditable)
        }
    }
    private var outputBinding: Binding<OutputFormat> {
        Binding(get: { job.trimDraft?.format ?? .mp4 }, set: { format in
            guard var draft = job.trimDraft else { return }
            draft.format = format
            if ![.mp4, .mov].contains(format), draft.muteAudio {
                draft.muteAudio = false; draft.audioStream = job.mediaTimeline?.audioTracks.first?.ordinal
            }
            model.changeTrim(job, to: draft, undo: undoManager)
        })
    }
    private func cutChoice(_ mode: TrimMode, title: String, detail: String) -> some View {
        let selected = job.trimDraft?.mode == mode
        return Button {
            guard var draft = job.trimDraft else { return }
            draft.mode = mode; model.changeTrim(job, to: draft, undo: undoManager)
        } label: {
            HStack(alignment: .top, spacing: 9) {
                Image(systemName: selected ? "largecircle.fill.circle" : "circle")
                    .foregroundStyle(selected ? FileformTheme.accent : FileformTheme.ink3)
                VStack(alignment: .leading, spacing: 4) {
                    Text(title).fontWeight(selected ? .semibold : .regular)
                    Text(detail).font(.caption).foregroundStyle(FileformTheme.ink3)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
            }.padding(10).frame(maxWidth: .infinity, alignment: .leading)
                .background(FileformTheme.field, in: RoundedRectangle(cornerRadius: 7))
                .overlay(RoundedRectangle(cornerRadius: 7).stroke(selected ? FileformTheme.focus : FileformTheme.border, lineWidth: selected ? 2 : 1))
        }.buttonStyle(.plain).accessibilityValue(selected ? "Selected" : "Not selected")
    }
    private func binding<Value>(_ key: WritableKeyPath<MediaTrimDraft, Value>) -> Binding<Value> {
        Binding(get: { job.trimDraft![keyPath: key] }, set: { value in
            guard var draft = job.trimDraft else { return }
            draft[keyPath: key] = value; model.changeTrim(job, to: draft, undo: undoManager)
        })
    }
}

/// Share the editor transport; native player chrome otherwise creates a second
/// timeline with unrelated skip, external playback and selection controls.
private struct TrimVideoSurface: NSViewRepresentable {
    let player: AVPlayer
    func makeNSView(context: Context) -> AVPlayerView {
        let view = AVPlayerView()
        view.controlsStyle = .none
        view.videoGravity = .resizeAspect
        view.player = player
        return view
    }
    func updateNSView(_ view: AVPlayerView, context: Context) { view.player = player }
    static func dismantleNSView(_ view: AVPlayerView, coordinator: ()) { view.player = nil }
}
