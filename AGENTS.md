# Kunkka Agent Instructions

## 沟通与范围

- 仓库文档、代理说明和回复默认使用中文；只有文件格式或外部协议要求时才用英文。
- 变更保持最小；行为改动优先先写或先改测试。
- 这个仓库目前没有 `.github/workflows/`、`justfile`、`Makefile`、`.pre-commit-config.yaml` 或 `opencode.json`；不要猜任务入口，直接使用 Cargo。

## 文档先行

- 改源码前，先把对应设计或计划落到仓库里，不要把聊天记录当作设计依据。
- 当前已落地的文档约定是：设计放在 `docs/superpowers/specs/YYYY-MM-DD-*-design.md`，实施计划放在 `docs/superpowers/plans/YYYY-MM-DD-*.md`。
- 可执行事实优先于 prose：根 `Cargo.toml`、crate 清单和源码入口，比 `docs/architecture.md` 里的目标布局更可信。

## 当前工作区

- 以根 `Cargo.toml` 为准，当前 workspace 只有 6 个成员：`kunkka-ipc`、`kunkka-protocol`、`kunkka-core`、`kunkka-worker-sdk`、`kunkka-native-host`、`kunkka-cli`。
- `docs/architecture.md` 里的 `kunkka-tui`、`apps-backend/`、`apps-frontend/`、`xtask/` 仍是目标布局，不是当前可改的现成目录。
- workspace 基线是 Rust 2021 edition，`rust-version = 1.80`。

## 常用命令

- 全量验证按开发日志里的固定顺序跑：`cargo fmt --all --check` -> `cargo test --workspace` -> `cargo clippy --workspace --all-targets -- -D warnings`
- 单 crate 验证：`cargo test -p kunkka-core`、`cargo test -p kunkka-cli`、`cargo test -p kunkka-native-host`
- 焦点集成测试示例：`cargo test -p kunkka-core --test frontend_dispatch_runtime`、`cargo test -p kunkka-cli --test integration`、`cargo test -p kunkka-native-host --test host_loop`
- 运行入口：`cargo run -p kunkka-core`、`cargo run -p kunkka-cli -- ping`、`cargo run -p kunkka-cli -- dispatch --app notes --method search --payload '{"query":"kunkka"}'`、`cargo run -p kunkka-native-host`
- `kunkka-cli` 的二进制名是 `kunkka`；不要去找单独的顶层 CLI 工具目录。

## 真实边界

- Kunkka 是本地能力平台，不是单一 CLI 或浏览器扩展；Browser Extension、CLI 和未来 TUI 都应该通过 core / worker 访问能力。
- `kunkka-ipc` 只放 frame、transport、opaque payload、codec 和 IPC error；不要把 typed business protocol 或权限逻辑塞进去。
- `kunkka-protocol` 承载共享 typed protocol；当前已实现的 schema 是 `kunkka.core-control.v1` 和 `kunkka.frontend-dispatch.v1`。
- `kunkka-core` 拥有 XDG 路径、runtime socket、app manifest 加载、SQLite/sqlx core 数据库、worker startup/dispatch、权限决策。
- `kunkka-worker-sdk` 拥有 worker registration / dispatch protocol 与客户端辅助。
- `kunkka-native-host` 只桥接 `WebExtension Native Messaging JSON <-> Kunkka IPC`；不能实现业务逻辑或权限决策。
- Browser Extension 进入本地系统只能通过 `kunkka-native-host`，不能直接连 Unix Domain Socket。

## 运行时约束

