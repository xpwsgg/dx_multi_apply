# release —— 分发成品存放目录

存放**本机手动构建**出来的可分发成品，作用等同 GitHub Release 的附件区。

⚠️ 本项目的正式发版仍走 **GitHub Actions**（见根 `README.md` 的「发布流程」：
同步三处版本号 → 打 `vX.Y.Z` tag → 推送 → Actions 自动构建并创建 Release）。
本目录放的是**本地临时构建**的产物，用于自测或临时分发，不替代 CI 发版。

## 二进制不入 git

`.gitignore` 已忽略本目录下的一切二进制，只有本 README 入库。
理由与 GitHub Release 的语义一致：附件是构建产物，不属于源码历史，入库会让仓库体积失控。

## 命名约定

tauri 按 `productName`（`BatchApply`）自动命名，带版本号：

| 平台 | 文件名 | 来源 |
|---|---|---|
| macOS (Intel) | `BatchApply_<版本>_x64.dmg` | 本地可构建 |
| macOS (Apple Silicon) | `BatchApply_<版本>_aarch64.dmg` | **仅 CI**（维护者本机是 Intel Mac） |
| Windows | `BatchApply_<版本>_x64-setup.exe` / `BatchApply-portable-x64.exe` | **仅 CI** |

**本机（Intel Mac）只能产出 x64 dmg**。需要其余三种形态时走 CI，不要用本地
x64 产物冒充其他平台/架构的版本。

## 本地构建

```bash
pnpm install
pnpm tauri build      # 产物落在 src-tauri/target/release/bundle/dmg/
```

无原生库/sidecar 前置依赖，`pnpm tauri build` 可直接跑。
构建完把 dmg 拷进本目录归档。
