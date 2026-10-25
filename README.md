# RTX HDR Switch

一个常驻 Windows 托盘的小工具，用全局快捷键开关 NVIDIA RTX Video HDR。

## 功能

- 默认快捷键：`Ctrl+Alt+H`
- 托盘左键打开设置，右键显示菜单
- 设置页可直接开启或关闭 RTX HDR
- 可录制新的快捷键
- 可选开机启动和切换通知
- 切换成功后，在当前屏幕下方居中显示约 300 ms 的半透明开启/关闭提示
- 切换前读取真实驱动状态，写入后回读验证
- 写入异常时自动恢复原始 HDR 设置
- 无管理员权限、无网络访问、无 WebView

## 限制

NVIDIA 没有提供控制 RTX Video HDR 的官方 CLI 或公共 API。本项目使用 NVIDIA
App 自带的未公开 `nvcpl.dll` 接口，并严格锁定到以下文件：

- 路径：`%ProgramW6432%\NVIDIA Corporation\NVIDIA App\NvCpl\nvcpl.dll`
- 版本：`8.17.15.5527`
- SHA-256：`EEE812AB8FF74975D3CC7FE77130F5766911B6CA2D9C42C1156376A6C13F79F0`

版本或哈希不匹配时，程序保持只读并禁止修改 HDR。NVIDIA App 更新后可能需要重新验证
新的二进制与字段布局。

程序要求 NVIDIA 显示设置服务正在运行，并会在独立的 STA COM 会话中连接该服务。
服务不可用或接口状态异常时，程序拒绝写入 HDR。

本项目不切换 Windows HDR，不修改 RTX Video Super Resolution，也不包含 Chrome
扩展或视频内容自动识别。

## 使用

1. 从 GitHub Actions 下载 `RtxHdrSwitch-win-x64.zip`。
2. 解压并运行 `RtxHdrSwitch.exe`。
3. 使用 `Ctrl+Alt+H` 切换 RTX Video HDR。

手动启动会打开设置窗口；关闭设置窗口不会退出程序。程序必须继续驻留在通知区域，
快捷键才会工作。`--background` 仅用于开机启动，启动时不显示设置窗口。

程序只写入以下当前用户注册表位置：

- `HKCU\Software\RTXHDRSwitch`
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`

## 构建

构建和自动化测试只由 GitHub Actions 的 `windows-latest` 执行：

```text
cargo generate-lockfile
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release --target x86_64-pc-windows-msvc
```

项目文档位于 [`docs/`](docs/)。
