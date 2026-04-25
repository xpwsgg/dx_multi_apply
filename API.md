# 第三方接口文档

本文档整理了项目所调用的全部第三方接口，包含认证、数据查询和表单提交等功能。

---

## 一、访客认证 API

**基础地址**: `https://dingtalk.avaryholding.com:8443/dingplus/visitorConnector`

**请求特征**:
- Content-Type: `application/json`
- User-Agent: 模拟钉钉移动端 `AliApp(DingTalk/7.6.0)`
- 超时: 连接 10s，请求 30s

### 1.1 发送验证码

- **路径**: `POST /sendCode`
- **源文件**: `src-tauri/src/auth_client.rs`
- **用途**: 向访客手机号发送登录验证码

**请求体**:

```json
{
  "phoneNum": "手机号",
  "areaCode": "86",
  "region": ""
}
```

**请求头**:

| Header | Value |
|--------|-------|
| Content-Type | application/json |
| Accept | application/json, text/json |
| User-Agent | Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 ...) AliApp(DingTalk/7.6.0) |

**响应体**:

```json
{
  "code": 200,
  "data": {
    "code": "验证码"
  },
  "message": "..."
}
```

- `code=200` 表示成功，`data.code` 为返回的验证码

---

### 1.2 访客登录

- **路径**: `POST /visitorLogin`
- **源文件**: `src-tauri/src/auth_client.rs`
- **用途**: 使用手机号和验证码登录，获取访问令牌

**请求体**:

```json
{
  "phoneNum": "手机号",
  "code": "验证码"
}
```

**请求头**:

| Header | Value |
|--------|-------|
| Content-Type | application/json |
| Accept | application/json, text/json |
| User-Agent | Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 ...) AliApp(DingTalk/7.6.0) |
| Origin | https://iw68lh.aliwork.com |
| Referer | https://iw68lh.aliwork.com/o/fk_login_app |

**响应体**:

```json
{
  "code": 200,
  "data": {
    "acToken": "访问令牌"
  },
  "message": "..."
}
```

- `code=200` 表示成功，`data.acToken` 为登录令牌，后续请求需携带

---

### 1.3 验证登录状态 / 查询入场申请记录

- **路径**: `POST /visitorStatus`
- **源文件**: `src-tauri/src/status_client.rs`
- **用途**: 同一接口承担两种用途：
  - **用法 A（`check_token_valid`）**：检查已保存的 acToken 是否仍然有效（不解析业务数据）
  - **用法 B（`query_visitor_status`）**：根据身份证号查询访客的入场申请记录

**请求头**（两种用法相同）:

| Header | Value |
|--------|-------|
| Content-Type | application/json |
| Accept | application/json, text/json |
| User-Agent | Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 ...) AliApp(DingTalk/7.6.0) |
| Origin | https://iw68lh.aliwork.com |

#### 用法 A：验证 Token 有效性

**请求体**:

```json
{
  "visitorIdNo": "",
  "regPerson": "手机号",
  "acToken": "令牌"
}
```

**响应处理规则**:

- HTTP `5xx` → 视为临时故障，返回 `Err`，调用方不应清除 Token
- 业务 `code=200` → Token 有效
- 业务 `code=401` → Token 已失效，需重新登录
- **其他 code（如参数校验错误 500 等）** → 服务端已通过认证，仅校验失败，视为 Token 仍然有效
- 响应体不能解析为 JSON 或缺少 `code` 字段 → 返回 `Err`

#### 用法 B：查询访客入场申请记录

**请求体**:

```json
{
  "visitorIdNo": "身份证号",
  "regPerson": "手机号",
  "acToken": "令牌"
}
```

**响应体**:

```json
{
  "code": 200,
  "data": [
    {
      "flowNum": "流水号",
      "visitorName": "访客姓名",
      "visitorPhone": "访客电话",
      "visitCompanyName": "访问公司",
      "gardenName": "园区",
      "visitorType": "访客类型",
      "rPersonName": "接待人姓名",
      "rPersonPhone": "接待人电话",
      "dateStart": "时间戳(毫秒)",
      "dateEnd": "时间戳(毫秒)",
      "flowStatus": "状态码",
      "createTime": "创建时间"
    }
  ]
}
```

