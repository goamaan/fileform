import SwiftUI
import FileformDomain

struct TaskSearchSheet: View {
    let model: WorkspaceModel
    @State private var query = ""
    @FocusState private var focused: Bool
    @Environment(\.dismiss) private var dismiss
    private var taskMatches: [TaskChoice] {
        TaskChoice.all.filter { query.isEmpty || ($0.title + " " + $0.keywords).localizedCaseInsensitiveContains(query) }
    }
    private var formatMatches: [Capability] {
        guard !model.restoringSession, let job = model.selectedJob, job.editable else { return [] }
        var seen = Set<OutputFormat>()
        return job.capabilities.filter {
            query.isEmpty || (OutputDescription.title($0.format) + " " + $0.format.fileExtension).localizedCaseInsensitiveContains(query)
        }.filter { seen.insert($0.format).inserted }
    }
    private var setupMatches: [TransformationRecipe] {
        model.setups.recipes.filter { query.isEmpty || ($0.name + " " + $0.setupSummary).localizedCaseInsensitiveContains(query) }
    }
    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Image(systemName: "magnifyingglass").foregroundStyle(FileformTheme.ink3)
                TextField("Find a task or format", text: $query).textFieldStyle(.plain).focused($focused)
                    .accessibilityIdentifier("task-search-field").onSubmit { selectFirstMatch() }
                Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
            }.padding(18)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 5) {
                    if !taskMatches.isEmpty {
                        sectionLabel("TASKS")
                        ForEach(taskMatches) { choice in
                            Button { selectTask(choice); dismiss() } label: {
                                Label(choice.title, systemImage: choice.symbol).frame(maxWidth: .infinity, alignment: .leading).padding(10)
                            }.buttonStyle(.plain).disabled(model.restoringSession)
                        }
                    }
                    if !formatMatches.isEmpty {
                        sectionLabel("OUTPUTS FOR SELECTED FILE")
                        ForEach(formatMatches) { capability in
                            Button { selectFormat(capability.format) } label: {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(OutputDescription.title(capability.format))
                                    Text(OutputDescription.subtitle(capability.format, input: model.selectedJob?.inspection))
                                        .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                                }.frame(maxWidth: .infinity, alignment: .leading).padding(10)
                            }.buttonStyle(.plain)
                        }
                    }
                    if !setupMatches.isEmpty {
                        sectionLabel("SAVED SETUPS")
                        ForEach(setupMatches, id: \.id) { recipe in
                            Button { model.presentedSheet = .setups(.apply(recipe.id)) } label: {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(recipe.name)
                                    Text(recipe.setupSummary).font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                                }.frame(maxWidth: .infinity, alignment: .leading).padding(10)
                            }.buttonStyle(.plain)
                        }
                    }
                    if taskMatches.isEmpty && formatMatches.isEmpty && setupMatches.isEmpty { Text("No matching task or output.").foregroundStyle(FileformTheme.ink3).padding(18) }
                    if model.selectedJob == nil { Text("Add a file to see available outputs.").font(.system(size: 12)).foregroundStyle(FileformTheme.ink3).padding(10) }
                }.padding(10)
            }
        }.frame(width: 500, height: 420).font(.system(size: 13)).background(FileformTheme.elevated)
            .defaultFocus($focused, true)
    }
    private func sectionLabel(_ title: String) -> some View {
        Text(title).font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink4).padding(.horizontal, 10).padding(.top, 12)
    }
    private func selectFirstMatch() {
        if let first = taskMatches.first { selectTask(first); dismiss() }
        else if let first = formatMatches.first { selectFormat(first.format) }
        else if let first = setupMatches.first { model.presentedSheet = .setups(.apply(first.id)) }
    }
    private func selectTask(_ choice: TaskChoice) {
        if let goal = choice.goal { model.startConversion(goal: goal) }
        else { model.chooseTask(choice.task) }
    }
    private func selectFormat(_ format: OutputFormat) {
        guard !model.restoringSession, let job = model.selectedJob, job.editable else { return }
        model.chooseTask(.convert)
        job.format = format
        if !job.availableGoals.contains(job.goal) { job.goal = job.availableGoals.first ?? .convert }
        dismiss()
    }
}

private struct TaskChoice: Identifiable {
    let id: String
    let title: String
    let symbol: String
    let keywords: String
    let task: WorkspaceTask
    var goal: ConversionGoal? = nil
    static let all = [
        TaskChoice(id: "convert", title: "Convert files", symbol: "arrow.triangle.2.circlepath", keywords: "format image video audio table", task: .convert),
        TaskChoice(id: "smaller", title: "Make a file smaller", symbol: "arrow.down.right.and.arrow.up.left", keywords: "compress reduce size", task: .convert, goal: .compress),
        TaskChoice(id: "fit", title: "Fit a size limit", symbol: "arrow.down.to.line", keywords: "size megabytes MB email attachment", task: .convert, goal: .fit),
        TaskChoice(id: "pages", title: "Organize PDFs", symbol: "doc.on.doc", keywords: "merge split rotate reorder pages", task: .pages),
        TaskChoice(id: "trim", title: "Trim audio or video", symbol: "waveform", keywords: "cut clip range duration recording", task: .trim),
        TaskChoice(id: "link", title: "Save from a link", symbol: "link", keywords: "download recording podcast URL", task: .link),
        TaskChoice(id: "text", title: "Extract text", symbol: "text.viewfinder", keywords: "ocr recognize extract words", task: .text)
    ]
}
