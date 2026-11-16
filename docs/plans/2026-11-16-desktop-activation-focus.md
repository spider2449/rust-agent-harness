# Desktop activation focus recovery

The user reports Chinese composition outside the composer after Desktop loses
focus. Clicking page content recovers input, but clicking the title bar does not.
The existing pointer IPC only covers page clicks. The native focus handler calls
WebView2 synchronously during activation, before native title-bar handling ends.

Defer the WebView handoff until MainEventsCleared, after the native event batch.
Cancel pending recovery on deactivation, retain it while minimized or after a
failed handoff, and clear it after success. Keep page pointer activation intact.
No kernel, service contract, protocol, or dependency changes are required.

Validate deferred recovery, deactivation cancellation and restore/retry with
deterministic unit tests, run Desktop and pointer bridge tests, and build Desktop.
Actual Chinese IME placement after title-bar activation still requires Windows
manual validation.
