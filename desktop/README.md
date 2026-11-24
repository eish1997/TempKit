# QuickGen Windows 宿主

> 2026-11-24 最新日常版已更新并启动：ACB65A1A1B89，统一禁用网页默认右键菜单，保留自定义右键。自动验证通过，实际桌面效果待观察；旧版备份及完整SHA见设置整理计划末尾。

> 2026-11-24 日常文件已按用户授权替换为草稿保护返修版7B2FFDEFAA9D（desktop/src-tauri/target/release/quickgen-desktop.exe）。未启动日常实例；添加连接错误未定位及部分原生待验收继续保留。旧版备份和回退见设置整理计划末尾；下方早期交付摘要属于历史。

> 当前状态：两项交付审查返修及验收已完成，包含隔离端到端、用户新增控件外观确认及新包身份核对。当前交付为QuickGen-Windows-candidate-9b2a33b08a6a.zip；原收口记录保留为历史，唯一状态见资源与任务计划。
## 当前正式入口（2026-11-13）

画板已经合回`src/main.rs → board.rs`。仍通过`scripts/desktop.cmd build`构建`quickgen-desktop.exe`，应用标识仍为`com.quickgen.desktop`，直接使用原config.json、covers和主题配置。`package-candidate.ps1`继续打包此入口。

独立`quickgen-board`产品目标已撤销；`scripts/board.cmd`仅为旧命令兼容转发。`quickgen-overlay-probe`是技术验证工具，不是另一产品版本。下文W阶段说明及独立验证说明属于历史背景，当前画板交互与验收状态以[迁移台账](../docs/quickgen/board-migration-plan.md)为准。

当前画板包含堆叠、大纲、框选、组拖动、逐张模拟任务与结果、PNG/JPG/WebP导入、自定义与预设管理、原生穿透及Alt＋·。可靠底座候选已把图片、场景与任务交给Rust资源库；旧IndexedDB只作为迁移源保留。R0—R4可靠底座已完成真实迁移、重启验证及交付打包，真实模型/Harness未接入；唯一进度见[资源与任务计划](../docs/quickgen/resource-task-plan.md)，恢复限制见[备份与恢复](../docs/quickgen/recovery.md)。

已接回原面板操作：拖入口或标题行空白处自由贴顶部/左右边，拖四边/四角调整尺寸（最窄120px，单列），上下拖高度图标切换32/56/80/112。布局沿用原config.json并在重启后恢复。Escape取消当前拖动；高度图标支持方向键和Home/End。布局测试：`node scripts/test-board-panel.cjs`。

## 日常版与诊断版

本轮交互基线与交付包见[收口记录](../docs/quickgen/desktop-baseline.md)。日常构建默认不启用desktop-diagnostics，不读取QUICKGEN_DIAGNOSTIC实验模式，不加载计时脚本。历史验收内容不因构建身份变化一律重测。

诊断构建用`scripts/build-diagnostics.ps1`，输出到`src-tauri/target/diagnostics/release`，不覆盖日常exe。诊断故障/性能实验必须传绝对路径的`--isolated-data`；`run-latency-experiment.ps1`要求`-IsolatedData`，且拒绝停止已有实例。新容量/副本脚本自动创建隔离目录，不能用真实用户现场作夹具。资源采样与证据见[延迟实验](../docs/quickgen/desktop-latency-experiments.md)。

## 独立画板浮层验证（B1，历史）

桌面交互收尾：刘海与面板以240ms沿同一表面伸缩，允许中途反向；快捷键和减少动态效果模式直接切换。外部图片在展开完成后再等待1秒，期间落到卡片也仅导入。附属面板保持12px间距，按右/左/下/上寻找空间；主面板与附属面板作为一组判断失焦，非固定时一起收起。动效和门限测试：`node desktop/scripts/test-board-motion.cjs`（项目根目录执行）。

见[画板迁移台账](../docs/quickgen/board-migration-plan.md)。`scripts/overlay-probe.cmd build`构建，`scripts/overlay-probe.cmd test`运行坐标与原生快捷键注册测试，`node scripts/test-overlay-layout.cjs`验证屏幕变小时图片保持可见。快捷键测试需先退出独立浮层候选，避免它占用Alt＋·。产物为`src-tauri/target/release/quickgen-overlay-probe.exe`，与旧QuickGen分开，不读写其配置。

启动显示两张可拖动图片和测试控制窗，Alt＋·全局切换操作/穿透，图钉控制常驻，X退出此独立验证程序。穿透时宿主读取鼠标位置，悬浮图片淡至30%。约每秒检查主屏工作区/缩放变化并更新两个窗口，主屏不可用时回退到可用显示器；超出新视口的图片移回可见区域，仍在视口内的图片不重排。目前只覆盖一个屏幕，不是横跨多屏的大画板；真实多屏热插拔仍待验收。测试控制窗的标题、文字和退出按钮仅用于验证，不代表最终产品面板。

此目录为 QuickGen Windows 桌面验证工程，已包含贴边窗口、图片输入、能力管理与配置恢复；真实交互尚待完整验收。没有模型调用、生成或保存结果。

项目文档从 [QuickGen 文档入口](../docs/quickgen/README.md) 阅读。整体架构与后续生成计划已记录，但模型接入尚未实施；旧自动执行目标已清除。

主面板标题行的主题图标按跟随系统 → 浅色 → 深色切换，所有面板同步，选择跨重启保存。面板和控件使用统一6px圆角，附属窗口与主内容区相隔12px。

