# 到访批量提交 GUI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 构建一个可用的 Tauri+React GUI：通过起止日期生成闭区间日期列表、支持手动删除、批量调用现有提交命令，并在日志中完整显示服务端响应；后端以 `success === true` 作为业务成功标准。

**Architecture:** 前端负责区间展开与可编辑日期列表，后端保留既有串行批处理主流程。`start_batch_submit`/`stop_batch_submit` 命令接口保持不变，仅增强 `submit_client` 的成功判定和响应透传。日志事件继续走 `batch-log`，前端实时订阅渲染。

**Tech Stack:** React 19 + TypeScript + Tauri 2 + Rust + serde_json + chrono

---

### Task 1: 后端提交成功判定与响应透传

**Files:**
- Modify: `src-tauri/src/submit_client.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/submit_client_tests.rs`

**Step 1: 写失败测试（业务失败）**

在 `src-tauri/src/submit_client_tests.rs` 新增测试：HTTP 200 但 body 为 `{"success":false,"errorMsg":"xx"}` 时应返回 `Err`。

```rust
#[tokio::test]
async fn should_mark_success_false_as_failure() {
    let fake = crate::submit_client::FakeHttp::new(200, r#"{"success":false,"errorMsg":"bad"}"#);
    let result = crate::submit_client::submit_once_with_client(&fake, "2026-03-08").await;
    assert!(result.is_err());
}
```

**Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test should_mark_success_false_as_failure -- --nocapture`
Expected: FAIL（当前实现仅按 HTTP 2xx 判定成功）

**Step 3: 最小实现通过测试**

在 `submit_once_with_client` 中增加解析与判定：
- 保持 HTTP 2xx 检查
- `serde_json::from_str::<serde_json::Value>(&response_text)`
- 检查 `value.get("success").and_then(Value::as_bool) == Some(true)`
- 否则返回 `SubmitError`，错误消息包含原始响应

并保留 `SubmitResult.response_text`，用于上层日志透传。

**Step 4: 扩展日志事件内容**

在 `src-tauri/src/lib.rs` 的成功/失败 emit 中附加响应原文字段：
- success 事件增加 `responseRaw`
- failed 事件增加 `responseRaw`（可选：若有）

示意：

```rust
json!({
  "date": date_text,
  "result": "success",
  "waitSeconds": wait_seconds,
  "responseRaw": submit_result.response_text
})
```

**Step 5: 运行测试确认通过**

Run: `cd src-tauri && cargo test submit_client -- --nocapture`
Expected: PASS

**Step 6: 提交**

```bash
git add src-tauri/src/submit_client.rs src-tauri/src/lib.rs src-tauri/src/submit_client_tests.rs
git commit -m "feat: enforce success flag and log raw response"
```

---

### Task 2: 前端区间展开与可删除日期列表（纯函数先行）

**Files:**
- Create: `src/dateRange.ts`
- Create: `src/dateRange.test.ts`
- Modify: `package.json`（仅在缺少测试脚本时）

**Step 1: 写失败测试（闭区间展开）**

在 `src/dateRange.test.ts` 写 3 个核心测试：
1) `2026-03-01` 到 `2026-03-03` 返回 3 天（含首尾）
2) 起止相同返回 1 天
3) start > end 抛错

```ts
import { describe, expect, it } from "vitest";
import { expandDateRange } from "./dateRange";

describe("expandDateRange", () => {
  it("expands inclusive range", () => {
    expect(expandDateRange("2026-03-01", "2026-03-03")).toEqual([
      "2026-03-01",
      "2026-03-02",
      "2026-03-03",
    ]);
  });
});
```

**Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/dateRange.test.ts`
Expected: FAIL（函数尚不存在）

**Step 3: 最小实现通过测试**

在 `src/dateRange.ts` 实现：
- 输入为 `YYYY-MM-DD` 字符串
- 生成闭区间数组
- start > end 抛 `Error`

```ts
export function expandDateRange(start: string, end: string): string[] {
  // 仅实现闭区间按天递增
}
```

**Step 4: 再加一个测试（无效输入）**

新增无效日期格式测试（例如空字符串），要求抛错。

**Step 5: 运行测试确认通过**

Run: `pnpm vitest run src/dateRange.test.ts`
Expected: PASS

**Step 6: 提交**

