<p align="center"><picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logo-wordmark.svg">
  <img src="assets/logo-wordmark-light.svg" alt="freenapse" width="360" height="80">
</picture></p>

# freenapse 中文版（免安装）

雷蛇鼠标的轻量替代品：一个约 1.2 MB 的单文件 exe，不用装雷云（Synapse），
DPI、回报率、按键、宏、传感器、电源和灯光设置全部存在鼠标自己的板载内存里。

本仓库是 [MatthewSaintBull/freenapse](https://github.com/MatthewSaintBull/freenapse) 的
**简体中文分支**（基于上游 v0.5.1，界面与提示全部汉化，逻辑与协议未改动）。
原版说明见 [README.en.md](README.en.md)。

---

## 直接使用（免安装）

1. 把 `freenapse.exe` 放到一个固定目录，例如 `C:\Tools\freenapse\`。
2. 双击运行：默认在托盘里常驻，右键托盘图标就有菜单；想看设置窗口，点菜单里的
   **设置…**，或者用命令行 `freenapse.exe ui`。
3. 想开机自动启动：设置页里打开 **开机启动**（只往注册表 `HKCU\...\Run` 写一条，不装服务）。

程序不写系统目录，也不需要管理员权限。删除 exe 就是卸载；
配置在 `%APPDATA%\freenapse\config.toml`，日志同样是这个目录。

> 鼠标未验证的型号默认**只读**。测试写入需要你在 **设备** 页手动打开
> 「允许写入这只鼠标」，风险自担。

## 汉化范围

- 设置窗口全部页面（自定义 / 性能 / 校准 / 电源 / 游戏 / 设备 / 路线图 / 设置）、
  侧栏、卡片说明、按钮、开关、状态提示、确认框、录制与宏编辑的说明文字。
- 托盘菜单与提示气泡：电量、休眠、DPI、回报率、游戏配置、更新提示。
- 命令行：`status`、`dpi`、`polling`、`buttons`、`macro`、`lighting`、`pair` 等
  子命令的输出与错误信息，以及 `--help`/用法说明。
- 保留英文的部分：鼠标型号名（Viper V3 Pro 等）、`Hypershift`、`DPI`、`Hz`、
  `profile:`/`key:`/`macro:` 这类配置键值、`probe` 探测报告（要贴到上游 GitHub
  issue，保持英文方便维护者对照）。

字体：界面字体优先用 **Microsoft YaHei UI / 微软雅黑**，缺字体时回退 Segoe UI；
图标仍用 Segoe Fluent Icons。

## 常用命令

```
freenapse.exe                # 托盘模式（等同 --tray）
freenapse.exe ui             # 打开设置窗口
freenapse.exe status         # 设备、电量、DPI 档位、回报率、传感器、配置
freenapse.exe dpi 800 1600   # 设置 1~5 个 DPI 档位
freenapse.exe polling 1000   # 回报率 125 / 500 / 1000 / 2000 / 4000 / 8000
freenapse.exe buttons        # 列出按键与当前分配
freenapse.exe apply -v       # 把配置文件里的每一项重新写入鼠标
freenapse.exe import         # 从鼠标读回设置，重建配置文件
freenapse.exe profile 2      # 切换板载配置
freenapse.exe macro list     # 宏列表（录制在设置窗口的“宏”区）
freenapse.exe lighting logo breathing
freenapse.exe pair           # 接收器配对
freenapse.exe probe          # 探测未知鼠标（输出贴到上游 issue）
```

加 `--log` 会在配置目录里写 `hid.log`，记录和鼠标的每一次通信。

## 从源码构建

需要 Rust 的 `stable-x86_64-pc-windows-gnu` 工具链和 mingw-w64
（`rust-toolchain.toml` 已固定目标；GNU 工具链在**不含空格**的路径下最稳）：

```
cargo build --release
# 产物：target\release\freenapse.exe
```

只改文案时也可以直接 `cargo test`，仓库自带 58 个单元测试。

## 目录里都有什么

| 文件 | 说明 |
|---|---|
| `src/ui/` | 全部界面：自绘的 Win32 窗口（GDI 绘制，无 web 引擎） |
| `src/device.rs` | 鼠标型号表（谁支持什么、产品 ID、DPI 上限、回报率命令） |
| `src/protocol.rs`、`src/hid.rs` | 雷蛇 HID 协议与传输 |
| `src/config.rs` | `config.toml` 的读写与校验 |
| `src/tray.rs` | 托盘图标、菜单、气泡 |
| `src/macros.rs`、`src/player.rs` | 宏的录制、编码与播放 |
| `FEATURES.md`、`ROADMAP.md` | 支持矩阵与路线图（英文，上游原文） |

## 致谢与许可

- 上游：[MatthewSaintBull/freenapse](https://github.com/MatthewSaintBull/freenapse)，MIT 许可。
- 鼠标协议与型号数据来自 [OpenRazer](https://github.com/openrazer/openrazer) 社区。
- 本分支只做中文本地化，未改动通信协议；如遇功能问题请先对照上游版本复现。
- 与雷蛇（Razer Inc.）及 OpenRazer 项目均无隶属关系。
