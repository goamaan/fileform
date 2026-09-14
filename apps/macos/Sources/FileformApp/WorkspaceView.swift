import SwiftUI
import UniformTypeIdentifiers
import FileformDomain

struct WorkspaceView: View {
    @Bindable var model: WorkspaceModel
    @State private var dropTargeted = false
    @State private var inspectorVisible = true
    @State private var railVisible = true
    @State private var wideToolbar = true
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency

    var body: some View {
        GeometryReader { geometry in
            let compact = geometry.size.width < 980
            VStack(spacing: 0) {
                if model.restoringSession {
                    HStack(spacing: 10) { ProgressView().controlSize(.small); Text("Restoring workspace…"); Spacer() }
                        .font(.system(size: 12)).padding(10).background(FileformTheme.field)
                }
                if let notice = model.persistenceNotice {
                    HStack {
                        Label(notice, systemImage: "exclamationmark.circle").font(.system(size: 12))
                        Spacer()
                        Button("Dismiss") { model.persistenceNotice = nil }
                    }.padding(10).background(FileformTheme.field)
                }
                HStack(spacing: 0) {
                    if !compact && railVisible { taskRail.frame(width: 208); Divider() }
                    if model.workspaceTask == .pages {
                        PDFWorkspaceView(model: model, compact: compact, inspectorVisible: inspectorVisible)
                    } else if model.workspaceTask == .link {
                        LinkWorkspaceView(model: model, compact: compact, link: model.linkWorkspace)
                    } else if model.workspaceTask == .trim {
                        MediaTrimWorkspaceView(model: model, compact: compact, inspectorVisible: inspectorVisible)
                    } else {
                        VStack(spacing: 0) {
                            if compact && !model.jobs.isEmpty { compactSettings; Divider() }
                            if model.jobs.isEmpty { emptyState }
                            else if model.jobs.count == 1, let job = model.selectedJob {
                                FileDetailView(model: model, job: job)
                            } else {
                                queue
                                if let job = model.selectedJob, !job.outputs.isEmpty {
                                    Divider()
                                    DisclosureGroup("Saved results · \(job.outputs.count)") {
                                        ScrollView { ResultSummaryView(model: model, job: job).padding(.top, 12) }
                                            .frame(maxHeight: min(220, geometry.size.height * 0.4))
                                    }.font(.system(size: 13)).padding(.horizontal, 18).padding(.vertical, 10)
                                }
                            }
                        }.frame(maxWidth: .infinity, maxHeight: .infinity)
                        if !compact && inspectorVisible, let job = model.selectedJob {
                            WorkbenchInspector { InspectorView(model: model, job: job) }
                        }
                    }
                }.frame(maxWidth: .infinity, maxHeight: .infinity)
                Divider()
                footer.frame(height: 54)
            }
            .onAppear { wideToolbar = !compact }
            .onChange(of: compact) { _, value in wideToolbar = !value }
            .onReceive(NotificationCenter.default.publisher(for: .fileformInspector)) { _ in
                if compact { if ![.trim, .link].contains(model.workspaceTask) { presentInspector() } } else { inspectorVisible.toggle() }
            }
        }
        .disabled(model.restoringSession)
        .background(FileformTheme.canvas)
        .foregroundStyle(FileformTheme.ink)
        .font(.system(size: 13))
        .overlay {
            if dropTargeted {
                RoundedRectangle(cornerRadius: 12).strokeBorder(FileformTheme.focus, lineWidth: 3)
                    .padding(10).allowsHitTesting(false)
            }
        }
        .dropDestination(for: URL.self) { urls, _ in
            guard !model.restoringSession else { return false }
            let files = urls.filter(\.isFileURL)
            if !files.isEmpty { model.addFiles(files); return true }
            if urls.count == 1, let url = urls.first, ["http", "https"].contains(url.scheme?.lowercased() ?? ""), !model.linkWorkspace.isBusy {
                model.chooseTask(.link); model.linkWorkspace.urlText = url.absoluteString; return true
            }
            return false
        } isTargeted: { dropTargeted = $0 && !model.restoringSession }
        .toolbar {
            ToolbarItem(placement: .navigation) {
                Menu {
                    Button("Convert files") { model.chooseTask(.convert) }
                    Button("Organize PDFs") { model.chooseTask(.pages) }
                    Button("Save from a link") { model.chooseTask(.link) }
                    Button("Trim audio or video") { model.chooseTask(.trim) }
                    Button("Extract text") { model.chooseTask(.text) }
                    Divider()
                    Menu("Saved setups") {
                        Button("Manage Saved Setups…") { model.presentedSheet = .setups(.manage) }
                        ForEach(model.setups.recipes, id: \.id) { recipe in
                            Button(recipe.name) { model.presentedSheet = .setups(.apply(recipe.id)) }
                        }
                    }
                    Toggle("Show task sidebar in wide windows", isOn: $railVisible)
                } label: { Label("Tasks", systemImage: "sidebar.left") }
                .accessibilityIdentifier("task-menu")
            }
            ToolbarItem(placement: .primaryAction) {
                Button { model.presentedSheet = .search } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "magnifyingglass")
                        if wideToolbar { Text("Find a task or format").font(.system(size: 12)); Text("⌘K").font(FileformTheme.mono(10)).foregroundStyle(FileformTheme.ink4) }
                    }
                }
                    .help("Find a task or format (⌘K)").accessibilityIdentifier("task-search")
            }
            ToolbarItem(placement: .primaryAction) {
                Button { model.chooseFiles() } label: {
                    HStack(spacing: 6) { Image(systemName: "plus"); if wideToolbar { Text("Add Files").font(.system(size: 12)) } }
                }
                    .help("Add files (⌘O)").accessibilityIdentifier("add-files").disabled(model.restoringSession)
            }
        }
        .navigationTitle("Fileform")
        .task(id: model.selectedJob?.planKey) {
            if let job = model.selectedJob { await model.ensurePlan(job) }
        }
        .onReceive(NotificationCenter.default.publisher(for: .fileformSearch)) { _ in model.presentedSheet = .search }
        .sheet(item: $model.presentedSheet) { sheet in
            switch sheet {
            case .batch(let source, let ids): BatchChoicesSheet(model: model, sourceID: source, sourceIDs: ids)
            case .search: TaskSearchSheet(model: model)
            case .inspector(let job): InspectorSheet(model: model, job: job)
            case .preview(let job): FilePreviewSheet(model: model, job: job)
            case .setups(let intent): SavedSetupsView(model: model, intent: intent)
            }
        }
        .alert("Fileform", isPresented: Binding(get: { model.alert != nil }, set: { if !$0 { model.alert = nil } })) {
            Button("Dismiss", role: .cancel) { model.alert = nil }
        } message: { Text(model.alert ?? "") }
    }

    private var taskRail: some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Tasks").font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink4).padding(.horizontal, 8).padding(.bottom, 6)
                taskButton("Convert files", symbol: "arrow.triangle.2.circlepath", task: .convert)
                taskButton("Organize PDFs", symbol: "doc.on.doc", task: .pages)
                taskButton("Trim audio or video", symbol: "waveform", task: .trim)
                taskButton("Save from a link", symbol: "link", task: .link)
                taskButton("Extract text", symbol: "text.viewfinder", task: .text)
            }
            Divider()
            VStack(alignment: .leading, spacing: 8) {
                Text("SAVED SETUPS").font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink4)
                ScrollView {
                    VStack(alignment: .leading, spacing: 7) {
                        ForEach(model.setups.recipes, id: \.id) { recipe in
                            Button(recipe.name) { model.presentedSheet = .setups(.apply(recipe.id)) }
                                .buttonStyle(.plain).font(.system(size: 13)).foregroundStyle(FileformTheme.ink2).lineLimit(2)
                                .frame(maxWidth: .infinity, alignment: .leading).padding(.vertical, 4)
                        }
                    }
                }.frame(maxHeight: min(160, CGFloat(model.setups.recipes.count) * 38))
                Button("Manage Setups…") { model.presentedSheet = .setups(.manage) }.buttonStyle(.plain).foregroundStyle(FileformTheme.accent)
            }.padding(.horizontal, 8)
            Spacer(minLength: 10)
        }
        .padding(.horizontal, 12).padding(.vertical, 18)
        .background(FileformTheme.chrome)
    }

    private func taskButton(_ title: String, symbol: String, task: WorkspaceTask) -> some View {
        Button { model.chooseTask(task) } label: {
            Text(title).font(.system(size: 13, weight: model.workspaceTask == task ? .medium : .regular))
                .foregroundStyle(model.workspaceTask == task ? FileformTheme.ink : FileformTheme.ink2)
                .lineLimit(1).frame(maxWidth: .infinity, minHeight: 28, alignment: .leading).padding(.horizontal, 8)
                .background(model.workspaceTask == task ? FileformTheme.field : .clear, in: RoundedRectangle(cornerRadius: 6))
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(model.workspaceTask == task ? FileformTheme.fieldLine : .clear, lineWidth: 1))
        }.buttonStyle(.plain)
        .accessibilityAddTraits(model.workspaceTask == task ? .isSelected : [])
    }

    private var emptyState: some View { WelcomeWorkspaceView(model: model) }

    private var compactSettings: some View {
        HStack(spacing: 14) {
            if let job = model.selectedJob { CompactFormatPicker(job: job) }
            Spacer(minLength: 0)
            Button { presentInspector() } label: { Label("All settings", systemImage: "slider.horizontal.3") }
                .disabled(model.selectedJob == nil).accessibilityIdentifier("show-inspector")
        }.padding(.horizontal, 18).padding(.vertical, 10)
    }

    private var queue: some View {
        VStack(spacing: 0) {
            HStack {
                Text("\(model.jobs.count) files").font(.system(size: 15, weight: .semibold))
                Spacer()
                Button("Preview") {
                    if let job = model.selectedJob { model.presentedSheet = .preview(job) }
                }.disabled(model.batchSelection.count != 1 || model.selectedJob == nil)
                    .accessibilityIdentifier("preview-selected-file")
            }.padding(18)
            Divider()
            HStack(spacing: 12) {
                Text("\(model.batchSelection.count) selected").font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink3)
                Button("Apply choices…") { model.reviewBatchChoices() }.disabled(model.batchSelection.count < 2 || model.isRunning)
                    .accessibilityIdentifier("review-batch-choices")
                Spacer(minLength: 0)
                Picker("Run scope", selection: $model.runSelectionOnly) {
                    Text("All ready files").tag(false)
                    Text("Selected files").tag(true)
                }.labelsHidden().pickerStyle(.segmented).frame(width: 245).disabled(model.isRunning)
            }.padding(.horizontal, 18).padding(.vertical, 10)
            Divider()
            List(selection: Binding(get: { model.batchSelection }, set: { model.selectBatch($0) })) {
                ForEach(model.jobs) { job in
                    JobRow(job: job).tag(job.id)
                        .listRowInsets(EdgeInsets(top: 0, leading: 12, bottom: 0, trailing: 12))
                        .contextMenu {
                            Button("Preview") { model.presentedSheet = .preview(job) }
                            if job.state == .missing { Button("Locate Source…") { model.locateSource(job) } }
                            Button("Output settings…") { model.presentedSheet = .inspector(job) }
                            ForEach(job.outputs, id: \.self) { output in Button("Reveal \(output.lastPathComponent)") { model.reveal(output) } }
                            if [.running, .queued].contains(job.state) { Button("Cancel") { model.cancel(job) } }
                            else if job.state != .inspecting { Button("Remove from List") { model.remove(job) } }
                        }
                }
            }.listStyle(.plain).scrollContentBackground(.hidden).accessibilityIdentifier("file-queue")
        }
    }

    private var footer: some View {
        HStack(spacing: 12) {
            Image(systemName: "folder").foregroundStyle(FileformTheme.accent)
            VStack(alignment: .leading, spacing: 2) {
                Text(model.outputFolder?.lastPathComponent ?? "Choose an output folder").font(.system(size: 12, weight: .medium)).lineLimit(1)
            }
            Button("Change") { model.chooseDestination() }.buttonStyle(.plain).foregroundStyle(FileformTheme.accent)
                .disabled(model.isRunning).help(model.outputFolder?.path ?? "Choose where results will be saved")
                .accessibilityIdentifier("choose-output-folder")
            Spacer(minLength: 12)
            if model.isRunning {
                ProgressView().controlSize(.small)
                Button("Cancel All") { model.cancelAll() }.accessibilityIdentifier("cancel-all")
            } else if model.jobs.isEmpty && ![.pages, .link].contains(model.workspaceTask) {
                Text("Originals stay untouched.").font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            } else {
                if model.completedCount > 0 { Text("\(model.completedCount) saved").font(.system(size: 12)).foregroundStyle(FileformTheme.ink3) }
                Button(model.primaryAction) { model.runReadyJobs() }.buttonStyle(PrimaryButtonStyle()).disabled(!model.canRun)
                    .accessibilityIdentifier("convert-files")
            }
        }.padding(.horizontal, 18)
        .background(reduceTransparency ? AnyShapeStyle(FileformTheme.chrome) : AnyShapeStyle(.bar))
    }

    private func presentInspector() {
        guard model.workspaceTask != .pages, let job = model.selectedJob else { return }
        model.presentedSheet = .inspector(job)
    }
}