**响应处理规则**:

- HTTP 非 2xx → 返回 `Err`
- 业务 `code=401` → 返回错误"登录已失效，请重新登录"
- 业务 `code != 200` → 返回错误（含 `message` 字段）
- 业务 `code=200` → 解析 `data` 数组，过滤掉缺少 `flowNum` 的项

**`dateStart` / `dateEnd` 时间戳处理**:

- 服务端返回毫秒级时间戳字符串
- 客户端使用 UTC+8（`FixedOffset::east_opt(8 * 3600)`）将其格式化为 `yyyy-MM-dd`
- 解析失败时原样保留字符串

**`flowStatus` 状态码映射**（来源：宜搭页面自定义 JS `switch (+item.flowStatus)`）:

| 状态码 | 含义 |
|--------|------|
| 1 | 审核中 |
| 3 | 审核拒绝 |
| 4 | 审核同意 |
| 5 | 审核通过，权限未生效 |
| 6 | 权限已生效 |
| 7 | 权限已失效 |

> 客户端会将状态码映射为中文文案；未在表中的状态码原样保留。

---

## 二、宜搭（YiDa）低代码平台 API

**基础地址**: `https://iw68lh.aliwork.com/o/...`

**请求特征**:
- Content-Type: `application/x-www-form-urlencoded`
- User-Agent: 桌面端 Chrome 145
- 需携带 Cookie 和 CSRF Token
- 查询参数统一包含 `_api=nattyFetch`、`_mock=false`、`_stamp=时间戳`
- 超时: 连接 10s，请求 30s

**通用鉴权参数**:

| 参数 | 值 | 说明 |
|------|----|------|
| Cookie | `isg=...; JSESSIONID=...` | 宜搭平台会话 Cookie |
| x-csrf-token | `c5683320-e1de-4fc0-b89d-65b268eaacd1` | CSRF 防护令牌 |
| formUuid | `FORM-2768FF7B2C0D4A0AB692FD28DBA09FD57IHQ` | 访客入场表单 UUID |
| appType | `APP_GRVPTEOQ6D4B7FLZFYNJ` | 应用类型标识 |
| bx-v | `2.5.11` | 宜搭平台版本号 |

---

### 2.1 查询访客信息

- **URL**: `POST https://iw68lh.aliwork.com/o/HW9663A19D6M1QDL6D7GNAO1L2ZC26DXQHOXL7`
- **源文件**: `src-tauri/src/visitor_client.rs`
- **用途**: 根据申请人手机号和身份证号，查询访客在宜搭平台中已录入的个人信息

**查询参数**:

| 参数 | 值 |
|------|----|
| _api | nattyFetch |
| _mock | false |
| _stamp | 当前时间戳(毫秒) |

**表单参数**:

| 参数 | 说明 |
|------|------|
| _csrf_token | CSRF 令牌 |
| _locale_time_zone_offset | 28800000 (UTC+8) |
| appType | 应用类型 |
| formUuid | 表单 UUID |
| linkDataNum | 6 |
| bindingComponentFormulaList | 绑定公式 JSON |
| data | 查询数据 JSON（含手机号和身份证号） |

**请求头**:

| Header | Value |
|--------|-------|
| accept | application/json, text/json |
| accept-language | zh-CN,zh;q=0.9,ja-JP;q=0.8,ja;q=0.7 |
| bx-v | 2.5.11 |
| content-type | application/x-www-form-urlencoded |
| cookie | (宜搭会话 Cookie) |
| dnt | 1 |
| origin | https://iw68lh.aliwork.com |
| priority | u=1, i |
| referer | https://iw68lh.aliwork.com/o/fk_ybfk?account={手机号}&company=... |
| sec-ch-ua | "Not:A-Brand";v="99", "Google Chrome";v="145", "Chromium";v="145" |
| sec-ch-ua-mobile | ?0 |
| sec-ch-ua-platform | "macOS" |
| sec-fetch-dest | empty |
| sec-fetch-mode | cors |
| sec-fetch-site | same-origin |
| user-agent | Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 |
| x-csrf-token | (CSRF 令牌) |
| x-requested-with | XMLHttpRequest |

