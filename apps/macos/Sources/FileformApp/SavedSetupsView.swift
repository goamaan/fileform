import SwiftUI
import UniformTypeIdentifiers
import FileformDomain

struct SavedSetupsView: View {
    let model: WorkspaceModel
    let intent: SetupIntent
    @State private var selection: UUID?
    @State private var nameEditor: SetupNameEditor?
    @State private var pendingImport: TransformationRecipe?
    @State private var applying: TransformationRecipe?
    @State private var message: String?
    @State private var removal: TransformationRecipe?
    @FocusState private var nameFocused: Bool
    @Environment(\.dismiss) private var dismiss
    private var selected: TransformationRecipe? { selection.flatMap(model.setups.recipe) }

    var body: some View {
        Group {
            if let applying { ApplySetupView(model: model, recipe: applying) }
            else { library }
        }
        .frame(width: 580, height: 480)
        .font(.system(size: 13)).background(FileformTheme.elevated).foregroundStyle(FileformTheme.ink)
        .onAppear {
            selection = model.setups.recipes.first?.id
            switch intent {
            case .manage: break
            case .saveCurrent: prepareNameEditor(.create)
            case .saveRequest(let request, _): nameEditor = .init(mode: .create, id: nil, name: "", request: request)
            case .apply(let id): applying = model.setups.recipe(id)
            }
        }
        .confirmationDialog("Remove this saved setup?", isPresented: Binding(get: { removal != nil }, set: { if !$0 { removal = nil } }), titleVisibility: .visible, presenting: removal) { value in
            Button("Remove Setup", role: .destructive) {
                perform { try model.setups.remove(value.id); selection = model.setups.recipes.first?.id }
                removal = nil
            }
            Button("Cancel", role: .cancel) { removal = nil }
        } message: { _ in Text("Sources and completed files stay on your Mac.") }
    }
    private var library: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Saved setups").font(.system(size: 17, weight: .semibold))
                Spacer(); Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
            }.padding(18)
            Divider()
            if let error = model.setups.storageError { notice(error).padding(.horizontal, 18).padding(.top, 12) }
            if let editor = nameEditor { nameForm(editor) }
            else if let pendingImport { importForm(pendingImport) }
            else {
                if model.setups.recipes.isEmpty {
                    ContentUnavailableView("Keep useful choices", systemImage: "bookmark",
                        description: Text("Reuse output settings or a PDF arrangement with new files."))
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    List(selection: $selection) {
                        ForEach(model.setups.recipes, id: \.id) { recipe in
                            VStack(alignment: .leading, spacing: 5) {
                                Text(recipe.name).font(.system(size: 13, weight: .medium)).lineLimit(2)
                                Text(recipe.setupSummary + " · revision \(recipe.revision)").font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink3)
                            }.padding(.vertical, 6).tag(recipe.id)
                        }
                    }.listStyle(.plain).scrollContentBackground(.hidden).accessibilityIdentifier("saved-setups-list")
                }
                if let message { notice(message).padding(.horizontal, 18).padding(.bottom, 10) }
                Divider()
                HStack(spacing: 10) {
                    Button("Import…") { chooseImport() }.accessibilityIdentifier("import-setup")
                    Button("Save Current…") { prepareNameEditor(.create) }.disabled(model.currentSetupRequest == nil)
                        .accessibilityIdentifier("save-current-setup")
                    Menu("More") {
                        Button("Rename…") { prepareNameEditor(.rename) }
                        Button("Update from Current Choices…") { prepareNameEditor(.update) }.disabled(model.currentSetupRequest == nil)
                        Button("Export…") { if let selected { chooseExport(selected) } }
                        Divider()
                        Button("Remove…", role: .destructive) { removal = selected }
                    }.disabled(selected == nil).accessibilityIdentifier("setup-actions")
                    Spacer()
                    Button("Use Setup…") { applying = selected }.buttonStyle(PrimaryButtonStyle()).disabled(selected == nil || model.isRunning || model.restoringSession)
                        .accessibilityIdentifier("use-setup")
                }.padding(18)
            }
        }
    }
    private func nameForm(_ editor: SetupNameEditor) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(editor.title).font(.headline)
            TextField("Setup name", text: Binding(get: { nameEditor?.name ?? "" }, set: { nameEditor?.name = $0 }))
                .textFieldStyle(.roundedBorder).focused($nameFocused).accessibilityIdentifier("setup-name")
                .onSubmit { saveNameEditor() }
            if let request = editor.request {
                Text((try? TransformationRecipe(name: "Preview", request: request).setupSummary) ?? "Current output choices")
                    .font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink3)
            }
            Text(editor.mode == .update ? "Replaces this setup’s choices. Existing drafts and outputs stay unchanged." : "Saves settings, without source files or folder permissions.")
                .foregroundStyle(FileformTheme.ink3).fixedSize(horizontal: false, vertical: true)
            if let message { notice(message) }
            Spacer()
            HStack {
                Button("Back") { nameEditor = nil; message = nil }
                Spacer()
                Button(editor.mode == .update ? "Update Setup" : "Save Setup") { saveNameEditor() }
                    .buttonStyle(PrimaryButtonStyle()).disabled(editor.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityIdentifier("confirm-save-setup")
            }
        }.padding(22).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .defaultFocus($nameFocused, true).task { nameFocused = true }.onDisappear { nameFocused = false }
    }
    private func importForm(_ recipe: TransformationRecipe) -> some View {
        let existing = model.setups.recipe(recipe.id)
        let newer = existing.map { recipe.revision > $0.revision } ?? true
        return VStack(alignment: .leading, spacing: 16) {
            Text("Import “\(recipe.name)”").font(.headline)
            Text(recipe.setupSummary).font(FileformTheme.mono(12)).foregroundStyle(FileformTheme.ink3)
            Text("Revision \(recipe.revision) · \(recipe.assetSlots.count) source slot\(recipe.assetSlots.count == 1 ? "" : "s")")
            Text("Import adds the setup. Choose files and review compatibility before applying.")
                .foregroundStyle(FileformTheme.ink3)
            if let existing {
                Text(newer ? "This will replace saved revision \(existing.revision) of “\(existing.name)”." : "This setup is already saved at revision \(existing.revision). Import a newer revision to replace it.")
                    .foregroundStyle(newer ? FileformTheme.ink2 : FileformTheme.danger)
            }
            if let message { notice(message) }
            Spacer()
            HStack {
                Button("Cancel Import") { pendingImport = nil; message = nil }
                Spacer()
                Button(existing == nil ? "Import Setup" : "Import and Replace") {
                    perform {
                        try model.setups.importRecipe(recipe, replaceExisting: existing != nil)
                        selection = recipe.id; pendingImport = nil
                    }
                }.buttonStyle(PrimaryButtonStyle()).disabled(!newer).accessibilityIdentifier("confirm-import-setup")
            }
        }.padding(22).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
    private func notice(_ value: String) -> some View {
        Label(value, systemImage: "info.circle").font(.system(size: 12)).foregroundStyle(FileformTheme.ink2)
            .fixedSize(horizontal: false, vertical: true)
    }
    private func prepareNameEditor(_ mode: SetupNameEditor.Mode) {
        message = nil
        if mode != .create && selected == nil { return }
        let request = mode == .rename ? nil : model.currentSetupRequest
        guard mode == .rename || request != nil else { message = "Choose a file with valid output settings or a ready PDF arrangement first."; return }
        nameEditor = .init(mode: mode, id: mode == .create ? nil : selected?.id,
                           name: mode == .create ? "" : (selected?.name ?? ""), request: request)
    }
    private func saveNameEditor() {
        guard let editor = nameEditor else { return }
        perform {
            if editor.mode == .rename, let id = editor.id { try model.setups.rename(id, to: editor.name); selection = id }
            else if let request = editor.request {
                selection = try model.setups.save(name: editor.name, request: request, replacing: editor.id).id
            }
            nameEditor = nil
        }
    }
    private func chooseImport() {
        guard let panel = model.makeOpenPanel() else { return }
        panel.title = "Import a Fileform setup"; panel.prompt = "Review Setup"
        panel.allowedContentTypes = [.json]; panel.allowsOtherFileTypes = false
        panel.canChooseFiles = true; panel.allowsMultipleSelection = false; panel.canChooseDirectories = false
        model.present(panel) { response in
            guard response == .OK, let url = panel.url else { return }
            perform { pendingImport = try SetupLibrary.readRecipe(at: url) }
        }
    }
    private func chooseExport(_ recipe: TransformationRecipe) {
        guard let panel = model.makeSavePanel() else { return }
        panel.title = "Export a portable Fileform setup"; panel.prompt = "Export"
        panel.allowedContentTypes = [.json]; panel.allowsOtherFileTypes = false
        let stem = String(recipe.name.prefix(80)).components(separatedBy: CharacterSet(charactersIn: "/:\\").union(.controlCharacters)).joined(separator: "_")
        panel.nameFieldStringValue = (stem.isEmpty ? "Setup" : stem) + ".json"
        panel.message = "Choose a new filename. This JSON setup also works with the free Fileform CLI."
        model.present(panel) { response in
            guard response == .OK, let url = panel.url else { return }
            perform { try SetupLibrary.export(recipe, to: url); message = "Exported \(url.lastPathComponent)." }
        }
    }
    private func perform(_ action: () throws -> Void) {
        message = nil
        do { try action() } catch { message = error.localizedDescription }
    }
}

private struct SetupNameEditor {
    enum Mode { case create, rename, update }
    let mode: Mode
    let id: UUID?
    var name: String
    let request: TransformationRequest?
    var title: String { switch mode { case .create: "Save current choices"; case .rename: "Rename setup"; case .update: "Update saved choices" } }
}

#Preview("Empty saved setups") {
    SavedSetupsView(model: WorkspaceModel(), intent: .manage)
}
