# tools-blender · Blender 5.3 场景工具

在 RAH 的 **Settings → Features** 启用 **Blender**，或启动时加上 `--with blender`。示例配置见 [blender.toml](blender.toml)。插件使用 `D:\spider\blender-5.3.0-windows-x64` 中的 Blender 和自带的 Python。

首次调用会连接已接入 RAH 的 Blender 窗口。如果没有已接入的窗口，插件会打开 Blender 并连接新场景。要操作已经打开的项目，请先在该窗口的 **Text Editor → Open** 打开 [scene_bridge.py](scene_bridge.py)，点击 **Run Script**。同一时间只连接一个 Blender 实例。关闭 RAH 的功能开关会卸载工具，窗口和未保存的编辑会保留。

可以对模型说：「创建一个叫 box 的立方体，把它移到世界坐标 (3, 2, 0)，再告诉我它的位置。」

| 工具 | 参数示例 | 返回 |
| --- | --- | --- |
| `blender.list` | `{"scope":"selection"}` | 已选对象名称。传入 `scene` 可列出整个场景。 |
| `blender.inspect` | `{"name":"box"}` | 对象名称、类型和世界坐标。 |
| `blender.create` | `{"shape":"cube","name":"box"}` | 新对象的信息。支持 cube、sphere、cylinder、plane 和 cone，需要 Object Mode。同名时请使用返回的实际名称。 |
| `blender.move` | `{"name":"box","x":3,"y":2,"z":0}` | 移动后对象的信息。坐标为绝对世界坐标，单位为 Blender 场景单位。 |

查询使用 RAH 的 `read` 权限，创建和移动使用 `write` 权限。桥接脚本通过当前 Windows 用户的 `%LOCALAPPDATA%\rah\blender` 目录接收请求，在 Blender 主线程执行。该目录属于本机信任边界，同一账户下能写入此目录的程序也能提交场景操作。调用中断或超时后，请先查询场景，确认修改是否生效。

在仓库根目录运行集成测试。测试使用独立的临时目录和 Blender 后台场景。

```powershell
& 'D:\spider\blender-5.3.0-windows-x64\5.3\python\bin\python.exe' -B plugins/tools-blender/test_blender.py
```