private struct InspectorSheet: View {
    let model: WorkspaceModel
    let job: FileJob
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        VStack(spacing: 0) {
            HStack { Text("File settings").font(.headline); Spacer(); Button("Done") { dismiss() }.keyboardShortcut(.cancelAction) }.padding(18)
            Divider()
            InspectorView(model: model, job: job, showsSourceSummary: true)
        }.frame(width: 430, height: 460).background(FileformTheme.elevated)
    }
}

private struct CompactFormatPicker: View {
    @Bindable var job: FileJob
    var body: some View {
        Picker("Output", selection: $job.format) {
            ForEach(job.availableFormats, id: \.self) { format in Text(format.title).tag(format) }
        }.frame(maxWidth: 230).disabled(!job.editable).accessibilityIdentifier("compact-output-format")
            .onChange(of: job.format) { _, _ in if !job.availableGoals.contains(job.goal) { job.goal = job.availableGoals.first ?? .convert } }
    }
}

private struct JobRow: View {
    let job: FileJob
    private var outputTitle: String {
        if job.state == .completed, let result = job.result { return OutputDescription.title(result.format) }
        if job.state == .completed, let url = job.outputs.last,
           let format = OutputFormat.allCases.first(where: { $0.fileExtension == url.pathExtension }) { return OutputDescription.title(format) }
        return job.outputTitle
    }
    private var status: String {
        switch job.state {
        case .interrupted: return "Interrupted"
        case .missing: return "Missing source"
        case .unchanged: return "Original kept"
        default: return job.statusText
        }
    }
    var body: some View {
        GeometryReader { geometry in
            HStack(spacing: 12) {
                Group {
                    if let data = job.originalPreview, let image = NSImage(data: data) {
                        Image(nsImage: image).resizable().scaledToFit()
                    } else {
                        Image(systemName: OutputDescription.symbol(job.inspection?.family ?? .image))
                            .font(.system(size: 22)).foregroundStyle(OutputDescription.color(job.inspection?.family ?? .image))
                    }
                }.frame(width: 32, height: 36).accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 4) {
                    Text(job.input.lastPathComponent).font(.system(size: 13, weight: .medium)).lineLimit(1).truncationMode(.middle)
                    HStack(spacing: 7) {
                        if let info = job.inspection { Text(OutputDescription.sourceFacts(info)).font(FileformTheme.mono(11)).lineLimit(1) }
                        if geometry.size.width < 550, job.inspection != nil { Text("→ \(outputTitle)").font(.system(size: 12)).lineLimit(1) }
                    }.foregroundStyle(FileformTheme.ink3)
                }.frame(maxWidth: .infinity, alignment: .leading)
                if geometry.size.width >= 550, job.inspection != nil {
                    Text(outputTitle).font(.system(size: 12)).foregroundStyle(FileformTheme.ink2).frame(width: 140, alignment: .leading)
                }
                HStack(spacing: 5) {
                    if [.inspecting, .running].contains(job.state) { ProgressView().controlSize(.mini) }
                    if job.state == .completed { Image(systemName: "checkmark.circle.fill").foregroundStyle(FileformTheme.success) }
                    Text(status).font(.system(size: 12)).lineLimit(1).accessibilityLabel(job.statusText)
                }.foregroundStyle(job.state == .failed || job.state == .missing ? FileformTheme.danger : FileformTheme.ink3)
                    .frame(width: 108, alignment: .trailing)
            }.frame(height: 52)
        }.frame(height: 52).accessibilityElement(children: .combine)
    }
}

