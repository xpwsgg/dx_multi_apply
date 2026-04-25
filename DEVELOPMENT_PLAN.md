# 访客入场申请系统 — 服务端重写开发计划

## 项目概述

将现有 Tauri 桌面客户端重写为基于 SaiAdmin 5.x 的 Web 服务端应用，部署到内网服务器，支持多用户通过浏览器完成访客查询、接待人查询、登录与 Token 管理、预约记录查询、批量提交、失败重试、日志审计与历史记录查询。

本计划以“保留当前桌面端核心业务能力”为前提，不再把目标收窄为单纯的“查询 + 提交”。当前桌面端已经具备以下关键能力，服务端迁移时默认纳入范围：

- 短信验证码登录
- 手动导入 `acToken`
- 启动时自动恢复并校验 Token
- 访客信息查询
- 接待人信息查询
- 预约记录查询
- 日期范围生成与批量任务清单
- 本地去重与强制重提
- 串行批量提交与随机等待
- 手动停止、失败重试、完成统计
- 操作日志与原始响应查看
- 表单状态恢复

---

## 一、迁移目标

### 1.1 业务目标

- 将单机桌面工具改造成可由多名运营人员共享的 Web 系统
- 保留当前桌面端的主要业务能力与提交规则
- 把本地文件与 SQLite 持久化迁移为服务端数据库与缓存
- 提供后台可见的任务记录、日志审计与失败重试能力
- 降低对单台电脑、单个本地环境和人工值守的依赖

### 1.2 成功标准

- 用户可以在 Web 页面完成登录或导入 Token，并查看当前 Token 状态
- 用户可以查询访客、接待人和访客历史预约记录
- 用户可以创建“日期 × 接待人”的批量提交任务并查看进度
- 系统按现有规则判定成功：HTTP 2xx 且响应体 `success === true`
- 系统能按“日期 + 接待人 + 访客集合”防重复提交
- 系统支持任务停止、失败重试、日志追踪和历史查询
- 部署后支持多用户使用，但同一账号/Token 的提交必须串行化

### 1.3 非目标

- 不在第一期改造第三方接口协议本身
- 不在第一期替换宜搭模板、字段映射或特殊接待人业务规则
- 不在第一期做对公网开放的 SaaS 化产品能力
- 不在第一期做复杂审批流、消息中心、报表中心

---

## 二、当前桌面端事实基线

当前项目并不是一个极简客户端，而是已经包含较完整的业务闭环。迁移计划必须以现有行为为基线，而不是重新定义一个缩水版产品。

### 2.1 当前技术栈

| 层级 | 技术 | 现状 |
|------|------|------|
| 桌面框架 | Tauri 2 | 已上线使用 |
| 前端 | React 19 + TypeScript 5.8 | 单页应用 |
| 后端逻辑 | Rust | 通过 Tauri command 暴露能力 |
| HTTP | reqwest | 调用第三方认证与宜搭接口 |
| 本地持久化 | SQLite + JSON 文件 | 存提交记录、Token、表单状态、日志 |
| 测试 | Vitest + Rust 单测 | 已有部分覆盖 |

### 2.2 当前已实现的能力

| 模块 | 当前能力 |
|------|----------|
| 登录 | 发送验证码、自动登录、导入 Token、校验 Token、清除 Token |
| 查询 | 访客查询、接待人查询、预约记录查询 |
| 提交 | 多访客、多接待人、多日期组合提交 |
| 去重 | 基于 `date + reception_id + visitor_ids` 的提交记录去重 |
| 运行控制 | 单任务运行锁、手动停止、失败统计 |
| 日志 | 前端实时日志 + 本地 JSONL 日志 |
| 状态恢复 | 表单自动保存与下次恢复 |

### 2.3 当前持久化形态

| 存储 | 当前位置 | 内容 |
|------|----------|------|
| SQLite | `records.db` | 已提交记录，去重用 |
| JSON | `ac_token.json` | 手机号、acToken、获取时间 |
| JSON | `form_state.json` | 手机号、访客列表、接待人列表 |
| 日志文件 | `logs/app.log` | 查询、登录、提交相关日志 |

### 2.4 当前关键业务规则

