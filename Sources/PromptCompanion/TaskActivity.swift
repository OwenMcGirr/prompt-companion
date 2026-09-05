import Foundation

/// Read only lifecycle markers from the local history path supplied by Codex.
/// Separate app-server processes can label another process's unfinished turn
/// "interrupted"; the persisted start/finish markers preserve its actual lifecycle.
enum TaskActivity {
    static func fromRollout(at path: String) throws -> Bool? {
        let file = try FileHandle(forReadingFrom: URL(fileURLWithPath: path))
        defer { try? file.close() }
        var offset = try file.seekToEnd()
        var remainder = Data()
        // Scan backwards in bounded chunks. Do not cache or log conversation text.
        while offset > 0 {
            let length = min(offset, 65_536)
            offset -= length
            try file.seek(toOffset: offset)
            var chunk = try file.read(upToCount: Int(length)) ?? Data()
            chunk.append(remainder)
            let lines = chunk.split(separator: 10, omittingEmptySubsequences: false)
            for line in lines.dropFirst().reversed() {
                if let active = marker(line) { return active }
            }
            remainder = lines.first.map { Data($0) } ?? Data()
        }
        return marker(remainder)
    }

    private static func marker(_ data: Data) -> Bool? {
        guard let record = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              record["type"] as? String == "event_msg",
              let payload = record["payload"] as? [String: Any] else { return nil }
        switch payload["type"] as? String {
        case "task_started": return true
        case "task_complete", "turn_aborted": return false
        default: return nil
        }
    }
}
