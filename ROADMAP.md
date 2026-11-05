# Capability and reliability audit

Source audit of main at `aa35afdd0568418874cab93a77005d3872b1847c` (2026-11-05). These are engineering opportunities, not claims of live desktop verification. Preserve the authority, privacy, cancellation and approval guarantees in README.md when expanding capabilities.

## Reliability priorities

- **Supervise the bridge.** An initialization error can terminate the bridge thread while the daemon remains running. Surface bridge failure to the daemon and distinguish recoverable retries from terminal failures. Test failed startup, reconnection and shutdown.
- **Keep failed remote cleanup retryable.** Stale connection deletion retries are followed by clearing local IDs even if deletion failed. Retain failed IDs durably and account for the bounded connection history; verify recovery with a simulated API failure and 404 responses.
- **Bound approval work.** Expired approvals can leave prompt threads waiting behind a blocking terminal read. Use a bounded prompt worker that discards expired requests, and test cancellation and shutdown.
- **Clarify session isolation.** MCP session IDs are caller-supplied and not tracked as issued sessions. Define whether the shared bearer is the sole trust boundary, or enforce issued, expiring sessions without breaking compatible clients.
- **Bound bridge shutdown.** The stop timeout is followed by an unconditional thread join. Make in-flight operations cancellable and verify the entire shutdown deadline.
- **Protect agent provisioning.** Agent creation and download can overwrite scripts, and downloaded code is not pinned to an integrity-checked version. Use explicit overwrite semantics and investigate pinned, verifiable sources.
- **Make access modes intelligible.** Limited and sandbox currently enforce the same tool policy. Document their actual equivalence or design a meaningful distinction; report the effective live mode rather than only the startup mode.

## Cross-platform experience

Semantic UI uses Praefectus accessibility backends (macOS accessibility, Linux AT-SPI, Windows UI Automation); image capture uses rs_peekaboo separately. Platform behavior below is from source inspection, not a live desktop test.

- **Linux sessions:** upstream rs_peekaboo selects capture and clipboard commands based on availability, not whether they work in the current X11 or Wayland session. Its screen listing uses `xrandr`, and its accessibility permission check tests for `xdotool` rather than AT-SPI. Fix selection and diagnostics upstream, exercising mixed-tool installations and avoiding retries after ambiguous writes. Define Wayland compositor/portal support before claiming screen enumeration parity.
- **Windows diagnostics:** `system_info` does not populate memory and invokes the Unix `uptime` command. Add native memory and uptime reporting with Windows tests.
- **All platforms:** semantic observation errors collapse to a generic unavailable message. Provide sanitized, actionable categories for missing permission, session, or accessibility service without exposing UI contents. Verify on real desktop sessions; OS build/test CI alone does not exercise these backends.

## Capability directions

- Add a **local, read-only readiness diagnostic** covering session type, semantic backend, capture backend and permissions, with actionable remediation. Keep remote capability reporting disabled until its privacy boundary is designed.
- Explore **narrow, cancellable alternatives** to disabled shell, web and UI tools: for example, host-approved scoped network fetches and semantic actions with bounded authority. Do not re-enable legacy handlers merely because code exists; require effect identity, cancellation and privacy guarantees first.
- Improve **operation visibility** locally: show effective access mode, connection health and whether an action was verified or has an unknown outcome. Do not conflate an accessibility dispatch receipt with a verified user-visible result.
