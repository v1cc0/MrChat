# 项目进度

## 当前重点
- 基于现有 Hummingbird 代码梳理的成果，制定聊天界面重构路径、Turso 一体化持久层方案，以及音乐播放模块的并存策略。

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

## 待办
- 丰富聊天域模型细节（上下文截断策略、消息元数据）并串联 Turso DAO。
- 拟定 UI 线路图：对齐现有窗口骨架（`WindowShadow`）及组件布局，规划对话列表区、消息区、输入区。
- 规划 API Key 与 Turso 连接配置的存放方式，确保热更新与启动流程一致。
- 设计音乐模块与聊天界面的模块化隔离（启动/挂起/控制接口），确保互不干扰。
- 评估并改造音乐库扫描/查询/缓存逻辑以适配 Turso API。
- 实现 Turso 连接配置加载 + 健康检查命令，补齐 CRUD 基础。
- 为聊天服务层提供最小 API（会话创建、消息写入）并准备集成测试框架。
- 设计聊天 UI 原型并将 `ChatDao` 的数据流接入 `ChatState` 以驱动前端视图。
- 将 `ChatOverview` 集成到主窗口布局，并绑定用户交互（选择会话、新建对话、发送消息）。
- 拆分聊天服务错误处理/日志策略，补充落地的 tracing 输出格式。
