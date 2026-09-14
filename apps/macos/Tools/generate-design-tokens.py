#!/usr/bin/env python3
"""Generate both platform token surfaces; --check fails if outputs are stale."""
import argparse
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
tokens = json.loads((root / "DesignSystem/tokens.json").read_text())

def kebab(name):
    return "".join("-" + c.lower() if c.isupper() else c for c in name)

css = ["/* Generated from DesignSystem/tokens.json. Do not edit directly. */", ":root {"]
for name, value in tokens["colors"].items():
    css.append(f"  --fileform-{kebab(name)}: {value['light']};")
for group in ["spacing", "radius"]:
    for name, value in tokens[group].items():
        css.append(f"  --fileform-{group}-{kebab(name)}: {value / 16:g}rem;")
for name, value in tokens["motion"].items():
    css.append(f"  --fileform-motion-{name}: {value}ms;")
for name, value in tokens["typography"]["web"].items():
    css.append(f"  --fileform-type-{name}: {value / 16:g}rem;")
css += ["}", "@media (prefers-color-scheme: dark) {", "  :root {"]
for name, value in tokens["colors"].items():
    css.append(f"    --fileform-{kebab(name)}: {value['dark']};")
css += ["  }", "}"]

swift = ["// Generated from DesignSystem/tokens.json. Do not edit directly.", "import SwiftUI", "import AppKit", "", "enum FileformTheme {"]
for name, value in tokens["colors"].items():
    def color(value):
        rgb, alpha = value[1:7], int(value[7:9], 16) / 255 if len(value) == 9 else 1
        return f"(0x{rgb}, {alpha:.8f})"
    swift.append(f"    static let {name} = adaptive(light: {color(value['light'])}, dark: {color(value['dark'])})")
for group in ["spacing", "radius", "motion"]:
    swift.append(f"    enum {group.title()} {{")
    for name, value in tokens[group].items():
        swift.append(f"        static let {name}: {'Double' if group == 'motion' else 'CGFloat'} = {value / 1000 if group == 'motion' else value}")
    swift.append("    }")
swift += [
    "    static func display(_ size: CGFloat) -> Font { .system(size: size, weight: .semibold) }",
    "    static func mono(_ size: CGFloat) -> Font { .custom(\"JetBrainsMono-Regular\", size: size) }",
    "    private static func adaptive(light: (UInt32, CGFloat), dark: (UInt32, CGFloat)) -> Color {",
    "        Color(nsColor: NSColor(name: nil) { appearance in",
    "            let (value, alpha) = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? dark : light",
    "            return NSColor(srgbRed: CGFloat((value >> 16) & 255) / 255, green: CGFloat((value >> 8) & 255) / 255,",
    "                           blue: CGFloat(value & 255) / 255, alpha: alpha)",
    "        })",
    "    }",
    "}"
]
outputs = {
    root / "Website/app/tokens.css": "\n".join(css) + "\n",
    root / "Sources/FileformApp/DesignTokens.swift": "\n".join(swift) + "\n",
}
parser = argparse.ArgumentParser(); parser.add_argument("--check", action="store_true"); parser.add_argument("--swift-only", action="store_true"); args = parser.parse_args()
for path, content in outputs.items():
    if args.swift_only and path.suffix != ".swift":
        continue
    if args.check:
        if not path.exists() or path.read_text() != content:
            raise SystemExit(f"Stale generated design tokens: {path.relative_to(root)}")
    else:
        path.parent.mkdir(parents=True, exist_ok=True); path.write_text(content)
scope = "native" if args.swift_only else "web and native"
print(f"Design tokens match {scope} outputs." if args.check else f"Generated {scope} design tokens.")
