import SwiftUI
import AppKit
import CompanionCore

private let ink = Color(red: 0.12, green: 0.20, blue: 0.20)
private let teal = Color(red: 0.08, green: 0.37, blue: 0.34)
private let paper = Color(red: 0.97, green: 0.97, blue: 0.94)

@main
struct PromptCompanionApp: App {
    @StateObject private var model = CompanionModel()
    var body: some Scene {
        Window("Prompt Companion", id: "companion") {
            CompanionView(model: model)
                .task { await model.connect() }
                .onReceive(NotificationCenter.default.publisher(for: NSApplication.willTerminateNotification)) { _ in model.shutdown() }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 620, height: 850)
        .windowResizability(.contentMinSize)
        .commands {
            CommandGroup(replacing: .undoRedo) {
                Button("Undo") { model.undo() }.keyboardShortcut("z").disabled(!model.undoAvailable)
            }
        }
    }
}

struct CompanionView: View {
    @ObservedObject var model: CompanionModel
    @State private var showSettings = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 10) {
                Image(systemName: "text.bubble.fill").font(.system(size: 24)).foregroundStyle(teal)
                VStack(alignment: .leading, spacing: 1) {
                    Text("Prompt Companion").font(.system(size: 22, weight: .semibold))
                    Text("Your words, a little sooner.").font(.system(size: 13)).foregroundStyle(.secondary)
                }
                Spacer()
                Button { showSettings = true } label: {
                    Image(systemName: "slider.horizontal.3").font(.system(size: 21)).frame(width: 46, height: 46)
                }.buttonStyle(.plain).background(.white.opacity(0.7), in: RoundedRectangle(cornerRadius: 12))
                    .accessibilityLabel("Settings")
                    .popover(isPresented: $showSettings) { SettingsView(model: model) }
            }
            Button { model.showTasks = true; Task { await model.loadTasks() } } label: {
                HStack(spacing: 10) {
                    Image(systemName: "bubble.left.and.bubble.right").font(.system(size: 19)).foregroundStyle(teal)
                    VStack(alignment: .leading, spacing: 4) {
                        Text("CONTEXT FROM").font(.system(size: 10, weight: .bold)).tracking(1.2).foregroundStyle(.secondary)
                        Text(model.selected?.title ?? "Choose a Codex task").font(.system(size: 16, weight: .medium)).lineLimit(1)
                    }
                    Spacer(minLength: 4)
                    Image(systemName: "chevron.down").foregroundStyle(.secondary)
                }.padding(.horizontal, 15).frame(maxWidth: .infinity, minHeight: 64)
                    .background(.white.opacity(0.85), in: RoundedRectangle(cornerRadius: 12))
                    .overlay(RoundedRectangle(cornerRadius: 12).stroke(teal.opacity(0.16)))
            }.buttonStyle(.plain).accessibilityLabel("Choose task. " + (model.selected?.title ?? "No task selected"))

            HStack {
                Text("YOUR PROMPT").font(.system(size: 11, weight: .bold)).tracking(1.2)
                Spacer()
                Text(model.selected == nil ? "" : "Saved on this Mac").font(.system(size: 12)).foregroundStyle(.secondary)
            }
            ZStack(alignment: .topLeading) {
                PromptEditor(model: model)
                if model.draft.text.isEmpty {
                    Text("Start typing, or choose a phrase below…")
                        .font(.system(size: model.fontSize)).foregroundStyle(.secondary.opacity(0.65))
                        .padding(.horizontal, 14).padding(.top, 13).allowsHitTesting(false)
                }
            }
            .frame(minHeight: 110, idealHeight: 145, maxHeight: .infinity)
            .background(.white, in: RoundedRectangle(cornerRadius: 12))
            .clipShape(RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(teal.opacity(0.28), lineWidth: 1.5))

            HStack {
                Text("CONTINUE WITH").font(.system(size: 11, weight: .bold)).tracking(1.2)
                Spacer()
                if model.isPredicting { ProgressView().controlSize(.small).accessibilityLabel("Preparing phrases") }
                Button { model.refreshPredictions() } label: {
                    Label("Refresh phrases", systemImage: "arrow.clockwise").font(.system(size: 14, weight: .medium))
                        .padding(.horizontal, 10).frame(height: 36)
                }.buttonStyle(.plain).foregroundStyle(teal).disabled(model.selected == nil || model.isConnecting)
            }
            VStack(spacing: 9) {
                ForEach(0..<3) { index in
                    Button { model.insert(index) } label: {
                        HStack(spacing: 14) {
                            Text(String(index + 1)).font(.system(size: 13, weight: .semibold, design: .monospaced))
                                .foregroundStyle(teal.opacity(0.75)).frame(width: 26, height: 26)
                                .background(teal.opacity(0.075), in: Circle())
                            Text(phrase(at: index)).font(.system(size: model.fontSize, weight: .medium))
                                .lineLimit(3).multilineTextAlignment(.leading)
                                .frame(maxWidth: .infinity, alignment: .leading)
                            Image(systemName: "plus").font(.system(size: 19, weight: .medium)).foregroundStyle(teal)
                        }.padding(.horizontal, 16)
                            .frame(maxWidth: .infinity, minHeight: model.effectiveButtonHeight, maxHeight: model.effectiveButtonHeight)
                            .background(model.canInsert ? Color.white : Color.white.opacity(0.45), in: RoundedRectangle(cornerRadius: 13))
                            .overlay(RoundedRectangle(cornerRadius: 13).stroke(teal.opacity(model.canInsert ? 0.3 : 0.09)))
                            .contentShape(RoundedRectangle(cornerRadius: 13))
                    }
                    .buttonStyle(PhraseButtonStyle())
                    .disabled(!model.canInsert || index >= model.phrases.count)
                    .accessibilityLabel(index < model.phrases.count ? "Insert: \(model.phrases[index])" : "Suggestion \(index + 1), waiting")
                }
            }
            .onHover { model.setHovering($0) }
            .background(GeometryReader { geometry in
                Color.clear.onAppear { model.setPhraseWidth(geometry.size.width - 105) }
                    .onChange(of: geometry.size.width) { _, width in model.setPhraseWidth(width - 105) }
            })

            HStack(spacing: 8) {
                Text(model.problem ?? model.status).font(.system(size: 13))
                    .foregroundStyle(model.problem == nil ? Color.secondary : Color(red: 0.52, green: 0.21, blue: 0.08))
                    .lineLimit(3).frame(maxWidth: .infinity, alignment: .leading)
                    .accessibilityLabel("Prediction status: " + (model.problem ?? model.status))
                if model.problem != nil {
                    Button("Reconnect") { Task { await model.connect() } }.controlSize(.large).disabled(model.isConnecting)
                }
            }.frame(height: 54)

            HStack(spacing: 10) {
                Button { model.undo() } label: {
                    Label("Undo", systemImage: "arrow.uturn.backward").font(.system(size: 17, weight: .medium))
                        .frame(width: 108, height: 52)
                }.buttonStyle(.plain).background(.white, in: RoundedRectangle(cornerRadius: 12)).disabled(!model.undoAvailable)
                Button { model.clearDraft() } label: {
                    Text("Clear").font(.system(size: 17, weight: .medium)).frame(width: 80, height: 52)
                }.buttonStyle(.plain).background(.white, in: RoundedRectangle(cornerRadius: 12)).disabled(model.draft.text.isEmpty)
                Button { model.copyPrompt() } label: {
                    Label(model.copied ? "Copied" : "Copy Prompt", systemImage: model.copied ? "checkmark" : "doc.on.doc")
                        .font(.system(size: 18, weight: .semibold)).frame(maxWidth: .infinity, minHeight: 52)
                        .foregroundStyle(.white).background(teal, in: RoundedRectangle(cornerRadius: 12))
                }.buttonStyle(.plain).disabled(model.draft.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("\(model.contextStatus)").font(.system(size: 11)).foregroundStyle(.secondary)
                    Text("Copy here, then paste into Codex. Nothing sends automatically.")
                        .font(.system(size: 11)).foregroundStyle(.secondary)
                }
                Spacer()
                Circle().fill(model.connected ? teal : Color.gray).frame(width: 7, height: 7).padding(.top, 4)
            }
        }
        .foregroundStyle(ink).padding(20).padding(.top, 14)
        .frame(minWidth: 520, minHeight: 760)
        .background(paper)
        .background(WindowConfiguration(floating: model.floating))
        .sheet(isPresented: $model.showTasks) { TaskPickerView(model: model) }
    }

    private func phrase(at index: Int) -> String {
        if index < model.phrases.count { return model.phrases[index] }
        return model.selected == nil ? ["Choose a task to begin", "Suggestions will use its conversation", "Click a phrase to add it"][index] : "Waiting for a useful phrase…"
    }
}

