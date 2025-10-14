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
