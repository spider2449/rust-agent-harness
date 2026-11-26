# tools-maya：RAH 的 Maya 2023+ 工具外掛

在 **RAH → Settings → Features** 打開 **Maya**。Windows 完整版使用隨附的 `runtime\bin\python.exe`，精簡版使用 PATH 上的 Python 3.10 以上版本。Maya 外掛測試使用各版本隨附的 `mayapy.exe`。

## 選擇 Maya 版本

`maya.locate {}` 會列出本機找到的 Maya 與相鄰的 `mayapy.exe`。`executable` 優先回傳目前 Windows 工作階段內已開啟的 Maya 路徑。找不到可讀取的執行中路徑時，依序查找 `RAH_MAYA_EXE`、PATH、`MAYA_LOCATION` 和 Windows 登錄中的 Maya 2023 以上版本。登錄項目從新版開始查找。若完全找不到 Maya，工具會請你提供安裝目錄。把 `MAYA_LOCATION` 設成包含 `bin\maya.exe` 的安裝根目錄的絕對路徑，再重新啟動 RAH。

RAH 預設連上已註冊 `commandportDefault` 的 Maya。沒有 Maya 執行時，RAH 才會使用首選執行檔。若停用過預設入口，可在 Maya 的 Applications 偏好設定恢復並重啟 Maya。

若同時開啟多個版本，`maya.locate` 按 PID 由小到大列出可讀取的執行中版本。要選擇場景操作的目標，請在對應版本的 Script Editor **MEL** 分頁開啟獨立的命名連接埠，再從設好 `RAH_MAYA_PORT_NAME` 的 PowerShell 啟動 RAH。為每個版本使用不同名稱，例如在 Maya 2023 中執行下列指令。

```mel
commandPort -name "rahMaya2023" -sourceType "mel" -securityWarning;
```

```powershell
$env:RAH_MAYA_PORT_NAME = 'rahMaya2023'
```

