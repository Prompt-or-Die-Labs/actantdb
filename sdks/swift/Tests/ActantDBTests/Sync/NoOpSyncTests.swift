import Foundation
import Testing
@testable import ActantDB

/// `NoOpSync` is the fallback `ActantSync` impl on platforms without
/// CloudKit (Linux / Windows / wasm). On Apple platforms the type isn't even
/// compiled; the test below is guarded the same way as the source.
#if !canImport(CloudKit) || os(Linux)

@Suite("NoOpSync (non-Apple platforms)")
struct NoOpSyncTests {

    @Test("enable() throws SyncError.unsupportedPlatform")
    func enableThrows() async {
        let sync = NoOpSync()
        do {
            try await sync.enable(container: "iCloud.test", options: SyncOptions())
            Issue.record("expected throw")
        } catch SyncError.unsupportedPlatform {
            // expected
        } catch {
            Issue.record("expected SyncError.unsupportedPlatform, got \(error)")
        }
    }

    @Test("disable() throws SyncError.unsupportedPlatform")
    func disableThrows() async {
        let sync = NoOpSync()
        do {
            try await sync.disable()
            Issue.record("expected throw")
        } catch SyncError.unsupportedPlatform {
            // expected
        } catch {
            Issue.record("expected SyncError.unsupportedPlatform, got \(error)")
        }
    }

    @Test("status() throws SyncError.unsupportedPlatform")
    func statusThrows() async {
        let sync = NoOpSync()
        do {
            _ = try await sync.status()
            Issue.record("expected throw")
        } catch SyncError.unsupportedPlatform {
            // expected
        } catch {
            Issue.record("expected SyncError.unsupportedPlatform, got \(error)")
        }
    }
}

#endif // !canImport(CloudKit) || os(Linux)
