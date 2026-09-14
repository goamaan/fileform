import SwiftUI

struct PrimaryButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var enabled
    @Environment(\.isFocused) private var focused
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(enabled ? FileformTheme.onAccent : FileformTheme.secondary)
            .padding(.horizontal, 14).frame(minHeight: 30)
            .background(enabled ? (configuration.isPressed ? FileformTheme.accentStrong : FileformTheme.accent) : FileformTheme.inset,
                        in: RoundedRectangle(cornerRadius: FileformTheme.Radius.control))
            .overlay {
                if focused { RoundedRectangle(cornerRadius: FileformTheme.Radius.control + 3).stroke(FileformTheme.focus, lineWidth: 2).padding(-4) }
            }
            .animation(reduceMotion ? nil : .easeOut(duration: FileformTheme.Motion.fast), value: configuration.isPressed)
    }
}
