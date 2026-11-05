# K3.1 — Native Applications Foundation

## Mission

Make SYSTEMES applications dependable, accessible, and useful for daily work by building on the existing Windows-native host and native engine. This phase is an incremental consolidation, not a rewrite of the desktop or application suite.

SYSTEMES is an independent project intended to remain completely free to use: no purchase price, paid activation key, subscription, monetized access, or paid lock on essential features. Third-party component licences must still be respected and documented.

## Existing architecture verified on main

- The desktop renderer communicates with the Windows host through WebView2.
- `src/native/NativeBridge.ts` is the browser-safe transport. It validates method names against an allowlist, bounds JSON payloads and concurrent requests, applies timeouts, and rejects pending calls on disconnect.
- `src/services/NativeBridge.ts` is the typed application-facing facade for Windows operations.
- `native/windows-host/SYSTEMES.WindowsHost.csproj` is the WPF + WebView2 host and publishes the native engine.
- The CI already runs frontend typecheck, unit tests, production build, Windows host/native-engine build, JSON handshake, and native operation smoke tests.
- `src/context/NativeBridge.ts` contains a separate legacy `window.nativeBridge` contract. A repository search found no current imports of that contract; it is intentionally not deleted in this foundation change until a dedicated cleanup review confirms all consumers and historical branches.

## Non-negotiable engineering rules

1. One transport: renderer applications must use the WebView2 transport; they must never spawn .NET, PowerShell, or arbitrary processes directly.
2. One application facade: application components call typed methods from `src/services/NativeBridge.ts`, not raw IPC strings.
3. No fabricated native state: unavailable, unsupported, denied, and failed operations must be represented explicitly. Never substitute demo data for a failed Windows query.
4. Least privilege: read-only operations stay read-only; mutations require input validation, clear user intent, and confirmation proportional to impact. No permanent administrator requirement.
5. Safe failures: errors must be actionable, localized, and must not crash the desktop or leave loading states stuck.
6. Accessibility by default: semantic controls, keyboard navigation, visible focus, accessible names, sufficient contrast, reduced-motion support, and useful empty/error/loading states.
7. Preserve existing behavior: no broad rewrites, no unrelated kernel changes, no removal of working application features.
8. Verify on the exact PR HEAD: frontend typecheck, tests, production build, Windows native build, and native smoke tests must all pass before merge.

## Shared application lifecycle contract

Every native-backed application should expose these distinct states:

- `checking`: initial native-host handshake or capability check is in progress.
- `ready`: the required native capability is available.
- `unavailable`: SYSTEMES is running outside the supported Windows host or the host is not reachable.
- `unsupported`: the host is reachable but the requested capability is not supported on this Windows/device configuration.
- `permission-required`: Windows denied the operation or elevation/consent is required.
- `error`: an operation failed; retain the actionable error and allow retry.

Do not infer `ready` from a successful frontend render. A capability is ready only after a real native response confirms it.

## Delivery sequence

### K3.1.1 — Native contract and capability inventory
- Inventory every renderer native operation and its native-engine handler.
- Detect missing, duplicate, and stale method definitions across the client allowlist, typed facade, and server dispatcher.
- Document operation type (read/query/mutation), required permissions, timeout, and user confirmation.
- Add automated contract tests so client/server drift fails CI.

### K3.1.2 — Shared native lifecycle and error model
- `src/services/NativeLifecycle.ts` provides checking, ready, unavailable, unsupported, permission-required, and error states with subscriptions and a reusable operation wrapper.
- `system/capabilities` reports Windows version/build, process architecture, administrator status, and detected application capabilities.
- `system/permissions/read` exposes the current Windows token/UAC context read-only. It never grants privileges or silently relaunches elevated.
- Native failures carry a stable `errorCode`, `retryable` flag, and operation context. Access-denied, unsupported, timeout, missing-resource, I/O, and unexpected errors remain distinguishable.
- Retry remains an application decision; no automatic retry is applied to mutations.

### K3.1.3 — Explorer reliability
- Preserve existing safe file-operation helpers.
- Improve empty/loading/error states, keyboard navigation, accessible labels, and large-folder responsiveness.
- Test path validation, collisions, cancellation, and native-host loss.

### K3.1.4 — Settings reliability
- Separate actual Windows settings/inventory from SYSTEMES-local preferences.
- Never show a setting as applied until Windows confirms it.
- Keep potentially disruptive actions behind explicit confirmation.

### K3.1.5 — Task Manager reliability
- Use measured Windows process and performance data only.
- Clearly label sampling intervals and unavailable counters.
- Protect process termination with PID/name/start-time revalidation and confirmation.

### K3.1.6 — Terminal and Notepad usability
- Keep PowerShell execution inside the native host with bounded input/output and explicit working-directory behavior.
- Keep Notepad file operations on the shared safe file facade; preserve unsaved-change protection.

### K3.1.7 — Security, backup, and app management
- Start with read-only inventories and transparent status.
- Do not claim antivirus, backup, restore, installation, or update functionality until the corresponding real operation and recovery path are implemented and tested.

## Scope of this foundation PR

This document establishes the architecture, safety boundaries, and delivery order only. It deliberately does not change application behavior, native permissions, kernel code, or existing bridge APIs. Implementation and contract tests follow in small, independently reviewable PRs.
