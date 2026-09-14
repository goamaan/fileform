import SwiftUI
import FileformDomain

struct BatchChoicesSheet: View {
    let model: WorkspaceModel
    let sourceID: UUID
    let sourceIDs: [UUID]
    @State private var review: SetupApplicationReview?
    @State private var error: String?
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text("Apply settings").font(.system(size: 17, weight: .semibold))
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
            }
            if let source = model.jobs.first(where: { $0.id == sourceID }) {
                Text("Using the output choices from \(source.input.lastPathComponent).")
                    .font(.system(size: 13)).foregroundStyle(FileformTheme.ink3)
            }
            Divider()
            if let review {
                Text(review.recipe.setupSummary).font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink2)
                Text("\(review.applicableCount) compatible · \(review.issues.count) unchanged").font(.headline)
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        ForEach(review.changes, id: \.job.id) { change in
                            Label(change.job.input.lastPathComponent, systemImage: "checkmark.circle").foregroundStyle(FileformTheme.success)
                        }
                        ForEach(review.issues) { issue in
                            VStack(alignment: .leading, spacing: 4) {
                                Text(issue.name).fontWeight(.medium)
                                Text(issue.message).font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                            }
                        }
                        ForEach(review.warnings, id: \.self) { Text($0).font(.system(size: 12)).foregroundStyle(FileformTheme.ink3) }
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }
                Text("Compatible drafts become the run selection. Saved results stay unchanged. Nothing is exported yet.")
                    .font(.system(size: 12)).foregroundStyle(FileformTheme.ink3)
                HStack {
                    Spacer()
                    Button("Apply to \(review.applicableCount) files") {
                        do {
                            try model.applySetup(review)
                            model.selectBatch(Set(review.changes.map(\.job.id)))
                            model.runSelectionOnly = true
                            model.saveSession()
                            dismiss()
                        } catch { self.error = error.localizedDescription }
                    }.buttonStyle(PrimaryButtonStyle()).disabled(review.applicableCount == 0)
                        .accessibilityIdentifier("apply-batch-choices")
                }
            } else if error == nil {
                ProgressView("Checking each file…").frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            if let error { WorkbenchNotice(text: error, tone: FileformTheme.danger) }
        }.padding(24).frame(width: 560, height: 470).background(FileformTheme.elevated)
        .task {
            do {
                guard let source = model.jobs.first(where: { $0.id == sourceID }) else { throw FileformError(.inputChanged, "The source selection changed.") }
                let recipe = try TransformationRecipe(name: "Batch choices", request: model.transformationRequest(for: source))
                let value = try await model.reviewSetup(recipe, sourceIDs: sourceIDs, outputName: "")
                try Task.checkCancellation()
                review = value
            } catch is CancellationError { }
            catch { self.error = error.localizedDescription }
        }
    }
}
