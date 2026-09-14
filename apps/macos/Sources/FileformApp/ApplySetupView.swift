import SwiftUI
import FileformDomain

struct ApplySetupView: View {
    let model: WorkspaceModel
    let recipe: TransformationRecipe
    @State private var selected = Set<UUID>()
    @State private var bindings: [UUID?] = []
    @State private var outputName = "Combined.pdf"
    @State private var review: SetupApplicationReview?
    @State private var reviewID: UUID?
    @State private var reviewing = false
    @State private var error: String?
    @Environment(\.dismiss) private var dismiss
    private var sourceIDs: [UUID] {
        recipe.isCompositionSetup ? bindings.compactMap { $0 } : model.jobs.filter { selected.contains($0.id) }.map(\.id)
    }
    private var snapshots: [SetupSourceSnapshot] { model.jobs.map(SetupSourceSnapshot.init) }
    private var chosenCount: Int { recipe.isCompositionSetup ? bindings.compactMap { $0 }.count : selected.count }
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 5) {
                    Text(recipe.name).font(.system(size: 17, weight: .semibold)).lineLimit(2)
                    Text(recipe.setupSummary + " · revision \(recipe.revision)").font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink3)
                }
                Spacer(); Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
            }.padding(18)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    if let review { reviewedChoices(review) }
                    else {
                        Text(recipe.isCompositionSetup ? "Choose sources in order. Applying replaces the page arrangement; saved outputs remain." : "Choose files to apply these settings. Compatibility is checked first.")
                            .foregroundStyle(FileformTheme.ink3).fixedSize(horizontal: false, vertical: true)
                        if recipe.isCompositionSetup { sourceBindings }
                        else { fileChoices }
                        if recipe.isCompositionSetup {
                            TextField("Output name", text: $outputName).textFieldStyle(.roundedBorder).accessibilityLabel("Page export output name")
                        }
                        Button { model.chooseDestination() } label: {
                            Label(model.outputFolder?.lastPathComponent ?? "Choose output folder…", systemImage: "folder")
                        }.disabled(model.isRunning)
                        Text(recipe.collisionPolicy == .rename ? "Existing files stay; duplicate names receive a number." : "Stops if the name exists. Choose another name or folder.")
                            .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                        Button("Add Files…") { model.chooseFiles() }
                    }
                    if let error { Label(error, systemImage: "exclamationmark.circle").foregroundStyle(FileformTheme.danger).fixedSize(horizontal: false, vertical: true) }
                }.padding(20)
            }.disabled(reviewing || model.restoringSession)
            Divider()
            HStack {
                if review != nil { Button("Change Selection") { resetReview() } }
                else { Text("\(chosenCount) selected").font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink3) }
                Spacer()
                if reviewing { ProgressView().controlSize(.small); Text("Checking choices…") }
                else if let review {
                    Button(recipe.isCompositionSetup ? "Apply Page Arrangement" : "Apply to \(review.applicableCount) \(review.applicableCount == 1 ? "File" : "Files")") {
                        do { try model.applySetup(review); dismiss() }
                        catch { self.error = error.localizedDescription }
                    }.buttonStyle(PrimaryButtonStyle()).disabled(review.applicableCount == 0 || model.isRunning || model.restoringSession)
                        .accessibilityIdentifier("apply-reviewed-setup")
                } else {
                    Button("Review Choices") { reviewID = UUID() }.buttonStyle(PrimaryButtonStyle())
                        .disabled(sourceIDs.isEmpty || model.isRunning || model.restoringSession || (recipe.isCompositionSetup && sourceIDs.count != recipe.assetSlots.count))
                        .accessibilityIdentifier("review-setup")
                }
            }.padding(18)
        }
        .onAppear {
            outputName = model.pdfWorkspace.outputName
            if let id = model.selection { selected = [id] }
            let candidates = model.jobs.filter { job in job.inspection.map { [.image, .pdf].contains($0.family) } ?? false }
            bindings = recipe.assetSlots.indices.prefix(128).map { $0 < candidates.count ? candidates[$0].id : nil }
        }
        .onChange(of: selected) { _, _ in resetReview() }
        .onChange(of: bindings) { _, _ in resetReview() }
        .onChange(of: outputName) { _, _ in resetReview() }
        .onChange(of: model.outputFolder) { _, _ in resetReview() }
        .onChange(of: snapshots) { old, new in
            let added = Set(new.map(\.id)).subtracting(old.map(\.id))
            if !recipe.isCompositionSetup { selected.formUnion(added) }
            resetReview()
        }
        .task(id: reviewID) {
            guard let id = reviewID else { return }
            reviewing = true; error = nil
            do {
                let value = try await model.reviewSetup(recipe, sourceIDs: sourceIDs, outputName: outputName)
                guard !Task.isCancelled, reviewID == id else { return }
                review = value
            } catch is CancellationError { }
            catch { if !Task.isCancelled, reviewID == id { self.error = error.localizedDescription } }
            if reviewID == id { reviewing = false }
        }
    }
    private var sourceBindings: some View {
        VStack(alignment: .leading, spacing: 12) {
            if recipe.assetSlots.count > 128 {
                Text("This setup needs \(recipe.assetSlots.count) sources. The current page editor supports up to 128; the saved setup is unchanged.").foregroundStyle(FileformTheme.danger)
            } else {
            ForEach(recipe.assetSlots.indices, id: \.self) { index in
                Picker("Source \(index + 1)", selection: Binding(get: { bindings.indices.contains(index) ? bindings[index] : nil }, set: { value in
                    if bindings.indices.contains(index) { bindings[index] = value }
                })) {
                    Text("Choose a file").tag(nil as UUID?)
                    ForEach(model.jobs) { job in Text(job.input.lastPathComponent).tag(Optional(job.id)) }
                }.accessibilityIdentifier("setup-source-\(index + 1)")
            }
            }
        }
    }
    private var fileChoices: some View {
        LazyVStack(alignment: .leading, spacing: 10) {
            HStack {
                Button("Select All") { selected = Set(model.jobs.map(\.id)) }
                Button("Select None") { selected = [] }
            }
            ForEach(model.jobs) { job in
                Toggle(isOn: Binding(get: { selected.contains(job.id) }, set: { value in
                    if value { selected.insert(job.id) } else { selected.remove(job.id) }
                })) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(job.input.lastPathComponent).lineLimit(1).truncationMode(.middle)
                        Text(job.statusText).font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                    }
                }.toggleStyle(.checkbox)
            }
            if model.jobs.isEmpty { Text("Add files to try this setup.").foregroundStyle(FileformTheme.ink3) }
        }
    }
    private func reviewedChoices(_ review: SetupApplicationReview) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(recipe.isCompositionSetup ? "Ready to apply the page arrangement" : "\(review.applicableCount) compatible file\(review.applicableCount == 1 ? "" : "s")")
                .font(.headline)
            Text("Applies to selected drafts. Other drafts stay unchanged; export from the workspace.")
                .foregroundStyle(FileformTheme.ink3)
            if let pdf = review.pdf {
                Text("\(pdf.pages.count) \(pdf.pages.count == 1 ? "page" : "pages") from \(pdf.jobs.count) \(pdf.jobs.count == 1 ? "source" : "sources") · \(pdf.plan.request.output.cardinality == .directory ? "output folder" : "one PDF")")
                    .font(FileformTheme.mono(12))
                ForEach(Array(pdf.jobs.enumerated()), id: \.element.id) { index, job in
                    Text("Source \(index + 1): \(job.input.lastPathComponent)").lineLimit(2)
                }
            } else {
                ForEach(review.changes, id: \.job.id) { change in Label(change.job.input.lastPathComponent, systemImage: "checkmark.circle").foregroundStyle(FileformTheme.success) }
            }
            ForEach(review.issues) { issue in
                VStack(alignment: .leading, spacing: 4) {
                    Text(issue.name).fontWeight(.medium)
                    Text(issue.message).font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                }
            }
            if !review.issues.isEmpty { Text("\(review.issues.count) incompatible file\(review.issues.count == 1 ? " stays" : "s stay") unchanged.").font(.system(size: 12)) }
            ForEach(review.warnings, id: \.self) { Text($0).font(.system(size: 12)).foregroundStyle(FileformTheme.ink3) }
        }.accessibilityIdentifier("setup-review-summary")
    }
    private func resetReview() { reviewID = nil; review = nil; reviewing = false; error = nil }
}
