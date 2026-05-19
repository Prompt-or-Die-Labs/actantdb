// ActantDBSupervisor spawns the `actantdb` Rust binary as a child process.
// That doesn't work in the iOS sandbox (no `posix_spawn` / `Process` to
// arbitrary binaries; no writable arbitrary filesystem paths). On iOS the
// path forward is direct FFI embedding via `actant-ffi` — see
// `docs/IOS_EMBEDDING.md`. Gating the whole supervisor here keeps the
// ActantAgent target buildable for `.iOS(.v26)` without runtime errors.
#if !os(iOS)

import Foundation
#if canImport(Darwin)
import Darwin
#elseif canImport(Glibc)
import Glibc
#endif
import ActantDB

/// Internal readiness probe seam — tests inject a stub; production calls
/// `ActantClient.healthzReady()`.
protocol ReadinessProbe: Sendable {
    func ready(baseURL: URL) async throws
}

struct DefaultReadinessProbe: ReadinessProbe {
    func ready(baseURL: URL) async throws {
        let client = ActantClient(baseURL: baseURL, timeout: 2.0)
        let h = try await client.healthzReady()
        if !h.isHealthy {
            throw ActantError.transport("healthz/ready not healthy")
        }
    }
}

/// Spawns and lifecycles a local `actantdb` Rust server subprocess.
///
/// Resolves the binary, starts it bound to (host, port), tails stderr/stdout
/// (mirroring to a log file or stderr), polls `/v1/healthz/ready` until
/// healthy, and exposes the resulting base URL. `stop()` SIGTERMs and then
/// SIGKILLs after a deadline.
public actor ActantDBSupervisor {
    public enum Error: Swift.Error, Sendable, LocalizedError, CustomStringConvertible {
        case binaryNotFound(searched: [String])
        case spawnFailed(String)
        case portParseFailed
        case notReady(after: TimeInterval, lastError: String?)
        case alreadyStarted
        case notRunning

        public var description: String {
            switch self {
            case .binaryNotFound(let searched):
                let paths = searched.joined(separator: ", ")
                return """
                actantdb binary not found. Searched: \(paths).
                Install with: cargo install --path crates/actant-cli
                Or set SWOOSH_ACTANTDB_PATH to the binary location.
                """
            case .spawnFailed(let msg):
                return "actantdb spawn failed: \(msg)"
            case .portParseFailed:
                return "actantdb supervisor failed to parse listening port from stderr"
            case .notReady(let after, let lastError):
                let tail = lastError.map { " (last error: \($0))" } ?? ""
                return "actantdb did not become ready within \(after)s\(tail)"
            case .alreadyStarted:
                return "actantdb supervisor already started"
            case .notRunning:
                return "actantdb supervisor is not running"
            }
        }

        public var errorDescription: String? { description }
    }

    // MARK: - Configuration

    private let binaryPath: URL?
    private let extraSearchPaths: [URL]
    private let logOutputTo: URL?
    private let probe: any ReadinessProbe
    private let sigkillAfter: TimeInterval

    // MARK: - Mutable state

    private var process: Process?
    private var stderrPipe: Pipe?
    private var stdoutPipe: Pipe?
    private var logFileHandle: FileHandle?
    private var parsedPort: UInt16?
    private var parsedHost: String?
    private var portContinuation: CheckedContinuation<UInt16, Swift.Error>?
    private var startedURL: URL?

    // MARK: - Init

    public init(
        binaryPath: URL? = nil,
        extraSearchPaths: [URL] = [],
        logOutputTo: URL? = nil
    ) {
        self.binaryPath = binaryPath
        self.extraSearchPaths = extraSearchPaths
        self.logOutputTo = logOutputTo
        self.probe = DefaultReadinessProbe()
        self.sigkillAfter = 10
    }

    /// Internal initializer used by tests to inject a readiness probe stub
    /// and a shorter SIGKILL fallback timeout.
    internal init(
        binaryPath: URL? = nil,
        extraSearchPaths: [URL] = [],
        logOutputTo: URL? = nil,
        probe: any ReadinessProbe,
        sigkillAfter: TimeInterval = 10
    ) {
        self.binaryPath = binaryPath
        self.extraSearchPaths = extraSearchPaths
        self.logOutputTo = logOutputTo
        self.probe = probe
        self.sigkillAfter = sigkillAfter
    }

    // MARK: - Public API

    /// Spawn the actantdb server bound to (host, port). When `port` is nil,
    /// pre-probe a free port via a transient socket bind, then parse the
    /// actual port from the server's stderr ("actantdb listening on
    /// http://HOST:PORT") to confirm. Polls /v1/healthz/ready until success
    /// or `readyTimeout`. Returns the base URL.
    public func start(
        dbPath: URL,
        host: String = "127.0.0.1",
        port: UInt16? = nil,
        readyTimeout: TimeInterval = 10
    ) async throws -> URL {
        if process != nil {
            throw Error.alreadyStarted
        }

        let resolvedBinary = try resolveBinary()
        let resolvedPort: UInt16
        if let port {
            resolvedPort = port
        } else {
            // Pre-probe a free port. The CLI echoes its bind string back
            // verbatim, so passing :0 yields ":0" in the log line — we have
            // to pick a concrete port ourselves.
            resolvedPort = try Self.findFreePort(host: host)
        }

        // Open the log file up front so the drain task can write to it
        // immediately.
        if let logURL = logOutputTo {
            try Self.ensureParentDirectoryExists(for: logURL)
            if !FileManager.default.fileExists(atPath: logURL.path) {
                FileManager.default.createFile(atPath: logURL.path, contents: nil)
            }
            logFileHandle = try FileHandle(forWritingTo: logURL)
            try logFileHandle?.seekToEnd()
        }

        let proc = Process()
        proc.executableURL = resolvedBinary
        proc.arguments = [
            "--db", dbPath.path,
            "serve",
            "--bind", "\(host):\(resolvedPort)",
        ]

        let stderr = Pipe()
        let stdout = Pipe()
        proc.standardError = stderr
        proc.standardOutput = stdout
        self.stderrPipe = stderr
        self.stdoutPipe = stdout

        // Install pipe readers BEFORE running. Each handler buffers bytes and
        // hops onto the actor via a Task to ingest.
        let stderrBuffer = LineBuffer()
        stderr.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty else {
                handle.readabilityHandler = nil
                return
            }
            let lines = stderrBuffer.append(data)
            guard !lines.isEmpty else { return }
            Task { [weak self] in
                await self?.ingestStderr(lines: lines)
            }
        }

        let stdoutBuffer = LineBuffer()
        stdout.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty else {
                handle.readabilityHandler = nil
                return
            }
            let lines = stdoutBuffer.append(data)
            guard !lines.isEmpty else { return }
            Task { [weak self] in
                await self?.ingestStdout(lines: lines)
            }
        }

        do {
            try proc.run()
        } catch {
            // Clean up open handles before reporting.
            stderr.fileHandleForReading.readabilityHandler = nil
            stdout.fileHandleForReading.readabilityHandler = nil
            self.stderrPipe = nil
            self.stdoutPipe = nil
            try? logFileHandle?.close()
            logFileHandle = nil
            throw Error.spawnFailed(error.localizedDescription)
        }

        self.process = proc

        // When `port` was nil, wait for stderr to confirm the chosen port
        // before we start polling readiness. Cap the wait at 5s (separate
        // from readyTimeout — the listening log appears very early).
        let confirmedPort: UInt16
        if port == nil {
            do {
                confirmedPort = try await waitForListeningPort(expected: resolvedPort, timeout: 5.0)
            } catch {
                await stop()
                throw error
            }
        } else {
            confirmedPort = resolvedPort
            parsedPort = resolvedPort
            parsedHost = host
        }

        let baseURL = URL(string: "http://\(host):\(confirmedPort)")!

        // Poll readiness.
        do {
            try await pollReady(baseURL: baseURL, timeout: readyTimeout)
        } catch {
            await stop()
            throw error
        }

        self.startedURL = baseURL
        return baseURL
    }

    /// SIGTERM the child and wait up to `sigkillAfter` seconds for it to
    /// exit; SIGKILL if it doesn't. Closes the stderr log if one was opened.
    ///
    /// Use raw `kill(pid, SIGTERM)` rather than `Process.terminate()` — the
    /// latter terminates the entire process group on Darwin, bypassing any
    /// SIGTERM handler the child has installed.
    public func stop() async {
        guard let proc = process else { return }

        if proc.isRunning {
            #if canImport(Darwin) || canImport(Glibc)
            kill(proc.processIdentifier, SIGTERM)
            #else
            proc.terminate()
            #endif
        }

        let deadline = Date().addingTimeInterval(sigkillAfter)
        while proc.isRunning && Date() < deadline {
            try? await Task.sleep(nanoseconds: 50 * 1_000_000)
        }

        if proc.isRunning {
            #if canImport(Darwin) || canImport(Glibc)
            kill(proc.processIdentifier, SIGKILL)
            #endif
            // Give it a beat to actually die.
            for _ in 0..<20 {
                if !proc.isRunning { break }
                try? await Task.sleep(nanoseconds: 50 * 1_000_000)
            }
        }

        stderrPipe?.fileHandleForReading.readabilityHandler = nil
        stdoutPipe?.fileHandleForReading.readabilityHandler = nil
        stderrPipe = nil
        stdoutPipe = nil

        try? logFileHandle?.close()
        logFileHandle = nil

        if let cont = portContinuation {
            portContinuation = nil
            cont.resume(throwing: Error.notRunning)
        }

        process = nil
        startedURL = nil
        parsedPort = nil
        parsedHost = nil
    }

    // MARK: - Actor-internal ingest

    private func ingestStderr(lines: [String]) {
        for line in lines {
            writeMirror(line: line, isStderr: true)
            parsePortIfNeeded(from: line)
        }
    }

    private func ingestStdout(lines: [String]) {
        for line in lines {
            writeMirror(line: line, isStderr: false)
        }
    }

    private func writeMirror(line: String, isStderr: Bool) {
        let withNewline = line + "\n"
        guard let data = withNewline.data(using: .utf8) else { return }
        if let handle = logFileHandle {
            try? handle.write(contentsOf: data)
        } else {
            let target = isStderr ? FileHandle.standardError : FileHandle.standardOutput
            try? target.write(contentsOf: data)
        }
    }

    /// Look for "actantdb listening on http(s)://HOST:PORT" and store host+port.
    private func parsePortIfNeeded(from line: String) {
        guard parsedPort == nil else { return }
        let marker = "actantdb listening on "
        guard let range = line.range(of: marker) else { return }
        var rest = String(line[range.upperBound...])

        // Strip scheme.
        if let schemeEnd = rest.range(of: "://") {
            rest = String(rest[schemeEnd.upperBound...])
        }
        // Trim trailing whitespace or path.
        if let slash = rest.firstIndex(of: "/") {
            rest = String(rest[..<slash])
        }
        rest = rest.trimmingCharacters(in: .whitespacesAndNewlines)

        // Split host:port from the right (IPv6 addresses use [::1]:port).
        guard let colon = rest.lastIndex(of: ":") else { return }
        let hostPart = String(rest[..<colon])
        let portStr = String(rest[rest.index(after: colon)...])
        guard let port = UInt16(portStr), port != 0 else { return }

        parsedHost = hostPart
        parsedPort = port

        if let cont = portContinuation {
            portContinuation = nil
            cont.resume(returning: port)
        }
    }

    private func waitForListeningPort(
        expected: UInt16,
        timeout: TimeInterval
    ) async throws -> UInt16 {
        if let p = parsedPort { return p }

        let deadline = Date().addingTimeInterval(timeout)

        // Race a continuation against a timeout task.
        let port: UInt16 = try await withThrowingTaskGroup(of: UInt16.self) { group in
            group.addTask { [weak self] in
                guard let self else { throw Error.portParseFailed }
                return try await withCheckedThrowingContinuation { cont in
                    Task { await self.setPortContinuation(cont) }
                }
            }
            group.addTask {
                let interval = max(0, deadline.timeIntervalSinceNow)
                if interval > 0 {
                    try? await Task.sleep(nanoseconds: UInt64(interval * 1_000_000_000))
                }
                throw Error.portParseFailed
            }
            let first = try await group.next()!
            group.cancelAll()
            return first
        }

        // Confirm port matches what we expected if we pre-probed.
        if port != expected {
            // The server bound to a different port than we asked for. This
            // shouldn't happen in normal operation, but trust the log line.
        }
        return port
    }

    private func setPortContinuation(_ cont: CheckedContinuation<UInt16, Swift.Error>) {
        if let p = parsedPort {
            cont.resume(returning: p)
            return
        }
        // If a previous continuation is set, discard it (shouldn't happen).
        if let prev = portContinuation {
            prev.resume(throwing: Error.portParseFailed)
        }
        portContinuation = cont
    }

    private func pollReady(baseURL: URL, timeout: TimeInterval) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        var delayMS: UInt64 = 100
        var lastError: String?
        while Date() < deadline {
            do {
                try await probe.ready(baseURL: baseURL)
                return
            } catch {
                lastError = error.localizedDescription
            }
            try? await Task.sleep(nanoseconds: delayMS * 1_000_000)
            delayMS = min(delayMS * 2, 1000)
        }
        throw Error.notReady(after: timeout, lastError: lastError)
    }

    // MARK: - Binary discovery

    private func resolveBinary() throws -> URL {
        var searched: [String] = []

        // 1. Explicit binaryPath arg.
        if let url = binaryPath {
            searched.append(url.path)
            if isExecutable(url) { return url }
        }

        // 2. SWOOSH_ACTANTDB_PATH env var.
        if let envPath = ProcessInfo.processInfo.environment["SWOOSH_ACTANTDB_PATH"],
           !envPath.isEmpty {
            let url = URL(fileURLWithPath: envPath)
            searched.append(url.path)
            if isExecutable(url) { return url }
        }

        // 3. PATH walk for "actantdb".
        if let pathEnv = ProcessInfo.processInfo.environment["PATH"] {
            for dir in pathEnv.split(separator: ":") {
                let candidate = URL(fileURLWithPath: String(dir))
                    .appendingPathComponent("actantdb")
                searched.append(candidate.path)
                if isExecutable(candidate) { return candidate }
            }
        }

        // 4. ~/.cargo/bin/actantdb.
        let cargo = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".cargo/bin/actantdb")
        searched.append(cargo.path)
        if isExecutable(cargo) { return cargo }

        // 5. extraSearchPaths + "actantdb".
        for base in extraSearchPaths {
            let candidate = base.appendingPathComponent("actantdb")
            searched.append(candidate.path)
            if isExecutable(candidate) { return candidate }
        }

        throw Error.binaryNotFound(searched: searched)
    }

    private func isExecutable(_ url: URL) -> Bool {
        let path = url.path
        return FileManager.default.isExecutableFile(atPath: path)
    }

    // MARK: - Port helpers

    /// Bind a transient socket to (host, 0) to discover a free ephemeral port,
    /// then close it. Small race window if another process grabs the port
    /// between close and the child's bind.
    nonisolated static func findFreePort(host: String) throws -> UInt16 {
        #if canImport(Darwin) || canImport(Glibc)
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        if fd < 0 {
            throw Error.spawnFailed("socket() failed: \(String(cString: strerror(errno)))")
        }
        defer { close(fd) }

        var addr = sockaddr_in()
        #if canImport(Darwin)
        addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        #endif
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = 0
        addr.sin_addr.s_addr = inet_addr(host)
        if addr.sin_addr.s_addr == INADDR_NONE {
            // Default to loopback for non-numeric or unrecognized host.
            addr.sin_addr.s_addr = inet_addr("127.0.0.1")
        }

        let bindResult = withUnsafePointer(to: &addr) { ptr -> Int32 in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockPtr in
                bind(fd, sockPtr, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        if bindResult != 0 {
            throw Error.spawnFailed("bind() failed: \(String(cString: strerror(errno)))")
        }

        var actual = sockaddr_in()
        var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        let nameResult = withUnsafeMutablePointer(to: &actual) { ptr -> Int32 in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockPtr in
                getsockname(fd, sockPtr, &len)
            }
        }
        if nameResult != 0 {
            throw Error.spawnFailed("getsockname() failed: \(String(cString: strerror(errno)))")
        }

        let port = UInt16(bigEndian: actual.sin_port)
        return port
        #else
        throw Error.spawnFailed("findFreePort: unsupported platform")
        #endif
    }

    nonisolated static func ensureParentDirectoryExists(for url: URL) throws {
        let parent = url.deletingLastPathComponent()
        try FileManager.default.createDirectory(
            at: parent,
            withIntermediateDirectories: true
        )
    }
}

// MARK: - LineBuffer

/// Splits a stream of bytes into UTF-8 lines. Thread-safe — the readability
/// handler may run on a background queue.
final class LineBuffer: @unchecked Sendable {
    private let lock = NSLock()
    private var buffer = Data()

    /// Append bytes; return completed lines (without the trailing newline).
    func append(_ data: Data) -> [String] {
        lock.lock(); defer { lock.unlock() }
        buffer.append(data)
        var lines: [String] = []
        while let newline = buffer.firstIndex(of: 0x0A) {
            let lineData = buffer.subdata(in: 0..<newline)
            buffer.removeSubrange(0...newline)
            if let line = String(data: lineData, encoding: .utf8) {
                // Strip trailing \r if any.
                if line.hasSuffix("\r") {
                    lines.append(String(line.dropLast()))
                } else {
                    lines.append(line)
                }
            }
        }
        return lines
    }
}

#endif // !os(iOS)
