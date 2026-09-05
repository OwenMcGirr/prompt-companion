import XCTest
@testable import PromptCompanion
import CompanionCore

final class ExpansionOutputTests: XCTestCase {
    func testExpandedOutputPreservesTheFullPrompt() throws {
        let json = #"{"kind":"expanded","prompt":"Make the buttons larger. Keep their arrangement unchanged.","question":"","choices":[]}"#
        XCTAssertEqual(try ExpansionOutput.parse(json, alreadyClarified: false), .expanded("Make the buttons larger. Keep their arrangement unchanged."))
    }
    func testClarificationIsLimitedToOneRound() throws {
        let json = #"{"kind":"clarification","prompt":"","question":"Which issue?","choices":["Slow suggestions","Small buttons"]}"#
        XCTAssertEqual(try ExpansionOutput.parse(json, alreadyClarified: false), .needsClarification(.init(question: "Which issue?", choices: ["Slow suggestions", "Small buttons"])))
        XCTAssertThrowsError(try ExpansionOutput.parse(json, alreadyClarified: true))
    }
    func testMalformedMixedEmptyAndOversizedOutputsAreRejected() {
        for json in [
            #"{"kind":"expanded","prompt":"","question":"","choices":[]}"#,
            #"{"kind":"expanded","prompt":"Change it","question":"Which?","choices":["A","B"]}"#,
            #"{"kind":"clarification","prompt":"","question":"Which?","choices":["same","SAME"]}"#,
            #"{"kind":"clarification","prompt":"","question":"Which?","choices":["only one"]}"#,
            "not JSON"
        ] { XCTAssertThrowsError(try ExpansionOutput.parse(json, alreadyClarified: false)) }
        let long = String(repeating: "word ", count: 181)
        let json = "{\"kind\":\"expanded\",\"prompt\":\"\(long)\",\"question\":\"\",\"choices\":[]}"
        XCTAssertThrowsError(try ExpansionOutput.parse(json, alreadyClarified: false))
    }
}

final class ExpansionModelTests: XCTestCase {
    private let choices = ExpansionClarification(question: "Which issue should change?", choices: ["Slow suggestions", "Small buttons"])
    @MainActor
    private func fixture() async -> (CompanionModel, FakeEngine) {
        let engine = FakeEngine()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("PromptExpansionTests-" + UUID().uuidString)
        let model = CompanionModel(support: directory, engine: engine, copyToClipboard: { _ in true })
        model.automatic = false
        await model.select(CodexTask(["id": "A", "name": "Task A"])!)
        model.edit(text: "bigger buttons same layout", selection: NSRange(location: 7, length: 7))
        return (model, engine)
    }
    @MainActor
    private func waitFor(_ condition: @escaping () -> Bool) async {
        for _ in 0..<100 {
            if condition() { return }
            try? await Task.sleep(nanoseconds: 5_000_000)
        }
        XCTFail("Condition did not become true")
    }
    @MainActor
    private func start(_ model: CompanionModel, _ engine: FakeEngine) async {
        let count = engine.expansionRequests.count
        model.expandDraft()
        await waitFor { engine.expansionRequests.count == count + 1 }
    }

    @MainActor
    func testExpansionIsExplicitReplacesEntireDraftAndSupportsUndoPersistenceAndFocus() async {
        let (model, engine) = await fixture()
        XCTAssertTrue(engine.expansionRequests.isEmpty)
        let original = model.draft
        let focus = model.focusRequest
        await start(model, engine)
        XCTAssertEqual(engine.expansionRequests[0].draft, original.text)
        XCTAssertEqual(model.draft, original)
        engine.deliverExpansion(.success(.expanded("Make the phrase buttons larger. Keep their current arrangement.")))
        await waitFor { !model.expansionActive }
        XCTAssertEqual(model.draft.text, "Make the phrase buttons larger. Keep their current arrangement.")
        XCTAssertGreaterThan(model.focusRequest, focus)
        XCTAssertEqual(model.draft.cursor, (model.draft.text as NSString).length)
        let restored = CompanionModel(support: model.support, engine: engine)
        restored.automatic = false
        await restored.select(CodexTask(["id": "A", "name": "Task A"])!)
        XCTAssertEqual(restored.draft, model.draft)
        model.undo()
        XCTAssertEqual(model.draft, original)
        model.shutdown(); restored.shutdown()
    }

    @MainActor
    func testClarificationPausesPredictionsAndKeepsOriginalUntilAChoiceIsExpanded() async {
        let (model, engine) = await fixture()
        let original = model.draft
        await start(model, engine)
        engine.deliverExpansion(.success(.needsClarification(choices)))
        await waitFor { model.clarification != nil }
        model.refreshPredictions()
        XCTAssertEqual(engine.requested, 0)
        XCTAssertFalse(model.canInsert)
        XCTAssertEqual(model.draft, original)
        model.chooseInterpretation(1)
        await waitFor { engine.expansionRequests.count == 2 }
        XCTAssertEqual(engine.expansionRequests[1].resolution, ExpansionResolution(question: choices.question, choice: "Small buttons"))
        XCTAssertEqual(engine.expansionRequests[1].draft, original.text)
        model.chooseInterpretation(0) // A second click while generating cannot start another request.
        engine.deliverExpansion(.success(.expanded("Make the phrase buttons larger without rearranging them.")))
        await waitFor { !model.expansionActive }
        XCTAssertEqual(engine.expansionRequests.count, 2)
        XCTAssertEqual(model.draft.text, "Make the phrase buttons larger without rearranging them.")
        model.shutdown()
    }

