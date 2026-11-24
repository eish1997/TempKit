# R0：统一领域契约 v1

2026-11-13。范围是可编译类型、纯校验及契约案例；没有注册服务、实际Harness连接、资源数据库或生产任务队列。代码源为[contracts.rs](../../../desktop/src-tauri/src/contracts.rs)，测试为[contracts.rs](../../../desktop/src-tauri/tests/contracts.rs)。此处的JSON是QuickGen内部协议草案，不宣称任何外部Harness已经支持。

## 契约与所有权

所有跨边界顶层记录携带schema_version=1；未知版本、字段和枚举值拒绝，不静默降级。API接入必须反序列化并调用对应validate，不能把serde解析成功等同于业务校验或权限授权。标识符采用不超过128字节的字母、数字、点、下划线及连字符；标识符只作逻辑键，禁止直接拼成磁盘路径。外部运行ID是适配器返回的opaque值，不要求符合本地ID格式。

| 对象 | 核心内容 | 权威归属 |
| --- | --- | --- |
| Resource | ID、受管blob键、类型专有元信息、MIME、长度、父资源/任务来源 | Rust资源服务；文件正文不穿过场景配置 |
| Capability | 连接+能力+不可变revision、输入输出role及数量、参数约束、权限、执行特性 | 能力目录 |
| Preset | 固定能力引用、默认参数、名称、可选封面资源引用 | 预设目录；不包含凭据或启动命令 |
| Connection | 适配器ID、受管profile ID、凭据引用、连接状态 | 宿主连接管理；profile内部格式由具体适配器定义 |
| TaskRequest | 提交ID、可选批次ID、预设ID、能力引用、资源role引用、完整有效参数 | Rust任务服务受理 |
| Task | 请求/能力快照、attempt、状态、事件版本、外部运行ID、输出引用 | Rust任务服务 |
| BoardScene | 放置实例ID与资源ID、位置、相机、选中实例、revision | 宿主持久化；前端可暂存未提交操作 |

2026-11-24 扩展：便签与会话画笔采用现有宿主与持久化服务，不增加模型/任务执行器。便签 `Note.document={version:1,doc:...}` 为正文权威，纯文本由宿主同事务投影；主题独立版本，字段并发、迁移与恢复规则见[便签保存恢复](../notes-recovery.md)。文档白名单和限额由 `note_document.rs` 校验，标签ID与首次保存时间由宿主维护；旧客户端不能将富文本静默压平。

桌面画笔 `InkHost` 只保存当前进程会话的对象和100步历史，退出绘制保留，程序退出清空，不加入资源数据库/便签/恢复包。颜色和线宽走既有配置；模式epoch、场景revision和渲染器提交序列保护迟到与重复请求。界面保存未确认操作供重试或明确放弃，宿主仍是已接受对象的权威。大场景快照复用IPC Channel传输，原生覆盖窗口退出时先释放鼠标捕获，再等待编辑确认。

image/text/file/link是已声明的类型集合，当前实际导入仍只有图片。每个执行器显式提供supported集合，类型有定义不代表启用。新类型须升级契约并实现导入/呈现适配。首版参数支持有上限的非空文本、有范围的整数、布尔；不引入任意JSON参数。未来确需列表/嵌套结构时版本化扩展。

角色名称由能力定义，首个示例用source/result；不要按数组顺序猜用途。单个任务输入数量按能力角色min/max校验。资源真实解码、磁盘存在性、权限与摘要核验属于R1/R3，当前内存校验不能证明文件存在。

## 注册和更新

[图片示例](image-registration.json)与[Harness会议清单示例](harness-registration.json)使用同一Registration结构，示例连接local-harness是测试标识，不会寻找或启动真实软件。

1. 用户建立受信Connection；宿主解析profile并验证对端身份、能力和权限。身份授权不是由导入JSON声称Ready即可完成，连接状态只能由宿主管理。
2. Harness提交能力定义和预设；注册请求的连接ID必须匹配已认证连接。接口入口、发现及配对机制待选实际Harness后决定。
3. 同连接+能力+revision下相同定义返回KeepExisting，不覆盖已编辑卡片；同revision内容改变返回version_conflict。新revision创建独立定义，旧预设仍固定旧revision。
4. 能力首次导入与默认预设创建在同一数据库事务中完成；已注册能力下补建/编辑用户预设是独立操作，重复注册不能偷偷重置它。导入封面如有资源引用，须由资源服务核验访问权。
5. 原版本消失则明确失败，不自动选择最新版。离线/未验证/撤销连接返回connection_unavailable，导入不产生可用卡片；连接恢复后重新校验。

