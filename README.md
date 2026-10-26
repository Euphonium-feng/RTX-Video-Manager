<div align="center">

<img src="assets/rtx-video-manager.svg" width="168" alt="RTX Video Manager icon">

# RTX Video Manager

**一块控制台，接管 RTX Video HDR + VSR。不是更多面板，而是更快地做对视觉决定。**

<p>
  <img alt="build" src="https://github.com/Euphonium-feng/RTX-HDR-Switch/actions/workflows/build.yml/badge.svg">
  <img alt="Windows" src="https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4?style=for-the-badge&logo=windows11&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-Win32%20%2B%20GPUI-B7410E?style=for-the-badge&logo=rust&logoColor=white">
  <img alt="No WebView" src="https://img.shields.io/badge/WebView-none-65DDE6?style=for-the-badge">
</p>

`Ctrl+Alt+H` 切换 HDR · `Ctrl+Alt+V` 切换 VSR · `Ctrl+Alt+G` 唤起/收起控制台 · 500 ms 浮层 · 双通道严格回滚

<img src="assets/rtx-video-manager.png" width="180" alt="RTX Video Manager dual-channel icon">

</div>

---

## 为什么要做这个工具？

RTX Video HDR 对真人电影、HDR 预告片和游戏实机画面很有吸引力，但它不是万能滤镜。
2D 动画、动态图形、AE 合成、大面积纯白背景和强 UI 画面经常会被“提亮过头”：

- 白色字幕和小图形出现过曝或边缘发灰。
- 原本平面的动画出现不自然的明暗层次。
- 对比度被拉高后，线稿和色块不再干净。
- 去 NVIDIA App 里翻 `System → Video → HDR`，再切回 Chrome，操作链太长。

这就不合理了。一个高频、瞬时、情境化的视觉判断，不应该要求你离开视频页面、
找到显卡控制面板、翻开多层设置，然后再回来继续看。

> **核心痛点：HDR 和超分是否合适是“逐视频”的，但两个入口却都藏在“系统级”驱动面板里。**

RTX Video Manager 不尝试猜测内容类型，也不做脆弱的自动分类。它把判断权留给你，
把 HDR、VSR 和质量档位的操作成本压到一次快捷键。

---

## 30 秒认识它

| 你可能正在经历 | 使用之后 |
| --- | --- |
| 为了关 HDR，打开 NVIDIA App 翻三层菜单 | `Ctrl+Alt+H`，一次按键 |
| 想让动画改用更高档 VSR，又要翻驱动设置 | `Ctrl+Alt+V`，再用控制台选 Auto / 1–4 |
| 想改 HDR 峰值或调色，又要重新翻驱动面板 | `Ctrl+Alt+G` 打开控制台，滑块与数值直接调 |
| 控制台挡在视频前，还要去点关闭 | 控制台在前台时再按 `Ctrl+Alt+G`，立即收起 |
| 不确定当前设置到底是开还是关 | 托盘图标和控制台实时显示状态 |
| 切完不知道有没有生效 | 500 ms 屏幕下方浮层即时反馈 |
| 担心修改 HDR 弄坏其他调校参数 | 严格读—改—写—回读—回滚事务 |
| 想看诊断信息又被大而全的驱动面板淹没 | 专用 GPUI 控制台只展示相关指标 |
| 想同时管理 HDR 与 VSR，但不想常驻 WebView | 轻量 Win32 宿主 + 按需 GPUI 双功能控制台 |

---

## 功能地图

```text
┌──────────────────────────────────────────────────────────────┐
│                        Windows 桌面                          │
│                                                              │
│   Ctrl+Alt+H ─────┤                                          │
│   Ctrl+Alt+V ─────┤                                          │
│   Ctrl+Alt+G ─────┤                                          │
│                    ▼                                          │
│   ┌─────────────────────────┐       ┌────────────────────┐   │
│   │ RtxVideoManager.exe     │◀─────▶│ RtxVideoManager-   │   │
│   │                         │       │ Console.exe        │   │
│   │ Win32 resident host     │ IPC   │ GPUI console       │   │
│   └───────────┬─────────────┘       └────────────────────┘   │
│               │                                                │
│       ┌───────┴────────┐                                       │
│       ▼                ▼                                       │
│  托盘子菜单        500 ms 原生浮层                             │
│       │                                                        │
│       ▼                                                        │
│  NVIDIA UXD / HDR 0x318 / VSR 0x315+0x317                    │
└──────────────────────────────────────────────────────────────┘
```

