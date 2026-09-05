import AppKit
import SwiftUI
import CompanionCore

struct SavedState: Codable {
    var selectedTaskID: String?
    var drafts: [String: Draft] = [:]
    var fontSize: Double = 22
    var buttonHeight: Double = 86
    var automatic = true
    var floating = true
}

@MainActor
final class CompanionModel: ObservableObject {
    @Published var tasks: [CodexTask] = []
    @Published var selected: CodexTask?
    @Published var draft = Draft()
    @Published var phrases: [String] = []
    @Published var status = "Connecting to Codex…"
    @Published var problem: String?
    @Published var contextStatus = "Choose a task for relevant suggestions"
    @Published var isConnecting = false
    @Published var isPredicting = false
    @Published var isLoadingTasks = false
    @Published var showTasks = false
    @Published var taskSearch = ""
    @Published var copied = false
    @Published var focusRequest = 0
    @Published var fontSize: Double = 22
    @Published var buttonHeight: Double = 86
    @Published var automatic = true
    @Published var floating = true
    @Published var typedCharacters = 0
    @Published var insertedCharacters = 0
    @Published var acceptedPhrases = 0
    @Published var lastLatency: Double?
    @Published var undoAvailable = false
    @Published var canLoadMore = false
    @Published private(set) var revision = 0

    let support: URL
    let engine: any PredictionService
    private var saved = SavedState()
    private var undoByTask: [String: [Draft]] = [:]
    private var context: ContextSnapshot?
    private var contextDate = Date.distantPast
    private var earlierSummary = ""
    private var batch: SuggestionBatch?
    private var pendingBatch: SuggestionBatch?
    private var hovering = false
    private var debounce: Task<Void, Never>?
    private var prediction: Task<Void, Never>?
    private var poll: Task<Void, Never>?
    private var searchTask: Task<Void, Never>?
    private var taskCursor: String?
    private var taskListRevision = 0
    private var selectionRevision = 0
    private var connectingRevision = 0
    private var phraseWidth: CGFloat = 430

    init(support: URL? = nil, engine: (any PredictionService)? = nil) {
        self.support = support ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("PromptCompanion")
        self.engine = engine ?? CompanionEngine(support: self.support)
        if let data = try? Data(contentsOf: self.support.appendingPathComponent("drafts.json")),
           let restored = try? JSONDecoder().decode(SavedState.self, from: data) {
            saved = restored
            fontSize = min(32, max(18, restored.fontSize)); buttonHeight = min(120, max(72, restored.buttonHeight))
            automatic = restored.automatic; floating = restored.floating
            draft = restored.drafts["__unassigned__"] ?? Draft()
        }
    }

    var canInsert: Bool { batch?.valid(for: draft, revision: revision) == true && batch?.phrases.isEmpty == false && context != nil }
    var connected: Bool { engine.connected }
    var modelName: String { engine.model }
    var effectiveButtonHeight: Double { max(buttonHeight, ceil(fontSize * 3.6)) }

    func connect() async {
        guard !isConnecting else { return }
        connectingRevision += 1
        let connection = connectingRevision
        isConnecting = true; problem = nil; status = "Connecting to Codex…"
        invalidate()
        engine.stop()
        defer { isConnecting = false }
        do {
            try await engine.connect()
            guard connection == connectingRevision else { return }
            await loadTasks()
            if let id = selected?.id ?? saved.selectedTaskID,
               let task = tasks.first(where: { $0.id == id }) {
                await select(task)
            } else {
                selected = nil; context = nil; status = "Choose the task you’re working on"
                showTasks = true
            }
            startPolling()
        } catch { problem = error.localizedDescription; status = "Connection unavailable" }
    }

    func loadTasks(more: Bool = false) async {
        taskListRevision += 1
        let request = taskListRevision
        isLoadingTasks = true
        do {
            let result = try await engine.listTasks(cursor: more ? taskCursor : nil, search: taskSearch)
            guard request == taskListRevision else { return }
            if more {
                let existing = Set(tasks.map(\.id))
                tasks += result.0.filter { !existing.contains($0.id) }
            } else { tasks = result.0 }
            taskCursor = result.1; canLoadMore = result.1 != nil
        } catch { if request == taskListRevision { problem = error.localizedDescription } }
        if request == taskListRevision { isLoadingTasks = false }
    }

    func searchChanged() {
        searchTask?.cancel()
        searchTask = Task { @MainActor in
            try? await Task.sleep(nanoseconds: 450_000_000)
            guard !Task.isCancelled else { return }
            await loadTasks()
        }
    }

