// SPDX-License-Identifier: Apache-2.0
// Fixture-only ImageIO encoding. Fileform does not advertise HEIC output.
import Foundation
import ImageIO
import UniformTypeIdentifiers
let input = URL(fileURLWithPath: CommandLine.arguments[1])
let output = URL(fileURLWithPath: CommandLine.arguments[2])
guard !FileManager.default.fileExists(atPath: output.path),
      let source = CGImageSourceCreateWithURL(input as CFURL, nil), CGImageSourceGetCount(source) == 1,
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil), image.bitsPerComponent <= 8,
      let destination = CGImageDestinationCreateWithURL(output as CFURL, UTType.heic.identifier as CFString, 1, nil)
else { fatalError("Use one eight-bit owned raster and a new fixture destination") }
CGImageDestinationAddImage(destination, image, [kCGImageDestinationLossyCompressionQuality: 1.0,
    kCGImagePropertyOrientation: 1] as CFDictionary)
guard CGImageDestinationFinalize(destination) else { fatalError("Fixture encoding failed") }
print(output.path)