### 宿主能力

- 默认快捷键 `Ctrl+Alt+H` 切换 HDR，`Ctrl+Alt+V` 切换 VSR，`Ctrl+Alt+G` 唤起或收起控制台，均可重新录制。
- 通知区域常驻，左键打开控制台，右键打开原生菜单。
- 开机启动默认开启，可随时关闭。
- 切换成功后在当前显示器底部居中显示 **500 ms**、50% 透明度的 HDR/VSR 状态提示。
- 托盘图标区分开启、关闭和不可用状态。
- Explorer 重启后自动重建托盘图标。

### 控制台能力

- 大号 HDR 状态，以及来自驱动 `0x318` 的峰值、峰值上下限、中间灰、对比度和饱和度。
- 直接开启/关闭 RTX Video HDR。
- 直接开启/关闭 RTX VSR，并在 `Auto`、`1`、`2`、`3`、`4` 之间选择质量。
- `总览 / HDR / VSR / 设置 / 诊断` 五页结构；总览同时显示两个增强通道。
- 峰值亮度、中间灰、对比度、饱和度均支持滑块和直接数值输入。
- `还原`无论何时点击都恢复驱动默认组合：峰值上限、中间灰 50、对比度 0、饱和度 0。
- HDR 调校卡片会撑满剩余页面高度并保持圆角包裹，底部状态行不会被页脚截断。
- 三个全局快捷键均可可视化录制，且任意两个不能重复。
- 开机启动与切换浮层开关。
- NVIDIA DLL 路径、允许哈希、错误码、HDR 调校字段、VSR 原始 `0x315/0x317` 与最近错误。
- 关闭窗口即退出 GPUI，释放渲染资源；托盘宿主继续工作。

---

## 快速开始

### 1. 获取发布包

从 GitHub Actions 最新绿色 `build` run 下载：

```text
RtxVideoManager-win-x64.zip
```

### 2. 完整解压

压缩包内至少需要这两个文件处在同一目录：

```text
RtxVideoManager.exe
RtxVideoManagerConsole.exe
```

### 3. 启动宿主

运行 `RtxVideoManager.exe`。首次启动会打开控制台，并默认写入当前用户开机启动项。

### 4. 开始切换

在 Chrome 播放 SDR 视频时按：

```text
Ctrl + Alt + H
```

看到屏幕下方出现 `RTX HDR 开启` 或 `RTX HDR 关闭` 即表示本次事务成功。

也可按 `Ctrl + Alt + V` 独立切换 VSR。退出工具不会主动改变当前 HDR/VSR 状态；
关闭控制台也不会退出后台宿主。

---

## 安全设计，不是“调个私有 DLL”这么简单

RTX Video HDR 没有 NVIDIA 官方公开 CLI 或支持的公共 API。本项目直接操作 NVIDIA App
使用的私有 `nvcpl.dll` 设置 `0x318`，因此对写入门槛设置了多层硬门：

| 门禁 | 行为 |
| --- | --- |
| 固定路径 | 只接受 `%ProgramW6432%\NVIDIA Corporation\NVIDIA App\NvCpl\nvcpl.dll` |
| 固定 SHA-256 | `eee812ab8ff74975d3cc7fe77130f5766911b6ca2d9c42c1156376a6c13f79f0` |
| Authenticode | 签名验证失败直接只读 |
| 必需导出 | Init / Get / Set / Close / Execute / UXD 服务检查 |
| STA COM | NVIDIA worker 独立初始化 STA |
| UXD 服务 | 服务未运行时禁止写入 |
| 48 字节读探针 | 字段或错误码异常时禁止写入 |
| VSR 双读探针 | `0x315` 与 `0x317` 档位一致后才允许操作 |
| Set + Execute | 写入后执行 `0x10000` 提交事务 |
| 回读确认 | 目标状态与六个调校字段逐项核对 |
| VSR 固定字段 | 只比较 support、quality、status，不把易变尾部当写条件 |
| 自动回滚 | HDR 恢复原始 48 字节，VSR 恢复并重新提交原档位 |

