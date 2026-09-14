import SwiftUI

struct WorkbenchInspector<Content: View>: View {
    private let content: Content
    init(@ViewBuilder content: () -> Content) { self.content = content() }
    var body: some View {
        content.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(FileformTheme.elevated, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(FileformTheme.border, lineWidth: 1))
            .padding(10).frame(width: 306)
    }
}
struct WorkbenchNotice: View {
    let text: String
    var tone = FileformTheme.warning
    var body: some View {
        Text(text).font(.system(size: 12)).foregroundStyle(FileformTheme.ink2)
            .fixedSize(horizontal: false, vertical: true).lineSpacing(3)
            .padding(.vertical, 10).padding(.leading, 16).padding(.trailing, 12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(tone.opacity(0.08), in: RoundedRectangle(cornerRadius: 7))
            .overlay(alignment: .leading) { RoundedRectangle(cornerRadius: 2).fill(tone).frame(width: 3).padding(.vertical, 9).padding(.leading, 7) }
    }
}
struct WorkbenchButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var enabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.system(size: 12, weight: .medium))
            .foregroundStyle(enabled ? FileformTheme.ink2 : FileformTheme.ink4)
            .padding(.horizontal, 10).frame(minHeight: 28)
            .background(configuration.isPressed ? FileformTheme.inset : FileformTheme.field, in: RoundedRectangle(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(FileformTheme.fieldLine, lineWidth: 1))
    }
}

struct WorkbenchChoice<Value: Hashable>: View {
    let title: String
    @Binding var selection: Value
    let options: [Value]
    var prominent = false
    let text: (Value) -> String
    @Environment(\.isEnabled) private var enabled
    @State private var showing = false
    @FocusState private var focusedOption: Value?
    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            if !prominent { Text(title.uppercased()).font(FileformTheme.mono(11)).foregroundStyle(FileformTheme.ink4) }
            Button { showing = true } label: {
                HStack {
                    Text(text(selection)).font(.system(size: prominent ? 17 : 13, weight: prominent ? .semibold : .regular)).lineLimit(1)
                    Spacer(minLength: 6)
                    Image(systemName: "chevron.down").font(.system(size: 9, weight: .semibold)).foregroundStyle(FileformTheme.ink4)
                }.foregroundStyle(FileformTheme.ink).padding(.horizontal, prominent ? 0 : 10).frame(height: 30)
                    .background(prominent ? .clear : FileformTheme.field, in: RoundedRectangle(cornerRadius: 6))
                    .overlay(RoundedRectangle(cornerRadius: 6).stroke(prominent ? .clear : FileformTheme.fieldLine, lineWidth: 1))
            }.buttonStyle(.plain).accessibilityLabel(title).accessibilityValue(text(selection))
                .popover(isPresented: $showing, arrowEdge: .bottom) {
                    VStack(spacing: 3) {
                        ForEach(options, id: \.self) { value in
                            Button { selection = value; showing = false } label: {
                                HStack {
                                    Text(text(value)).font(.system(size: 13))
                                    Spacer(minLength: 12)
                                    if selection == value { Image(systemName: "checkmark").foregroundStyle(FileformTheme.accent) }
                                }.foregroundStyle(FileformTheme.ink).padding(.horizontal, 10).frame(minHeight: 30)
                                    .background(selection == value ? FileformTheme.field : .clear, in: RoundedRectangle(cornerRadius: 5))
                            }.buttonStyle(.plain).focused($focusedOption, equals: value)
                        }
                    }.padding(6).frame(minWidth: 230)
                        .background(FileformTheme.elevated).onExitCommand { showing = false }
                        .onAppear { focusedOption = selection }
                }
        }.opacity(enabled ? 1 : 0.5)
    }
}
