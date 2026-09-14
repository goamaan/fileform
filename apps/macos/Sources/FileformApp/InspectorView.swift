import SwiftUI
import FileformDomain

struct InspectorView: View {
    let model: WorkspaceModel
    @Bindable var job: FileJob
    var showsSourceSummary = false
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if showsSourceSummary { header }
                if job.inspection != nil {
                    settings.disabled(!job.editable)
                    if job.optimizesPDF, job.pdfCompression.compressImages {
                        if [.completed, .unchanged, .failed].contains(job.state), let details = job.lastPDFOptimizationDetails {
                            PDFCompressionReport(details: details, title: job.state == .completed ? "Saved PDF" : "Last attempt")
                        } else if let details = job.plan?.pdfOptimization {
                            PDFCompressionReport(details: details, title: "Images in this PDF")
                        } else if job.editable && job.error == nil {
                            HStack(spacing: 8) {
                                ProgressView().controlSize(.small)
                                Text("Checking PDF images…").font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                            }
                        }
                    }
                    if let plan = job.plan, !plan.warnings.isEmpty {
                        DisclosureGroup("What changes") {
                            VStack(alignment: .leading, spacing: 9) {
                                ForEach(plan.warnings, id: \.self) {
                                    Text($0).font(.system(size: 12)).foregroundStyle(FileformTheme.ink2)
                                        .fixedSize(horizontal: false, vertical: true)
                                }
                            }.padding(.top, 10)
                        }.font(.system(size: 13))
                    }
                }
                if job.inspection != nil {
                    VStack(alignment: .leading, spacing: 7) {
                        Text("NAME").font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink4)
                        TextField(job.automaticOutputName, text: $job.outputName).textFieldStyle(.roundedBorder)
                            .font(FileformTheme.mono(12)).disabled(!job.editable).accessibilityIdentifier("conversion-output-name")
                        Text(".\(job.format.fileExtension) · Leave blank for an automatic name").font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                    }
                }
                if let error = job.error {
                    Label(error, systemImage: "exclamationmark.circle").foregroundStyle(FileformTheme.danger).font(.callout)
                        .fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("job-error")
                }
                if job.state == .unchanged {
                    Label("No smaller file could be made.", systemImage: "checkmark.shield").foregroundStyle(.secondary)
                }
                if job.state == .cancelled { Text("Cancelled.").foregroundStyle(.secondary) }
                if [.running, .queued].contains(job.state) { Button("Cancel This File") { model.cancel(job) } }
                if job.editable && model.outputFolder == nil {
                    Text("Choose an output folder to save.").font(.callout).foregroundStyle(.secondary)
                }
                Button("Save as Setup…") {
                    if let request = model.setupRequest(for: job) { model.presentedSheet = .setups(.saveRequest(request, UUID())) }
                }
                    .disabled(model.setupRequest(for: job) == nil).accessibilityIdentifier("save-choices-as-setup")
                Spacer(minLength: 10)
            }
            .padding(.horizontal, 15).padding(.vertical, 17)
            .font(.system(size: 13))
            .foregroundStyle(FileformTheme.ink)
        }
        .task(id: job.planKey) { await model.ensurePlan(job) }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Output settings").font(.system(size: 13, weight: .semibold))
            Text(job.input.lastPathComponent).font(.system(size: 12)).textSelection(.enabled).lineLimit(2)
            if let info = job.inspection {
                VStack(alignment: .leading, spacing: 5) {
                    Text(ByteCountFormatter.string(fromByteCount: info.identity.bytes, countStyle: .file))
                    if let width = info.width, let height = info.height { Text("\(width) × \(height)") }
                    if let duration = info.duration { Text(Duration.seconds(duration).formatted(.time(pattern: .minuteSecond))) }
                    if let count = info.pageCount { Text("\(count) pages") }
                    if let rows = info.tableRows, let columns = info.tableColumns { Text("\(rows) rows · \(columns) columns") }
                }.font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink3)
            }
        }
    }

    private var settings: some View {
        VStack(alignment: .leading, spacing: 14) {
            OutputPicker(job: job)
            if job.availableGoals.count > 1 {
                VStack(alignment: .leading, spacing: 9) {
                    Text("File size").font(.callout.weight(.medium))
                    Picker("File size", selection: $job.goal) {
                        if job.availableGoals.contains(.convert) { Text("Standard").tag(ConversionGoal.convert) }
                        if job.availableGoals.contains(.compress) { Text("Smaller").tag(ConversionGoal.compress) }
                        if job.availableGoals.contains(.fit) { Text("Fit a limit").tag(ConversionGoal.fit) }
                    }.pickerStyle(.segmented).labelsHidden().accessibilityIdentifier("conversion-goal")
                }
            }
            if job.optimizesPDF {
                PDFCompressionSettings(job: job)
            } else if job.inspection?.family == .pdf {
                VStack(alignment: .leading, spacing: 7) {
                    HStack {
                        Text(job.format == .txt ? "Page (optional)" : "Page to export")
                        Spacer()
                        TextField("1–\(job.inspection?.pageCount ?? 1)", text: $job.pdfPage)
                            .frame(width: 110).accessibilityLabel("PDF page number").accessibilityIdentifier("pdf-page")
                    }
                    Text(job.format == .txt ? "Blank includes all pages." : "Only the page you choose will be exported.")
                        .font(.caption).foregroundStyle(FileformTheme.secondary)
                }
            }
            if job.goal == .fit {
                HStack {
                    Text("Maximum size")
                    Spacer()
                    TextField("10", text: $job.sizeLimitMB).frame(width: 90).multilineTextAlignment(.trailing)
                        .accessibilityLabel("Maximum size in MB").accessibilityIdentifier("maximum-size")
                    Text("MB").foregroundStyle(.secondary)
                }
                Text("Per file. 1 MB = 1,000,000 bytes.").font(.caption).foregroundStyle(.secondary)
            }
            if job.inspection?.hasAlpha == true && job.format.family == .image && !job.format.supportsAlpha {
                Picker("Transparency", selection: $job.background) {
                    Text("Choose a background…").tag("preserve")
                    Text("White background").tag("white")
                    Text("Black background").tag("black")
                }.accessibilityIdentifier("transparency-background")
            }
            if job.inspection?.family == .image && job.format.family == .image {
                VStack(alignment: .leading, spacing: 10) {
                    Toggle("Crop image", isOn: $job.crop.enabled).accessibilityIdentifier("crop-enabled")
                    if job.crop.enabled {
                        Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 8) {
                            GridRow {
                                cropField("X", value: $job.crop.x, identifier: "crop-x")
                                cropField("Y", value: $job.crop.y, identifier: "crop-y")
                            }
                            GridRow {
                                cropField("Width", value: $job.crop.width, identifier: "crop-width")
                                cropField("Height", value: $job.crop.height, identifier: "crop-height")
                            }
                        }
                        Text("Whole pixels from the upright image’s top-left. Crop precedes resize.")
                            .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
            if job.supportsResize || (job.inspection?.family == .image && job.format.isLossyImage) {
            DisclosureGroup("More options") {
                VStack(alignment: .leading, spacing: 16) {
                    if job.inspection?.family == .image && job.format.isLossyImage {
                        HStack { Text("Quality"); Spacer(); Text(job.quality.formatted(.percent.precision(.fractionLength(0)))).monospacedDigit().foregroundStyle(.secondary) }
                        Slider(value: $job.quality, in: 0.05...1, step: 0.01).accessibilityLabel("Image quality")
                            .onChange(of: job.quality) { _, value in if job.minimumQuality > value { job.minimumQuality = value } }
                        if job.goal == .fit {
                            HStack { Text("Minimum quality"); Spacer(); Text(job.minimumQuality.formatted(.percent.precision(.fractionLength(0)))).monospacedDigit().foregroundStyle(.secondary) }
                            Slider(value: $job.minimumQuality, in: 0.05...max(0.051, job.quality), step: 0.01).accessibilityLabel("Minimum image quality")
                        }
                    }
                    if job.supportsResize {
                        Toggle("Resize longest edge", isOn: $job.resize)
                        if job.resize { HStack { TextField("1920", text: $job.longestEdge).frame(width: 100).accessibilityLabel("Longest edge in pixels"); Text("pixels maximum").foregroundStyle(.secondary) } }
                    }
                    if job.inspection?.videoCodec != nil && job.goal == .fit && [.mp4, .mov].contains(job.format) {
                        TextField("Minimum video bitrate (bits/second)", text: $job.minimumVideoBitrate)
                    }
                }.padding(.top, 14)
            }
            }
        }
        .textFieldStyle(.roundedBorder)
    }

    private func cropField(_ title: String, value: Binding<String>, identifier: String) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title).font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            HStack(spacing: 5) {
                TextField(title, text: value).accessibilityLabel("Crop \(title.lowercased()) in pixels")
                    .accessibilityIdentifier(identifier)
                Text("px").font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink4)
            }
        }
    }
}