- 所有 config/data/state/cache/runtime 路径都必须走 XDG；禁止默认落到 `~/.kunkka`、`./.kunkka`、`./data`、`/tmp/kunkka`。
- runtime socket 路径是 `$XDG_RUNTIME_DIR/kunkka/core.sock`；没有 `XDG_RUNTIME_DIR` 时回退到 `/tmp/kunkka-runtime-<uid>/core.sock`，且目录权限必须是 `0700`。
- `kunkka-cli` 和 `kunkka-native-host` 都自己按上面的规则解析 socket；`kunkka-native-host` 不会自动拉起 core。
- core 数据库固定在 `$XDG_DATA_HOME/kunkka/kunkka.db`，migrations 由 `crates/kunkka-core/migrations/` 里的 SQL 通过 `sqlx::migrate!()` 嵌入执行。
- app manifest 从 `$XDG_CONFIG_HOME/kunkka/apps/*.json` 加载，dispatch 路由键是 manifest 内的 `app_id`，不是文件名。
- core 拉起 worker 时会注入 `KUNKKA_CORE_SOCKET`、`KUNKKA_APP_ID`、`KUNKKA_WORKER_ID`。
- frontend dispatch 权限在 `kunkka-core` 内按 `permissions.frontend_dispatch.allowed_methods` 做精确匹配；缺失、空列表或未命中都等于 deny all，且大小写敏感。
- core runtime 当前按 `Payload.schema` 分发：`kunkka.worker.v1` 给 worker registration，`kunkka.core-control.v1` 给 ping/status，`kunkka.frontend-dispatch.v1` 给 frontend dispatch。

## 测试习惯

- 现有 integration tests 普遍自己构造临时 XDG 目录并在进程内启动 `prepare_core_runtime()`；通常不依赖外部服务或真实系统状态。
- 行为改动优先在所属 crate 的 `tests/*.rs` 里补或改集成测试，尤其是 `kunkka-core`、`kunkka-cli`、`kunkka-native-host` 这三个 crate。
- 集成测试里有一个反复出现的 `test_paths()` 辅助函数，构造 `tempdir` + `KunkkaPaths`；新测试应复用同一模式，不要硬编码系统路径。
- `kunkka-cli` 和 `kunkka-native-host` 的 dev-dependencies 依赖 `kunkka-core` 和 `kunkka-worker-sdk`，用于在测试里构造完整 runtime 场景。
- 新增 migration 放在 `crates/kunkka-core/migrations/`，文件名按序号递增（`0001_*`、`0002_*` …），用 `sqlx::migrate!()` 嵌入；不需要 `SQLX_OFFLINE` 或 `.sqlx/` 目录。

## SDK 文档

上层应用开发的 SDK 文档位于 `docs/SDK/`：

- `docs/SDK/worker.md` — Worker 后端开发指南
- `docs/SDK/worker-llm-usage.md` — Worker 侧 LLM Capability 使用指南
- `docs/SDK/browser-extension.md` — Browser Extension 开发指南
- `docs/SDK/frontend-theme.md` — Frontend 主题系统集成指南
- `docs/SDK/ipc.md` — IPC 协议规范
- `docs/SDK/permissions.md` — 权限系统说明
- `docs/SDK/storage.md` — 存储路径规范

### 能力层文档

- `docs/SDK/capability-fs.md` — 文件系统能力
- `docs/SDK/capability-http.md` — HTTP 能力
- `docs/SDK/capability-sqlite.md` — SQLite 能力
- `docs/SDK/capability-shell.md` — Shell 能力
- `docs/SDK/capability-llm.md` — LLM 能力

### SDK 文档铁律

每次开发任务完成后，必须评估是否需要更新或新建 SDK 文档，确保文档内容始终与实现保持同步。具体检查点：

1. 新增或修改了 capability → 更新或创建对应的 `docs/SDK/capability-*.md`
2. 新增或修改了协议消息 → 更新 `docs/SDK/ipc.md`
3. 新增或修改了权限逻辑 → 更新 `docs/SDK/permissions.md`
4. 新增或修改了 worker 相关逻辑 → 更新 `docs/SDK/worker.md`
5. 新增或修改了存储路径 → 更新 `docs/SDK/storage.md`
6. 新增了上层应用开发相关的功能 → 创建对应的 SDK 文档