- 访客查询支持优先使用访客手机号，未提供时回落为申请人手机号
- 预约记录查询必须依赖有效 `acToken`
- 批量任务执行顺序为串行
- 每次提交后随机等待 30 到 50 秒；若后续任务全部可跳过，则可省略等待
- 成功判定为 HTTP 成功且响应体 `success === true`
- 特殊接待人工号 `52091191` 需要覆盖到访区域字段
- 当前桌面端使用全局运行锁，避免重复启动第二个批次

---

## 三、服务端重写范围

### 3.1 第一阶段必须保留的能力

- 登录与 Token 管理
- 访客信息查询
- 接待人信息查询
- 访客预约记录查询
- 批量任务创建、执行、停止、重试
- 去重校验
- 提交记录查询
- 操作日志查询
- 前端任务进度展示

### 3.2 可延后到第二阶段的能力

- 更丰富的统计报表
- WebSocket/SSE 之外的消息推送能力
- 更细粒度的 RBAC 权限模型
- 系统级告警中心
- 多租户、跨园区抽象

### 3.3 迁移策略

采用“先等价迁移，再做优化”的策略：

1. 先保证现有桌面端关键能力能在 Web 端跑通
2. 再将本地持久化切换为服务端数据库/缓存
3. 再补强多用户并发、审计、安全与部署细节

不建议采用“先做一个精简版管理后台，再补桌面端能力”的路径，否则大概率上线后无法替代现有工具。

---

## 四、技术方案

### 4.1 技术选型

| 层级 | 技术 | 说明 |
|------|------|------|
| 后端框架 | SaiAdmin 5.x / webman | 作为 Web 服务端与后台管理框架 |
| 编程语言 | PHP 8.1+ | 与 SaiAdmin/webman 对齐 |
| 数据库 | MySQL 8.0+ | 存任务、记录、日志、Token 元数据 |
| 缓存/队列 | Redis | 任务队列、运行态、分布式锁、进度缓存 |
| 前端 | SaiAdmin 内置 Vue3 | 提供后台页面与管理界面 |
| 任务执行 | webman 自定义进程 | 消费队列并按规则串行提交 |
| 反向代理 | Nginx | 统一入口、静态资源与反向代理 |

### 4.2 设计原则

- 同一手机号/Token 的提交任务必须串行
- API 服务与任务消费进程职责分离
- 第三方接口调用封装成独立服务，避免控制器堆业务
- 数据库存审计结果和业务状态，Redis 存运行态和短生命周期数据
- 日志和任务状态必须可追踪，不依赖前端内存态

### 4.3 建议目录结构

```text
plugin/saithink/visit/
├── app/
│   ├── controller/
│   │   ├── VisitorController.php
│   │   ├── ReceptionController.php
│   │   ├── StatusController.php
│   │   ├── AuthTokenController.php
│   │   ├── SubmissionTaskController.php
│   │   ├── SubmissionRecordController.php
│   │   └── OperationLogController.php
│   ├── model/
│   │   ├── AuthToken.php
│   │   ├── SubmissionTask.php
│   │   ├── SubmissionTaskItem.php
│   │   ├── SubmissionRecord.php
│   │   └── OperationLog.php
│   ├── service/
│   │   ├── AuthApiClient.php
│   │   ├── YiDaHttpClient.php
│   │   ├── PayloadBuilder.php
│   │   ├── TokenService.php
│   │   ├── VisitorService.php
│   │   ├── ReceptionService.php
│   │   ├── VisitorStatusService.php
│   │   ├── SubmissionTaskService.php
│   │   └── SubmissionExecutor.php
│   ├── process/
│   │   └── SubmissionWorker.php
│   └── validate/
│       ├── VisitorValidate.php
│       ├── ReceptionValidate.php
│       ├── StatusValidate.php
│       ├── TokenValidate.php
│       └── SubmissionTaskValidate.php
├── config/
│   └── visit.php
├── api.php
└── install.sql
```

---

## 五、核心差异与迁移要求

