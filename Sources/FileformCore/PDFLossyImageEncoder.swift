// SPDX-License-Identifier: Apache-2.0
import Foundation
import CoreGraphics
import ImageIO
import FileformDomain

/// Isolated worker implementation. RGB/gray only; no alpha or profile discard.
enum PDFLossyImageEncoder {
    static func encode(_ input: URL, destination: URL, width: Int, height: Int, channels: Int,
                       encodedJPEG: Bool, outputWidth: Int, outputHeight: Int, quality: Double) throws {
        let count = try FileSafety.identity(input).bytes
        guard count <= 256 * 1024 * 1024 else { throw FileformError(.resourceLimit, "Image stream exceeds 256 MiB.") }
        let space = channels == 1 ? CGColorSpaceCreateDeviceGray() : CGColorSpaceCreateDeviceRGB()
        let image: CGImage
        if encodedJPEG {
            let bytes = try Data(contentsOf: input)
            // JPEG APP2 ICC profiles affect sample interpretation independently
            // of the PDF Device color space; keep such images unchanged.
            guard bytes.range(of: Data("ICC_PROFILE\0".utf8)) == nil else { throw FileformError(.unsupported, "Embedded JPEG ICC profiles are unsupported.") }
            guard let source = CGImageSourceCreateWithURL(input as CFURL, nil),
                  CGImageSourceGetType(source) as String? == "public.jpeg", CGImageSourceGetCount(source) == 1,
                  let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
                  (properties[kCGImagePropertyOrientation] as? Int ?? 1) == 1,
                  properties[kCGImagePropertyPixelWidth] as? Int == width, properties[kCGImagePropertyPixelHeight] as? Int == height,
                  let decoded = CGImageSourceCreateImageAtIndex(source, 0, [kCGImageSourceShouldCacheImmediately: true] as CFDictionary),
                  CGImageSourceGetStatusAtIndex(source, 0) == .statusComplete,
                  decoded.width == width, decoded.height == height, decoded.bitsPerComponent == 8,
                  decoded.colorSpace?.numberOfComponents == channels else { throw FileformError(.unsupported, "JPEG samples or orientation do not match the supported PDF image policy.") }
            try ImageIntegrity.validateEndRecord(input, type: "public.jpeg", byteCount: count)
            image = decoded
        } else {
            guard count == Int64(width * height * channels), let provider = CGDataProvider(data: try Data(contentsOf: input) as CFData),
                  let decoded = CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: channels * 8,
                    bytesPerRow: width * channels, space: space, bitmapInfo: CGBitmapInfo(rawValue: 0), provider: provider,
                    decode: nil, shouldInterpolate: true, intent: .defaultIntent) else { throw FileformError(.verificationFailed, "Image sample count is invalid.") }
            image = decoded
        }
        let bitmap = channels == 1 ? CGImageAlphaInfo.none.rawValue : CGImageAlphaInfo.noneSkipLast.rawValue
        guard let context = CGContext(data: nil, width: outputWidth, height: outputHeight, bitsPerComponent: 8,
            bytesPerRow: 0, space: space, bitmapInfo: bitmap) else { throw FileformError(.resourceLimit, "Resampling allocation failed.") }
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: 0, y: 0, width: outputWidth, height: outputHeight))
        guard let rendered = context.makeImage(), let encoder = CGImageDestinationCreateWithURL(destination as CFURL, "public.jpeg" as CFString, 1, nil) else { throw FileformError(.engineFailed, "JPEG encoding could not start.") }
        CGImageDestinationAddImage(encoder, rendered, [kCGImageDestinationLossyCompressionQuality: quality] as CFDictionary)
        guard CGImageDestinationFinalize(encoder) else { throw FileformError(.engineFailed, "JPEG encoding failed.") }
        let bytes = try FileSafety.identity(destination).bytes
        try ImageIntegrity.validateEndRecord(destination, type: "public.jpeg", byteCount: bytes)
        guard let encoded = CGImageSourceCreateWithURL(destination as CFURL, nil),
              CGImageSourceGetType(encoded) as String? == "public.jpeg", CGImageSourceGetCount(encoded) == 1,
              let decoded = CGImageSourceCreateImageAtIndex(encoded, 0, [kCGImageSourceShouldCacheImmediately: true] as CFDictionary),
              CGImageSourceGetStatusAtIndex(encoded, 0) == .statusComplete, decoded.width == outputWidth,
              decoded.height == outputHeight, decoded.bitsPerComponent == 8, decoded.colorSpace?.numberOfComponents == channels else { throw FileformError(.verificationFailed, "Encoded JPEG geometry or sample model is invalid.") }
    }
}