private struct FilePreviewSheet: View {
    let model: WorkspaceModel
    let job: FileJob
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Preview").font(.headline)
                Spacer()
                Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
            }.padding(16)
            Divider()
            FileDetailView(model: model, job: job)
        }.frame(width: 700, height: 520)
            .foregroundStyle(FileformTheme.ink).background(FileformTheme.canvas)
    }
}

private struct FileDetailView: View {
    let model: WorkspaceModel
    let job: FileJob
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(job.input.lastPathComponent).font(.system(size: 17, weight: .semibold)).textSelection(.enabled)
                    if let info = job.inspection { Text(OutputDescription.sourceFacts(info)).font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink3) }
                }.frame(maxWidth: .infinity, alignment: .leading)
                HStack(alignment: .top, spacing: 12) {
                    preview(job.originalPreview, title: "Original")
                    if job.resultPreview != nil { preview(job.resultPreview, title: "Saved result") }
                }
                if job.state != .completed {
                    Text(job.statusText).font(.system(size: 13)).foregroundStyle(FileformTheme.ink2)
                    if let error = job.error { Label(error, systemImage: "exclamationmark.circle").foregroundStyle(FileformTheme.danger).textSelection(.enabled) }
                    if job.state == .missing { Button("Locate Source…") { model.locateSource(job) } }
                }
                if !job.outputs.isEmpty { ResultSummaryView(model: model, job: job) }
            }.padding(20)
        }
    }
    private func preview(_ data: Data?, title: String) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(title.uppercased()).font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink4)
                Spacer()
                if title == "Original", job.uprightImageSize != nil, job.format.family == .image {
                    ImageCropControls(model: model, job: job)
                }
            }.frame(height: 28)
            Group {
                if let data, let image = NSImage(data: data) {
                    if title == "Original", job.uprightImageSize != nil, job.format.family == .image {
                        ImageCropPreview(model: model, job: job, image: image)
                    } else {
                        Image(nsImage: image).resizable().aspectRatio(contentMode: .fit).padding(16).accessibilityLabel("\(title) preview")
                    }
                } else {
                    VStack(spacing: 12) {
                        Image(systemName: OutputDescription.symbol(job.inspection?.family ?? .image)).font(.system(size: 38, weight: .light))
                        Text(job.state == .inspecting ? "Preparing preview…" : "No preview available").font(.system(size: 12))
                    }.foregroundStyle(FileformTheme.ink3)
                }
            }.frame(maxWidth: .infinity).frame(height: 280)
                .background(FileformTheme.inset, in: RoundedRectangle(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(FileformTheme.border, lineWidth: 1))
        }.frame(maxWidth: .infinity)
    }
}