```bash
git add src/dateRange.ts src/dateRange.test.ts package.json
git commit -m "test: add date range expansion utility with coverage"
```

---

### Task 3: 前端页面替换为业务 GUI

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.css`
- Use: `src/dateRange.ts`

**Step 1: 写失败测试（可选最小 UI 测试）**

若项目已有 React Testing Library，则新增：
- 点击“生成日期列表”后出现闭区间日期
- 点击“删除”后日期消失

若当前未配置 UI 测试框架，则跳过自动化 UI 测试，改为 Task 5 的手工验收清单（YAGNI，避免额外测试基建）。

**Step 2: 实现页面状态与交互**

在 `src/App.tsx` 中实现：
- `startDate`, `endDate`, `dates`, `isRunning`, `logs`, `errorMessage`
- 生成按钮：调用 `expandDateRange(startDate, endDate)`
- 删除按钮：`setDates(prev => prev.filter(d => d !== target))`
- 开始按钮：`invoke("start_batch_submit", { dates })`
- 停止按钮：`invoke("stop_batch_submit")`
- 运行态禁用输入与删除操作

**Step 3: 订阅 batch-log 事件并展示完整响应**

在组件生命周期中：
- 使用 `listen("batch-log", ...)`
- 每条日志写入 `logs`
- 若 payload 包含 `responseRaw`，完整展示（`<pre>`）
- 收到 `failed/stopped` 后退出运行态

**Step 4: 更新样式**

`src/App.css` 改为简洁业务布局：
- 上部输入区
- 中部日期列表（可滚动）
- 下部日志区（`pre-wrap` 保留原文）

**Step 5: 构建检查**

Run: `pnpm build`
Expected: PASS（TypeScript + Vite 构建通过）

**Step 6: 提交**

```bash
git add src/App.tsx src/App.css
git commit -m "feat: add date-range batch submit gui"
```

---

### Task 4: Tauri 端到端联调与回归

**Files:**
- Verify: `src-tauri/src/lib.rs`
- Verify: `src-tauri/src/submit_client.rs`
- Verify: `src/App.tsx`

**Step 1: 后端测试全跑**

Run: `cd src-tauri && cargo test -- --nocapture`
Expected: PASS

**Step 2: 前端测试/构建全跑**

Run: `pnpm test`（若配置了 test 脚本）
Run: `pnpm build`
Expected: PASS

**Step 3: 本地手工验证（关键路径）**

Run: `pnpm tauri dev`

手工检查：
1) `2026-03-01 ~ 2026-03-03` 生成 3 条日期（闭区间）
2) 删除 `2026-03-02` 后仅提交剩余 2 条
3) 成功返回 `{"success":true,...}` 时日志显示 success + `responseRaw`
4) 业务失败（`success:false`）时日志显示 failed 并停止
5) 点击停止后显示 stopped，不再继续下一个日期

**Step 4: 最终提交**

```bash
git add src-tauri/src/lib.rs src-tauri/src/submit_client.rs src-tauri/src/submit_client_tests.rs src/App.tsx src/App.css src/dateRange.ts src/dateRange.test.ts
git commit -m "feat: support editable date-range batch submit workflow"
```

---

### Task 5: 文档与交付说明

**Files:**
- Modify: `README.md`

**Step 1: 更新使用说明**

在 README 增加：
- 如何选择开始/结束日期
- 如何生成/删除日期
- 成功与失败判定规则（`success===true`）
- 日志中 `responseRaw` 用途

**Step 2: 验证文档命令可执行**

Run: `pnpm tauri dev`
Expected: 能按 README 流程操作

**Step 3: 提交**

```bash
git add README.md
git commit -m "docs: add batch submit gui usage and debug logging notes"
```

---

## 风险与防呆

- 日期格式依赖浏览器 `input[type=date]` 返回 `YYYY-MM-DD`；若为空必须前端拦截。
- 日志可能较长，UI 需采用滚动容器，避免页面卡顿。
- `responseRaw` 可能包含转义字符，展示使用 `<pre>` 保留原文，不做结构化改写。

## 完成定义（DoD）

- 用户可通过起止日期生成闭区间列表并手动删除。
- 批量提交与停止按钮行为正确。
- 后端业务成功判定使用 `success===true`。
- 前端日志可完整看到服务端响应原文。
- `cargo test` 与 `pnpm build` 通过。