### 事务流程

```mermaid
flowchart LR
    A[读取 0x318 原始 48 字节] --> B{支持 HDR？}
    B -- 否 --> X[拒绝写入]
    B -- 是 --> C[只反转 enabled 位]
    C --> D[NvCplApiSetSetting]
    D --> E[NvCplApiExecute 0x10000]
    E --> F[重新读取]
    F --> G{状态生效且调校字段一致？}
    G -- 是 --> H[发布新快照]
    G -- 否 --> I[写回原始字节并再次验证]
    I --> J[成功恢复：报告原错误]
    I --> K[恢复失败：严重错误]
```

### VSR 事务

```mermaid
flowchart LR
    A[读取 0x315 + 0x317] --> B{支持且档位一致？}
    B -- 否 --> X[拒绝写入]
    B -- 是 --> C[只写 0x317 四字节档位]
    C --> D[SetSetting + Execute]
    D --> E[回读 0x315 / 0x317]
    E --> F{目标档位一致且 support 未变？}
    F -- 是 --> G[发布完整 VSR 快照]
    F -- 否 --> H[恢复原档位并再次提交]
    H --> I[成功恢复：报告原错误]
    H --> J[恢复失败：严重错误]
```

`Auto=5`，`Off=0`，画质档位 `1..=4`。VSR 关闭时改档位只改变“下次启用质量”，
不会调用 NVIDIA 写接口。不会访问已知会触发访问冲突的 `0x31b`。

**不修改 NVIDIA 驱动注册表。不自动化点击 NVIDIA App。不切换 Windows HDR。**

---

## 架构：轻宿主，按需前端

```mermaid
flowchart TB
    subgraph Native["Win32 / Rust host"]
        H[Message loop]
        W[NVIDIA worker]
        T[Tray + global hotkey]
        O[500 ms overlay]
        P[Versioned named pipe]
        H --> W
        H --> T
        H --> O
        H --> P
    end

    subgraph UI["On-demand GPUI console"]
        G[Calibration dashboard]
        I[Single-owner pipe I/O]
        G --> I
    end

    P <-->|Snapshot / Command| I
    W -->|STA COM + NvCpl| N[NVIDIA UXD]
    C[Chrome / SDR video] --> N
```

### 两个进程的分工

| 进程 | 职责 | 生命周期 |
| --- | --- | --- |
| `RtxVideoManager.exe` | 托盘、三组热键、自启、HDR/VSR 事务、IPC 服务、原生浮层 | 常驻 |
| `RtxVideoManagerConsole.exe` | 五页仪表盘、设置草稿、快捷键录入、双功能诊断 | 按需启动，关闭即退出 |

宿主**不链接 GPUI**。只有你真正打开控制台时才启动渲染器，所以每次按快捷键都不会
拉起 GPU 窗口，也不会把渲染内存长期挂在后台。

### 本地 IPC

- 当前用户命名管道：`\\.\pipe\RtxHdrSwitch.Settings.v1`
- 拒绝远程客户端。
- 校验客户端进程 SID 与会话 ID。
- 协议版本明确为 `4`，完整快照同时携带 HDR 与 VSR。
- 4 字节小端长度前缀 + UTF-8 JSON。
- 单帧上限 64 KiB。
- **每个连接只有一个线程拥有全部管道 I/O**，避免同步句柄上并发读写导致卡死。

---

## “校准仪表盘”设计语言

不是通用后台，也不是堆卡片的设置页。界面参考硬件监看器和校准台：

- 石墨视频框、冰蓝 HDR 增强通道、琥珀 VSR 增强通道和中央信号节点组成产品图标。
- 石墨黑底保留视频周边的低干扰环境。
- 冰蓝色表达“实时信号 / 可操作”。
- 琥珀色只用于录制、校准刻度和注意状态。
- 珊瑚色只用于不可用与错误。
- Bahnschrift 负责数字与仪表标题，Microsoft YaHei UI 负责中文信息层级。
- 动效只在状态切换时出现，不做持续闪烁和无意义装饰。