| 维度 | 当前桌面端 | 新服务端要求 |
|------|------------|--------------|
| 登录 | 短信登录 + 手动导入 Token + 本地校验 | 保留两种登录方式，Token 变成服务端托管 |
| 批量执行 | 进程内串行循环 | 任务入库 + Redis 队列 + Worker 串行消费 |
| 去重 | SQLite 唯一约束 | MySQL 记录 + 唯一键/哈希约束 |
| 运行锁 | 单进程原子锁 | Redis 分布式锁 + 按账号串行 |
| 日志 | 本地 JSONL | 数据库审计日志 + 页面查询 |
| 状态恢复 | 本地表单文件 | 服务端草稿或最近一次提交上下文 |
| 多用户 | 不支持共享 | 支持多用户，但同账号任务不可并发执行 |

---

## 六、数据模型设计

### 6.1 设计要点

- 不直接用 `visitor_ids` 的前缀索引做唯一约束
- 对访客身份证列表做排序、去重、拼接，生成规范化字符串
- 对规范化访客集生成 `visitor_set_hash`
- 用 `date + reception_id + visitor_set_hash` 作为核心去重维度

### 6.2 提交任务表 `v_submission_task`

记录一次用户发起的批量任务。

```sql
CREATE TABLE v_submission_task (
    id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    task_no VARCHAR(64) NOT NULL,
    account VARCHAR(20) NOT NULL COMMENT '申请人手机号',
    visitor_set_text TEXT NOT NULL COMMENT '排序后的访客身份证集合',
    visitor_set_hash CHAR(64) NOT NULL COMMENT '访客集合哈希',
    force_resubmit TINYINT NOT NULL DEFAULT 0,
    status TINYINT NOT NULL DEFAULT 0 COMMENT '0待执行 1执行中 2已完成 3部分失败 4已停止',
    total_count INT NOT NULL DEFAULT 0,
    success_count INT NOT NULL DEFAULT 0,
    failed_count INT NOT NULL DEFAULT 0,
    skipped_count INT NOT NULL DEFAULT 0,
    created_by BIGINT UNSIGNED DEFAULT NULL,
    started_at DATETIME DEFAULT NULL,
    finished_at DATETIME DEFAULT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uk_task_no (task_no),
    KEY idx_account_status (account, status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='批量提交任务';
```

### 6.3 提交任务明细表 `v_submission_task_item`

记录单个“日期 × 接待人”的执行单元。

```sql
CREATE TABLE v_submission_task_item (
    id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    task_id BIGINT UNSIGNED NOT NULL,
    date DATE NOT NULL,
    reception_id VARCHAR(32) NOT NULL,
    reception_name VARCHAR(64) NOT NULL DEFAULT '',
    status TINYINT NOT NULL DEFAULT 0 COMMENT '0待执行 1执行中 2成功 3失败 4跳过 5已停止',
    response_raw MEDIUMTEXT DEFAULT NULL,
    error_message VARCHAR(512) DEFAULT NULL,
    wait_seconds INT DEFAULT NULL,
    started_at DATETIME DEFAULT NULL,
    finished_at DATETIME DEFAULT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    KEY idx_task_status (task_id, status),
    KEY idx_reception_date (reception_id, date)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='批量提交任务明细';
```

### 6.4 提交记录表 `v_submission_record`

记录成功或失败后的可查询结果，用于去重与历史回溯。

```sql
CREATE TABLE v_submission_record (
    id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    date DATE NOT NULL,
    reception_id VARCHAR(32) NOT NULL,
    reception_name VARCHAR(64) NOT NULL DEFAULT '',
    account VARCHAR(20) NOT NULL,
    visitor_set_text TEXT NOT NULL,
    visitor_set_hash CHAR(64) NOT NULL,
    status TINYINT NOT NULL DEFAULT 0 COMMENT '1成功 2失败 3跳过',
    response_raw MEDIUMTEXT DEFAULT NULL,
    error_message VARCHAR(512) DEFAULT NULL,
    submitted_at DATETIME DEFAULT NULL,
    source_task_id BIGINT UNSIGNED DEFAULT NULL,
    created_by BIGINT UNSIGNED DEFAULT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uk_dedup (date, reception_id, visitor_set_hash),
    KEY idx_account_created (account, created_at),
    KEY idx_status_created (status, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='提交记录';
```

