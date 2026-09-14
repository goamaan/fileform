# Website direction

September 14, 2026: minimal, but not empty. The user wants the richer product
widgets of the earlier Fileform site within the calmer Vicinae-inspired layout.

## Reference inspection

- Live reference: https://www.vicinae.com/
- Source: https://github.com/vicinaehq/website
- Inspected commit: b747e78c49fd83271d39d4b9dcd373117d506dcb.
- Read Hero, Features, BuiltInModules, ExtensionShowcase and globals.css.
- Compared Fileform's earlier homepage at canonical commit 53028ef.

Vicinae uses a centered hero, platform-specific installation control, a large
screenshot, bordered feature cards, compact module pills and overlapping code/UI
examples. Its React components use Framer Motion for one-time entrances and CSS
hover feedback. The useful lesson is varied product evidence within a restrained
layout, not reducing everything to text. Fileform does not copy its code/assets.

## Fileform implementation

- Retain teal, system typography, concise copy and obvious source/download paths.
- Restore three real editor screenshots through accessible, manual preview tabs.
- Add four capability cards with small file/page/waveform examples.
- Restore the three-step file workflow and a truthful portable CLI example.
- Use short CSS entrances and interaction transitions; no autoplay or ticker.
- Support Arrow Left/Right, Home/End and visible keyboard focus in preview tabs.
- Disable motion when reduced motion is requested. Keep light/dark tokens.
- Clearly label macOS reference screenshots and partial Electron support.
- Never trade supported-format limits or release truthfulness for marketing copy.
