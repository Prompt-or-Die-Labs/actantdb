import Foundation

actor CloudKitOutboxStore {
    private struct Envelope: Codable {
        let version: Int
        var events: [EventRow]
    }

    private let fileURL: URL
    private var events: [EventRow]

    init(fileURL: URL) throws {
        self.fileURL = fileURL
        self.events = try Self.load(from: fileURL)
    }

    static func defaultStore(containerID: String) throws -> CloudKitOutboxStore {
        try CloudKitOutboxStore(fileURL: defaultFileURL(containerID: containerID))
    }

    func append(_ newEvents: [EventRow]) throws {
        guard !newEvents.isEmpty else { return }
        var seen = Set(events.map(\.id))
        for event in newEvents where !seen.contains(event.id) {
            events.append(event)
            seen.insert(event.id)
        }
        try persist()
    }

    func all() -> [EventRow] {
        events
    }

    func count() -> Int {
        events.count
    }

    func remove(ids: Set<String>) throws {
        guard !ids.isEmpty else { return }
        events.removeAll { ids.contains($0.id) }
        try persist()
    }

    private func persist() throws {
        let parent = fileURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: parent, withIntermediateDirectories: true)
        let data = try JSONEncoder().encode(Envelope(version: 1, events: events))
        try data.write(to: fileURL, options: .atomic)
    }

    private static func load(from fileURL: URL) throws -> [EventRow] {
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return [] }
        let data = try Data(contentsOf: fileURL)
        guard !data.isEmpty else { return [] }
        return try JSONDecoder().decode(Envelope.self, from: data).events
    }

    private static func defaultFileURL(containerID: String) throws -> URL {
        let base = try FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        let safeContainer = containerID.map { character -> Character in
            if character.isLetter || character.isNumber || character == "." || character == "-" {
                return character
            }
            return "_"
        }
        return base
            .appendingPathComponent("actantdb", isDirectory: true)
            .appendingPathComponent("cloudkit-outbox-\(String(safeContainer)).json")
    }
}