<p align="center">
  <img src="assets/rtx-video-manager.png" width="128" alt="RTX Video Manager product icon">
</p>

图标以 [`assets/rtx-video-manager.svg`](assets/rtx-video-manager.svg) 为可编辑母版，并导出
16–256 px 多尺寸 ICO 编进 `RtxVideoManagerConsole.exe`；托盘另有开启、关闭、不可用
三态 ICO。运行时不需要旁边再放图片文件。

<details>
<summary><strong>视觉令牌</strong></summary>

| Token | Value | 用途 |
| --- | --- | --- |
| Ink | `#0B1215` | 主背景 |
| Panel | `#121C20` | 仪表面板 |
| Raised | `#19262C` | 控件与键帽 |
| Rule | `#2A4046` | 分隔与边框 |
| Text | `#EDF5F4` | 正文 |
| Muted | `#8CA1A5` | 次要说明 |
| Signal | `#65DDE6` | 实时信号 |
| Warm | `#F0B35C` | 录制 / 警告 |
| Danger | `#FF7B70` | 错误 |

</details>

---

## 兼容状态

| 状态 | 程序行为 |
| --- | --- |
| 受支持 + HDR10 显示器 + Windows HDR 开启 | 允许切换 |
| VSR 支持且 `0x315`/`0x317` 读取一致 | 允许 VSR 开关与档位写入 |
| 未找到 HDR10 显示器 | 只读，提示错误码 1 |
| Windows HDR 未开启 | 只读，提示错误码 2 |
| NVIDIA Surround 开启 | 只读，提示错误码 3 |
| 哈希未验证，但 NVIDIA 签名、导出和 HDR/VSR 探针均通过 | 只读兼容，只显示状态与诊断 |
| 签名、导出或读取探针失败 | 功能不可用，禁止写入 |
| UXD 服务不可用 | 只读 |
| 写入或回读不一致 | 自动恢复原始状态 |

### 参考环境

- Windows 10 Pro build 26300 / 26H2
- NVIDIA GeForce RTX 5060
- NVIDIA 驱动 `621.01`
- NVIDIA App `11.0.9.565`
- `nvcpl.dll` `8.17.15.5527`
- Chrome 155

其他驱动或 NVIDIA App 版本可能改变私有 ABI。未知哈希只有在 NVIDIA 签名、导出和
HDR/VSR 探针全部通过时才进入只读兼容；任何一项失败都会关闭写入路径。

---

## 性能目标

| 指标 | 目标 / 观察值 |
| --- | --- |
| 宿主 EXE | 约 0.5 MB |
| GPUI 控制台 EXE | 约 17.7 MB |
| 宿主空闲私有内存 | 约 6–13 MB |
| 控制台私有内存 | 实测约 90–100 MB，仅在打开时存在 |
| 空闲 CPU | 接近 0 |
| 切换浮层 | 500 ms |
| IPC 帧 | 长度前缀 JSON，≤ 64 KiB |

> 控制台不是常驻进程。关闭后，GPU 渲染内存随之释放。

---

## 项目结构

```text
RTX-HDR-Switch/
├─ src/
│  ├─ lib.rs             # 领域类型、HDR/VSR 编解码与回滚事务
│  ├─ ipc.rs             # 协议 v4、双功能 DTO 与帧协议
│  ├─ host_ipc.rs        # 命名管道服务 / 身份校验 / 单线程 I/O
│  └─ main.rs            # Win32 宿主、托盘、三热键、NVIDIA worker
├─ crates/ui/            # GPUI 控制台 RtxVideoManagerConsole.exe
├─ assets/               # 图标、清单与资源
├─ prototype/            # HTML/CSS 视觉原型
├─ docs/                 # 五份长期项目文档
└─ .github/workflows/    # Windows 云端构建与打包
```

---

## 构建与验证

本机不要求 Rust/MSVC。构建、格式检查、Clippy、测试和打包全部交给 GitHub Actions
的 `windows-latest`。

```text
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --release --target x86_64-pc-windows-msvc -p rtx-video-manager --bin RtxVideoManager
cargo build --locked --profile ui-release --target x86_64-pc-windows-msvc -p rtx-video-manager-console --bin RtxVideoManagerConsole
```