Registration.validate只是纯决策，尚无目录写入、身份认证或联网可用性检测。未来适配器应明确保存流程的指令、依赖和必要上下文；会话编号本身不构成可重复调用能力。

## 任务受理和多图

首期只实现each：多张图片先拆成逐张TaskRequest，各自唯一submission_id，共用batch_id，每个请求只含一张source；每批首期串行并发1。已有示例拒绝意外把两张source塞进单任务。together以后必须由能力显式声明，本期没有对外together入口。

受理事务：校验版本→解析预设与固定能力→校验权限和连接→校验资源/角色/参数→冻结快照→去重入队。参数在客户端显示后由宿主合并预设默认值与显式覆盖；冻结后不能因卡片编辑而变化。客户端提交完整参数，宿主仍需重新校验；不能仅信任preset_id或请求中的能力引用。

submission_id在本应用任务域唯一：相同ID与相同规范化请求返回原任务；相同ID不同内容返回submission_conflict。重试另建attempt，不改变原输入和能力快照；成功项不重复执行。批次受理需保留每项ID/错误，不能因一项失败让整批成功项重跑。持久化和并发去重在R3实现。

## 状态、结果与事件

合法单attempt迁移：

- queued → running / cancelled / failed。
- running → waiting_user / finalizing / failed / interrupted / cancelled。
- waiting_user → running / failed / interrupted / cancelled。
- finalizing → succeeded / failed / interrupted。
- succeeded / failed / interrupted / cancelled为该attempt终态；重试创建新attempt。

running→cancelled只允许执行器确认取消，或能够证明尚未产生副作用；请求取消不等于取消成功。不支持取消时如实显示，不能伪造终态。waiting_user是扩展状态，审批交互未实现。

执行器输出ExecutionOutput含待接收的transfer handle；资源服务取得实际内容、解码/校验/归档后，形成TaskResult的稳定资源引用。TaskState.transition的outputs_archived参数只能由宿主归档结果决定，不能从前端请求取得。没有输出的能力也必须按其输出约束验证并完成结果记录事务。

FailureStage区分admission/execution/archive，outcome_unknown表示外部结果未知。归档失败保留已取得输出与传输信息，重试仅归档，不调用生成；若临时输出已失效，则报告无法恢复，重新生成是另一次显式执行。

事件携带task_id、attempt、单调event_version；同attempt仅应用更大版本。切换到新attempt须先刷新任务快照，不能被迟到旧attempt事件覆盖。事件接收仍需任务服务核对状态迁移，不以“版本较大”跳过合法状态检查。

重启queued项可保留排队；running/waiting_user未知执行结果置interrupted，不自动重发；finalizing只可从持久输出恢复归档。具体重启执行策略在R3落实并验证。能力声明query/cancel/idempotency/progress，不支持就是false。未知进度不编百分比。

## Harness资源传输边界

资源引用首先是QuickGen本地ID，对端不能默认读取本机路径。适配器负责将指定输入转换成可访问内容：本机CLI可由宿主创建本次任务的受控临时副本；远端接口可上传并映射对端对象。具体方式取决于选定接口。

TransferRef是适配器内部的短期句柄，绑定connection/task/attempt和读写范围，由宿主登记解析；导入方不能用它请求任意路径/URL读取。传输完成须验证内容类型、长度、输出角色和数量限制，不能信任对端声明。凭据只用于适配器连接，不进入资源、事件或日志。临时句柄永不充当持久结果ID。

## 存储决策

选择SQLite管理资源元信息、场景引用、能力/预设、任务/attempt、迁移映射与提交去重；文件正文放受管blob目录，当前config.json及covers在本阶段保持原有职责，避免同时改所有配置。