### 6.5 Token 管理表 `v_auth_token`

服务端保存 Token 元数据。敏感字段建议加密存储。

```sql
CREATE TABLE v_auth_token (
    id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    phone VARCHAR(20) NOT NULL,
    ac_token_ciphertext TEXT NOT NULL COMMENT '加密后的acToken',
    token_fingerprint CHAR(64) NOT NULL COMMENT '用于排查的指纹',
    is_valid TINYINT NOT NULL DEFAULT 1,
    obtained_at DATETIME NOT NULL,
    last_checked_at DATETIME DEFAULT NULL,
    last_check_result VARCHAR(128) DEFAULT NULL,
    created_by BIGINT UNSIGNED DEFAULT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uk_phone (phone)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='认证令牌';
```

### 6.6 操作日志表 `v_operation_log`

```sql
CREATE TABLE v_operation_log (
    id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    trace_id VARCHAR(64) DEFAULT NULL,
    operation VARCHAR(64) NOT NULL,
    account VARCHAR(20) DEFAULT NULL,
    request_summary VARCHAR(512) DEFAULT NULL,
    status SMALLINT NOT NULL DEFAULT 0,
    response_body MEDIUMTEXT DEFAULT NULL,
    error_message VARCHAR(512) DEFAULT NULL,
    created_by BIGINT UNSIGNED DEFAULT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    KEY idx_operation_created (operation, created_at),
    KEY idx_account_created (account, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='操作日志';
```

---

## 七、服务设计

### 7.1 `AuthApiClient`

职责：

- 发送验证码
- 验证码登录
- 校验 Token 是否有效
- 查询访客预约记录

建议方法：

```php
class AuthApiClient
{
    public function sendCode(string $phone): array;
    public function visitorLogin(string $phone, string $code): array;
    public function checkTokenValid(string $phone, string $acToken): bool;
    public function queryVisitorStatus(string $phone, string $idCard, string $acToken): array;
}
```

### 7.2 `YiDaHttpClient`

职责：

- 统一宜搭请求头、查询参数与超时
- 封装访客查询、接待人查询、申请提交
- 保留当前模板字段和 Referer 规则

```php
class YiDaHttpClient
{
    public function fetchVisitorInfo(string $account, string $idCard, ?string $visitorPhone = null): array;
    public function fetchReceptionInfo(string $employeeId): array;
    public function submitApplication(string $account, array $payload): array;
}
```

### 7.3 `PayloadBuilder`

职责：

- 迁移 `request_template.json`
- 迁移 `binding_formulas.json`
- 迁移日期映射逻辑
- 保留 `52091191` 的到访区域覆盖规则

```php
class PayloadBuilder
{
    public function buildPayload(
        \DateTimeInterface $date,
        string $account,
        array $visitors,
        array $reception
    ): array;
}
```

### 7.4 `SubmissionExecutor`

职责：

- 读取任务明细
- 按顺序逐条提交
- 控制 30 到 50 秒随机等待
- 支持停止、失败、跳过和重试
- 写回任务、明细、提交记录、日志

---

## 八、API 设计

### 8.1 认证与 Token

| 接口 | 方法 | 路径 | 说明 |
|------|------|------|------|
| 短信登录 | POST | `/api/visit/auth/login` | 发送验证码并完成登录 |
| 导入 Token | POST | `/api/visit/auth/token/import` | 手动导入 `acToken` |
| 查询 Token 状态 | GET | `/api/visit/auth/token/status` | 返回当前 Token 状态 |
| 校验 Token | POST | `/api/visit/auth/token/check` | 主动检测有效性 |
| 清除 Token | DELETE | `/api/visit/auth/token` | 删除当前账号 Token |

### 8.2 查询能力

| 接口 | 方法 | 路径 | 说明 |
|------|------|------|------|
| 访客查询 | POST | `/api/visit/visitor/fetch` | 支持申请人手机号或访客手机号 |
| 接待人查询 | POST | `/api/visit/reception/fetch` | 根据工号查询 |
| 预约记录查询 | POST | `/api/visit/visitor/status` | 依赖有效 Token |

### 8.3 批量任务

