// Generated from DesignSystem/tokens.json. Do not edit directly.
import SwiftUI
import AppKit

enum FileformTheme {
    static let canvas = adaptive(light: (0xFFFFFF, 1.00000000), dark: (0x0C0D0F, 1.00000000))
    static let surface = adaptive(light: (0xFAFAFB, 1.00000000), dark: (0x13151A, 1.00000000))
    static let elevated = adaptive(light: (0xFAFAFB, 1.00000000), dark: (0x13151A, 1.00000000))
    static let chrome = adaptive(light: (0xF7F8F9, 1.00000000), dark: (0x101216, 1.00000000))
    static let inset = adaptive(light: (0xFAFAFB, 1.00000000), dark: (0x101216, 1.00000000))
    static let ink = adaptive(light: (0x0D0E11, 1.00000000), dark: (0xF3F4F6, 1.00000000))
    static let secondary = adaptive(light: (0x43474F, 1.00000000), dark: (0xB6BBC4, 1.00000000))
    static let ink2 = adaptive(light: (0x43474F, 1.00000000), dark: (0xB6BBC4, 1.00000000))
    static let ink3 = adaptive(light: (0x5A606A, 1.00000000), dark: (0xA7ADB6, 1.00000000))
    static let ink4 = adaptive(light: (0x6E747E, 1.00000000), dark: (0x8A9099, 1.00000000))
    static let border = adaptive(light: (0x090B0F, 0.08627451), dark: (0xFFFFFF, 0.07450980))
    static let field = adaptive(light: (0xFFFFFF, 1.00000000), dark: (0xFFFFFF, 0.04313725))
    static let fieldLine = adaptive(light: (0x090B0F, 0.12941176), dark: (0xFFFFFF, 0.10196078))
    static let accent = adaptive(light: (0x0A7A69, 1.00000000), dark: (0x37D3B4, 1.00000000))
    static let accentStrong = adaptive(light: (0x06584B, 1.00000000), dark: (0x5EE0C7, 1.00000000))
    static let accentSoft = adaptive(light: (0x0A7A69, 0.09019608), dark: (0x37D3B4, 0.12156863))
    static let onAccent = adaptive(light: (0xFFFFFF, 1.00000000), dark: (0x04211C, 1.00000000))
    static let focus = adaptive(light: (0x0A7A69, 1.00000000), dark: (0x37D3B4, 1.00000000))
    static let success = adaptive(light: (0x0C7550, 1.00000000), dark: (0x4ECE9A, 1.00000000))
    static let warning = adaptive(light: (0x96650F, 1.00000000), dark: (0xE7B85C, 1.00000000))
    static let danger = adaptive(light: (0xC0332C, 1.00000000), dark: (0xF0817C, 1.00000000))
    static let image = adaptive(light: (0x0A7A69, 1.00000000), dark: (0x37D3B4, 1.00000000))
    static let media = adaptive(light: (0x6653A6, 1.00000000), dark: (0xA59AF0, 1.00000000))
    static let document = adaptive(light: (0x0A7A69, 1.00000000), dark: (0x37D3B4, 1.00000000))
    static let table = adaptive(light: (0x0C7550, 1.00000000), dark: (0x4ECE9A, 1.00000000))
    static let controlFill = adaptive(light: (0x0A7A69, 1.00000000), dark: (0x37D3B4, 1.00000000))
    enum Spacing {
        static let xs: CGFloat = 4
        static let sm: CGFloat = 8
        static let md: CGFloat = 12
        static let base: CGFloat = 16
        static let lg: CGFloat = 24
        static let xl: CGFloat = 32
        static let xxl: CGFloat = 48
        static let section: CGFloat = 80
    }
    enum Radius {
        static let control: CGFloat = 6
        static let surface: CGFloat = 9
        static let panel: CGFloat = 12
    }
    enum Motion {
        static let fast: Double = 0.1
        static let standard: Double = 0.16
        static let reflow: Double = 0.22
    }
    static func display(_ size: CGFloat) -> Font { .system(size: size, weight: .semibold) }
    static func mono(_ size: CGFloat) -> Font { .custom("JetBrainsMono-Regular", size: size) }
    private static func adaptive(light: (UInt32, CGFloat), dark: (UInt32, CGFloat)) -> Color {
        Color(nsColor: NSColor(name: nil) { appearance in
            let (value, alpha) = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? dark : light
            return NSColor(srgbRed: CGFloat((value >> 16) & 255) / 255, green: CGFloat((value >> 8) & 255) / 255,
                           blue: CGFloat(value & 255) / 255, alpha: alpha)
        })
    }
}
