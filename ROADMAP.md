# Capability and reliability audit

Source audit of main at `aa35afdd0568418874cab93a77005d3872b1847c` (2026-11-05). These are engineering opportunities, not claims of live desktop verification. Preserve the authority, privacy, cancellation and approval guarantees in README.md when expanding capabilities.

## Reliability priorities

- **Bound retry history without losing cleanup.** Failed remote connection deletions remain in state for retry and are no longer evicted after ten entries. Persistent failures can therefore grow the history indefinitely; design a safe quota or compaction strategy that never silently forgets a live connection.
- **Interrupt stale approval prompts.** A single prompt worker prevents thread accumulation, and queued stale approvals are skipped. A prompt already displayed may still block on terminal input after invalidation; make input interruptible without allowing an old answer to approve another request.
- **Clarify session isolation.** MCP session IDs are caller-supplied and not tracked as issued sessions. Define whether the shared bearer is the sole trust boundary, or enforce issued, expiring sessions without breaking compatible clients.
- **Cancel bridge work at shutdown.** The bridge stop timeout no longer performs an unbounded join, but in-flight work should still be cancellable and tested against the entire shutdown deadline.
- **Protect agent provisioning.** Agent creation and download can overwrite scripts, and downloaded code is not pinned to an integrity-checked version. Use explicit overwrite semantics and investigate pinned, verifiable sources.
- **Make access modes intelligible.** Limited and sandbox currently enforce the same tool policy. Document their actual equivalence or design a meaningful distinction; report the effective live mode rather than only the startup mode.

## Cross-platform experience

Semantic UI uses Praefectus accessibility backends (macOS accessibility, Linux AT-SPI, Windows UI Automation); image capture uses rs_peekaboo separately. Platform behavior below is from source inspection, not a live desktop test.

- **Linux sessions:** upstream rs_peekaboo selects capture and clipboard commands based on availability, not whether they work in the current X11 or Wayland session. Its screen listing uses `xrandr`, and its accessibility permission check tests for `xdotool` rather than AT-SPI. Fix selection and diagnostics upstream, exercising mixed-tool installations and avoiding retries after ambiguous writes. Define Wayland compositor/portal support before claiming screen enumeration parity.
- **Windows diagnostics:** `system_info` uses Windows CIM for memory and uptime. Verify these probes on supported Windows desktops, including restricted environments where they may be unavailable.
- **All platforms:** semantic observation errors collapse to a generic unavailable message. Provide sanitized, actionable categories for missing permission, session, or accessibility service without exposing UI contents. Verify on real desktop sessions; OS build/test CI alone does not exercise these backends.

## Capability directions

- Add a **local, read-only readiness diagnostic** covering session type, semantic backend, capture backend and permissions, with actionable remediation. Keep remote capability reporting disabled until its privacy boundary is designed.
- Explore **narrow, cancellable alternatives** to disabled shell, web and UI tools: for example, host-approved scoped network fetches and semantic actions with bounded authority. Do not re-enable legacy handlers merely because code exists; require effect identity, cancellation and privacy guarantees first.
- Improve **operation visibility** locally: show effective access mode, connection health and whether an action was verified or has an unknown outcome. Do not conflate an accessibility dispatch receipt with a verified user-visible result.
