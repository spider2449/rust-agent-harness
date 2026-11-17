# UI scale and conversation width

## Scope

Make the shared Web and Desktop UI scale configurable, with 100% matching the original 14px base. Use root-relative dimensions for text, controls, icons, spacing and responsive breakpoints, preserving hairline borders and viewport-relative sizing. Remove the 880px conversation width constraint by using fixed side gutters. Desktop startup and error pages use the same stylesheet.

## Validation

Run the frontend plugin tests and git diff checks. No kernel, service contract or ADR changes are required.

## User preference

Add UI scale (75-200%, default 100%) to Settings display preferences. Save through the existing server preferences storage, validate the range, apply immediately and restore on startup. Retain the previous scale and show an error if saving fails.

Use a dropdown for UI scale with 75%, 90%, 100%, 110%, 125%, 150%, 175% and 200% presets. Include an existing custom value so saved preferences remain selectable.

## Validation results

Frontend plugin tests passed: 32 passed, one performance test ignored.
JavaScript syntax checks for app.js and settings.js, cargo fmt --check and
git diff --check passed. Browser and native Desktop visual verification were
not performed in this commit task. No kernel, service contract or ADR changes.
