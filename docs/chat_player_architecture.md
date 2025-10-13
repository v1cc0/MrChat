# Chat + Player Architecture Plan

## 目标
- 在保持 Hummingbird 播放器完整功能的同时，引入面向 AI 对话的聊天体验。
- 共用一套 gpui 窗口/组件体系，避免重复造轮子。
- 以 Turso（libSQL）作为全局持久层，替代现有 SQLite 以及播放器内部的嵌入式数据库。
- 统一的计划→实现→CI/CD 流程，仅面向当前双人协作。

## 模块拆分
```
src/
  app.rs            // 新入口，装配两个功能模块
  modules/
    chat/           // 聊天域（新建）
      ui/           // 聊天视图、对话列表、输入框等
      models.rs     // Conversation/Message 等 gpui Global
      services.rs   // LLM HTTP 客户端、流式响应处理
      storage.rs    // Turso DAO 封装（聊天相关）
    player/         // 从原 Hummingbird 抽取，保持功能
      ui/           // 控制栏、队列、库浏览等
      models.rs     // PlaybackInfo/Queue 等
      services.rs   // 播放线程、MMB、设备控制
      storage.rs    // Turso DAO 封装（媒体库/偏好）
  shared/
    theme.rs        // 保留主题系统
    components/     // 公共 UI 组件（按钮、输入框、模态框等）
    db.rs           // Turso 连接池封装、查询工具
    settings.rs     // 环境变量 + 本地 JSON（轻量配置）
```
- `modules/player` 初期直接复用现有 `playback`, `library`, `media`, `services`，在新目录中分层迁移。
- 播放器模块通过接口暴露：`PlayerFacade`（启动/停止、播放控制、Now Playing 状态访问）。
- 聊天模块定义 `ChatFacade`，负责会话 CRUD、消息发送、API 对接。
- `app.rs` 或 `ui/app.rs` 中注入两个 Facade，并利用 gpui `Global` 控制模块启用状态（例如 Minimal Player 面板可隐藏）。

## UI 结构
- `WindowShell`（原 `WindowShadow`）分为三块：
  1. 主视图：聊天消息流（复用现有 flex 布局逻辑）。
  2. 侧边栏：会话列表，提供按标签/模型分类的过滤器。
  3. 底部可折叠播放控制条（Mini Player），点击可展开完整播放器（沿用旧 `Controls + Queue + Library` 布局）。
- 快捷键：
  - `Ctrl+K`: 打开命令面板（复用 `global_actions` 实现）。
  - `Ctrl+M`: 切换播放器面板显示状态。
  - `Ctrl+N`: 新建对话。
- 主题与资源：沿用 `assets` 热加载机制，新增加载聊天相关图标、头像占位符。

## 状态管理
- chat 模型：
  - `Entity<Vec<ConversationSummary>>`
  - `Entity<Option<ConversationId>>` 当前会话
  - `Entity<Vec<Message>>` 当前消息流
  - `Entity<LlmRequestState>` 跟踪请求/流式响应
  - `Entity<ApiProfile>` 用于区分远程/本地 LLM
- player 模型：沿用 `PlaybackInfo`、`Queue`、`Models`，只需将 LastFM/Turso 相关依赖调整为新的存储实现。
- 全局设置：
  - `SettingsGlobal` 扩展字段：`chat`（默认模型、最大上下文长度）、`player`（输出设备、默认音量）、`integrations`（Turso/LLM API Key）。

## Turso 数据模型
### 聊天
```sql
CREATE TABLE conversations (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  model_id TEXT NOT NULL,
  metadata TEXT
);

CREATE TABLE messages (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  role TEXT NOT NULL,             -- system / user / assistant / tool
  content TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  token_usage INTEGER,
  metadata TEXT
);

CREATE TABLE conversation_settings (
  conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
  temperature REAL,
  top_p REAL,
  max_tokens INTEGER,
  context_limit INTEGER,
  extra JSON
);
```

### 播放器
```sql
CREATE TABLE media_tracks (
  id TEXT PRIMARY KEY,
  path TEXT UNIQUE NOT NULL,
  title TEXT,
  album_id TEXT,
  artist_id TEXT,
  duration_ms INTEGER,
  bitrate INTEGER,
  metadata JSON
);

CREATE TABLE media_albums (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  artist_id TEXT,
  release_date TEXT,
  cover_hash TEXT,
  metadata JSON
);

CREATE TABLE media_artists (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  metadata JSON
);

CREATE TABLE playback_queue (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  track_id TEXT NOT NULL REFERENCES media_tracks(id) ON DELETE CASCADE,
  inserted_at INTEGER NOT NULL,
  position INTEGER NOT NULL
);

CREATE TABLE playback_state (
  id INTEGER PRIMARY KEY CHECK (id = 0),
  current_track TEXT REFERENCES media_tracks(id),
  position_ms INTEGER,
  volume REAL,
  repeat_mode TEXT,
  shuffle INTEGER
);
```

### 公共
```sql
CREATE TABLE api_credentials (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,      -- turso / openai / local-llm 等
  value TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE migrations (
  version TEXT PRIMARY KEY,
  applied_at INTEGER NOT NULL
);
```
- Turso 连接通过 `libsql-client` 或 `turso` SDK（Rust）。需要包装查询层，以便聊天/播放器模块共享。
- 原 `migrations/` SQL 迁移需要改写为兼容 Turso（保留同名目录，改用 `libsql` CLI 或 `sqld`）。

## 服务层计划
- 聊天：
  - `ChatService`：封装 LLM API 请求、流式响应（SSE/TCP chunk）解析。
  - `ConversationService`：对 Turso DB 提供 CRUD + 搜索接口。
  - `HistorySynchronizer`：负责将进行中的对话增量写入数据库，类似现在播放器保存播放状态逻辑。
- 播放器：
  - 替换 `sqlx::SqlitePool` 为 Turso 客户端；扫描线程扫描到文件后写入 Turso，而不是本地 SQLite。
  - 保留 `PlaybackThread`、`GPUIPlaybackInterface`，在队列变更时同步 `playback_queue` 表。

## 渐进式迁移路线
1. **底层重构**：引入 `shared::db::TursoPool`，提供 async 查询和简单迁移执行；新建聊天模块骨架。
2. **聊天 MVP**：实现会话列表 + 消息流 + API 调用通路（未集成播放器）。
3. **播放器迁移**：将 `library::db` / `scan` 改造为调用 Turso；保留 UI/线程逻辑不变。
4. **模块整合**：新建主窗口布局，加入播放器面板切换；统一快捷键。
5. **增量增强**：添加高级搜索、向量检索（待 Turso 向量模块稳定后落地）。

## 关键风险
- Turso SDK 对某些 SQL 特性不完全兼容，需要分批验证现有查询。
- 播放线程依赖实时性，确保数据库写入不会阻塞音频线程（可采用 channel + 后台写入策略）。
- 聊天流式响应需要精确同步 UI 状态，避免阻塞 gpui 主线程。

## 后续工作指引
1. 实现 `shared::db`，完成 Turso 连接/配置读取。
2. 搭建 `modules/chat` 骨架，定义 `ChatFacade` 与 UI 协议。
3. 创建 Turso 初始化脚本，替换原 `migrations/` 内容并验证。
4. 抽取 Hummingbird 播放器代码到 `modules/player`，保持功能完整。
5. 重新设计 `ui/app.rs` 的窗口布局，接入两个模块。