命名連接埠需先在目標 Maya 登錄。呼叫 `maya.session {}` 核對連上的 Maya、API、Python、Qt 版本、執行檔及場景路徑，再開始修改場景。用完可在 Maya 執行 `commandPort -name "rahMaya2023" -close;`。Maya 顯示核准視窗時，只允許預期的指令。[Autodesk commandPort 文件](https://help.autodesk.com/cloudhelp/2026/ENU/Maya-Tech-Docs/CommandsPython/commandPort.html)

## 場景操作

可以對模型說「建立名為 box 的方塊，移到世界座標 (3, 2, 0)，再查出它的位置。」依序呼叫 `maya.create`、`maya.move`、`maya.inspect`，用上一步回傳的實際名稱或完整路徑。操作既有場景時，先以 `maya.list` 找到物件。

| 工具 | 輸入範例 | 用途 |
| --- | --- | --- |
| `maya.create` | `{"shape":"cube","name":"box"}` | 建立方塊、球、圓柱、平面或圓錐。結果陣列的第一項是 Maya 實際採用的名稱。 |
| `maya.move` | `{"node":"box","x":3,"y":2,"z":0}` | 設定世界座標的絕對位置。若有同名物件，使用完整路徑。 |
| `maya.rotate` | `{"node":"box","x":0,"y":30,"z":0}` | 設定絕對世界旋轉，沿用場景的角度單位。 |
| `maya.scale` | `{"node":"box","x":2,"y":1,"z":1}` | 設定各軸的局部縮放倍率。 |
| `maya.list` | `{"scope":"selection"}` | 列出選取的 transform 完整路徑；用 `scene` 查整個場景。 |
| `maya.inspect` | `{"node":"box"}` | 查完整路徑、世界位置、世界旋轉、局部縮放和 shape 路徑。遇到同名物件時，改用 `maya.list` 回傳的完整路徑。 |
| `maya.select` | `{"node":"\|group1\|box"}` | 選取指定 transform，取代原先的選取，並回傳實際選取的完整路徑。 |
| `maya.help` | `{"command":"xform"}` | 向已安裝的 Maya 查詢 MEL 命令旗標。 |
| `maya.eval_mel` | ``{"mel":"string $v = `about -version`; print($v);"}`` | 直接傳入 Script Editor 的 MEL 原始碼。變數與反引號沿用 MEL 語法，字串只需按 JSON 規則轉義。 |
| `maya.eval_python` | `{"python":"import MetalFkikSwitch\nprint(MetalFkikSwitch.__file__)"}` | 在 Maya 的 Python 環境執行程式碼。匯入的模組與變數可在同一個 Maya 工作階段繼續使用。 |
| `maya.python_paths` | `{"module":"MetalFkikSwitch"}` | 查詢 Maya 的 `sys.path`、模組載入狀態及 `__file__`。傳入 `{}` 可單查搜尋路徑。 |

`maya.eval_mel` 和 `maya.eval_python` 回傳原始碼、`result`、`stdout` 和 `stderr`。例如 MEL 的 `about -version;` 會把版本放在 `result`，上表的 `print` 範例則把版本放在 `stdout`。Python 單一運算式會回傳其值，多行陳述式回傳 `null`，可用 `print` 輸出所需資料。執行失敗時，工具錯誤訊息包含 traceback 及已擷取的輸出。`maya.python_paths` 只查已載入的模組，尚未載入時會回傳 `loaded: false` 和 `file: null`。

範例設定允許查詢，修改場景和執行自訂 MEL 或 Python 需經 RAH 確認。本機其他程式也能連線到 command port，Maya 會顯示連線指令的核准視窗。請只允許預期的指令。若執行中斷或報錯，先查場景再決定下一步，部分指令可能已生效。

## 開發與測試自己的 Maya 外掛

1. 呼叫 `maya.plugin_guide {}` 取得 Python API 2.0 指令外掛範本，將 `python_source` 存成該工具回傳的檔名。已有外掛時，直接使用你要驗證的檔案。
2. 呼叫 `maya.plugin_guide {"topic":"standalone"}` 並儲存回傳的 `test_maya_plugin.py`。把測試中的指令、輸入與預期結果換成你的外掛行為。範本會用絕對路徑載入，核對載入來源，驗證指令註冊與實際回傳，再卸載外掛。
3. 用 `maya.locate` 查出各個目標版本的 `mayapy` 路徑，在獨立程序中各跑一次。每個版本都應有 `PASS Maya …` 且結束碼為 0。失敗時從 traceback 判斷是載入、註冊、行為或清理步驟出了問題。 [Autodesk standalone 文件](https://help.autodesk.com/cloudhelp/2023/ENU/Maya-Scripting/files/GUID-D457D6A0-1E7F-4ED2-B0B4-8B57153B563B.htm)

先把 `maya.locate` 回傳的目標版本 `mayapy` 路徑存到 PowerShell 變數 `$mayapy`，再執行下列命令。

```powershell
& $mayapy -B 'C:/work/test_maya_plugin.py' 'C:/work/rah_hello.py'
```

使用可丟棄的場景驗證 UI、視窗與 viewport 操作。把 `maya.plugin_guide {}` 回傳的 `test_python` 貼到 Script Editor 的 **Python** 分頁執行，先將範例路徑換成外掛的絕對路徑。若外掛支援 undo/redo，核對操作前、操作後、復原後與重做後的狀態。自訂節點應測試建立、設定輸入屬性及讀取輸出，卸載前刪除測試建立的節點。Windows 的 C++ `.mll` 請用對應 Maya 版本的 SDK 建置，並在每個目標版本用同樣的載入與行為檢查驗證。 [Autodesk loadPlugin 文件](https://help.autodesk.com/cloudhelp/2025/ENU/Maya-Tech-Docs/CommandsPython/loadPlugin.html)

排查時可用 `maya.plugin_status {}` 列出已載入的外掛，或用 `maya.plugin_status {"plugin":"C:/work/rah_hello.py"}` 查載入狀態、實際檔案、指令與節點類型。遇到 Python 相依模組載入問題，用 `maya.python_paths` 對照匯入來源。修改外掛後，先在測試場景卸載再載入，或以新的 `mayapy` 程序重跑。 [Autodesk pluginInfo 文件](https://help.autodesk.com/cloudhelp/2025/ENU/Maya-Tech-Docs/CommandsPython/pluginInfo.html)

## 驗證 RAH 的 Maya 工具

本目錄的整合測試使用獨立的 Maya standalone 場景與臨時命名連接埠，無需模型。把 `$mayapy` 設成 Maya 2023 或 2025 的 `mayapy` 路徑後，可分別執行下列命令。

```powershell
& $mayapy -B plugins/tools-maya/test_maya.py
```
