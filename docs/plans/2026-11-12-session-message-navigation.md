# Current session message navigation

Add a compact horizontal row below the session header, with one numbered, text-only summary per recorded user message. Buttons scroll the transcript to that message and highlight the current reading section. Keep the row visible above the independently scrolling transcript; allow horizontal scrolling for many prompts and keyboard access. Rebuild on transcript load, new turns and session switching; clear it when closed or empty. Preserve the existing uncommitted history search work.

No backend, dependency, ADR, service-contract or protocol changes. Validate in the existing headless browser harness with navigation, scroll tracking, untrusted text, overflow, session switching and empty sessions, plus JavaScript syntax, formatting and diff checks.

## Result and validation

Implemented in `plugins/frontend-web/assets/index.html`, `app.js` and `app.css`, with a dedicated browser regression in `crates/rah-cli/tests/web_browser.rs`. The row contains recorded user messages and refreshes when a completed turn reloads the transcript. Active sections track transcript scrolling; native buttons support keyboard activation. Stale transcript responses cannot replace a different active session.

Passed the new navigation browser test and existing session lifecycle/search browser regression using local Chrome. Coverage includes four long messages, literal HTML input, Enter activation, anchor positioning, manual scrolling, horizontal overflow at 480px, switching to an empty session, reopening and closing. Visually inspected desktop and narrow screenshots under `target/message-navigation-preview/`. Passed `cargo fmt --check`, `node --check plugins/frontend-web/assets/app.js` and `git diff --check`. The initial browser test build used an incorrect fixture field name (`kernel` instead of `_kernel`); corrected it and reran successfully.

No backend, ADR, dependency, plugin-contract or protocol changes. Existing uncommitted search work is preserved. No commit or deployment performed. Full workspace tests were not run for this frontend-only change. Suggested next task: rebuild and restart the local RAH installation to review the navigation with real sessions.

## Visual revision

The first row looked too subtle to the user. Replace small single-line chips with a distinct conversation map: a heading and question count, two-line summary cards, numbered badges, a filled accent card for the current section, and previous/next question buttons. Keep the map above the transcript, horizontally scrollable on narrow screens, and retain text-only rendering and native keyboard controls. Validate both navigation controls and desktop/narrow screenshots.

Visual revision validation passed in local Chrome: the navigation regression now checks the question count, previous/next question transitions and disabled end states as well as existing anchor, keyboard, scrolling, literal HTML and session-switch behavior. Updated the synthetic prompt text so preview cards show readable topics. Inspected refreshed desktop, dark-theme and 480px screenshots in `target/message-navigation-preview/`; cards and active contrast are visible without page overflow. JavaScript syntax, Rust formatting and diff checks passed. Changes remain local and uncommitted; no running installation was replaced.

## Single-row revision from user screenshot

Remove the heading and question-count row. Place the previous/next controls to the right of the horizontally scrolling cards in the same row. Use six alternating muted card colors, with a stronger matching border and inset marker on the active card, rather than sharing the session/message blue selection color. Retain two-line summaries, keyboard controls and anchor behavior. Extend the browser regression to check the single-row height, absent heading count and distinct card/message colors.

Single-row revision validation: the local Chrome navigation test passed, including a row-layout and maximum 90px height check, absent heading count, distinct neighboring card colors and a selection background different from the user bubble. Inspected refreshed light, dark and 480px screenshots. The first height assertion used a strict less-than-90 bound and failed at exactly 90px; verified the actual single-row geometry and corrected the inclusive boundary. Shortened the synthetic long prompts while retaining overflowing transcript coverage. Syntax, formatting and diff checks passed. No backend or contract changes, commit or running-installation update.

## Compact text-height revision

Reduce anchors and arrow controls to 24px height and the entire navigation row to at most 35px. Render each summary on one line with CSS ellipsis according to available width, without JavaScript character truncation. Keep the full normalized message in the tooltip and accessible label. Retain distinct card tones, keyboard navigation and horizontal scrolling, without a scrollbar consuming extra height. Validate compact row height and single-line ellipsis at desktop and narrow widths with the existing browser regression.

Compact revision validation passed in local Chrome: the browser regression verifies a maximum 35px row height, one-line 18px summary text, overflowing text with CSS ellipsis, distinct anchor colors, keyboard activation, previous/next navigation, scroll tracking and session changes. Inspected refreshed dark and narrow screenshots. `cargo fmt --check`, JavaScript syntax and `git diff --check` passed. Changes are limited to frontend rendering/style, the existing browser regression and this plan; no ADR or service-contract changes. No commit or running-installation update performed.