只有当 `gh run watch --exit-status` 成功退出，构建才算通过。

### 验收清单

- [x] NVIDIA `0x318` 可逆切换与 Chrome `hdrStatus` 变化
- [x] NVIDIA VSR `0x317` `4 → 3 → 4` 可逆探针与 `0x315` 回读
- [x] VSR Auto / 1–4 编码、固定字段校验、回滚事务与单元测试
- [x] 三快捷键唯一性、整组注册与失败回滚
- [x] 峰值、范围、中间灰、对比度、饱和度保持不变
- [x] 全局热键注册与 300 ms 内重复触发抑制
- [x] 托盘图标、菜单与 Explorer 重建
- [x] 单实例打开控制台
- [x] GPUI 控制台握手、快照和快捷键录制路径
- [x] 四项 HDR 调校写入、逐项回读与恢复原始值
- [x] `Ctrl+Alt+G` 唤起控制台的热键注册与真实按键验证
- [x] 控制台产品图标嵌入与 EXE 关联图标复核
- [x] 关闭控制台后释放辅助进程
- [x] 双功能 HTML 原型：五个页签、双通道总览、100%/125% 预览审查
- [ ] `feature/rtx-video-manager` 首次绿色云端构建
- [ ] 改名后的 host/console 包、任务栏图标与旧 HKCU 迁移实测
- [ ] Chrome 播放/暂停时 VSR `status` 与活动档位变化实测
- [ ] 500 ms 浮层在最终图标构建上的人工视觉复核
- [ ] 无 HDR 显示器 / Windows HDR 关闭 / Surround 冲突的人工复测

---

## 已知边界

- 不自动识别动画、白底或内容类型；开关仍是用户判断。
- 不支持 Windows HDR 模式切换。
- 不提供 VSR GPU 优先级、游戏检测或自动关闭。
- 不修改 NVIDIA 注册表，不做 UI 自动化。
- 不承诺第三方修改版 `nvcpl.dll` 兼容。
- 当前为私有项目，未声明开源许可证。
- 发布包必须保留两个 EXE 在同一目录。

### 明确不做

```text
Chrome 扩展      ❌
CSS 视频滤镜     ❌
WebView 前端     ❌
Tauri            ❌
内容自动分类     ❌
驱动注册表魔改   ❌
NVIDIA App 点击  ❌
```

---

## 常见问题

<details>
<summary><strong>`1440 nits` 是写死的吗？</strong></summary>

不是。它是宿主通过 NVIDIA `0x318` 读取到的当前峰值亮度，旁边的最小/最大值也来自驱动。
不同显示器或 HDR 配置会得到不同的数值和可选区间。

</details>

<details>
<summary><strong>为什么快捷键有反馈，但控制台显示不可用？</strong></summary>

宿主、快捷键和 NVIDIA 事务是独立路径。控制台通过 IPC 读取宿主快照；如果 DLL、
签名、UXD 服务或显示状态不满足条件，宿主仍可工作，但 HDR 写入会被门禁拒绝。

</details>

<details>
<summary><strong>为什么关闭控制台后托盘还在？</strong></summary>

这是设计目标。常驻宿主负责托盘、热键、自启和 HDR；GPUI 只在需要看设置和诊断时启动。

</details>

<details>
<summary><strong>为什么不直接修改注册表？</strong></summary>

注册表值不等于 NVIDIA UXD 的完整事务状态。这个项目沿用 NVIDIA App 的
Set + Execute + 回读路径，并有原始字节回滚。

</details>

<details>
<summary><strong>为什么 NVIDIA App 更新后可能不能切换？</strong></summary>

私有 ABI 没有稳定性承诺。程序锁定了已验证的文件哈希；变化后会自动进入只读，
避免用错误布局写坏设置。

</details>

---

<div align="center">

**Less panel archaeology. More watching.**

`Ctrl+Alt+H` 切换 · `Ctrl+Alt+G` 控制台

项目细节见 [`docs/`](docs/) · 双功能视觉原型见 [`prototype/rtx-video-manager.html`](prototype/rtx-video-manager.html)

</div>