| 接口 | 方法 | 路径 | 说明 |
|------|------|------|------|
| 创建批量任务 | POST | `/api/visit/submission/task` | 创建任务并入队 |
| 查看任务详情 | GET | `/api/visit/submission/task/{id}` | 返回汇总和明细 |
| 查看任务进度 | GET | `/api/visit/submission/task/{id}/progress` | 返回实时进度 |
| 停止任务 | POST | `/api/visit/submission/task/{id}/stop` | 请求停止运行中任务 |
| 重试失败项 | POST | `/api/visit/submission/task/{id}/retry` | 仅重试失败项 |

### 8.4 历史与日志

| 接口 | 方法 | 路径 | 说明 |
|------|------|------|------|
| 查询已存在记录 | GET | `/api/visit/submission/existing` | 用于前端预检 |
| 提交记录列表 | GET | `/api/visit/submission/records` | 支持筛选和分页 |
| 操作日志列表 | GET | `/api/visit/log/list` | 支持筛选和分页 |

---

## 九、批量任务方案

### 9.1 推荐方案

采用“数据库任务表 + Redis 队列 + Worker 消费 + Redis 锁”的组合方案。

### 9.2 核心约束

- 同一 `account` 同一时刻只允许一个运行中的任务
- 同一 `account` 下的任务明细必须串行提交
- 不同账号可并行，但需要设置系统总并发上限
- 提交前仍需再次检查去重记录，不能只依赖创建任务时的预检

### 9.3 流程

```text
用户创建任务
    -> API 校验参数
    -> 生成 visitor_set_text 与 visitor_set_hash
    -> 写入 v_submission_task / v_submission_task_item
    -> 尝试获取 account 级分布式锁
    -> 投递 task_id 到 Redis 队列

SubmissionWorker 消费 task_id
    -> 标记任务执行中
    -> 按 date 升序、接待人维度顺序执行
    -> 每条执行前检查 stop 标记与重复记录
    -> 调用 PayloadBuilder + 第三方接口
    -> 写入 record / log / item 状态
    -> 若后续仍有待执行项，则等待 30~50 秒
    -> 完成后释放锁并更新任务汇总
```

### 9.4 进度展示

第一期建议使用轮询：

- 每 2 到 3 秒查询任务进度
- 进度接口返回总数、成功数、失败数、跳过数、当前执行项、剩余数

第二期可升级为 SSE 或 WebSocket。

---

## 十、前端页面规划

### 10.1 第一阶段页面

- 登录与 Token 管理页面
- 访客查询页面
- 接待人查询页面
- 预约记录查询页面
- 批量提交页面
- 提交记录页面
- 操作日志页面

### 10.2 批量提交页面必须保留的交互

- 多访客、多接待人录入
- 日期范围与快捷日期
- 任务清单预览
- 已存在任务标识
- 强制重提确认
- 运行进度、等待倒计时、当前任务展示
- 失败项重试
- 手动停止

---

## 十一、开发阶段

### 阶段 0：需求对齐与迁移基线（2-3 天）

- [ ] 盘点桌面端现有能力，形成功能对齐表
- [ ] 明确第一期保留项、延后项、废弃项
- [ ] 固化第三方接口字段、模板、特殊规则
- [ ] 输出接口清单、页面清单和验收口径

### 阶段 1：项目初始化与基础设施（2-4 天）

- [ ] 初始化 SaiAdmin 5.x 项目骨架
- [ ] 配置 MySQL、Redis、日志和环境变量
- [ ] 创建插件目录结构、配置文件和基础路由
- [ ] 完成数据库建表脚本
- [ ] 建立加密/解密与敏感配置读取机制

### 阶段 2：第三方接口与模板迁移（4-6 天）

- [ ] 迁移认证接口调用逻辑
- [ ] 迁移访客/接待人查询逻辑
- [ ] 迁移 `request_template.json`
- [ ] 迁移 `binding_formulas.json`
- [ ] 迁移日期映射与 `52091191` 特殊规则
- [ ] 单元测试对比关键 payload 输出

### 阶段 3：核心业务 API（4-6 天）