本机已确认：Windows 10 build 19045、Node 22.20.0、Rust 1.93.0、VS2022 Community、WebView2 150。

从本目录执行：

```powershell
npm.cmd ci
.\scripts\desktop.cmd dev
.\scripts\desktop.cmd build
.\scripts\package-candidate.ps1 -ExpectedSHA256 <已验收exe的SHA256> -SourceManifest <冻结的source-hashes.json>
```

`desktop.cmd` 为当前机器的开发环境入口，使用已有 VS2022 与 Rust toolchain 的明确路径，只修改子进程环境；换机器时须更新脚本中的路径。Cargo 缓存在 `.cargo-home`，构建输出在 `src-tauri/target`。`package-lock.json` 与 `src-tauri/Cargo.lock` 应随源码保留。

`package-candidate.ps1` 将现有 release exe 打包到 `dist/QuickGen-Windows-candidate-<SHA前12位>.zip`，先核对指定候选哈希及冻结源码，附使用说明、恢复文档、构建信息与 SHA256，再核对压缩包内程序哈希。相同候选不会覆盖；包不包含用户配置或图片，运行时沿用当前用户的应用数据目录。

构建目标为 `src-tauri/target/release/quickgen-desktop.exe`，不打安装包、不注册开机启动、不改系统 PATH。启动后显示贴边小入口，点击展开。操作行的 X 只收起面板，不退出应用。聚焦主窗口后按 Alt+F4 保存布局并退出。构建是否完成及真实运行证据以 `../docs/quickgen/windows-desktop-plan.md` 为准。


当前源码包含 W4 能力管理与恢复实现，尚待桌面验收。测试命令：

```powershell
.\scripts\desktop.cmd test
node scripts/test-input.cjs
node scripts/test-manager.cjs
node scripts/test-manager-lifecycle.cjs
node scripts/test-ability-save.cjs
node scripts/test-list-refresh.cjs
node scripts/test-resize-order.cjs
node scripts/test-display-watch.cjs
node scripts/test-motion.cjs
node scripts/test-drop-flow.cjs
node scripts/test-theme.cjs
```

配置位于 Tauri app_data_dir 返回的应用目录中的 config.json，封面在 covers/；实际绝对路径依当前 Windows 用户环境确定。模拟触发写到 app_log_dir 下 triggers.jsonl。待处理图片不写配置。不要在旧开发宿主运行时切换原生命令版本；正式验收优先使用同次构建的内嵌资源 exe。自动测试不替代原生拖放、实际图片解码、穿透或动画录屏验收。

当前桌面入口位于系统托盘：左键打开面板，右键可打开面板或退出QuickGen；主窗口与能力管理窗口不显示任务栏按钮。图片与面板共用1px描边及主题色。


## 设置整理开发候选（2026-11-23）

设置入口位于主面板底部工具区。连接在“设置 → 模型连接”，调用额度独立展示，结果暂存/拖出副本/完整恢复副本位于“存储与恢复”。具体模型配置从预设库已添加项或卡片右键“模型设置”进入。

本轮独立输出至 `target-settings-reorganization`，保留原 `src-tauri/target/release` 运行程序。候选使用 `--isolated-data <绝对空目录>` 验证，不迁移生产数据、不调用收费模型。默认实例仍使用原全局快捷键，验证前须由用户正常退出占用快捷键的日常实例；不要强杀或覆盖在用程序。最终候选SHA与原生待验收项见 `docs/quickgen/settings-reorganization-plan.md`。


2026-11-23 设置整理交付补记：候选9E86D2A709D2为最终交付程序，菜单点击问题已修复并经用户确认；用户确认其余已测项目通过及正常退出重开。实际范围和未单独验证的分支见设置整理计划末尾，不沿用前文开发候选“尚待菜单复验”的临时状态。保留原程序及全部旧包，未进行生产切换或付费模型调用。

## 富文本便签与桌面画笔（M阶段）

当前实现与证据见 `../docs/quickgen/notes-integration-plan.md` 第9节及末尾检查点，使用说明为 `notes-usage.md`、恢复规则为 `notes-recovery.md`。开发构建和npm正式构建先执行 `scripts/build-note-editor.mjs`，把固定ProseMirror依赖离线打入界面；第三方许可位于 `ui/vendor/licenses`。

独立输出使用 `CARGO_TARGET_DIR=<绝对路径>/src-tauri/target/notes-rich` 后运行 `scripts/desktop.cmd build`；诊断版另加 `-- --features desktop-diagnostics`。不要覆盖日常 `target/release` 或已启动验收exe。`package-candidate.ps1 -Stage notes-rich`核对冻结构建输入，并附隔离启动脚本及许可；诊断版不能冒充默认交付。

本阶段起，任何显式 `--isolated-data` 实例都不注册全局Alt+Backquote，可与日常版并存；面板及窗口内操作仍可用。日常实例快捷键不变。测试候选必须使用独立绝对目录，不带参数直接启动会使用日常数据。

`test-rich-desktop.ps1`只在独立且不切换的Windows桌面运行指定隔离候选，记录完整子进程内存、宿主结果及原生窗口状态；不发送物理输入。`verify-rich-recovery.py`仅接受M阶段证据目录，检查复制/旧库升级/异常退出后恢复。原生输入法、实际拖擦、跨应用拖出和混合DPI现场仍需最终手测，不能用这些脚本替代。
