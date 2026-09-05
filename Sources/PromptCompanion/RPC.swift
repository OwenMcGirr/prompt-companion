import Foundation

struct CompanionError: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

@MainActor
final class CodexRPC {
    private var process: Process?
    private var input: FileHandle?
    private var output: FileHandle?
    private var errors: FileHandle?
    private var buffer = Data()
    private var serial = 0
    private var pending: [Int: CheckedContinuation<[String: Any], Error>] = [:]
    var notification: ((String, [String: Any]) -> Void)?
    private(set) var codexHome = ""
    var isRunning: Bool { process?.isRunning == true }

    static func executable() throws -> URL {
        let candidates = [
            "/Applications/ChatGPT.app/Contents/Resources/codex",
            "/Applications/Codex.app/Contents/Resources/codex",
            NSHomeDirectory() + "/Applications/Codex.app/Contents/Resources/codex",
            "/opt/homebrew/bin/codex", "/usr/local/bin/codex"
        ] + (ProcessInfo.processInfo.environment["PATH"] ?? "").split(separator: ":").map { String($0) + "/codex" }
        guard let path = candidates.first(where: { FileManager.default.isExecutableFile(atPath: $0) }) else {
            throw CompanionError("Codex could not be found. Install or update the Codex desktop app, then reopen Prompt Companion.")
        }
        return URL(fileURLWithPath: path)
    }

    func start(arguments: [String] = [], workingDirectory: URL) async throws {
        if isRunning { return }
        let p = Process(), stdin = Pipe(), stdout = Pipe(), stderr = Pipe()
        p.executableURL = try Self.executable()
        p.arguments = ["app-server", "--listen", "stdio://"] + arguments
        p.currentDirectoryURL = workingDirectory
        p.standardInput = stdin; p.standardOutput = stdout; p.standardError = stderr
        p.environment = ProcessInfo.processInfo.environment
        input = stdin.fileHandleForWriting; output = stdout.fileHandleForReading; errors = stderr.fileHandleForReading
        output?.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            Task { @MainActor in self?.receive(data) }
        }
        // Drain stderr without storing conversation fragments or credentials.
        errors?.readabilityHandler = { handle in _ = handle.availableData }
        p.terminationHandler = { [weak self, weak p] _ in
            Task { @MainActor in
                guard let self, self.process === p else { return }
                self.failAll(CompanionError("The Codex connection closed. Click Reconnect; your draft is safe."))
            }
        }
        process = p
        do {
            try p.run()
            let info = try await call("initialize", [
                "clientInfo": ["name": "prompt_companion", "title": "Prompt Companion", "version": "1.0.0"],
                "capabilities": ["experimentalApi": true]
            ])
            codexHome = info["codexHome"] as? String ?? NSHomeDirectory() + "/.codex"
            try send(["method": "initialized", "params": [:]])
        } catch { stop(); throw error }
    }

    func call(_ method: String, _ params: [String: Any] = [:], timeout: Double = 25) async throws -> [String: Any] {
        guard isRunning else { throw CompanionError("Codex is disconnected. Click Reconnect.") }
        serial += 1
        let id = serial
        return try await withTaskCancellationHandler(operation: {
            try await withCheckedThrowingContinuation { continuation in
                pending[id] = continuation
                do { try send(["id": id, "method": method, "params": params]) }
                catch { pending.removeValue(forKey: id)?.resume(throwing: error) }
                Task { @MainActor [weak self] in
                    try? await Task.sleep(nanoseconds: UInt64(timeout * 1_000_000_000))
                    self?.pending.removeValue(forKey: id)?.resume(throwing: CompanionError("Codex took too long to respond. Your draft has been kept; try again."))
                }
            }
        }, onCancel: { [weak self] in
            Task { @MainActor in self?.pending.removeValue(forKey: id)?.resume(throwing: CancellationError()) }
        })
    }

    private func send(_ object: [String: Any]) throws {
        var data = try JSONSerialization.data(withJSONObject: object)
        data.append(10)
        try input?.write(contentsOf: data)
    }

    private func receive(_ data: Data) {
        guard !data.isEmpty else { return }
        buffer.append(data)
        while let newline = buffer.firstIndex(of: 10) {
            let line = buffer.prefix(upTo: newline)
            buffer.removeSubrange(...newline)
            guard let object = try? JSONSerialization.jsonObject(with: line) as? [String: Any] else { continue }
            if let method = object["method"] as? String {
                if let requestID = object["id"] {
                    // This client never executes tools or answers model questions.
                    try? send(["id": requestID, "error": ["code": -32601, "message": "Prompt prediction does not permit tool calls or interactive requests."]])
                } else { notification?(method, object["params"] as? [String: Any] ?? [:]) }
            } else if let id = object["id"] as? Int, let continuation = pending.removeValue(forKey: id) {
                if let error = object["error"] as? [String: Any] {
                    continuation.resume(throwing: CompanionError(error["message"] as? String ?? "Codex returned an error."))
                } else { continuation.resume(returning: object["result"] as? [String: Any] ?? [:]) }
            }
        }
    }

    private func failAll(_ error: Error) {
        let continuations = pending.values
        pending.removeAll()
        for continuation in continuations { continuation.resume(throwing: error) }
        notification?("connection/closed", [:])
    }

    func stop() {
        output?.readabilityHandler = nil; errors?.readabilityHandler = nil
        let p = process
        process = nil
        try? input?.close()
        if p?.isRunning == true { p?.terminate() }
        input = nil; output = nil; errors = nil; buffer.removeAll()
        failAll(CancellationError())
    }
}