- [ ] 完成登录与 Token API
- [ ] 完成访客查询 API
- [ ] 完成接待人查询 API
- [ ] 完成预约记录查询 API
- [ ] 完成记录与日志查询 API
- [ ] 完成参数校验与错误码规范

### 阶段 4：任务系统与执行器（5-8 天）

- [ ] 创建批量任务与任务明细
- [ ] 实现 account 级串行锁
- [ ] 实现 Redis 队列与 Worker
- [ ] 实现停止、跳过、失败重试
- [ ] 实现进度查询接口
- [ ] 端到端验证主流程

### 阶段 5：前端页面与联调（5-8 天）

- [ ] 实现 Token 管理页面
- [ ] 实现查询相关页面
- [ ] 实现批量提交页面
- [ ] 实现记录与日志页面
- [ ] 完成前后端联调与交互细节补齐

### 阶段 6：安全、稳定性与发布（3-5 天）

- [ ] 接口鉴权和菜单权限控制
- [ ] Token 加密存储与日志脱敏
- [ ] 限流、超时、重试与熔断策略
- [ ] 部署脚本、Nginx 配置、环境说明
- [ ] 预发布验证与上线检查

---

## 十二、验证方案

### 12.1 单元测试

- `PayloadBuilder` 输出正确性
- 访客集合规范化与哈希生成
- Token 有效性判定逻辑
- 提交成功判定逻辑

### 12.2 集成测试

- 登录与 Token 导入流程
- 访客查询、接待人查询、预约记录查询
- 创建任务、查看进度、停止任务、重试失败项

### 12.3 端到端验证

- 多访客 + 多接待人 + 多日期主流程
- 已存在记录跳过
- 强制重提
- 手动停止
- 失败后重试

### 12.4 上线前验证

- 单账号串行是否生效
- 多账号并发是否在阈值内
- 第三方限流场景是否可恢复
- 任务与日志数据是否可追踪

---

## 十三、风险与应对

| 风险 | 影响 | 应对 |
|------|------|------|
| 宜搭 Cookie / CSRF Token 过期 | 查询与提交失败 | 后台配置可刷新；增加健康检查与失效提示 |
| `acToken` 失效 | 预约记录查询失败 | 保留登录与导入两种方式；提供失效检测 |
| 第三方接口限流 | 批量提交失败率升高 | 串行执行、随机等待、系统总并发限制 |
| 同账号并发提交 | 重复提交或状态污染 | `account` 级 Redis 锁 + DB 状态校验 |
| 去重误判 | 错误跳过或错误重提 | 使用 `visitor_set_hash`，避免前缀索引 |
| 敏感数据泄露 | 合规与安全风险 | Token 加密、日志脱敏、最小权限访问 |
| Worker 异常退出 | 任务中断 | 任务状态恢复机制 + 死信/补偿检查 |
| 重写范围失控 | 工期失真 | 先做等价迁移，分阶段交付 |

---

## 十四、时间估算

| 阶段 | 预计时间 |
|------|----------|
| 阶段 0：需求对齐与迁移基线 | 2-3 天 |
| 阶段 1：项目初始化与基础设施 | 2-4 天 |
| 阶段 2：第三方接口与模板迁移 | 4-6 天 |
| 阶段 3：核心业务 API | 4-6 天 |
| 阶段 4：任务系统与执行器 | 5-8 天 |
| 阶段 5：前端页面与联调 | 5-8 天 |
| 阶段 6：安全、稳定性与发布 | 3-5 天 |
| **合计** | **25-40 天** |

说明：

- 以上估时默认 1 名熟悉项目的工程师全职推进
- 若要求与桌面端完全等价并补齐发布文档、上线验证，建议按 5-8 周预期管理
- 若只做 MVP，可将“预约记录查询、失败重试、日志页面、细粒度权限”调整为第二阶段

---

## 十五、建议执行顺序

如果马上进入实施，建议按以下优先级推进：

1. 先冻结当前桌面端行为，形成迁移验收基线
2. 先完成第三方接口与模板迁移，再做 UI
3. 先打通单账号任务链路，再扩展多用户并发
4. 先用轮询完成进度展示，再评估是否升级 SSE
5. 先保证可替代桌面端，再做管理后台增强能力