struct PhraseButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.overlay(RoundedRectangle(cornerRadius: 13).fill(teal.opacity(configuration.isPressed ? 0.1 : 0)))
    }
}

struct SettingsView: View {
    @ObservedObject var model: CompanionModel
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Make it comfortable").font(.title2.weight(.semibold))
            VStack(alignment: .leading) {
                Text("Text size · \(Int(model.fontSize)) pt")
                Slider(value: $model.fontSize, in: 18...32, step: 1).accessibilityLabel("Text size")
            }
            VStack(alignment: .leading) {
                Text("Phrase button height · \(Int(model.buttonHeight)) pt")
                Slider(value: $model.buttonHeight, in: 72...120, step: 2).accessibilityLabel("Phrase button height")
            }
            Toggle("Suggest automatically as I type", isOn: $model.automatic)
            Toggle("Keep this window above other apps", isOn: $model.floating)
            Divider()
            Text("This session").font(.headline)
            Text("\(model.typedCharacters) characters typed · \(model.insertedCharacters) inserted\n\(model.acceptedPhrases) phrases selected")
                .font(.system(size: 14)).lineSpacing(5)
            if let latency = model.lastLatency { Text(String(format: "Last prediction: %.1f seconds", latency)).font(.system(size: 13)).foregroundStyle(.secondary) }
            Divider()
            Text("Uses your Codex ChatGPT sign-in and usage allowance. The selected conversation and draft go to OpenAI for predictions. Drafts stay saved on this Mac.")
                .font(.system(size: 13)).foregroundStyle(.secondary)
            Text("Model: \(model.modelName)").font(.system(size: 11)).foregroundStyle(.secondary)
        }.padding(24).frame(width: 370)
            .onChange(of: model.fontSize) { _, _ in model.settingsChanged() }
            .onChange(of: model.buttonHeight) { _, _ in model.settingsChanged() }
            .onChange(of: model.automatic) { _, _ in model.settingsChanged() }
            .onChange(of: model.floating) { _, _ in model.settingsChanged() }
    }
}

