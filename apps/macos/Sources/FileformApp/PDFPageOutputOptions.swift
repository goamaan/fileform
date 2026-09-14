import Foundation
import FileformDomain

struct PDFPageOutputOptions: Codable, Equatable {
    var format: OutputFormat = .pdf
    var dpi = "144"
    var quality = 0.85
    var selectedOnly = false
    var isRaster: Bool { format == .png || format == .jpeg }
    var isExtraction: Bool { format == .images }
    var isImageOutput: Bool { isRaster || isExtraction }
    var isValidRecord: Bool {
        [.pdf, .png, .jpeg, .images].contains(format) && dpi.utf8.count <= 32 && quality.isFinite && (0.05...1).contains(quality)
    }
}
