import XCTest
@testable import PromptCompanion
import CompanionCore

@MainActor
final class FakeEngine: PredictionService {
    var connected = true
    var model = "test"
    var requested = 0
    var completions: [CheckedContinuation<PredictionResult, Error>] = []
    func connect() async throws { connected = true }
    func listTasks(cursor: String?, search: String) async throws -> ([CodexTask], String?) { ([], nil) }
    func context(for id: String) async throws -> ContextSnapshot {
        ContextSnapshot(messages: [ConversationMessage(role: "user", text: id)], isPartial: false)
    }
    func predict(target: CompletionTarget, context: ContextSnapshot, title: String, earlierSummary: String) async throws -> PredictionResult {
        requested += 1
        return try await withCheckedThrowingContinuation { completions.append($0) }
    }
    func cancelPrediction() {} // Deliberately simulate a backend that finishes after cancellation.
    func stop() { connected = false }
    func deliver(_ phrases: [String]) {
        guard !completions.isEmpty else { return }
        completions.removeFirst().resume(returning: PredictionResult(phrases: phrases, summary: "", duration: 0.1))
    }
}

final class ModelTests: XCTestCase {
    func testOpeningGoalSurvivesLongContext() {
        let goal = ConversationMessage(role: "user", text: "Keep all buttons large and left-clickable.")
        let middle = (0..<60).map { _ in ConversationMessage(role: "assistant", text: String(repeating: "x", count: 3000)) }
        let snapshot = ContextSnapshot(messages: [goal] + middle, isPartial: true)
        XCTAssertEqual(snapshot.earlier.first, goal)
        XCTAssertLessThanOrEqual(snapshot.earlier.reduce(0) { $0 + $1.text.count }, 8000)
    }
    @MainActor
    private func fixture() -> (CompanionModel, FakeEngine) {
        let engine = FakeEngine()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("PromptCompanionTests-" + UUID().uuidString)
        let model = CompanionModel(support: directory, engine: engine)
        model.automatic = false
        return (model, engine)
    }
    private func task(_ id: String) -> CodexTask { CodexTask(["id": id, "name": id])! }
    @MainActor
    private func waitFor(_ condition: @escaping () -> Bool) async {
        for _ in 0..<100 {
            if condition() { return }
            try? await Task.sleep(nanoseconds: 5_000_000)
        }
        XCTFail("Condition did not become true")
    }

    @MainActor
    func testLatePredictionCannotInsertAfterTyping() async {
        let (model, engine) = fixture()
        await model.select(task("A"))
        model.refreshPredictions()
        await waitFor { engine.requested == 1 }
        model.edit(text: "No", selection: NSRange(location: 2, length: 0))
        engine.deliver(["Fix login", "Add tests", "Explain the error"])
        await Task.yield()
        model.insert(0)
        XCTAssertEqual(model.draft.text, "No")
        XCTAssertFalse(model.canInsert)
        model.shutdown()
    }

    @MainActor
    func testHoverFreezesLabelsUntilPointerLeaves() async {
        let (model, engine) = fixture()
        await model.select(task("A"))
        model.setHovering(true)
        model.refreshPredictions()
        await waitFor { engine.requested == 1 }
        engine.deliver(["Fix login", "Add tests", "Explain the error"])
        await waitFor { !model.isPredicting }
        XCTAssertTrue(model.phrases.isEmpty)
        model.setHovering(false)
        XCTAssertEqual(model.phrases.first, "Fix login")
        XCTAssertTrue(model.canInsert)
        model.insert(0)
        XCTAssertEqual(model.draft.text, "Fix login ")
        model.undo()
        XCTAssertEqual(model.draft.text, "")
        model.shutdown()
    }

    @MainActor
    func testTaskSwitchPreservesSeparateDraftsAndRejectsLateResults() async {
        let (model, engine) = fixture()
        await model.select(task("A"))
        model.edit(text: "fix", selection: NSRange(location: 3, length: 0))
        model.refreshPredictions()
        await waitFor { engine.requested == 1 }
        await model.select(task("B"))
        model.edit(text: "explain", selection: NSRange(location: 7, length: 0))
        engine.deliver(["fix login", "fix the layout", "fix tests"])
        await Task.yield()
        XCTAssertFalse(model.canInsert)
        await model.select(task("A"))
        XCTAssertEqual(model.draft.text, "fix")
        await model.select(task("B"))
        XCTAssertEqual(model.draft.text, "explain")
        model.shutdown()
    }

    @MainActor
    func testClearIsUndoableAndDraftPersists() async {
        let (model, engine) = fixture()
        await model.select(task("A"))
        model.edit(text: "Keep my words", selection: NSRange(location: 13, length: 0))
        model.clearDraft()
        XCTAssertTrue(model.draft.text.isEmpty)
        model.undo()
        XCTAssertEqual(model.draft.text, "Keep my words")
        let restored = CompanionModel(support: model.support, engine: engine)
        restored.automatic = false
        await restored.select(task("A"))
        XCTAssertEqual(restored.draft.text, "Keep my words")
        model.shutdown(); restored.shutdown()
    }

    @MainActor
    func testTypingMoreLettersReusesMatchingPhrases() async {
        let (model, engine) = fixture()
        await model.select(task("A"))
        model.edit(text: "fi", selection: NSRange(location: 2, length: 0))
        model.refreshPredictions()
        await waitFor { engine.requested == 1 }
        engine.deliver(["fix login", "fix the layout", "fix tests"])
        await waitFor { model.canInsert }
        model.edit(text: "fix", selection: NSRange(location: 3, length: 0))
        XCTAssertTrue(model.canInsert)
        XCTAssertEqual(engine.requested, 1)
        model.insert(0)
        XCTAssertEqual(model.draft.text, "fix login ")
        model.shutdown()
    }
}
