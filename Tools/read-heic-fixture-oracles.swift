// SPDX-License-Identifier: Apache-2.0
// Read owned HEIC metadata cases through the original ImageIO reference decoder.
import Foundation
import CoreGraphics
import ImageIO
let input = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
let output = URL(fileURLWithPath: CommandLine.arguments[2], isDirectory: true)
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
for path in try FileManager.default.contentsOfDirectory(at: input, includingPropertiesForKeys: nil) where path.pathExtension == "heic" {
    guard let source = CGImageSourceCreateWithURL(path as CFURL, nil), let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { continue }
    let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [String: Any] ?? [:]
    let metadata: [String: Any] = ["width": image.width, "height": image.height, "depth": image.bitsPerComponent,
        "count": CGImageSourceGetCount(source), "orientation": properties[kCGImagePropertyOrientation as String] ?? 1,
        "alpha": properties[kCGImagePropertyHasAlpha as String] ?? false]
    let name = path.deletingPathExtension().lastPathComponent
    try JSONSerialization.data(withJSONObject: metadata, options: [.prettyPrinted,.sortedKeys]).write(to: output.appendingPathComponent(name + ".json"))
    guard image.bitsPerComponent <= 8 else { continue }
    var bytes = [UInt8](repeating: 0, count: image.width * image.height * 4)
    let drawn = bytes.withUnsafeMutableBytes { buffer -> Bool in
        guard let context = CGContext(data: buffer.baseAddress, width: image.width, height: image.height,
            bitsPerComponent: 8, bytesPerRow: image.width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height)); return true
    }
    if drawn { try Data(bytes).write(to: output.appendingPathComponent(name + ".srgb.rgba")) }
}
print(output.path)
