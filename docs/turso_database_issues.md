## Turso Busy Handler 状态说明（2025-10-14）

> 本文档长期保留，用于提醒当前 Turso busy handler 的行为差异及本仓库的应对策略。请勿删除。

### 版本与补丁概况
- **现用 Turso / libSQL 版本**：v0.2.2--2025-10-08。  
- **关键补丁**：PR [tursodatabase/turso#3147](https://github.com/tursodatabase/turso/pull/3147) —— 修复 busy handler，实现指数回退。该补丁已经包含在 v0.2.2 中。

### 与原生 SQLite 的差异
| 项目 | SQLite 行为 | Turso v0.2.2 行为 |
|------|-------------|--------------------|
| `PRAGMA busy_timeout = N` | 成功返回 OK，并设置线性重试（每次等待 N 毫秒） | 若执行时数据库被锁，直接返回 `database is locked`，导致 busy handler 实际未开启（历史遗留）|
| Busy handler 重试节奏 | 线性等待 (N, N, N, …) | PR #3147 后采用指数节奏（1ms, 2ms, …，至多 100ms/phase，累计时限由 `busy_timeout` 控制） |
| 默认超时 | 0（需显式设置） | 0（仍需显式设置） |

> **结论**：仅调用 `PRAGMA busy_timeout` 无法可靠启用 Turso 的 busy handler，需要使用 Turso 绑定的 `Connection::busy_timeout(Duration)`。

### 本项目的应对策略
1. **连接初始化** (`src/db/mod.rs`)  
   - `PRAGMA journal_mode = WAL`：使用 `run_with_retry` 指数退避（最多 10 次，100ms~1500ms），确保 WAL 成功；失败仅 Warn。  
   - `busy_timeout(5s)`：改用 turso rust crate 的 `Connection::busy_timeout`，若仍遇锁仅记录 Warn，不 panic。
2. **扫描/写入逻辑**  
   - `ScanThread::update_metadata`：保留指数退避重试（最多 5 次），遇锁时等待 50ms, 100ms, …。  
   - 如日志反复出现 `WARN mrchat::db: Failed to set busy timeout...`，说明连接创建时间隔过短或有长事务，应减少扫描并发或排查其他进程。
3. **调试注意事项**  
   - 避免在多个实例同时写 Turso；若必须并发，确保 busy_timeout 生效（日志无 Warn）。  
   - 任何新的 Turso 版本发布后，需要验证 busy handler 行为是否仍有差异，必要时更新此文档。

### 后续工作（TODO）
- 监控日志，如仍有大量 `database is locked`，考虑在 `busy_timeout` 设置同样套用 `run_with_retry`。
- 跟踪 Turso 官方后续更新，确认 PRAGMA 在未来版本是否得到兼容修复。

---

## Turso Option 参数绑定 Panic 问题（2025-10-15）

> 本节记录 turso crate 在处理 Option 类型参数时的已知问题及解决方案。

### 问题描述
**症状**：当使用 turso crate 的 `query_one` 或类似方法传递包含 `Option<T>` 的元组参数时，程序在运行时 panic。

**错误信息**：
```
thread 'main' panicked at turso_core-0.2.2/types.rs:563:18:
internal error: entered unreachable code: invalid value type
```

**受影响版本**：turso v0.2.2, turso_core v0.2.2

### 根本原因
turso crate 的参数绑定机制无法正确处理元组中的 `Option<T>` 类型。具体表现：
- 当传递 `Option<i64>`、`Option<Vec<u8>>`、`Option<&str>` 等作为元组成员时
- turso_core 的 `FromValue` trait 实现在类型检查阶段进入 unreachable 分支
- 导致 panic，而不是正常处理 None/Some 语义

### 典型触发场景
```rust
// ❌ 这样会 panic
conn.query_one(
    "INSERT INTO album (...) VALUES ($1, $2, $3) RETURNING id",
    (
        album_name,           // String - OK
        artist_id,            // Option<i64> - PANIC!
        image_data,           // Option<Vec<u8>> - PANIC!
    ),
    |row| Ok(row.get::<i64>(0)?)
).await?;
```

### 解决方案模式

#### 方案 1：将 Option 转换为具体值 + SQL 层 NULL 转换（推荐）

**Rust 代码**：
```rust
// 将所有 Option 转为具体值
let artist_id_val = artist_id.unwrap_or(0);
let image_val = image_data.unwrap_or_else(Vec::new);
let label_val = label.as_deref().unwrap_or("");

// 使用 execute 代替 query_one
conn.execute(
    include_str!("query.sql"),
    (album_name, artist_id_val, image_val, label_val),
).await?;

// 获取插入的 ID
let id = conn.query_scalar::<i64>("SELECT last_insert_rowid()", ()).await?;
```

**SQL 文件（query.sql）**：
```sql
INSERT INTO album (name, artist_id, image, label)
VALUES (
    $1,
    NULLIF($2, 0),                           -- i64: 0 → NULL
    CASE WHEN length($3) = 0 THEN NULL ELSE $3 END,  -- Vec<u8>: 空 → NULL
    NULLIF($4, '')                           -- &str: 空串 → NULL
)
ON CONFLICT (name, artist_id) DO UPDATE SET
    image = EXCLUDED.image,
    label = EXCLUDED.label;
```

**优点**：
- 完全绕过 turso crate 的 Option 处理缺陷
- SQL 层面的 NULLIF/CASE 清晰表达语义
- 使用 DO UPDATE SET 确保冲突时也能获取 id

#### 方案 2：拆分查询，避免 Option 参数

将复杂插入拆分为多个简单查询，每个查询只传递非 Option 参数。

### 项目中的应用实例

1. **`src/library/scan.rs::insert_album` (行 509-546)**
   - 转换 7 个 Option 参数：artist_id, image, thumb, date, label, catalog, isrc
   - 使用 `execute` + `last_insert_rowid()` 代替 `query_one`
   - 对应 SQL：`queries/scan/create_album.sql`

2. **`src/library/scan.rs::insert_track` (行 600-629)**
   - 类似模式，转换 10 个参数
   - 使用内联 SQL 字符串 + execute

### 数据一致性注意事项

**⚠️ 旧数据损坏风险**：如果代码在修复前已运行过，数据库中可能存在损坏数据：
- Option 参数的值可能错位（如 release_date 字段存储了 image blob）
- 必须删除旧数据库并重新扫描：
  ```bash
  rm -f ~/.local/share/mrchat/music.db{,-wal,-shm}
  ```

### 验证方法

**检查是否存在数据损坏**：
```sql
-- release_date 应为 integer/null，不应为 blob
SELECT id, title, typeof(release_date) FROM album LIMIT 10;
```

如果看到 `typeof(release_date)` 返回 `blob`，说明数据已损坏，需要重建。

### 后续建议

1. **避免在元组参数中使用 Option**
   - 总是在 Rust 层转换为具体值
   - 在 SQL 层使用 NULLIF/CASE 处理 NULL 语义

2. **监控 turso crate 更新**
   - 跟踪 https://github.com/tursodatabase/turso-client-rust/issues
   - 如官方修复此问题，可考虑恢复直接使用 Option 参数

3. **添加集成测试**
   - 验证包含 NULL 值的插入/更新操作
   - 确保数据类型正确存储和读取

### 相关 Commits
- `c480ac1` - Fix turso crate Option parameter binding panic in album insertion (2025-10-15)