    @MainActor
    func testChoicesDoNotAppearUnderThePointer() async {
        let (model, engine) = await fixture()
        await start(model, engine)
        model.setHovering(true)
        engine.deliverExpansion(.success(.needsClarification(choices)))
        await waitFor { !model.isExpanding }
        XCTAssertNil(model.clarification)
        XCTAssertTrue(model.expansionActive)
        model.setHovering(false)
        XCTAssertEqual(model.clarification, choices)
        model.keepOriginal()
        XCTAssertFalse(model.expansionActive)
        XCTAssertEqual(model.draft.text, "bigger buttons same layout")
        model.shutdown()
    }

    @MainActor
    func testKeepOriginalRejectsLateResultAndPhrasePredictionsCanResume() async {
        let (model, engine) = await fixture()
        let original = model.draft
        await start(model, engine)
        model.keepOriginal()
        engine.deliverExpansion(.success(.expanded("Late expansion")))
        await Task.yield()
        XCTAssertEqual(model.draft, original)
        model.refreshPredictions()
        await waitFor { engine.requested == 1 }
        engine.deliver(["layout unchanged", "layout intact", "layout as it is"])
        await waitFor { model.canInsert }
        model.shutdown()
    }

    @MainActor
    func testTypingTaskSwitchClearAndCopyEachRejectLateExpansion() async {
        for action in ["edit", "switch", "clear", "copy"] {
            let (model, engine) = await fixture()
            await start(model, engine)
            switch action {
            case "edit": model.edit(text: "why slow", selection: NSRange(location: 8, length: 0))
            case "switch": await model.select(CodexTask(["id": "B", "name": "Task B"])!)
            case "clear": model.clearDraft()
            default: model.copyPrompt()
            }
            let expected = model.draft
            engine.deliverExpansion(.success(.expanded("Obsolete expansion")))
            await Task.yield()
            XCTAssertEqual(model.draft, expected, action)
            XCTAssertFalse(model.expansionActive, action)
            model.shutdown()
        }
    }

    @MainActor
    func testConversationChangeRejectsExpansionAndClearsChoices() async {
        let (model, engine) = await fixture()
        let original = model.draft
        await start(model, engine)
        engine.contextSuffix = "New correction"
        await model.refreshContext()
        engine.deliverExpansion(.success(.expanded("Obsolete expansion")))
        await Task.yield()
        XCTAssertEqual(model.draft, original)
        XCTAssertFalse(model.expansionActive)
        XCTAssertNil(model.clarification)
        XCTAssertTrue(model.status.contains("Conversation changed"))
        model.shutdown()
    }

    @MainActor
    func testFailureKeepsOriginalAndAllowsRetry() async {
        let (model, engine) = await fixture()
        let original = model.draft
        await start(model, engine)
        engine.deliverExpansion(.failure(CompanionError("Connection lost")))
        await waitFor { !model.expansionActive }
        XCTAssertEqual(model.draft, original)
        XCTAssertNotNil(model.problem)
        XCTAssertTrue(model.canExpand)
        await start(model, engine)
        engine.deliverExpansion(.success(.expanded("Larger buttons, same arrangement.")))
        await waitFor { !model.expansionActive }
        XCTAssertEqual(model.draft.text, "Larger buttons, same arrangement.")
        model.shutdown()
    }

    @MainActor
    func testSecondClarificationIsRejectedWithoutChangingDraft() async {
        let (model, engine) = await fixture()
        let original = model.draft
        await start(model, engine)
        engine.deliverExpansion(.success(.needsClarification(choices)))
        await waitFor { model.clarification != nil }
        model.chooseInterpretation(0)
        await waitFor { engine.expansionRequests.count == 2 }
        engine.deliverExpansion(.success(.needsClarification(choices)))
        await waitFor { !model.expansionActive }
        XCTAssertEqual(model.draft, original)
        XCTAssertNotNil(model.problem)
        model.shutdown()
    }

    @MainActor
    func testExpandedPromptStillCopiesAndClearsWithUndo() async {
        let (model, engine) = await fixture()
        await start(model, engine)
        engine.deliverExpansion(.success(.expanded("Make the buttons larger, keeping their arrangement.")))
        await waitFor { !model.expansionActive }
        let expanded = model.draft
        model.copyPrompt()
        XCTAssertEqual(model.draft, Draft())
        XCTAssertTrue(model.copied)
        model.undo()
        XCTAssertEqual(model.draft, expanded)
        model.undo()
        XCTAssertEqual(model.draft.text, "bigger buttons same layout")
        model.shutdown()
    }

    @MainActor
    func testOldPhraseResultCannotInterruptExpansion() async {
        let (model, engine) = await fixture()
        model.refreshPredictions()
        await waitFor { engine.requested == 1 }
        await start(model, engine)
        engine.deliver(["old phrase one", "old phrase two", "old phrase three"])
        await Task.yield()
        XCTAssertTrue(model.expansionActive)
        XCTAssertFalse(model.canInsert)
        engine.deliverExpansion(.success(.expanded("Make the buttons larger.")))
        await waitFor { !model.expansionActive }
        XCTAssertEqual(model.draft.text, "Make the buttons larger.")
        model.shutdown()
    }
}