    func select(_ task: CodexTask) async {
        save()
        let unassigned = selected == nil ? draft : Draft()
        selectionRevision += 1
        let selection = selectionRevision
        invalidate()
        selected = task; context = nil; earlierSummary = ""; contextDate = .distantPast
        draft = saved.drafts[task.id] ?? unassigned
        if saved.drafts[task.id] == nil && !unassigned.text.isEmpty { saved.drafts["__unassigned__"] = Draft() }
        undoAvailable = !(undoByTask[task.id] ?? []).isEmpty
        showTasks = false; problem = nil; phrases = []; batch = nil; pendingBatch = nil
        contextStatus = "Reading this task…"; status = "Loading conversation…"
        saved.selectedTaskID = task.id
        focusRequest += 1
        save()
        do {
            let snapshot = try await engine.context(for: task.id)
            guard selection == selectionRevision else { return }
            context = snapshot; contextDate = Date()
            contextStatus = snapshot.isPartial ? "Recent conversation + opening messages" : "Conversation connected"
            status = "Ready when you are"
            schedulePrediction(immediate: true)
        } catch {
            guard selection == selectionRevision else { return }
            problem = error.localizedDescription
            contextStatus = "Conversation unavailable"; status = "Your draft is still editable"
        }
    }

    private func startPolling() {
        poll?.cancel()
        poll = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 8_000_000_000)
                guard !Task.isCancelled, let self else { return }
                await self.refreshContext()
            }
        }
    }

    func refreshContext() async {
        guard let selected, !isConnecting else { return }
        let selection = selectionRevision
        do {
            let snapshot = try await engine.context(for: selected.id)
            guard selection == selectionRevision else { return }
            contextDate = Date()
            contextStatus = snapshot.isPartial ? "Recent conversation + opening messages" : "Conversation connected"
            guard snapshot != context else { return }
            invalidate(); context = snapshot; earlierSummary = ""
            schedulePrediction()
        } catch {
            guard selection == selectionRevision else { return }
            contextStatus = "Conversation refresh paused"
            if Date().timeIntervalSince(contextDate) > 30 {
                invalidate(); context = nil
                problem = "Couldn’t refresh this conversation. Reconnect to resume suggestions; your draft is safe."
            }
        }
    }

    func edit(text: String, selection: NSRange) {
        let changed = Draft(text: text, cursor: selection.location, selectionLength: selection.length)
        guard changed != draft else { return }
        if text != draft.text {
            rememberUndo()
            typedCharacters += max(0, text.count - draft.text.count)
        }
        let oldBatch = batch
        let oldTarget = CompletionTarget(draft)
        invalidate()
        draft = changed; copied = false
        // Continuing an existing partial word can reuse contextual phrases instantly.
        let target = CompletionTarget(draft)
        if let oldBatch, target.before == oldTarget.before, target.after == oldTarget.after,
           !target.partial.isEmpty, target.partial.hasPrefix(oldTarget.partial) {
            let reusable = oldBatch.phrases.compactMap { target.normalized($0) }
            if !reusable.isEmpty { present(SuggestionBatch(revision: revision, target: target, phrases: reusable)) }
        }
        save()
        if batch?.valid(for: draft, revision: revision) == true && batch?.phrases.count == 3 { status = "Choose a phrase or keep typing" }
        else { schedulePrediction() }
    }

    func insert(_ index: Int) {
        guard let batch, batch.valid(for: draft, revision: revision), batch.phrases.indices.contains(index),
              let next = batch.target.inserting(batch.phrases[index]) else { return }
        rememberUndo()
        insertedCharacters += max(0, next.text.count - draft.text.count)
        acceptedPhrases += 1
        invalidate(); draft = next; copied = false; focusRequest += 1
        save(); schedulePrediction()
    }

    private func rememberUndo() {
        let id = selected?.id ?? "__unassigned__"
        var history = undoByTask[id] ?? []
        history.append(draft)
        undoByTask[id] = Array(history.suffix(100))
        undoAvailable = true
    }

    func undo() {
        let id = selected?.id ?? "__unassigned__"
        guard let previous = undoByTask[id]?.popLast() else { return }
        invalidate(); draft = previous; copied = false
        undoAvailable = !(undoByTask[id] ?? []).isEmpty
        focusRequest += 1; save(); schedulePrediction()
    }

    func clearDraft() {
        guard !draft.text.isEmpty else { return }
        rememberUndo(); invalidate(); draft = Draft(); copied = false
        focusRequest += 1; save(); schedulePrediction()
    }

    func copyPrompt() {
        guard !draft.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(draft.text.trimmingCharacters(in: .whitespacesAndNewlines), forType: .string)
        copied = true; status = "Copied — paste into your Codex prompt"
    }

    func setHovering(_ value: Bool) {
        hovering = value
        if !value, let pendingBatch {
            self.pendingBatch = nil
            if pendingBatch.valid(for: draft, revision: revision) { present(pendingBatch) }
        }
    }

    private func present(_ candidate: SuggestionBatch) {
        guard candidate.valid(for: draft, revision: revision) else { return }
        if hovering { pendingBatch = candidate; return }
        // Never make a clickable insertion whose full text is hidden by ellipsis.
        let visible = candidate.phrases.filter(phraseFits)
        batch = SuggestionBatch(revision: candidate.revision, target: candidate.target, phrases: visible)
        phrases = visible
    }

    private func phraseFits(_ text: String) -> Bool {
        let size = (text as NSString).boundingRect(with: NSSize(width: phraseWidth, height: .greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin, .usesFontLeading],
            attributes: [.font: NSFont.systemFont(ofSize: fontSize, weight: .medium)]).size
        return size.height <= min(effectiveButtonHeight - 10, fontSize * 3.5)
    }

    func setPhraseWidth(_ width: CGFloat) {
        guard abs(width - phraseWidth) > 1 else { return }
        phraseWidth = max(100, width)
        if let batch, batch.valid(for: draft, revision: revision) { present(batch) }
    }

    private func invalidate() {
        revision += 1
        debounce?.cancel(); debounce = nil
        prediction?.cancel(); prediction = nil
        engine.cancelPrediction()
        pendingBatch = nil; isPredicting = false
    }

    func refreshPredictions() {
        problem = nil
        invalidate()
        if context == nil, let selected { Task { await select(selected) } }
        else { schedulePrediction(immediate: true, force: true) }
    }

    func settingsChanged() {
        save()
        if let batch, batch.valid(for: draft, revision: revision) { present(batch) }
        for window in NSApplication.shared.windows where window.identifier?.rawValue == "companion" {
            window.level = floating ? .floating : .normal
        }
        if !automatic { invalidate(); status = "Automatic suggestions paused" }
        else if !canInsert { schedulePrediction() }
    }

    private func schedulePrediction(immediate: Bool = false, force: Bool = false) {
        guard engine.connected, context != nil, selected != nil else { return }
        guard automatic || force else { status = "Click Refresh phrases when you’re ready"; return }
        debounce?.cancel()
        let version = revision
        status = "Thinking of useful phrases…"
        debounce = Task { @MainActor [weak self] in
            if !immediate { try? await Task.sleep(nanoseconds: 850_000_000) }
            guard !Task.isCancelled, let self, version == self.revision else { return }
            self.prediction = Task { @MainActor [weak self] in await self?.runPrediction(version: version) }
        }
    }

    private func runPrediction(version: Int) async {
        guard let selected, let context, version == revision else { return }
        let target = CompletionTarget(draft)
        isPredicting = true
        do {
            let result = try await engine.predict(target: target, context: context, title: selected.title, earlierSummary: earlierSummary)
            guard !Task.isCancelled, version == revision else { return }
            earlierSummary = result.summary; lastLatency = result.duration
            present(SuggestionBatch(revision: version, target: target, phrases: result.phrases))
            status = hovering ? "New phrases ready — move off the buttons to show them" :
                (phrases.isEmpty ? "These phrases were too long to show fully. Refresh for shorter phrases." : "Choose a phrase or keep typing")
            problem = nil
        } catch is CancellationError { }
        catch {
            guard version == revision else { return }
            problem = error.localizedDescription; status = "Keep typing, or refresh phrases"
        }
        if version == revision { isPredicting = false }
    }

    func save() {
        saved.drafts[selected?.id ?? "__unassigned__"] = draft
        saved.fontSize = fontSize; saved.buttonHeight = buttonHeight
        saved.automatic = automatic; saved.floating = floating
        do {
            try FileManager.default.createDirectory(at: support, withIntermediateDirectories: true)
            try JSONEncoder().encode(saved).write(to: support.appendingPathComponent("drafts.json"), options: .atomic)
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: support.appendingPathComponent("drafts.json").path)
        } catch { problem = "This draft could not be saved. Keep this window open and use Copy Prompt before quitting." }
    }

    func shutdown() { save(); poll?.cancel(); searchTask?.cancel(); invalidate(); engine.stop() }
}
