# BatchApply — 批量入场申请工具

公司园区访客管理系统每次只能提交一天的入场申请，本工具支持一次性提交多天、多访客、多接待人的批量申请，大幅减少重复操作。

## 功能特性

- **短信验证码登录** — 输入手机号获取验证码，登录后自动保存 token
- **访客信息查询** — 输入身份证号自动获取访客姓名、电话等信息，支持多访客
- **接待人信息查询** — 输入工号自动获取接待人姓名、部门等信息，支持多接待人
- **手动导入 Token** — 支持直接粘贴 acToken 登录，适用于无法收到短信的场景
- **批量日期提交** — 选择日期范围，自动跳过已提交日期，串行提交并随机延迟防限流
- **停止任务** — 支持手动中止正在执行的批量提交
- **预约记录查询** — 查询已有预约记录，显示审核状态、权限状态等完整信息
- **实时日志** — 所有操作实时显示在日志面板，支持复制和清空
- **表单状态持久化** — 自动保存上次填写的内容，下次启动时恢复
- **清除表单** — 一键清空所有数据并重置登录状态
- **关闭保护** — 任务执行中关闭窗口时弹出确认对话框，防止误操作

## 技术栈

| 层级 | 技术 |
|------|------|
| 框架 | Tauri 2 |
| 前端 | React 19 + TypeScript |
| 构建 | Vite 7 |
| 后端 | Rust |
| 数据库 | SQLite (rusqlite) |

## 快速开始

### 前置条件

- Node.js 22+、pnpm 9+
- Rust (stable)
- 系统依赖参考 [Tauri 官方文档](https://v2.tauri.app/start/prerequisites/)

### 开发模式

```bash
pnpm install
pnpm tauri dev
```

### 生产构建

```bash
pnpm tauri build
```

## 下载安装

从 [Releases](../../releases) 页面下载对应平台的安装包：

| 平台 | 文件 | 说明 |
|------|------|------|
| macOS (Apple Silicon) | `BatchApply_x.x.x_aarch64.dmg` | 双击安装 |
| macOS (Intel) | `BatchApply_x.x.x_x64.dmg` | 双击安装 |
| Windows | `BatchApply_x.x.x_x64-setup.exe` | 安装版 |
| Windows | `BatchApply-portable-x64.exe` | **免安装版**，双击直接运行 |

## 使用流程

1. 输入手机号，获取验证码并登录
2. 输入访客身份证号，查询并添加访客
3. 输入接待人工号，查询并添加接待人
4. 选择开始日期和结束日期
5. 点击”开始提交”执行批量提交
6. 在日志面板查看提交进度和结果

## 项目文档

- [需求说明](docs/需求说明.md) — 项目背景、功能需求、非功能需求
- [开发指南](docs/开发指南.md) — 技术架构、项目结构、环境搭建、CI/CD 流程

## 发布流程

1. 同步更新三处版本号：`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`
2. 提交代码并创建 `vX.Y.Z` 格式的 tag
3. 推送到 GitHub，Actions 自动构建并创建 Release

## License

MIT
