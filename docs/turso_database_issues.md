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

---

## Turso 混合类型参数绑定 Bug（2025-10-15）

> **严重度：CRITICAL** - 影响所有混合类型参数的插入操作，导致数据损坏

### 问题发现

在修复 Option 参数绑定问题后，进一步测试发现 turso crate 0.2.2 存在更严重的参数绑定缺陷：**无法可靠处理混合类型的元组参数**。

### 源代码调查

对 turso 和 turso_core 源代码进行了深入分析：

**turso crate (v0.2.2) - params.rs**：
```rust
// 使用宏生成 1-16 参数的 tuple 实现
macro_rules! tuple_into_params {
    ($count:literal : $(($field:tt $ftype:ident)),* $(,)?) => {
        impl<$($ftype,)*> IntoParams for ($($ftype,)*)
        where $($ftype: IntoValue,)* {
            fn into_params(self) -> Result<Params> {
                let params = Params::Positional(vec![$(self.$field.into_value()?),*]);
                Ok(params)
            }
        }
    }
}
```

**turso crate - value.rs**：
```rust
impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Value {
        Value::Blob(value)  // 看起来正确
    }
}
```

**turso_core (v0.2.2) - types.rs**：
```rust
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Integer(i64),
    Float(f64),
    Text(Text),
    Blob(Vec<u8>),
}
```

**结论**：从代码层面看实现正确，但实际运行时参数绑定存在严重 bug。

### Bug #1: BLOB + 其他类型混合导致参数错位

**症状**：
```rust
// ❌ 参数顺序完全打乱
conn.execute(
    "INSERT INTO album (title, artist_id, release_date, image, thumb, label, mbid) VALUES ...",
    (
        album_title,        // String
        artist_id,          // i64
        release_date,       // i64
        image_data,         // Vec<u8> - BLOB
        thumbnail,          // Vec<u8> - BLOB
        label,              // String
        mbid,              // String
    ),
).await?;
```

**实际后果**：
- `release_date` (INTEGER 列) 被写入了 BLOB 图片数据
- 数据库查询：`SELECT typeof(release_date) FROM album` 返回 `blob`
- 其他字段也可能错位

**验证方法**：
```bash
sqlite3 ~/.local/share/mrchat/music.db << 'EOF'
SELECT id, title, typeof(release_date), length(image), length(thumb)
FROM album LIMIT 5;
EOF
# 期望：typeof(release_date) = 'integer' 或 'null'
# 实际（bug）：typeof(release_date) = 'blob'
```

**Workaround（已实现）**：
```rust
// ✅ 分两步：先插入非BLOB字段，后UPDATE BLOB字段
// Step 1: 仅String和i64参数
let insert_sql = "INSERT INTO album (title, title_sortable, artist_id, release_date, label, catalog_number, isrc, mbid)
    VALUES (?, ?, NULLIF(?, 0), NULLIF(?, 0), NULLIF(?, ''), NULLIF(?, ''), NULLIF(?, ''), ?)";
conn.execute(insert_sql, (title, title_sort, artist_id, date, label, catalog, isrc, mbid)).await?;

let id = conn.query_scalar::<i64>("SELECT last_insert_rowid()", ()).await?;

// Step 2: 单独UPDATE BLOB字段
if resized_image.is_some() || thumb.is_some() {
    let update_sql = "UPDATE album SET image = CASE WHEN length(?) = 0 THEN NULL ELSE ? END,
                                        thumb = CASE WHEN length(?) = 0 THEN NULL ELSE ? END
                      WHERE id = ?";
    conn.execute(update_sql, (image.clone(), image, thumb.clone(), thumb, id)).await?;
}
```

**结果**：✅ Album 插入成功（49 albums）

### Bug #2: String + i64 混合导致参数错位

**症状**：即使不包含 BLOB，混合 String 和 i64 类型也会导致参数错位。

**测试案例 1 - 8 参数（4 String + 4 i64）**：
```rust
// ❌ 失败 - location显示为数字
conn.execute(
    "INSERT INTO track (title, title_sortable, location, album_id, track_number, disc_number, duration, folder)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    (
        name.clone(),       // String - $1
        name.clone(),       // String - $2
        path_str.clone(),   // String - $3
        album_id,           // i64 - $4
        track_num,          // i64 - $5
        disc_num,           // i64 - $6
        duration,           // i64 - $7
        folder,             // String - $8
    ),
).await?;
```

**实际后果**：
```sql
SELECT id, title, location, duration FROM track LIMIT 3;
-- 期望：location = "/home/vc/music/..."
-- 实际：location = "355"  (实际是duration的值！)
```

