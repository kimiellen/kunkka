# Frontend Theme SDK

## 概述

Kunkka 提供统一的主题系统，支持 Catppuccin Latte（浅色）和 Macchiato（深色）两种主题。

上层应用 Frontend（TUI 应用、Browser Extension、CLI 工具）和底座管理工具（kunkka-cli、kunkka-tui）可以通过 IPC 协议查询和切换主题，并接收主题变更事件实现即时刷新。

## 主题配置

主题配置文件位于：

```text
$XDG_CONFIG_HOME/kunkka/theme.json
```

配置结构：

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
    }
  ]
}
```

## 主题协议

主题功能通过 `kunkka.core-control.v1` 协议提供。

### 查询当前主题

**请求：**

```rust
CoreControlMessage::GetTheme(CoreGetThemeRequest)
```

**响应：**

```rust
CoreControlMessage::GetThemeResult(CoreGetThemeResponse {
    flavor: ThemeFlavor::Macchiato,  // 或 ThemeFlavor::Latte
})
```

### 切换主题

**请求：**

```rust
CoreControlMessage::SetTheme(CoreSetThemeRequest {
    flavor: ThemeFlavor::Latte,
})
```

**响应：**

```rust
CoreControlMessage::SetThemeResult(CoreSetThemeResponse)
```

### 主题变更事件

当主题发生变化时，Core 会向所有已连接的 Frontend 发送 Event 帧：

```rust
CoreControlMessage::ThemeChanged(ThemeChangedEvent {
    flavor: ThemeFlavor::Latte,
})
```

**重要：** 主题变更事件只发送给 Frontend，不影响 Worker 的超时注销机制。

## 色板数据

每种主题包含 26 色 Catppuccin 色板：

| 颜色名 | Latte | Macchiato |
|--------|-------|-----------|
| rosewater | #dc8a78 | #f4dbd6 |
| flamingo | #dd7878 | #f0c6c6 |
| pink | #ea76cb | #f5bde6 |
| mauve | #8839ef | #c6a0f6 |
| red | #d20f39 | #ed8796 |
| maroon | #e64553 | #ee99a0 |
| peach | #fe640b | #f5a97f |
| yellow | #df8e1d | #eed49f |
| green | #40a02b | #a6da95 |
| teal | #179299 | #8bd5ca |
| sky | #04a5e5 | #91d7e3 |
| sapphire | #209fb5 | #7dc4e4 |
| blue | #1e66f5 | #8aadf4 |
| lavender | #7287fd | #b7bdf8 |
| text | #4c4f69 | #cad3f5 |
| subtext1 | #5c5f77 | #b8c0e0 |
| subtext0 | #6c6f85 | #a5adcb |
| overlay2 | #7c7f93 | #939ab7 |
| overlay1 | #8c8fa1 | #8087a2 |
| overlay0 | #9ca0b0 | #6e738d |
| surface2 | #acb0be | #5b6078 |
| surface1 | #bcc0cc | #494d64 |
| surface0 | #ccd0da | #363a4f |
| base | #eff1f5 | #24273a |
| mantle | #e6e9ef | #1e2030 |
| crust | #dce0e8 | #181926 |

## Frontend 集成指南

### TUI 应用（上层应用）

上层 TUI 应用应保持与 Core 的长连接，监听主题变更事件并即时刷新界面：

```rust
// 1. 连接 Core
let mut connection = connect_to_core(&socket_path).await?;

// 2. 查询当前主题
let response = send_control_message(&mut connection, CoreControlMessage::GetTheme(CoreGetThemeRequest)).await?;
let current_flavor = match response {
    CoreControlMessage::GetThemeResult(resp) => resp.flavor,
    _ => panic!("unexpected response"),
};

// 3. 应用主题到界面
apply_theme(current_flavor);

// 4. 监听主题变更事件
loop {
    let frame = connection.recv_frame().await?;
    if let Frame::Event { payload, .. } = frame {
        if let Ok(CoreControlMessage::ThemeChanged(event)) = decode_control_message(&payload) {
            apply_theme(event.flavor);
            // 立即刷新界面
            terminal.draw(|f| ui(f, &app))?;
        }
    }
}
```

### Browser Extension

Browser Extension 通过 `kunkka-native-host` 与 Core 通信：

```javascript
// 1. 查询当前主题
const response = await browser.runtime.sendNativeMessage("kunkka", {
    id: "theme-query",
    command: "get_theme"
});

// 2. 应用主题
applyTheme(response.result.flavor);

// 3. 监听主题变更（需要保持长连接）
const port = browser.runtime.connectNative("kunkka");
port.onMessage.addListener((message) => {
    if (message.type === "theme_changed") {
        applyTheme(message.flavor);
    }
});
```

## CLI 命令

### 查看当前主题

```bash
kunkka theme status
```

### 切换主题

```bash
kunkka theme switch latte
kunkka theme switch macchiato
kunkka theme toggle
```

### 设置定时切换

```bash
kunkka theme schedule --light 07:00 --dark 19:00
kunkka theme schedule --disable
```

## Hook 机制

系统应用（Hyprland、Kitty、Neovim 等）通过 Hook 脚本同步主题。

### Hook 环境变量

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

## 与 Worker 的关系

- **Frontend** 保持与 Core 的长连接，接收主题变更事件
- **Worker** 完全不感知主题，超时注销机制不受影响
- 主题变更事件只广播给 Frontend，不发送给 Worker
