import SwiftUI
import FileformDomain

enum OutputDescription {
    static func sourceFacts(_ info: Inspection) -> String {
        var parts = [ByteCountFormatter.string(fromByteCount: info.identity.bytes, countStyle: .file)]
        if let width = info.width, let height = info.height { parts.append("\(width) × \(height)") }
        if let pages = info.pageCount { parts.append("\(pages) pages") }
        if let rows = info.tableRows, let columns = info.tableColumns { parts.append("\(rows) rows · \(columns) columns") }
        if let duration = info.duration, duration.isFinite, duration >= 0, duration < 1_000_000_000 {
            parts.append(TrimTimeText.edit(.init(ticks: Int64((duration * 1000).rounded()), timescale: 1000)))
        }
        return parts.joined(separator: " · ")
    }

    static func title(_ format: OutputFormat) -> String {
        switch format {
        case .jpeg, .png, .tiff, .heic, .avif, .webp: "\(format.title) image"
        case .mp4: "MP4 video"; case .mov: "QuickTime video"
        case .m4a, .wav, .flac, .mp3: "\(format.title) audio"
        case .images: "Embedded images"
        case .pdf: "PDF document"; case .txt: "Editable text"; case .markdown: "Markdown document"
        case .csv, .tsv, .json: "\(format.title) table"
        }
    }
    static func subtitle(_ format: OutputFormat, input: Inspection?) -> String {
        if format == .txt { return input?.family == .pdf ? "Extract words from PDFs and scans" : "Recognize words in this image" }
        if input?.videoCodec != nil && [.m4a, .wav, .flac, .mp3].contains(format) { return "Keep just the audio from this video" }
        if input?.family == .pdf { return format == .pdf ? "Export a page or reduce the whole PDF" : "Export a selected page" }
        switch format {
        case .jpeg: return "Widely compatible · No transparency"
        case .png: return "Lossless · Keeps transparency"
        case .tiff: return "Lossless image format"
        case .pdf: return "Create a one-page document"
        case .mp4: return "Compatible H.264 video"
        case .mov: return "QuickTime container"
        case .m4a: return "Compact AAC audio"
        case .mp3: return "Lossy audio for broad compatibility"
        case .wav: return "Uncompressed PCM audio"
        case .flac: return "Lossless audio compression"
        case .csv: return "Comma-separated table"
        case .tsv: return "Tab-separated table"
        case .json: return "A table for apps and scripts"
        default: return "Convert to \(format.title)"
        }
    }
    static func symbol(_ family: FileFamily) -> String {
        switch family { case .image: "photo"; case .media: "waveform"; case .pdf: "doc.richtext"; case .table: "tablecells"; case .text: "text.alignleft" }
    }
    static func color(_ family: FileFamily) -> Color {
        switch family { case .image: FileformTheme.image; case .media: FileformTheme.media; case .pdf, .text: FileformTheme.document; case .table: FileformTheme.table }
    }
}

struct OutputPicker: View {
    @Bindable var job: FileJob
    @State private var showing = false
    @State private var query = ""
    @FocusState private var searchFocused: Bool
    private var matches: [Capability] {
        var seen = Set<OutputFormat>()
        return job.capabilities.filter {
            if ["smaller", "compress", "make smaller"].contains(query.lowercased()) { return $0.goals.contains(.compress) }
            if ["fit", "size limit", "limit"].contains(query.lowercased()) { return $0.goals.contains(.fit) }
            let aliases = $0.format == .txt ? " ocr extract words text" : ""
            return query.isEmpty || (OutputDescription.title($0.format) + " " + $0.format.fileExtension + " " +
                                    OutputDescription.subtitle($0.format, input: job.inspection) + aliases).localizedCaseInsensitiveContains(query)
        }.filter { seen.insert($0.format).inserted }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Output").font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink)
            Button { query = ""; showing = true } label: {
                HStack {
                    Text(job.outputTitle).font(.system(size: 13, weight: .medium)).foregroundStyle(FileformTheme.ink)
                    Spacer()
                    Image(systemName: "chevron.down").font(.system(size: 9, weight: .semibold)).foregroundStyle(FileformTheme.ink4)
                }.padding(.horizontal, 10).frame(height: 30)
                    .background(FileformTheme.field, in: RoundedRectangle(cornerRadius: 6))
                    .overlay(RoundedRectangle(cornerRadius: 6).stroke(FileformTheme.fieldLine, lineWidth: 1))
            }
            .buttonStyle(.plain).accessibilityLabel("Output: \(job.outputTitle)").accessibilityHint("Choose a format or action")
            .accessibilityIdentifier("output-format")
            .popover(isPresented: $showing, arrowEdge: .bottom) {
                VStack(spacing: 0) {
                    TextField("Search formats or actions", text: $query).textFieldStyle(.roundedBorder)
                        .focused($searchFocused).padding(14).accessibilityIdentifier("output-search")
                        .onSubmit { if let first = matches.first { select(first.format) } }
                    Divider()
                    ScrollView {
                        VStack(spacing: 3) {
                            ForEach(matches) { capability in
                                Button { select(capability.format) } label: {
                                    HStack(spacing: 12) {
                                        Image(systemName: OutputDescription.symbol(capability.format.family))
                                            .frame(width: 24).foregroundStyle(OutputDescription.color(capability.format.family))
                                        VStack(alignment: .leading, spacing: 3) {
                                            Text(OutputDescription.title(capability.format)).foregroundStyle(FileformTheme.ink)
                                            Text(OutputDescription.subtitle(capability.format, input: job.inspection)).font(.caption).foregroundStyle(FileformTheme.secondary)
                                        }
                                        Spacer()
                                        if capability.format == job.format { Image(systemName: "checkmark").foregroundStyle(FileformTheme.accent) }
                                    }
                                    .padding(11).contentShape(Rectangle())
                                }.buttonStyle(.plain).accessibilityIdentifier("output-option-\(capability.format.rawValue)")
                            }
                            if matches.isEmpty { Text("No matching output for this file.").foregroundStyle(.secondary).padding(22) }
                        }.padding(7)
                    }.frame(maxHeight: 340)
                }
                .frame(width: 360)
                .onAppear { searchFocused = true }
            }
            Text(job.optimizesPDF ? "Reduce file size · Keep every page" : OutputDescription.subtitle(job.format, input: job.inspection))
                .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3).fixedSize(horizontal: false, vertical: true)
        }
    }
    private func select(_ format: OutputFormat) {
        job.format = format
        if ["smaller", "compress", "make smaller"].contains(query.lowercased()) { job.goal = .compress }
        if ["fit", "size limit", "limit"].contains(query.lowercased()) { job.goal = .fit }
        if !job.availableGoals.contains(job.goal) { job.goal = job.availableGoals.first ?? .convert }
        showing = false
    }
}
