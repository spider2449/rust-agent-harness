// A focused DOM element does not imply that the native host is active.
// Request activation for editors even when document.hasFocus is true.
// Refocusing the native host during a text-selection drag interrupts selection.
document.addEventListener("pointerdown", (event) => {
  if (event.isTrusted && event.button === 0 &&
      event.target?.closest?.('input, textarea, [contenteditable]:not([contenteditable="false"])')) {
    window.ipc.postMessage("rah:activate");
  }
}, { capture: true });
