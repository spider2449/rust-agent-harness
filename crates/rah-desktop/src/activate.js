// A focused DOM element does not imply that the native host is active.
// Request activation on real pointer input even when document.hasFocus is true.
document.addEventListener("pointerdown", (event) => {
  if (event.isTrusted && event.button === 0) {
    window.ipc.postMessage("rah:activate");
  }
}, { capture: true });
