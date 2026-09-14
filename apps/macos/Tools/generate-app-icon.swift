import Foundation
import CoreGraphics
import ImageIO

let root = URL(fileURLWithPath: CommandLine.arguments.dropFirst().first ?? FileManager.default.currentDirectoryPath)
let output = root.appendingPathComponent("Resources/Assets.xcassets/AppIcon.appiconset")
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
func color(_ red: CGFloat, _ green: CGFloat, _ blue: CGFloat) -> CGColor {
    CGColor(red: red / 255, green: green / 255, blue: blue / 255, alpha: 1)
}
func drawMark(_ context: CGContext) {
    context.translateBy(x: 0, y: 1024); context.scaleBy(x: 1, y: -1)
    context.setFillColor(color(21, 27, 34))
    context.addPath(CGPath(roundedRect: CGRect(x: 62, y: 62, width: 900, height: 900), cornerWidth: 190, cornerHeight: 190, transform: nil))
    context.fillPath()
    context.setFillColor(color(10, 122, 105))
    context.addPath(CGPath(roundedRect: CGRect(x: 250, y: 220, width: 430, height: 540), cornerWidth: 40, cornerHeight: 40, transform: nil))
    context.fillPath()
    let page = CGMutablePath()
    page.move(to: CGPoint(x: 340, y: 300)); page.addLine(to: CGPoint(x: 670, y: 300))
    page.addLine(to: CGPoint(x: 770, y: 400)); page.addLine(to: CGPoint(x: 770, y: 820))
    page.addLine(to: CGPoint(x: 340, y: 820)); page.closeSubpath()
    context.setFillColor(color(55, 211, 180)); context.addPath(page); context.fillPath()
    let fold = CGMutablePath()
    fold.move(to: CGPoint(x: 670, y: 300)); fold.addLine(to: CGPoint(x: 670, y: 400))
    fold.addLine(to: CGPoint(x: 770, y: 400)); fold.closeSubpath()
    context.setFillColor(color(94, 224, 199)); context.addPath(fold); context.fillPath()
    context.setStrokeColor(color(4, 48, 41)); context.setLineWidth(26); context.setLineCap(.round)
    for (y, end) in [(510, 690), (600, 690), (690, 610)] {
        context.move(to: CGPoint(x: 420, y: y)); context.addLine(to: CGPoint(x: end, y: y)); context.strokePath()
    }
}
var entries = [[String: String]]()
for size in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let pixels = size * scale
        let context = CGContext(data: nil, width: pixels, height: pixels, bitsPerComponent: 8, bytesPerRow: pixels * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.scaleBy(x: CGFloat(pixels) / 1024, y: CGFloat(pixels) / 1024)
        drawMark(context)
        let filename = "icon-\(size)@\(scale)x.png"
        let writer = CGImageDestinationCreateWithURL(output.appendingPathComponent(filename) as CFURL, "public.png" as CFString, 1, nil)!
        CGImageDestinationAddImage(writer, context.makeImage()!, nil)
        guard CGImageDestinationFinalize(writer) else { fatalError("Could not encode app icon") }
        entries.append(["idiom": "mac", "size": "\(size)x\(size)", "scale": "\(scale)x", "filename": filename])
    }
}
let catalog: [String: Any] = ["images": entries, "info": ["version": 1, "author": "Fileform"]]
try JSONSerialization.data(withJSONObject: catalog, options: [.prettyPrinted, .sortedKeys]).write(to: output.appendingPathComponent("Contents.json"))
print("Generated Fileform's teal document icon.")
