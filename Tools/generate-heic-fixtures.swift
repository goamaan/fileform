// SPDX-License-Identifier: Apache-2.0
// Synthetic source-owned fixtures and ImageIO metadata/pixel oracle; macOS only.
import Foundation
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers
let output = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
func image(space: CGColorSpace, alpha: Bool = false) -> CGImage {
    let width = 128, height = 96
    var bytes = [UInt8](repeating: 255, count: width * height * 4)
    for y in 0..<height { for x in 0..<width {
        let index = (y * width + x) * 4
        bytes[index] = UInt8(x * 2); bytes[index + 1] = UInt8(y * 2)
        bytes[index + 2] = UInt8((x + y) % 256)
        bytes[index + 3] = alpha ? UInt8(x < 32 ? 0 : x < 96 ? 127 : 255) : 255
    } }
    let provider = CGDataProvider(data: Data(bytes) as CFData)!
    return CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32,
        bytesPerRow: width * 4, space: space, bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.last.rawValue),
        provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)!
}
for (name, space, alpha, orientation, count) in [
    ("srgb", CGColorSpace(name: CGColorSpace.sRGB)!, false, 1, 1),
    ("p3", CGColorSpace(name: CGColorSpace.displayP3)!, false, 1, 1),
    ("alpha", CGColorSpace(name: CGColorSpace.sRGB)!, true, 1, 1),
    ("multiple", CGColorSpace(name: CGColorSpace.sRGB)!, false, 1, 2)
] + (2...8).map({ ("orientation-\($0)", CGColorSpace(name: CGColorSpace.sRGB)!, false, $0, 1) }) {
    let path = output.appendingPathComponent(name + ".heic")
    guard let destination = CGImageDestinationCreateWithURL(path as CFURL, UTType.heic.identifier as CFString, count, nil) else { fatalError("HEIC destination unavailable") }
    for _ in 0..<count { CGImageDestinationAddImage(destination, image(space: space, alpha: alpha),
        [kCGImageDestinationLossyCompressionQuality: 1.0, kCGImagePropertyOrientation: orientation] as CFDictionary) }
    guard CGImageDestinationFinalize(destination) else { fatalError("HEIC fixture encoding failed") }
    guard let source = CGImageSourceCreateWithURL(path as CFURL, nil), let decoded = CGImageSourceCreateImageAtIndex(source, 0, nil) else { fatalError("ImageIO fixture decoding failed") }
    let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [String: Any] ?? [:]
    let metadata: [String: Any] = ["width": decoded.width, "height": decoded.height,
        "depth": decoded.bitsPerComponent, "count": CGImageSourceGetCount(source),
        "orientation": properties[kCGImagePropertyOrientation as String] ?? 1,
        "alpha": properties[kCGImagePropertyHasAlpha as String] ?? false]
    try JSONSerialization.data(withJSONObject: metadata, options: [.prettyPrinted, .sortedKeys]).write(to: output.appendingPathComponent(name + ".json"))
    // Decoder-space oracle. ICC conversion to sRGB is an independent Rust gate.
    var pixels = [UInt8](repeating: 0, count: decoded.width * decoded.height * 4)
    let drawn = pixels.withUnsafeMutableBytes { buffer -> Bool in
        guard let context = CGContext(data: buffer.baseAddress, width: decoded.width, height: decoded.height,
            bitsPerComponent: 8, bytesPerRow: decoded.width * 4, space: space,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
        context.draw(decoded, in: CGRect(x: 0, y: 0, width: decoded.width, height: decoded.height)); return true
    }
    guard drawn else { fatalError("Could not render ImageIO oracle") }
    try Data(pixels).write(to: output.appendingPathComponent(name + ".rgba"))
}
print(output.path)