**响应体**:

```json
{
  "success": true,
  "content": {
    "data": [
      {
        "fieldId": "tableField_lxv44os5",
        "fieldData": {
          "value": [
            [
              { "fieldId": "textField_lxv44orw", "fieldData": { "value": "姓名" } },
              { "fieldId": "textField_lxv44orz", "fieldData": { "value": "电话" } },
              { "fieldId": "textField_lxv44ory", "fieldData": { "value": "身份证号" } },
              { "fieldId": "imageField_ly9i5k5q", "fieldData": { "value": "[...]" } },
              { "fieldId": "attachmentField_lxv44osj", "fieldData": { "value": "[...]" } },
              { "fieldId": "attachmentField_lxv44osk", "fieldData": { "value": "[...]" } }
            ]
          ]
        }
      }
    ]
  }
}
```

**字段映射**:

| 宜搭字段 ID | 含义 |
|-------------|------|
| textField_ly2ugh3m | 申请人手机号（查询条件） |
| textField_lxv44orw | 访客姓名 |
| textField_lxv44orz | 访客电话 |
| textField_lxv44ory | 身份证号（查询条件） |
| imageField_ly9i5k5q | 访客照片 |
| attachmentField_lxv44osj | 身份证照片 |
| attachmentField_lxv44osk | 社保证明 |

---

### 2.2 提交入场申请

- **URL**: `POST https://iw68lh.aliwork.com/o/HW9663A19D6M1QDL6D7GNAO1L2ZC2NBXQHOXL3`
- **源文件**: `src-tauri/src/submit_client.rs` + `src-tauri/src/request_template.rs` + `src-tauri/src/request_template.json`
- **用途**: 向宜搭平台提交访客入场申请表单

**查询参数**:

| 参数 | 值 |
|------|----|
| _api | nattyFetch |
| _mock | false |
| _stamp | 当前时间戳(毫秒) |

**表单参数**:

| 参数 | 说明 |
|------|------|
| _csrf_token | CSRF 令牌 |
| formUuid | 表单 UUID |
| appType | 应用类型 |
| value | 表单数据 JSON 数组（详见下方"value 字段结构"） |
| _schemaVersion | 669 |

**请求头**: 与查询访客信息接口相同

**value 字段结构**:

`value` 是基于 `request_template.json` 模板渲染出的 JSON 数组，每个元素描述一个表单字段。提交时通过 `build_payload()` 注入运行时数据：

