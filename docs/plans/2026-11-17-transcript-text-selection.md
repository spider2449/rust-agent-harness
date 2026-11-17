# Preserve transcript text selection

Desktop pointer activation currently schedules native WebView focus recovery on
every primary pointer press, including presses in an already active window.
Limit the pointer activation bridge to editable elements so dragging transcript
text can retain native selection. Preserve editor activation and title-bar IME
recovery. Tao's window focus flag alone is insufficient because WebView child
focus can clear the parent window's focus flag.
Also skip automatic composer focus restoration while a text selection exists.

Validate editor activation, transcript pointer input, and selected-text focus
preservation with deterministic JavaScript regression tests. Run
Desktop tests, JavaScript checks and a Desktop build. Native selection and IME
behavior require manual verification in the rebuilt Desktop application.
No ADR, service contract, protocol or dependency changes are required.