private struct ResultSummaryView: View {
    let model: WorkspaceModel
    let job: FileJob
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Saved", systemImage: "checkmark.circle.fill").font(.system(size: 14, weight: .semibold)).foregroundStyle(FileformTheme.success)
            if let bytes = job.result?.outputBytes {
                Text("Latest result: \(ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file))")
                    .font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink3)
            }
            ForEach(job.outputs, id: \.self) { output in
                VStack(alignment: .leading, spacing: 10) {
                    Text(output.lastPathComponent).font(.system(size: 13)).textSelection(.enabled)
                    HStack {
                        Button("Open Result") { model.open(output) }.accessibilityIdentifier("open-result")
                        Button("Reveal in Finder") { model.reveal(output) }.accessibilityIdentifier("reveal-result")
                        if model.canTrimResult(output, from: job) {
                            Button("Trim") { model.trimResult(output, from: job) }.disabled(model.isRunning)
                        }
                        ShareLink(item: output) { Label("Share", systemImage: "square.and.arrow.up") }
                    }
                }.draggable(output)
            }
            Button("Save as Setup…") { model.presentedSheet = .setups(.saveCurrent) }.disabled(model.currentSetupRequest == nil)
            Button("Make another version") { model.makeAnotherVersion(job) }.disabled(model.isRunning || job.state == .missing)
            if job.state == .missing { Button("Locate Source…") { model.locateSource(job) } }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
}
