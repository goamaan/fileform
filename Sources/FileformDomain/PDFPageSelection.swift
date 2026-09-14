// SPDX-License-Identifier: Apache-2.0
import Foundation

/// Page positions are one-based in user input and zero-based in the returned groups.
/// Order and duplicate positions are intentional; no source contents are read here.
public enum PDFPageSelection {
    public static func groups(ranges: String, pageCount: Int) throws -> [[Int]] {
        guard ranges.utf8.count <= 16384 else { throw FileformError(.resourceLimit, "The page selection is too long.") }
        guard pageCount > 0 else { throw FileformError(.invalidRequest, "Add pages before choosing ranges.") }
        guard !ranges.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { throw FileformError(.invalidRequest, "Enter at least one page or range.") }
        var total = 0
        return try ranges.split(separator: ";", omittingEmptySubsequences: false).map { group in
            var pages: [Int] = []
            for part in group.split(separator: ",", omittingEmptySubsequences: false) {
                let bounds = part.split(separator: "-", omittingEmptySubsequences: false).map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                guard (1...2).contains(bounds.count), let start = Int(bounds[0]), start > 0,
                      let end = bounds.count == 2 ? Int(bounds[1]) : start, end >= start, end <= pageCount else {
                    throw FileformError(.invalidRequest, "Use existing one-based page numbers or increasing ranges, separated by commas; use semicolons between PDFs.")
                }
                guard end - start < 1000 - total else { throw FileformError(.resourceLimit, "Select at most 1000 pages, including duplicates.") }
                total += end - start + 1
                pages += (start...end).map { $0 - 1 }
            }
            guard !pages.isEmpty else { throw FileformError(.invalidRequest, "Each output PDF needs at least one page.") }
            return pages
        }
    }

    public static func groups(every count: Int, pageCount: Int) throws -> [[Int]] {
        guard count > 0, pageCount > 0 else { throw FileformError(.invalidRequest, "Choose a positive number of pages per PDF.") }
        guard pageCount <= 1000 else { throw FileformError(.resourceLimit, "Split at most 1000 pages at a time.") }
        return stride(from: 0, to: pageCount, by: min(count, pageCount)).map { start in
            Array(start..<min(pageCount, start + min(count, pageCount)))
        }
    }
}