struct TaskPickerView: View {
    @ObservedObject var model: CompanionModel
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Choose your context").font(.system(size: 25, weight: .semibold))
                    Text("Pick the task you’re writing a prompt for.").foregroundStyle(.secondary)
                }
                Spacer()
                Button("Done") { model.showTasks = false }.controlSize(.large)
            }
            HStack {
                TextField("Search task titles", text: $model.taskSearch).textFieldStyle(.roundedBorder).controlSize(.large)
                    .onChange(of: model.taskSearch) { _, _ in model.searchChanged() }
                Button { Task { await model.loadTasks() } } label: {
                    Image(systemName: "arrow.clockwise").frame(width: 38, height: 38)
                }.accessibilityLabel("Refresh task list")
            }
            if model.isLoadingTasks { ProgressView("Loading tasks…").controlSize(.small) }
            ScrollView {
                LazyVStack(spacing: 8) {
                    ForEach(model.tasks) { task in
                        Button { Task { await model.select(task) } } label: {
                            HStack(spacing: 12) {
                                Image(systemName: model.selected?.id == task.id ? "checkmark.circle.fill" : "bubble.left")
                                    .foregroundStyle(teal).font(.system(size: 20))
                                Text(task.title).font(.system(size: 18, weight: .medium)).lineLimit(2).multilineTextAlignment(.leading)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                Image(systemName: "chevron.right").foregroundStyle(.secondary)
                            }.padding(14).frame(maxWidth: .infinity, minHeight: 74)
                                .background(.white, in: RoundedRectangle(cornerRadius: 10))
                                .contentShape(Rectangle())
                        }.buttonStyle(.plain).accessibilityLabel(task.title)
                    }
                    if model.canLoadMore {
                        Button("Load more tasks") { Task { await model.loadTasks(more: true) } }
                            .controlSize(.large).padding(10).disabled(model.isLoadingTasks)
                    }
                    if model.tasks.isEmpty && !model.isLoadingTasks {
                        Text("No matching local tasks. Clear the search or reconnect to Codex.")
                            .foregroundStyle(.secondary).padding(20)
                    }
                }
            }
            Text("The selected task stays selected until you change it here.").font(.system(size: 12)).foregroundStyle(.secondary)
            if let problem = model.problem { Text(problem).font(.system(size: 12)).foregroundStyle(.red).lineLimit(3) }
        }.padding(24).frame(width: 540, height: 580).background(paper).foregroundStyle(ink)
    }
}

