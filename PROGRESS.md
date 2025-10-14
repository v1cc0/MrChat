# 项目进度

## 当前重点
- 基于现有 Hummingbird 代码梳理的成果，制定聊天界面重构路径、Turso 一体化持久层方案，以及音乐播放模块的并存策略。
- （永久保留）调试或检查本地 Turso 数据库时统一使用 `tursodb` CLI。

## 已完成
- 读取并梳理 `src/ui`、`src/settings`、`src/services`、`src/media` 等核心模块的职责。
- 识别出 `gpui` 驱动的组件体系（按钮、输入框、模态框、主题）可作为聊天界面的基础。
- 初步整理 Playback/Library 扫描线程等与音频播放高度耦合的部分，后续将作为迁移/替换重点。
- 明确聊天后端统一通过 HTTP API 访问（无论远程还是本地 LLM 服务）。
- 决定保留 Hummingbird 播放器逻辑，作为工作流内的可选媒体播放模块。
- 确认 Turso 作为唯一数据库后端，取代原有 SQLite 依赖。
- 形成《docs/chat_player_architecture.md》架构方案，涵盖模块拆分与 Turso 表设计。
- 搭建 `shared::db::TursoPool` 封装与 `modules/chat` 骨架（状态模型、服务占位、占位 UI）。
- 完成 `modules/chat/storage::ChatDao`，实现 Turso 表建模与会话/消息 CRUD，并接入 `ChatServices`。
- 扩展 `ChatServices` 暴露会话/消息 API，新增 `chat::bootstrap_state` 以异步同步数据；提供 `ChatOverview` 占位视图展示会话/消息概览。
- 将 Turso 聊天服务接入应用启动流程，并在主窗口嵌入 `ChatOverview` 支持基本会话切换与消息加载。
- 新增聊天会话面板与输入框，支持新建会话、发送消息并即时刷新 Turso 状态。
- 引入 `config.sample.toml` 与配置加载逻辑，整合 LLM API 调用并在前端显示助手回复。
- 创建 `config.toml` 模板，集中记录应用、聊天、播放器及 Turso 配置示例。
- 修复编译错误：更正 `TursoDatabase` API 调用（`connection()` → `connect()`），清理未使用的宏定义和导入。
- **完成数据库层完全迁移至 Turso**：
  - 从 `Cargo.toml` 移除 `sqlx` 依赖，添加 `turso` 和 `turso_core` 依赖
  - 为 `TursoDatabase` 添加 `Clone` trait 和多个辅助查询方法（`query_one`, `query_optional`, `query_scalar`, `query_map` 等）
  - 完全重写 `src/library/types.rs`，移除所有 sqlx derive 宏，为所有类型添加手动 `from_row()` 方法
- 完全重写 `src/library/db.rs`，将所有 `sqlx::query_as` 调用转换为 Turso API
  - 更新 `src/ui/app.rs`，采用分离的数据库架构：
    - **`music.db`**: 音乐库功能（扫描、播放、专辑封面等）
    - **`mrchat.db`**: AI 聊天功能（会话、消息等）
  - 迁移 `src/library/scan.rs` 的所有数据库操作（`insert_artist`, `insert_album`, `insert_track`, `delete_track`）至 Turso
  - 更新 `src/ui/assets/db.rs`，使用 Turso 加载专辑封面和缩略图
- 修复类型转换问题：`DateTime<Utc>` 转换为 timestamp，`Vec<u8>` 转换为 `Box<[u8]>`
- 项目成功编译，所有 sqlx 依赖已完全移除
- 移除 Turso 不支持的数据库触发器迁移，转而在扫描线程的删除逻辑中手动清理 `album`、`artist`、`album_path` 依赖，确保迁移可执行且库表保持一致性。
- 为 `TursoDatabase::run_migrations` 引入 `mrchat_migrations` 记录表，按文件粒度跳过已执行迁移，并在检测到重复列/索引时给出告警而不中断。
- 调整迁移脚本以满足 libSQL 要求：`mbid` 默认值改用单引号常量，并保留旧的专辑唯一索引以绕过 DROP 限制，保证现有库可顺利升级。

## 待办
- 丰富聊天域模型细节（上下文截断策略、消息元数据）并串联 Turso DAO。
- 拟定 UI 线路图：对齐现有窗口骨架（`WindowShadow`）及组件布局，规划对话列表区、消息区、输入区。
- 规划 API Key 与 Turso 连接配置的存放方式，确保热更新与启动流程一致。
- 设计音乐模块与聊天界面的模块化隔离（启动/挂起/控制接口），确保互不干扰。
- ~~评估并改造音乐库扫描/查询/缓存逻辑以适配 Turso API~~（已完成）
- ~~实现 Turso 连接配置加载 + 健康检查命令，补齐 CRUD 基础~~（已完成）
- 为聊天服务层提供最小 API（会话创建、消息写入）并准备集成测试框架。
- 设计聊天 UI 原型并扩展 `ChatOverview`（新建会话、消息输入/流式呈现、错误提示）。
- 拆分聊天服务错误处理/日志策略，补充落地的 tracing 输出格式。
- 测试数据库迁移的功能完整性（library 扫描、封面加载等）。
- 为音乐库删除路径补充集成测试/回归案例，验证手动级联清理行为（album/artist/album_path）。
- 排查 playlist 初始化数据解析失败（`created_at` 字段格式），补充兼容逻辑。
