# tools-maya：RAH 的 Maya 2025 工具外掛

在 **RAH → Settings → Features** 打開 **Maya**。要寫外掛，可以說「幫我寫一個 Maya Python 指令外掛」。模型可先用 `maya.plugin_guide {}` 取得 Python API 2.0 範本，以及在 Script Editor 的 Python 分頁載入、測試和卸載的步驟。讀取範本全程在 RAH 本機完成。

Windows 完整版使用隨附的 `runtime\bin\python.exe`。精簡版使用 PATH 上的 Python 3.10 以上版本。模型可呼叫 `maya.locate {}` 查找本機的 Maya。工具依序檢查 PATH、`MAYA_LOCATION` 及 Maya 2025 以上版本的安裝登錄資料。也可在啟動 RAH 前用 `RAH_MAYA_EXE` 指定要使用的執行檔絕對路徑。

使用 `maya.session {}` 可查詢實際連線的 Maya 執行檔、連接埠、Maya、API、Python、Qt 版本及場景路徑。連線工具會使用已開啟的 Maya。若 Maya 尚未執行，RAH 會啟動查到的程式並等待就緒。Maya 顯示指令核准視窗時，確認後即可繼續。

RAH 透過 Maya 2025 在 Windows 預設註冊的 `commandportDefault` 找到場景。若手動關閉過這個入口，可在 Maya 的 Applications 偏好設定恢復預設 command port 並重啟 Maya。關閉 RAH 的 Maya 開關會卸載工具，場景仍由 Maya 管理。

可以對模型說：「建立名為 box 的方塊，移到世界座標 (3, 2, 0)，再告訴我它的位置。」小模型可以每次呼叫一個工具，讀取回傳值後再決定下一步：

| 工具 | 輸入範例 | 用途 |
| --- | --- | --- |
| `maya.create` | `{"shape":"cube","name":"box"}` | 建立方塊、球、圓柱、平面或圓錐。結果陣列的第一項是 Maya 實際採用的名稱。 |
| `maya.move` | `{"node":"box","x":3,"y":2,"z":0}` | 設定世界座標的絕對位置。若有同名物件，使用完整路徑。 |
| `maya.list` | `{"scope":"selection"}` | 列出選取的 transform 完整路徑；用 `scene` 查整個場景。 |
| `maya.help` | `{"command":"xform"}` | 向已安裝的 Maya 查詢 MEL 命令旗標。 |
| `maya.eval_mel` | `{"mel":"xform -q -ws -t \"box\";"}` | 執行 MEL 並傳回結果，例如查詢物件座標。 |

連線工具的成功回應含有實際執行的 `mel` 和 `result`。範例設定允許查詢，修改場景和執行自訂 MEL 需經 RAH 確認。本機其他程式也能連線到 command port，Maya 會顯示連線指令的核准視窗。請只允許預期的指令。若執行中斷或 MEL 報錯，先查場景再決定下一步，部分指令可能已生效。

整合測試使用獨立的 Maya standalone 場景與臨時連接埠，無需模型：

```powershell
mayapy -B plugins/tools-maya/test_maya.py
```
