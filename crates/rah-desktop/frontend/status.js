const applicationRows = [
  ["Version", "appVersion"],
  ["Platform", "platform"],
  ["Desktop shell", "desktopShell"],
];

const runtimeRows = [
  ["RAH Runtime", "runtimeStatus"],
  ["Codex", "codexStatus"],
  ["Profile", "profileStatus"],
  ["Repository", "repositoryStatus"],
];

function renderRows(element, rows, status) {
  element.replaceChildren(
    ...rows.map(([label, field]) => {
      const row = document.createElement("div");
      const term = document.createElement("dt");
      const detail = document.createElement("dd");

      term.textContent = label;
      detail.textContent = status[field];
      detail.dataset.status = status[field];
      row.append(term, detail);
      return row;
    }),
  );
}

async function loadStatus() {
  const error = document.querySelector("#backend-error");

  try {
    const status = await window.__TAURI_INTERNALS__.invoke("app_status");
    renderRows(document.querySelector("#application-status"), applicationRows, status);
    renderRows(document.querySelector("#runtime-status"), runtimeRows, status);
  } catch (_error) {
    error.hidden = false;
  }
}

void loadStatus();
