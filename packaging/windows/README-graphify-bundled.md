# RAH with bundled Graphify — Windows quick start

This package contains RAH CLI, RAH Desktop, and a bundled Node/Python/Graphify
runtime. Graphify setup and indexing need no separate Node, Python, uv or pip
installation and no downloads. Model inference uses the provider you configure;
cloud providers still need network access and credentials.

## Install and start

1. Run the setup executable. Installation is per-user and needs no administrator
   rights. Keep the PATH option enabled if you want to run `rah` in a terminal.
2. Open **RAH Desktop** from the Start menu. On first launch, choose your model,
   permissions and workspace in the setup form. Desktop requires Microsoft
   Edge WebView2 Runtime. If it is missing, use **RAH Web** or install WebView2
   from https://developer.microsoft.com/microsoft-edge/webview2/.
3. Close RAH, open a new PowerShell window, and enable Graphify:

   ```powershell
   rah graphify install --offline
   ```

4. Reopen **RAH Desktop**. Open your source workspace. Click **Graphify** in the
   footer, or open **Plugins > Plugin pages > Graphify**, to check indexing.
   Wait for Ready before expecting graph queries to succeed.

If `rah` is not on PATH, open PowerShell in the installed RAH folder and run
`.\rah.exe graphify install --offline` instead. The installer places this README
beside `rah.exe`; the **RAH README** Start menu shortcut opens it.

## Portable ZIP

Extract the entire ZIP into a writable folder. Keep `graphify-runtime` beside
`rah.exe` and `rah-desktop.exe`; copying the executables alone is insufficient.
Run `.\rah-desktop.exe` once to configure RAH, close it, then run
`.\rah.exe graphify install --offline` from that folder and reopen Desktop.

## Data, updates and moving to another computer

Settings and sessions are stored in `%USERPROFILE%\.rah` by default. `RAH_HOME`
can select a different data directory. Graphify installation backs up settings
before changing them and preserves unrelated configuration and stricter policy.

Close RAH before updating. After moving a portable folder or copying settings
to another computer, run the Graphify installation command again from the new
package so stored runtime paths point to the correct location. If you use a
custom profile, pass the same `--data-dir` and `--config` options you normally use.

Uninstalling asks whether to delete user data; keeping it preserves settings
and sessions. The bundled runtime is part of the application installation.

## Troubleshooting

- **Command not found:** reopen PowerShell after installation, or run `.\rah.exe`
  from the installation folder.
- **Runtime not found:** check that `graphify-runtime\runtime.json` exists beside
  `rah.exe`. Reinstall or extract the complete bundle, then rerun setup above.
- **Graphify is not Ready:** inspect the Graphify page for the indexing error,
  confirm the selected workspace contains readable source files, and allow the
  initial scan to finish. Indexes are stored in `.rah\graphify-out` within the
  selected workspace. An indexing failure does not make an older graph current.
- **Desktop does not open:** check WebView2, or use the RAH Web shortcut.
- **More command options:** run `rah --help` or `rah graphify install --help`.

Graph results are navigation aids. Verify relevant source before relying on a
reported relationship. Tool access remains governed by your RAH permissions.
