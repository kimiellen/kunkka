# Theme System 设计

## [S1] 概述

Kunkka theme system 提供统一的主题切换能力，覆盖：

- Kunkka 自研应用（TUI、Browser Extension、CLI）
- 系统应用（Hyprland、Kitty、Neovim、Chrome 等）

主题方案使用 Catppuccin 的 Latte（浅色）和 Macchiato（深色）。

核心设计原则：

1. **Core 作为主题状态的单一真相源**
2. **Frontend 通过 IPC event 长连接即时刷新**
3. **Worker 完全不感知主题，超时注销机制不受影响**
4. **系统应用通过用户定义的 hook 脚本同步**

## [S2] Theme 配置文件

### 文件位置

```
$XDG_CONFIG_HOME/kunkka/theme.json
```

### 数据结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// 当前激活的主题 flavor
    pub active_flavor: ThemeFlavor,
    /// 主题切换调度（可选）
    pub schedule: Option<ThemeSchedule>,
    /// 系统应用同步钩子列表
    pub hooks: Vec<ThemeHook>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeFlavor {
    Latte,
    Macchiato,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSchedule {
    /// 浅色主题开始时间（HH:MM 格式，24小时制）
    pub light_at: String,
    /// 深色主题开始时间（HH:MM 格式，24小时制）
    pub dark_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeHook {
    /// 钩子名称（用于日志和错误报告）
    pub name: String,
    /// 钩子脚本路径
    pub script: String,
}
```

### 示例配置

```json
{
  "active_flavor": "macchiato",
  "schedule": {
    "light_at": "07:00",
    "dark_at": "19:00"
  },
  "hooks": [
    {
      "name": "hyprland",
      "script": "~/.config/kunkka/hooks/hyprland.sh"
    },
    {
      "name": "kitty",
      "script": "~/.config/kunkka/hooks/kitty.sh"
    }
  ]
}
```

## [S3] Catppuccin 色板数据

### 内嵌色板

Core 内嵌 Catppuccin Latte 和 Macchiato 的完整 26 色色板，作为颜色值的权威来源。

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatppuccinPalette {
    pub rosewater: String,
    pub flamingo: String,
    pub pink: String,
    pub mauve: String,
    pub red: String,
    pub maroon: String,
    pub peach: String,
    pub yellow: String,
    pub green: String,
    pub teal: String,
    pub sky: String,
    pub sapphire: String,
    pub blue: String,
    pub lavender: String,
    pub text: String,
    pub subtext1: String,
    pub subtext0: String,
    pub overlay2: String,
    pub overlay1: String,
    pub overlay0: String,
    pub surface2: String,
    pub surface1: String,
    pub surface0: String,
    pub base: String,
    pub mantle: String,
    pub crust: String,
}
```

### 色板查询 API

```rust
impl ThemeFlavor {
    pub fn palette(&self) -> &'static CatppuccinPalette { ... }
}
```

## [S4] Core 主题状态管理

### ThemeManager

Core 内部新增 `ThemeManager`，负责：

1. 加载/保存 `theme.json`
2. 维护当前活跃 flavor
3. 提供色板查询
4. 执行定时切换

```rust
pub struct ThemeManager {
    config: ThemeConfig,
    config_path: PathBuf,
}

impl ThemeManager {
    pub fn load(paths: &KunkkaPaths) -> Result<Self> { ... }
    pub fn active_flavor(&self) -> ThemeFlavor { ... }
    pub fn palette(&self) -> &'static CatppuccinPalette { ... }
    pub fn switch_flavor(&mut self, flavor: ThemeFlavor) -> Result<()> { ... }
    pub fn config(&self) -> &ThemeConfig { ... }
    pub fn update_config(&mut self, config: ThemeConfig) -> Result<()> { ... }
    pub fn check_schedule(&mut self) -> Option<ThemeFlavor> { ... }
}
```

### 集成到 CoreRuntime

```rust
pub struct CoreRuntime {
    // ... existing fields ...
    theme_manager: ThemeManager,
}
```

Core runtime loop 新增定时检查主题调度：

```rust
loop {
    tokio::select! {
        accepted = self.server.accept_one() => { ... }
        _ = reap_interval.tick() => { ... }
        _ = theme_check_interval.tick() => {
            if let Some(new_flavor) = self.theme_manager.check_schedule() {
                self.broadcast_theme_change(new_flavor).await;
            }
        }
    }
}
```

## [S5] 主题事件广播

### 协议扩展

在 `kunkka.core-control.v1` 中新增主题相关消息：

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeChangedEvent {
    pub flavor: ThemeFlavor,
    pub palette: CatppuccinPalette,
}

// CoreControlMessage 新增变体
pub enum CoreControlMessage {
    // ... existing variants ...
    ThemeChanged(ThemeChangedEvent),
    GetTheme(CoreGetThemeRequest),
    GetThemeResult(CoreGetThemeResponse),
    SetTheme(CoreSetThemeRequest),
    SetThemeResult(CoreSetThemeResponse),
}
```

### 广播机制

Core 维护已连接 frontend 的连接列表，主题变更时向所有 frontend 发送 Event 帧：

```rust
impl CoreRuntime {
    async fn broadcast_theme_change(&self, flavor: ThemeFlavor) {
        let event = ThemeChangedEvent {
            flavor,
            palette: flavor.palette().clone(),
        };
        // 向所有已连接的 frontend 发送 Event 帧
        for conn in self.frontend_connections.iter() {
            conn.send_frame(&Frame::Event { ... }).await;
        }
    }
}
```

### Frontend 连接管理

Core 需要跟踪已连接的 frontend，用于事件广播：

```rust
pub struct CoreRuntime {
    // ... existing fields ...
    frontend_connections: Vec<IpcConnection>,
}
```

## [S6] CLI 命令

### 查看当前主题

```bash
kunkka theme status
```

输出：
```
Active flavor: macchiato
Schedule: light at 07:00, dark at 19:00
Hooks: hyprland, kitty
```

### 切换主题

```bash
kunkka theme switch latte
kunkka theme switch macchiato
kunkka theme toggle  # 在 latte 和 macchiato 之间切换
```

### 设置定时切换

```bash
kunkka theme schedule --light 07:00 --dark 19:00
kunkka theme schedule --disable
```

### 管理 hooks

```bash
kunkka theme hooks list
kunkka theme hooks add --name hyprland --script ~/.config/kunkka/hooks/hyprland.sh
kunkka theme hooks remove hyprland
```

## [S7] Hook 执行机制

### 执行时机

1. 主题切换后（手动或定时）
2. Core 依次执行所有配置的 hooks
3. Hook 失败不阻断其他 hook 执行

### 环境变量

Hook 脚本接收以下环境变量：

| 变量 | 说明 |
|------|------|
| `KUNKKA_THEME_FLAVOR` | 当前 flavor（`latte` 或 `macchiato`） |
| `KUNKKA_THEME_PALETTE_JSON` | 完整色板 JSON |

### Hook 脚本示例

```bash
#!/bin/bash
# ~/.config/kunkka/hooks/kitty.sh

if [ "$KUNKKA_THEME_FLAVOR" = "latte" ]; then
    kitty @ set-colors --all ~/.config/kitty/catppuccin-latte.conf
else
    kitty @ set-colors --all ~/.config/kitty/catppuccin-macchiato.conf
fi
```

## [S8] 实现架构

### 新增文件

- `crates/kunkka-core/src/theme/mod.rs` — ThemeManager、ThemeConfig、ThemeFlavor
- `crates/kunkka-core/src/theme/palette.rs` — Catppuccin 色板数据

### 修改文件

- `crates/kunkka-core/src/lib.rs` — 添加 `pub mod theme`
- `crates/kunkka-core/src/runtime.rs` — 集成 ThemeManager、主题广播
- `crates/kunkka-protocol/src/core_control.rs` — 新增主题相关消息
- `crates/kunkka-cli/src/cli.rs` — 新增 `theme` 子命令

### 测试文件

- `crates/kunkka-core/tests/theme_manager.rs` — ThemeManager 单元测试
- `crates/kunkka-cli/tests/theme_cli.rs` — CLI 主题命令集成测试

## [S9] 安全考虑

- 主题配置文件存放在用户级 XDG 目录，权限由系统保证
- Hook 脚本以用户权限执行，Core 不提权
- Hook 脚本路径不支持变量展开（`~` 由 shell 展开）
- 主题切换不涉及敏感数据

## [S10] 不做的事

- 不支持自定义色板（只支持 Catppuccin Latte/Macchiato）
- 不支持 per-app 主题（全局统一）
- 不支持主题继承或覆盖
- Hook 不支持超时控制（第一刀）
- Hook 不支持并行执行（第一刀串行）
