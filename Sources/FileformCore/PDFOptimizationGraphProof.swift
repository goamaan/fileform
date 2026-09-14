// SPDX-License-Identifier: Apache-2.0
import Foundation
import CryptoKit
import CoreFoundation
import FileformDomain

/// qpdf's generalized decoded stream representation permits serialization and
/// object-number changes, but no unplanned reachable document-object changes.
struct PDFOptimizationGraphProof {
    var objects: [String: Any]
    init(data: Data) throws {
        guard data.count <= 128 * 1024 * 1024,
              let root = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let qpdf = root["qpdf"] as? [[String: Any]], qpdf.count == 2, qpdf[1].count <= 100_000,
              (qpdf[1]["trailer"] as? [String: Any])?["value"] is [String: Any] else {
            throw FileformError(.resourceLimit, "PDF preservation proof exceeds 128 MiB or 100000 objects, or has invalid structure.")
        }
        objects = qpdf[1]
    }
    mutating func replace(reference: String, dictionary: [String: Any], jpeg: Data) {
        objects["obj:" + reference] = ["stream": ["dict": dictionary, "data": jpeg.base64EncodedString()]]
    }
    func digest() throws -> String {
        var hash = SHA256(), seen: [String: Int] = [:], count = 0
        func add(_ value: String) {
            let data = Data(value.utf8)
            var size = UInt64(data.count).bigEndian
            withUnsafeBytes(of: &size) { hash.update(data: Data($0)) }; hash.update(data: data)
        }
        func walk(_ value: Any, depth: Int, streamDictionary: Bool = false, trailer: Bool = false) throws {
            try Task.checkCancellation()
            count += 1
            guard depth <= 128, count <= 1_000_000 else { throw FileformError(.resourceLimit, "PDF preservation traversal exceeds depth or object limits.") }
            if let reference = value as? String, PDFImageGraph.reference(reference) != nil {
                if let index = seen[reference] { add("reference:\(index)"); return }
                guard let object = objects["obj:" + reference] else { throw FileformError(.verificationFailed, "Missing referenced PDF object.") }
                let index = seen.count; seen[reference] = index; add("object:\(index)")
                try walk(object, depth: depth + 1); return
            }
            if let dictionary = value as? [String: Any] {
                add("dictionary")
                for key in dictionary.keys.sorted() {
                    if streamDictionary && key == "/Length" { continue }
                    if trailer && ["/ID", "/Size", "/Prev", "/XRefStm"].contains(key) { continue }
                    // A cross-reference stream carries serialization-only keys
                    // in qpdf's trailer view. Scope this normalization strictly
                    // to a trailer explicitly typed as XRef, never PDF content.
                    if trailer && dictionary["/Type"] as? String == "/XRef" && ["/Type", "/W", "/Index", "/Length", "/Filter", "/DecodeParms"].contains(key) { continue }
                    add(key)
                    try walk(dictionary[key]!, depth: depth + 1, streamDictionary: key == "dict" && dictionary["data"] != nil)
                }
                add("end-dictionary")
            } else if let array = value as? [Any] {
                add("array:\(array.count)"); for item in array { try walk(item, depth: depth + 1) }; add("end-array")
            } else if let value = value as? String { add("string"); add(value) }
            else if let number = value as? NSNumber {
                if CFGetTypeID(number) == CFBooleanGetTypeID() { add(number.boolValue ? "true" : "false") }
                else { add("number:" + number.stringValue) }
            } else if value is NSNull { add("null") }
            else { throw FileformError(.verificationFailed, "Unknown PDF preservation value.") }
        }
        try walk((objects["trailer"] as! [String: Any])["value"]!, depth: 0, trailer: true)
        return hash.finalize().map { String(format: "%02x", $0) }.joined()
    }
}