struct WindowConfiguration: NSViewRepresentable {
    let floating: Bool
    func makeNSView(context: Context) -> NSView { NSView() }
    func updateNSView(_ view: NSView, context: Context) {
        DispatchQueue.main.async {
            view.window?.identifier = NSUserInterfaceItemIdentifier("companion")
            view.window?.level = floating ? .floating : .normal
            view.window?.isMovableByWindowBackground = true
        }
    }
}

struct PromptEditor: NSViewRepresentable {
    @ObservedObject var model: CompanionModel
    func makeCoordinator() -> Coordinator { Coordinator(model) }
    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true; scroll.drawsBackground = false
        let editor = NSTextView(frame: .zero)
        editor.isRichText = false; editor.isEditable = true; editor.isSelectable = true
        editor.isAutomaticQuoteSubstitutionEnabled = false; editor.isAutomaticDashSubstitutionEnabled = false
        editor.isAutomaticTextReplacementEnabled = false; editor.isAutomaticSpellingCorrectionEnabled = false
        editor.isContinuousSpellCheckingEnabled = false; editor.isGrammarCheckingEnabled = false
        editor.allowsUndo = false; editor.drawsBackground = false
        editor.textContainerInset = NSSize(width: 10, height: 12)
        editor.isVerticallyResizable = true; editor.isHorizontallyResizable = false
        editor.autoresizingMask = [.width]
        editor.textContainer?.widthTracksTextView = true
        editor.textContainer?.containerSize = NSSize(width: 0, height: CGFloat.greatestFiniteMagnitude)
        editor.delegate = context.coordinator
        editor.setAccessibilityLabel("Prompt draft")
        scroll.documentView = editor
        return scroll
    }
    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let editor = scroll.documentView as? NSTextView else { return }
        context.coordinator.updating = true
        editor.font = .systemFont(ofSize: model.fontSize)
        editor.textColor = NSColor(red: 0.12, green: 0.20, blue: 0.20, alpha: 1)
        editor.insertionPointColor = NSColor(red: 0.08, green: 0.37, blue: 0.34, alpha: 1)
        if editor.string != model.draft.text { editor.string = model.draft.text }
        if editor.selectedRange() != model.draft.selection { editor.setSelectedRange(model.draft.selection) }
        context.coordinator.updating = false
        if context.coordinator.lastFocus != model.focusRequest {
            context.coordinator.lastFocus = model.focusRequest
            DispatchQueue.main.async {
                editor.window?.makeFirstResponder(editor)
                editor.scrollRangeToVisible(editor.selectedRange())
            }
        }
    }
    @MainActor final class Coordinator: NSObject, NSTextViewDelegate {
        let model: CompanionModel
        var updating = false
        var lastFocus = -1
        init(_ model: CompanionModel) { self.model = model }
        func textDidChange(_ notification: Notification) { changed(notification) }
        func textViewDidChangeSelection(_ notification: Notification) { changed(notification) }
        private func changed(_ notification: Notification) {
            guard !updating, let editor = notification.object as? NSTextView else { return }
            // A synchronous callback keeps the text and selection snapshot coherent.
            model.edit(text: editor.string, selection: editor.selectedRange())
        }
    }
}
