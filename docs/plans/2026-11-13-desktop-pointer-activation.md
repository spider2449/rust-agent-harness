# Desktop pointer activation and IME

The user reports that clicking the Desktop input after deactivation does not
restore Chinese IME, while clicking the native title bar does. DOM focus
restoration does not cover missing top-level window activation.

Inject a Windows Desktop-only pointer listener. Real primary pointer input
requests native activation through the existing WebView IPC/event loop. Activate
the host window before returning keyboard focus to WebView, preserving the DOM
editor and selection. Keep setup form IPC restricted to its existing setup state.
No timer, background activation, service contract or new dependency is needed.

Validate the pointer bridge deterministically, run Desktop tests, compile a
Desktop binary, and inspect native activation where automation is available.
Native Chinese IME candidate placement needs a real Windows input-method check.

Validation: the pointer bridge test and all five Desktop tests passed, along
with debug/release builds, targeted Clippy, formatting and diff checks. The user
tested the release Desktop and reported that input now appears normal. The
temporary native activation known issue is closed on that manual confirmation;
automated coverage does not exercise a real IME candidate window.
