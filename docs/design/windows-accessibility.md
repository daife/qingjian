# Windows 辅助功能

设置新增「辅助功能」同级导航，管理截图 OCR、通用语音识别的开关与渠道；语音语言为中文或英文。
两项功能默认关闭，渠道目前只有 `youdao_public`，独立于 `[predict]`；无需 API 密钥。
快捷键仅放「快捷键」页，分别为 `[shortcut] screenshot_ocr`（Ctrl+Shift+S）与 `speech_recognition`（Ctrl+Shift+H），支持关闭、字母 / 数字与修饰键组合。

## 进程与资源

Server 的 `accessibility` 模块拥有独立消息线程，使用 `RegisterHotKey(MOD_NOREPEAT)`，冲突提示原组合与修改入口。
空闲时只有配置 mtime 检查，无录音、截图、低级钩子或网络请求。每次只执行一个任务，重复快捷键忽略；配置关闭功能时取消当前任务。
COM MTA 的任务工作线程负责 UIA 目标查询、采集、PNG 编码、HTTP 请求与结果交付，不借用输入引擎、不占候选窗口线程；独立消息线程持续响应取消钩子。任务结束释放线程、Tokio current-thread 运行时和请求客户端。

## 截图

GDI 一次抓取虚拟桌面的 32 位 top-down DIB；每块显示器创建一个普通不透明置顶窗口，统一以物理像素计算跨屏选区，支持负坐标。
蒙层绘制使用原始截图加 1×1 黑色 AlphaBlend 源图，避免保存第二张全尺寸截图；选区重画原图与细边框，只更新旧 / 新矩形并集和尺寸提示区域。
选区内没有透明洞，不设置 `WS_EX_TRANSPARENT`，全部区域接收鼠标事件；拖动时 `SetCapture`。
松开左键后从原始 DIB 裁剪，按行转 RGB 并编码 PNG，避免额外分配整张 RGB 图片。取消或失去前台时不上传。
退出遮罩时只在遮罩仍是前台的情况下恢复原窗口，不抢回用户主动切走的应用。

## 录音与取消

WASAPI 共享模式、事件驱动采集，系统音频引擎降混和重采样到 16 kHz、单声道、16 位 PCM；最长 60 秒，到时自动识别。
PCM 上限 1,920,000 字节，WAV 头 44 字节。采集结束立即停止音频客户端并释放设备，音频与截图均不落盘。
不抢焦点的原生提示窗显示时长；空格结束并识别，Esc 或右键取消。临时低级钩子只处理这些操作，按下与松开配对消费；结束后排空旧事件才卸钩。
网络阶段仍可取消，取消后丢弃 HTTP future，迟到结果不能写剪贴板。结果交付与取消使用原子状态互斥，开始交付后不会再把已复制或已受理写入的任务提示为取消。截图取消信号通过活动期间的窗口定时器检查。

## 结果写入

先写 Unicode 剪贴板，再检查原窗口、键盘焦点句柄和 UI Automation 元素是否仍一致；不一致只提示复制成功。
UI Automation 使用 `CUIAutomation8`，设置有限连接 / 事务超时且关闭自动聚焦。
青简 TSF 的既有消息窗口定向接收目标捕获与识别结果：`ASSIST_CAPTURE_MESSAGE` 保存任务令牌和当前可写 `ITfContext`，
`WM_COPYDATA` 传 u32 令牌 + UTF-16 文本。该轻量控制消息无需修改输入按键协议或增加轮询。
DLL 校验令牌后一次性消费目标，在异步编辑会话申请后再次检查文档焦点、只读 / 私密状态与组句状态；在选区插入并把光标移至末尾。
取消或任务销毁定向释放目标，DLL 停用也释放。TSF 已发出的请求不因超时再回退粘贴，以免重复插入。
无可用 TSF 目标时，仅在 UIA 确认控件可编辑且非密码框后发送一次 Ctrl+V；持有修饰键、只读、权限隔离或特殊控件时只复制并提示。

## 有道协议与验证

平台无关的 `qingjian-recognize` 提供 `Recognizer::ocr` / `speech`，不接触输入引擎。
协议依据 `D:\Documents\ChatGPT\Youdao_public`：OCR 为 `/ocrapi1` 表单，PNG Data URL；ASR 为 `/asr` multipart `audioData`，URL 参数保留 `mutiSentences` 的原拼写。
连接超时 5 秒，总请求超时 60 秒，响应上限 2 MiB，PNG 本地上限 5 MiB。禁止自动重定向与重试，HTTP / 结构 / 业务错误分别处理；正文与媒体不进日志。
OCR 按服务返回顺序连接 `lines[].words`，语音按分句换行。空结果、取消、错误不修改剪贴板。

2026-10-19 用参考项目的合成英文语音重复构造 60 秒 WAV，实际上传有道返回 HTTP 200、`errorCode=0`。
这只验证该输入格式与该样本，不代表接口配额或稳定性承诺。
可复测 Rust 客户端（会上传指定目录的合成测试图片和音频）：

```powershell
$env:QINGJIAN_UIACCESS = '0'
$env:QINGJIAN_RECOGNITION_FIXTURES = 'D:\Documents\ChatGPT\Youdao_public\tests\fixtures'
cargo test --locked -p qingjian-windows-server --test recognition_live -- --ignored
```

Rust 客户端另有显式网络测试，验证 OCR 与整段 60 秒 ASR；默认跳过，不会上传桌面或录制麦克风。

日常测试离线覆盖配置兼容、60 秒边界、裁图像素与颜色顺序、负坐标反向拖动，以及取消和结束事件配对消费。
界面验收需在可操作桌面验证：不同 DPI 多屏框选、遮罩内外不穿透、Esc / 右键取消、空格结束录音、60 秒自动结束、记事本与浏览器输入、切换输入目标只复制。
