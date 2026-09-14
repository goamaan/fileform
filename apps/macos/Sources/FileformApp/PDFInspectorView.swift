import SwiftUI
import FileformDomain

struct PDFInspectorView: View {
    let model: WorkspaceModel
    private var draft: PDFWorkspaceModel { model.pdfWorkspace }

    var body: some View {
        @Bindable var draft = draft
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                VStack(alignment: .leading, spacing: 8) {
                    heading("Source documents")
                    ForEach(draft.orderedSourceJobs) { job in
                        HStack(spacing: 8) {
                            RoundedRectangle(cornerRadius: 2).fill(pdfSourceColor(job.id.uuidString, draft: draft)).frame(width: 7, height: 7)
                            Text(job.input.lastPathComponent).font(.system(size: 13)).lineLimit(1).truncationMode(.middle)
                            Spacer(minLength: 0)
                            Text("\(draft.pages.filter { $0.sourceID == job.id.uuidString }.count)")
                                .font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink4)
                            if job.state == .missing {
                                Button { model.locateSource(job) } label: { Image(systemName: "folder.badge.questionmark") }
                                    .buttonStyle(.plain).foregroundStyle(FileformTheme.warning).accessibilityLabel("Locate \(job.input.lastPathComponent)")
                            }
                        }.padding(.horizontal, 9).frame(minHeight: 34)
                            .background(FileformTheme.field, in: RoundedRectangle(cornerRadius: 6))
                            .overlay(RoundedRectangle(cornerRadius: 6).stroke(FileformTheme.fieldLine, lineWidth: 1))
                            .help(job.input.lastPathComponent)
                    }
                    Text("Each image becomes one page. ⌘-click selects multiple pages; Shift-click selects a range. Select group selects a source’s pages.")
                        .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3).fixedSize(horizontal: false, vertical: true)
                }
                VStack(alignment: .leading, spacing: 8) {
                    heading("Output")
                    Picker("Output format", selection: Binding(get: { draft.pageOutput.format }, set: { draft.chooseOutputFormat($0) })) {
                        Text("PDF document").tag(OutputFormat.pdf)
                        Text("PNG images").tag(OutputFormat.png)
                        Text("JPEG images").tag(OutputFormat.jpeg)
                        Text("Embedded images").tag(OutputFormat.images)
                    }.labelsHidden().accessibilityIdentifier("pdf-output-format").disabled(draft.isRunning)
                    if draft.pageOutput.isExtraction {
                        extractionOptions
                    } else if draft.pageOutput.isRaster {
                        Picker("Pages to export", selection: $draft.pageOutput.selectedOnly) {
                            Text("All pages").tag(false)
                            Text("Selected pages").tag(true)
                        }.pickerStyle(.segmented).labelsHidden().accessibilityIdentifier("pdf-export-scope").disabled(draft.isRunning)
                        Text("\(draft.outputCount) image\(draft.outputCount == 1 ? "" : "s") in a new folder, following the order above. Split markers are ignored.")
                            .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3).fixedSize(horizontal: false, vertical: true)
                        HStack {
                            Text("Resolution").font(.system(size: 13))
                            Spacer()
                            TextField("144", text: $draft.pageOutput.dpi).font(FileformTheme.mono(12))
                                .textFieldStyle(.roundedBorder).frame(width: 72)
                                .accessibilityLabel("Page image resolution in DPI").accessibilityIdentifier("pdf-export-dpi")
                            Text("DPI").font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink3)
                        }.disabled(draft.isRunning)
                        if draft.pageOutput.format == .jpeg {
                            HStack {
                                Text("Image quality").font(.system(size: 13))
                                Spacer()
                                Text(draft.pageOutput.quality.formatted(.percent.precision(.fractionLength(0)))).font(FileformTheme.mono(11))
                            }
                            Slider(value: $draft.pageOutput.quality, in: 0.05...1, step: 0.01)
                                .accessibilityLabel("JPEG page image quality").disabled(draft.isRunning)
                        }
                        Text("Higher DPI makes larger files. Text becomes pixels; crops and rotations apply.")
                            .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3).fixedSize(horizontal: false, vertical: true)
                    } else {
                        PDFSplitControls(draft: draft)
                        Text(draft.outputCount == 1 ? "One PDF. Add splits to create separate PDFs." : "Split markers divide the pages into \(draft.outputCount) PDFs, saved together in a new folder.")
                            .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3).fixedSize(horizontal: false, vertical: true)
                    }
                }
                VStack(alignment: .leading, spacing: 7) {
                    heading(draft.pageOutput.isImageOutput ? "Folder name" : "Name")
                    TextField(draft.pageOutput.isExtraction ? "Embedded images" : draft.pageOutput.isRaster ? "Page images" : "Combined.pdf", text: $draft.outputName).font(FileformTheme.mono(12)).textFieldStyle(.plain)
                        .padding(.horizontal, 10).frame(height: 32)
                        .background(FileformTheme.field, in: RoundedRectangle(cornerRadius: 6))
                        .overlay(RoundedRectangle(cornerRadius: 6).stroke(FileformTheme.fieldLine, lineWidth: 1))
                        .accessibilityLabel(draft.pageOutput.isImageOutput ? "Image folder name" : "Output PDF name").accessibilityIdentifier("pdf-output-name").disabled(draft.isRunning)
                        .onChange(of: draft.outputName) { _, _ in draft.refresh() }
                    if !draft.pageOutput.isImageOutput && draft.exportAsDirectory && draft.outputCount == 1 {
                        Text("Saves in a new folder.").font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                        Button("Use a Single File") { draft.exportAsDirectory = false; draft.refresh() }.disabled(draft.isRunning)
                    }
                }
                if draft.isPlanning {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text(draft.pageOutput.isExtraction ? "Finding embedded images…" : "Checking pages…").font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                    }
                }
                if let plan = draft.plan, !plan.warnings.isEmpty {
                    DisclosureGroup("What changes") {
                        ForEach(plan.warnings, id: \.self) { WorkbenchNotice(text: $0) }
                    }.font(.system(size: 13))
                }
                Divider().overlay(FileformTheme.border)
                Button("Convert each file instead") { model.chooseTask(.convert) }
                    .buttonStyle(.plain).font(.system(size: 13)).foregroundStyle(FileformTheme.accent)
            }.foregroundStyle(FileformTheme.ink).padding(.horizontal, 15).padding(.vertical, 17)
        }
    }
    private var extractionOptions: some View {
        @Bindable var draft = draft
        return VStack(alignment: .leading, spacing: 10) {
            Picker("PDF pages to search", selection: $draft.pageOutput.selectedOnly) {
                Text("All PDF pages").tag(false)
                Text("Selected PDF pages").tag(true)
            }.pickerStyle(.segmented).labelsHidden().accessibilityIdentifier("pdf-extraction-scope").disabled(draft.isRunning)
            Text("Original-size images in a new folder; reused images saved once. Page rotations and splits do not affect extraction.")
                .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            if draft.excludedImagePageCount > 0 {
                Text("\(draft.excludedImagePageCount) standalone image page\(draft.excludedImagePageCount == 1 ? " is" : "s are") excluded from this search; those sources are already image files.")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
            }
            if let details = draft.extractionDetails {
                Text("\(details.supportedCount) ready to extract · \(details.skippedCount) skipped")
                    .font(.system(size: 13, weight: .medium)).accessibilityIdentifier("pdf-extraction-counts")
                if details.discoveredCount == 0 {
                    Text("No embedded images found on these pages.")
                        .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                }
                if !details.candidates.isEmpty {
                    DisclosureGroup("Images found") {
                        VStack(alignment: .leading, spacing: 12) {
                            ForEach(Array(details.candidates.enumerated()), id: \.offset) { index, candidate in
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(candidate.artifactName ?? "Image \(index + 1)")
                                        .font(.system(size: 12, weight: .medium))
                                    Text(extractionSource(candidate)).font(.system(size: 11)).foregroundStyle(FileformTheme.ink3)
                                    if let width = candidate.width, let height = candidate.height {
                                        Text("\(width) × \(height) pixels").font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink3)
                                    }
                                    Text(candidate.skipReason.map { "Skipped: " + $0 } ?? (candidate.encodingOutcome == .preservedEncodedBytes ? "Original JPEG bytes" : "Pixels reconstructed as PNG"))
                                        .font(.system(size: 11)).foregroundStyle(candidate.skipReason == nil ? FileformTheme.ink3 : FileformTheme.warning)
                                }
                            }
                        }.padding(.top, 6)
                    }.font(.system(size: 12))
                }
            }
            Text("Images may include unused PDF resources. Unsupported encodings and inline images are not extracted.")
                .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
        }.fixedSize(horizontal: false, vertical: true)
    }
    private func extractionSource(_ candidate: PDFEmbeddedImageCandidate) -> String {
        let name = draft.sources[candidate.sourceID]?.input.lastPathComponent ?? "Source PDF"
        let pages = Array(Set(candidate.resourcePages.map { $0.pageIndex + 1 })).sorted().map(String.init).joined(separator: ", ")
        return name + " · " + (candidate.resourcePages.count == 1 ? "page " : "pages ") + pages
    }
    private func heading(_ text: String) -> some View {
        Text(text.uppercased()).font(FileformTheme.mono(11)).tracking(0.8).foregroundStyle(FileformTheme.ink4)
    }
}
