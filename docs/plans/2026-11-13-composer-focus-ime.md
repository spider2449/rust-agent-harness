# Composer focus and IME recovery

Restore composer focus when the window or page becomes active and focus is on
the document or composer. Preserve other focused controls and open dialogs.
Ignore Enter and completion shortcuts during IME composition, including the
legacy key code 229 used by WebViews.

Validate with a deterministic browser regression for focus recovery, preserved
search/dialog focus, and composition confirmation without submission. Native
Windows IME placement after Alt+Tab and minimize/restore needs manual validation.