**测试案例 2 - 4 参数（3 String + 1 i64 at end）**：
```rust
// ❌ 仍然失败
conn.execute(
    "INSERT INTO track (title, title_sortable, location, duration) VALUES ($1, $2, $3, $4)",
    (
        name.clone(),       // String - $1
        name.clone(),       // String - $2
        path_str.clone(),   // String - $3
        duration,           // i64 - $4 (只有一个i64，放在最后)
    ),
).await?;
```

**结果**：❌ 仍然参数错位，0 tracks 插入成功

**尝试的其他组合**：
- 全 String 参数，i64 用 SQL 字面值 → 未测试（但应该会牺牲性能和安全性）
- 完全分离类型到不同 SQL 语句 → 正在尝试

### 根本问题分析

**推测的原因**：
1. turso_core 的参数绑定实现在处理不同类型时存在内存布局或序列化问题
2. 可能与类型的内存大小不一致有关（String 是指针+长度，i64 是固定8字节）
3. 元组参数的序列化/反序列化过程中类型信息丢失或错位

**影响范围**：
- ✅ 纯 String 参数 - 可能正常
- ✅ 纯 i64 参数 - 可能正常
- ❌ String + i64 混合 - 错位
- ❌ BLOB + 任何类型 - 严重错位

### 当前状态总结

| 操作 | 参数类型 | 状态 | 记录数 |
|------|---------|------|--------|
| insert_artist | 2 String | ✅ 成功 | 42 artists |
| insert_album (基本字段) | 8 混合 (String + i64) | ⚠️ 用 workaround | 49 albums |
| insert_album (BLOB字段) | 4 Vec<u8> + 1 i64 | ⚠️ 用 workaround | 49 albums |
| insert_track | 8 混合 (String + i64) | ❌ 失败 | 0 tracks |

### 建议的解决路径

#### 短期方案（紧急）：
1. **完全避免混合类型参数**
   ```rust
   // Step 1: 仅 String 参数
   conn.execute("INSERT INTO track (title, location, folder) VALUES (?, ?, ?)",
                (title, location, folder)).await?;

   // Step 2: 仅 i64 参数（WHERE 用字面值）
   let update_sql = format!("UPDATE track SET album_id = ?, duration = ? WHERE location = '{}'",
                            location.replace("'", "''"));
   conn.execute(&update_sql, (album_id, duration)).await?;
   ```

2. **使用 SQL 字面值（安全性需注意）**
   ```rust
   let sql = format!("INSERT INTO track (...) VALUES ('{}', {}, {})",
                     title.replace("'", "''"), album_id, duration);
   conn.execute(&sql, ()).await?;
   ```

#### 中期方案：
1. **提交 Issue 到 turso 项目**
   - Repository: https://github.com/tursodatabase/turso-client-rust
   - 包含最小可复现示例
   - 附上本文档的调查结果

2. **寻找替代方案**
   - 考虑使用标准 `rusqlite` + Turso embedded replica
   - 或使用 HTTP API 而非本地 embedded

#### 长期方案：
1. 等待官方修复
2. 或考虑贡献 PR 修复 turso_core 的参数绑定实现

### 数据完整性检查清单

修复后必须执行：
```bash
# 1. 删除旧数据库
rm -f ~/.local/share/mrchat/music.db{,-wal,-shm}
rm -f ~/.local/share/mrchat/scan_record.json

# 2. 重新扫描
cargo run --release

# 3. 验证数据
sqlite3 ~/.local/share/mrchat/music.db << 'EOF'
-- 检查 artists
SELECT COUNT(*) FROM artist;

-- 检查 albums（不应有 blob 类型的 release_date）
SELECT COUNT(*) FROM album;
SELECT id, title, typeof(release_date), typeof(image), typeof(thumb)
FROM album LIMIT 5;

-- 检查 tracks（location 应为完整路径字符串）
SELECT COUNT(*) FROM track;
SELECT id, title, substr(location, 1, 30), duration, album_id
FROM track LIMIT 5;
EOF
```

### 相关 Commits
- `f6a4b9e` - Fix turso crate parameter binding bugs (2025-10-15)
  - 实现 album BLOB workaround（成功）
  - 尝试多种 track 混合类型 workaround（失败）
  - 详细记录调查过程

### 参考资料
- turso-client-rust: https://github.com/tursodatabase/turso-client-rust
- turso_core params.rs: https://github.com/tursodatabase/turso-client-rust/blob/main/crates/core/src/params.rs
- 相关讨论（待创建 issue）