理由是任务受理、资源引用与迁移标记需要跨记录事务；整体JSON索引会逐渐需要自行实现事务、并发与恢复。SQLite事务机制参见[官方原子提交说明](https://www.sqlite.org/atomiccommit.html)。这是工程选型，数据库尚未接入；Rust依赖版本、打包和迁移SQL由R1验证。

首版单写入者，短事务，启用外键与持久同步；不把网络调用或图片解码放进事务。先用默认rollback journal，不为尚未证明的并发读取需求开启WAL。数据库事务不覆盖外部blob文件，必须设计补偿：先写同盘临时文件并同步、再移至不可变唯一blob键，最后事务写资源记录。中断可能留下孤立文件，但不能提交指向未完成文件的可用资源。恢复扫描只处理受管区域，校验后回收孤立临时文件；已登记文件缺失显示损坏，不静默删记录。文件与数据库断电恢复仍需R1故障测试证明。

## 删除、保留与迁移

移除场景实例只移除引用，不删除用户原文件。场景、封面、任务快照及输出引用都参与保留判断；父子来源关系保留ID和历史信息，不无限递归强制保留所有原文件。活动任务输入必须钉住，显式删除不得破坏执行中任务。

首期不做关闭自动清空，也不做用户资源定时自动删除。仅显式删除资源/历史后且没有有效引用，才标记待回收；二次核验后清理受管blob。任务历史保留期默认不自动过期，后续产品设置另定，避免未经决定的数据丢失。孤立导入临时文件与用户已导入资源分开处理。

迁移读取当前IndexedDB quickgen-board-single-v1/scene/current：保留items、jobs及能力快照、相机scale/tx/ty、选择、pinned和开关状态；面板布局/主题/能力沿用config.json。旧item ID映射新资源ID及放置实例ID，旧job ID映射历史模拟任务，模拟结果明确标记来源，不能升级成真实生成记录。

先冻结旧快照并备份，逐项导入验证，持久保存旧ID→新ID映射与进度。中断可续传；全量核对资源内容、位置、能力关联和现场状态后，原子切换迁移标记。切换前旧路径权威，切换后Rust权威；不并行双写。旧库保留只读备份。回退旧程序只能看到旧快照；新增Rust内容需保留/导出，不能承诺无损任意回退。R2负责迁移准备与演练；正式active切换在R4消费者全部就绪后进行，执行顺序以[可靠底座计划](../resource-task-plan.md)为准。本轮未读写用户数据。

## 验证与后续

运行项目根目录命令：`desktop/scripts/desktop.cmd test -- --offline --test contracts`。

自动案例覆盖图片逐张/非图片统一协议、非法role/type/参数、缺失资源、未知版本字段、重复注册、版本冲突、离线连接、归档门槛、终态重试约束、迟到事件及场景悬空引用。内存模拟不等同真实Harness或生产服务验收。

R0之后接R1：实现SQLite元信息和受管图片导入/校验/恢复。连接配置、实际Harness桥接和可见导入UI仍在后续G专项；现有桌面交互继续使用已验收版本。

### R0验证源码指纹

命令退出码0，9项新增契约测试＋28项原有Rust测试通过。无新桌面候选发布。

| 文件 | SHA256 |
| --- | --- |
| desktop/src-tauri/src/lib.rs | a1807d944d15a01914f3ae5998d3af79fbca153be0960a43a65b166eb7aaa7bf |
| desktop/src-tauri/src/contracts.rs | 6d8cef5b71be0831a93260fc1450ec912046283e4b56f4a562f4e02917d013c7 |
| desktop/src-tauri/tests/contracts.rs | 0327454a24b7e71eeb14e3e3d472d461578d399806df932e5bab5a133ed677a4 |
| docs/quickgen/contracts/image-registration.json | 6de85617f2a3683535a9c6444a68f3e82e8333bbdff150daac7538989081f9b3 |
| docs/quickgen/contracts/harness-registration.json | a296745eeb85078a3b6bce8a6af88d15f055c099fb175cd1a161e3134af094a5 |


## 2026-11-13 审查返修兼容扩展

Task增加可选failure（缺省None，成功时省略），复用TaskFailure的code/stage/message/outcome_unknown。归档失败仍为finalizing，但必须携带archive阶段错误，并在与task_failures同一事务内递增event_version和写入任务事件。成功归档原子清除错误；retry_archive不创建attempt、不调用执行器。重启补发旧版仅存在task_failures中的归档错误一次，保留等待显式重试语义；旧任务JSON仍可读取。

本地scene.jobs仅为任务呈现缓存，hidden只影响卡片显示。结果恢复遍历宿主任务快照/事件，持久result_placements负责去重和明确删除后的不复活。排队取消、隐藏任务、移除画板图片是三种不同操作。