| 顺序 | 字段类型 | fieldId | label | 数据来源 |
|------|----------|---------|-------|----------|
| 1 | SerialNumberField | serialNumberField_lxn9o9dx | 单号信息 | 模板留空（由服务端生成） |
| 2 | TextField | textField_lxn9o9e0 | 申请类型 | 固定 `一般访客` |
| 3 | TextField | textField_ly2ugh3m | 申请人ID | 申请人手机号（即 `account`） |
| 4 | TextField | textField_lydnpzas | 地区代码 | 固定 `HA` |
| 5 | TextField | textField_ly3uw4as | 法人代码 | 固定 `1040` |
| 6 | TextField | textField_ly3uw4ar | 园区代码 | 固定 `HB` |
| 7 | TextField | textField_m2lk8mr2 | 供应商code | 固定 `VCN00507` |
| 8 | RadioField | radioField_m4g9sf7c | 是否外籍 | 固定 `否` |
| 9 | SelectField | selectField_ly3o95xh | 到访园区 | 固定 `淮安第二园区` |
| 10 | SelectField | selectField_ly3o95xf | 到访公司 | 固定 `庆鼎精密电子(淮安)有限公司` |
| 11 | SelectField | selectField_lxn9o9eb | 身份类型 | 固定 `生产服务（厂商）` |
| 12 | SelectField | selectField_lxn9o9ed | 服务性质/到访事由 | 固定 `设备维护` |
| 13 | SelectField | selectField_lxn9o9ei | 到访区域 | 默认 `生产区域外围（不进入制造现场）`，特定接待人会被覆盖（详见下方业务规则） |
| 14 | TextareaField | textareaField_lxn9o9eg | 服务/事由描述 | 固定 `镭射机维护保养` |
| 15 | SelectField | selectField_lxn9o9em | 所属公司 | 固定 `VCN00507(镭富电子设备(上海)有限公司)` |
| 16 | TextField | textField_lxn9o9gc | 所属公司/单位名称 | 固定 `VCN00507(镭富电子设备(上海)有限公司)` |
| 17 | RadioField | radioField_lzs3fswt | 是否为竞商？ | 固定 `否` |
| 18 | TableField | tableField_lxv44os5 | 人员信息 | 由前端传入的访客数组构造（每个访客一行，详见下方"人员信息行结构"） |
| 19 | TextField | textField_lxn9o9f9 | 接待人工号 | 接待人 `employee_id` |
| 20 | TextField | textField_lxn9o9f7 | 接待人员 | 接待人 `name` |
| 21 | TextField | textField_lxn9o9fc | 接待部门 | 接待人 `department` |
| 22 | TextField | textField_lxn9o9fe | 接待人联系方式 | 接待人 `phone` |
| 23 | DateField | dateField_lxn9o9fh | 到访日期 | 当日 UTC+8 零点的毫秒时间戳（`to_midnight_timestamp_ms`） |
| 24 | TextField | textField_mjdmoase | 到访日期文本 | `yyyy-MM-dd` 格式日期字符串（`to_date_text`） |
| 25 | TextField | textField_m4c5a419 | 涉外签核 | 模板留空 |
| 26 | TextField | textField_m4c5a41a | 门岗保安 | 固定 `15851745806` |

**人员信息行结构**（`tableField_lxv44os5.fieldData.value` 数组中每个元素是一行）:

| 字段类型 | fieldId | label | 数据来源 |
|----------|---------|-------|----------|
| SelectField | selectField_lxv44orx | 有效身份证件 | 固定 `身份证` |
| TextField | textField_lxv44ory | 证件号码 | `visitor.id_card` |
| TextField | textField_lxv44orw | 姓名 | `visitor.name` |
| SelectField | selectField_mbyjhot6 | 区号 | 固定 `86` / `+86` |
| TextField | textField_lxv44orz | 联系方式 | `visitor.phone` |
| ImageField | imageField_ly9i5k5q | 免冠照片 | `visitor.photo`（来自 2.1 查询） |
| AttachmentField | attachmentField_lxv44osj | 身份证照片 | `visitor.id_photo`（来自 2.1 查询） |
| AttachmentField | attachmentField_lxv44osk | 社保/在职证明 | `visitor.social_proof`（来自 2.1 查询） |
| AttachmentField | attachmentField_lxv44osn | 其他附件 | 固定空数组 |

> 一次申请支持多名访客，对应 `value` 数组中"人员信息"行可重复多次。

**特殊业务规则**：

- 当 `reception.employee_id == "52091191"` 时，"到访区域"会被覆盖为：
  - `value` = `进入制造现场`
  - `text` = `进入车间/管制区域`
  - 同时替换 `options` 数组的取值

  常量定义在 `request_template.rs` 中的 `SPECIAL_VISIT_AREA_RECEPTION_ID`。

**响应体**:

```json
{
  "success": true,
  ...
}
```

**响应处理规则**:

- HTTP 非 2xx → 返回 `SubmitError`（`response_raw` 含原始响应）
- 响应体不能解析为 JSON → 返回 `SubmitError`
- `success` 不为 `true` → 返回 `SubmitError`（标记为业务失败）
- `success=true` → 提交成功

---

## 三、钉钉表单搜索 API（接待人查询）

**地址**: `POST https://dingtalk.avaryholding.com:8443/dingplus/searchFormData`

**源文件**: `src-tauri/src/reception_client.rs`

**用途**: 根据员工工号查询接待人的姓名、部门和电话

**请求体**:

