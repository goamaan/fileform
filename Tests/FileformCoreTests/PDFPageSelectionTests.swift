// SPDX-License-Identifier: Apache-2.0
import Testing
import FileformDomain

@Test func pdfPageSelectionPreservesGroupsOrderAndDuplicates() throws {
    #expect(try PDFPageSelection.groups(ranges: "2-5; 4, 2, 4; 1", pageCount: 5) == [[1,2,3,4],[3,1,3],[0]])
    #expect(try PDFPageSelection.groups(every: 2, pageCount: 5) == [[0,1],[2,3],[4]])
    #expect(try PDFPageSelection.groups(every: Int.max, pageCount: 3) == [[0,1,2]])
}

@Test func pdfPageSelectionRejectsMalformedAndOversizedInputs() throws {
    for text in ["", " ", "1;", ";1", "1,,2", "0", "-1", "3-2", "1-6", "1-2-3", "999999999999999999999999"] {
        #expect(throws: FileformError.self) { try PDFPageSelection.groups(ranges: text, pageCount: 5) }
    }
    #expect(throws: FileformError.self) { try PDFPageSelection.groups(ranges: "1-1000;1", pageCount: 1000) }
    #expect(throws: FileformError.self) { try PDFPageSelection.groups(every: 0, pageCount: 5) }
    #expect(throws: FileformError.self) { try PDFPageSelection.groups(every: 1, pageCount: 1001) }
}
