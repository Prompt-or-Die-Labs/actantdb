import Foundation

/// A fully Codable representation of arbitrary JSON. Used wherever the Rust
/// contracts hold a `serde_json::Value` (tool args, command input, replay
/// payloads). Bridges to/from `Any` via `unwrapped` / `init(any:)`.
public enum JSONValue: Codable, Sendable, Equatable, Hashable {
    case null
    case bool(Bool)
    case int(Int64)
    case double(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    // MARK: - Codable

    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null; return }
        if let b = try? c.decode(Bool.self) { self = .bool(b); return }
        if let i = try? c.decode(Int64.self) { self = .int(i); return }
        if let d = try? c.decode(Double.self) { self = .double(d); return }
        if let s = try? c.decode(String.self) { self = .string(s); return }
        if let a = try? c.decode([JSONValue].self) { self = .array(a); return }
        if let o = try? c.decode([String: JSONValue].self) { self = .object(o); return }
        throw DecodingError.dataCorruptedError(in: c, debugDescription: "Unsupported JSON value")
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .null:           try c.encodeNil()
        case .bool(let b):    try c.encode(b)
        case .int(let i):     try c.encode(i)
        case .double(let d):  try c.encode(d)
        case .string(let s):  try c.encode(s)
        case .array(let a):   try c.encode(a)
        case .object(let o):  try c.encode(o)
        }
    }

    // MARK: - Bridges

    /// Convert from a Foundation `Any` (the result of `JSONSerialization`).
    public init(any: Any) throws {
        switch any {
        case is NSNull:           self = .null
        case let b as Bool:       self = .bool(b)
        case let i as Int:        self = .int(Int64(i))
        case let i as Int64:      self = .int(i)
        case let d as Double:     self = .double(d)
        case let n as NSNumber:
            // NSNumber masks bool/int/double — disambiguate by objCType.
            let t = String(cString: n.objCType)
            switch t {
            case "c", "B":        self = .bool(n.boolValue)
            case "q", "i", "l", "s": self = .int(n.int64Value)
            default:              self = .double(n.doubleValue)
            }
        case let s as String:     self = .string(s)
        case let a as [Any]:      self = .array(try a.map { try JSONValue(any: $0) })
        case let o as [String: Any]:
            var dict: [String: JSONValue] = [:]
            dict.reserveCapacity(o.count)
            for (k, v) in o { dict[k] = try JSONValue(any: v) }
            self = .object(dict)
        default:
            throw DecodingError.typeMismatch(
                JSONValue.self,
                .init(codingPath: [], debugDescription: "Unsupported type: \(type(of: any))")
            )
        }
    }

    /// Unwrap to a Foundation `Any` suitable for `JSONSerialization`.
    public var unwrapped: Any {
        switch self {
        case .null:           return NSNull()
        case .bool(let b):    return b
        case .int(let i):     return i
        case .double(let d):  return d
        case .string(let s):  return s
        case .array(let a):   return a.map { $0.unwrapped }
        case .object(let o):  return o.mapValues { $0.unwrapped }
        }
    }

    // MARK: - Convenience accessors

    public var stringValue: String? { if case .string(let s) = self { return s } else { return nil } }
    public var intValue: Int64?     { if case .int(let i) = self { return i } else { return nil } }
    public var boolValue: Bool?     { if case .bool(let b) = self { return b } else { return nil } }
    public var arrayValue: [JSONValue]? { if case .array(let a) = self { return a } else { return nil } }
    public var objectValue: [String: JSONValue]? { if case .object(let o) = self { return o } else { return nil } }

    /// Dictionary-style lookup. Returns `nil` if this is not an object or the key is missing.
    public subscript(key: String) -> JSONValue? {
        if case .object(let o) = self { return o[key] } else { return nil }
    }
}

// MARK: - ExpressibleBy* for ergonomic construction

extension JSONValue: ExpressibleByNilLiteral {
    public init(nilLiteral: ()) { self = .null }
}
extension JSONValue: ExpressibleByBooleanLiteral {
    public init(booleanLiteral value: Bool) { self = .bool(value) }
}
extension JSONValue: ExpressibleByIntegerLiteral {
    public init(integerLiteral value: Int64) { self = .int(value) }
}
extension JSONValue: ExpressibleByFloatLiteral {
    public init(floatLiteral value: Double) { self = .double(value) }
}
extension JSONValue: ExpressibleByStringLiteral {
    public init(stringLiteral value: String) { self = .string(value) }
}
extension JSONValue: ExpressibleByArrayLiteral {
    public init(arrayLiteral elements: JSONValue...) { self = .array(elements) }
}
extension JSONValue: ExpressibleByDictionaryLiteral {
    public init(dictionaryLiteral elements: (String, JSONValue)...) {
        self = .object(Dictionary(uniqueKeysWithValues: elements))
    }
}