```json
{
  "formUUid": "FORM-B965E22437E1415BBBBA33011BF20FB54VP8",
  "appType": "APP_GRVPTEOQ6D4B7FLZFYNJ",
  "systemToken": "DC666GC1PN6LT8C7C64FD9N62P2E3F9V1SFWLKQ61",
  "json": "{\"employeeField_m3o6fym4\": [\"员工工号\"]}"
}
```

**请求头**:

| Header | Value |
|--------|-------|
| accept | application/json, text/json |
| accept-language | zh-CN,zh;q=0.9,ja-JP;q=0.8,ja;q=0.7 |
| content-type | application/json |
| dnt | 1 |
| origin | https://iw68lh.aliwork.com |
| referer | https://iw68lh.aliwork.com/ |
| sec-ch-ua | "Not:A-Brand";v="99", "Google Chrome";v="145", "Chromium";v="145" |
| sec-ch-ua-mobile | ?0 |
| sec-ch-ua-platform | "macOS" |
| sec-fetch-dest | empty |
| sec-fetch-mode | cors |
| sec-fetch-site | cross-site |
| sec-fetch-storage-access | active |
| user-agent | Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 |

**响应体**:

```json
{
  "body": {
    "data": [
      {
        "formData": {
          "textField_m3pkk1ez": "姓名",
          "textField_m3pgo9p1": "部门",
          "textField_m3pollg0": "电话"
        }
      }
    ]
  }
}
```

**字段映射**:

| 宜搭字段 ID | 含义 |
|-------------|------|
| employeeField_m3o6fym4 | 员工工号（查询条件） |
| textField_m3pkk1ez | 接待人姓名 |
| textField_m3pgo9p1 | 接待人部门 |
| textField_m3pollg0 | 接待人电话 |

---

## 四、接口调用流程

```
┌─────────────────────────────────────────────────────┐
│                     用户操作                          │
└─────────────────────┬───────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────┐
│  1. 输入手机号                                       │
│     → POST /sendCode        （发送验证码）            │
│     → POST /visitorLogin    （登录获取 acToken）      │
│     → POST /visitorStatus   （验证 Token 有效性）     │
└─────────────────────┬───────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────┐
│  2. 输入访客身份证号                                  │
│     → POST 宜搭 nattyFetch  （查询访客已有信息）      │
└─────────────────────┬───────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────┐
│  3. 输入接待人工号                                    │
│     → POST /searchFormData  （查询接待人信息）         │
└─────────────────────┬───────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────┐
│  4. 选择日期并提交                                    │
│     → POST 宜搭 nattyFetch  （提交入场申请表单）      │
└─────────────────────────────────────────────────────┘
```

---

## 五、HTTP 客户端说明

项目使用 Rust `reqwest` 库，在 `src-tauri/src/http_common.rs` 中定义了两个客户端构造函数：

| 构造函数 | 主要用途 | 默认 User-Agent | 超时设置 |
|----------|----------|-----------------|----------|
| `yida_client()` | 宜搭平台请求（查询访客、提交申请） | 未设置（每次请求显式设置桌面端 Chrome 145 UA） | 连接 10s / 请求 30s |
| `auth_client()` | 钉钉认证 API（发送验证码、登录、Token 校验、状态查询） | 未设置（每次请求显式设置钉钉移动端 UA） | 连接 10s / 请求 30s |

> **说明**: 两个客户端构造函数本身不内置默认 Header；具体的 `User-Agent`、`Origin`、`Referer` 等头部由各调用方在每次请求时显式指定。`yida_client` 与 `auth_client` 实际差异仅在调用方传入的头部和指向的目标域名。
>
> **接待人查询接口**（`/searchFormData`，源文件 `src-tauri/src/reception_client.rs`）使用 `reqwest::Client::new()` 创建独立临时客户端，**没有自定义连接/请求超时**，使用 reqwest 默认行为。该客户端使用与 `yida_client` 相同的桌面端 Chrome 145 UA。

---

## 六、业务常量

| 常量 | 值 | 用途 |
|------|----|------|
| COMPANY | 庆鼎精密电子(淮安)有限公司 | 构建 referer 中的公司参数 |
| PART | 淮安第二园区 | 构建 referer 中的园区参数 |
| APPLY_TYPE | 一般访客 | 构建 referer 中的访客类型参数 |
