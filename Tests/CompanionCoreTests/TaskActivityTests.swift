import XCTest
@testable import PromptCompanion

final class TaskActivityTests: XCTestCase {
    private func marker(_ type: String) -> String {
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"\(type)\"}}\n"
    }

    func testLatestLifecycleMarkerAcrossChunkBoundaries() throws {
        let path = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: path) }
        let unrelated = "{\"type\":\"response_item\",\"payload\":\"" + String(repeating: "x", count: 140_000) + "\"}\n"
        try (marker("task_complete") + marker("task_started") + unrelated).write(to: path, atomically: true, encoding: .utf8)
        XCTAssertEqual(try TaskActivity.fromRollout(at: path.path), true)
        for terminal in ["task_complete", "turn_aborted"] {
            try (marker("task_started") + unrelated + marker(terminal)).write(to: path, atomically: true, encoding: .utf8)
            XCTAssertEqual(try TaskActivity.fromRollout(at: path.path), false)
        }
        try (marker("task_started") + "{\"type\":").write(to: path, atomically: true, encoding: .utf8)
        XCTAssertEqual(try TaskActivity.fromRollout(at: path.path), true, "Ignore an incomplete trailing record while Codex is writing")
        try unrelated.write(to: path, atomically: true, encoding: .utf8)
        XCTAssertNil(try TaskActivity.fromRollout(at: path.path))
    }

    func testMissingHistoryFailsRatherThanAssumingInactive() {
        XCTAssertThrowsError(try TaskActivity.fromRollout(at: "/nonexistent-" + UUID().uuidString))
    }
}
