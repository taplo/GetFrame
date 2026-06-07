# 前端补全设计文档

> 2026-06-06 | 对应 UI-10: 规则管理 / UI-11: 流详情 / UI-12: 任务创建

## 概述

补全 GetFrame 前端缺失的三个核心页面，共享新建 RuleEditor 组件。

## 后端改动

### 全局规则 API

新增端点（在 `src/api/rules.rs` 中实现）：

```
GET /api/v1/rules?stream_id=<uuid>&type=<string>
```

- 从 `StreamRegistry` 收集所有流规则（in-memory，无需数据库）
- 支持按 `stream_id` 和规则 `type` 过滤
- admin-only 认证
- utoipa 文档化

**响应结构：**

```json
{
  "rules": [
    {
      "stream_id": "uuid",
      "stream_name": "Camera-01",
      "source_url": "rtsp://...",
      "index": 0,
      "rule": { "type": "interval", "interval_seconds": 5.0 }
    }
  ]
}
```

**实现方式：**

- `streams_routes` 已经在路由器中持有 `StreamManager` 作为状态
- 全局规则路由使用 `StreamManager` 的 `registry()` 方法遍历已注册流
- 路由挂载：`/api/v1/rules` 使用独立的 `Router`，同样注入 `StreamManager`

### GlobalRuleItem/RulesListResponse 数据类型

新增序列化类型用于响应。

### 前端 API 扩展

在 `web/src/api/rules.ts` 中新增方法：

```typescript
rulesApi.listGlobal(params?: { stream_id?: string; type?: string })
  → Promise<{ rules: GlobalRuleItem[] }>
```

## 前端改动

### 共享组件：RuleEditor

将现有 `components/TaskForm.tsx` 中的规则编辑器抽取为独立组件：

```
components/RuleEditor.tsx
```

支持功能：
- 规则列表展示（类型标签 + 配置摘要）
- 添加规则（选择类型 → 填充参数 → 确认）
- 编辑规则（修改规则参数）
- 删除规则（确认弹窗）
- 复合规则嵌套编辑

被 `RulesPage`, `StreamDetail`, `TaskCreatePage` 三个页面共享。

### 新路由表

```
/streams         流管理列表（已有）
/streams/:id     流详情页
/rules           规则全局管理
/tasks           任务列表（已有）
/tasks/create    任务创建页
/tasks/:id       任务详情（已有）
/activity        活动日志（已有）
```

导航栏新增"规则管理"链接。

### 页面 1: `RulesPage` (`/rules`)

| 区域 | 内容 |
|------|------|
| 标题栏 | "规则管理" + "新建规则" 按钮 |
| 筛选栏 | 流选择器 dropdown + 规则类型 dropdown |
| 表格 | 流名称（可跳转）→ 规则类型 → 配置 → 索引 → 操作按钮 |
| 操作 Modal | 复用 RuleEditor 做新建/编辑 |

### 页面 2: `StreamDetail` (`/streams/:id`)

| 区块 | 内容 |
|------|------|
| 基本信息卡片 | 名称、源 URL、类型标签、状态标签、帧数/FPS/在线时长、编辑/删除按钮 |
| 规则管理区 | 嵌入的规则 CRUD（使用 RuleEditor） |
| 最新帧预览 | FramePreview 组件加载最新帧缩略图 |

### 页面 3: `TaskCreatePage` (`/tasks/create`)

| 区域 | 内容 |
|------|------|
| 标题 | "新建任务" + 返回按钮 |
| 表单 | 任务名称 input |
| 流选择器 | Dropdown 列表（显示流名+状态） |
| 规则编辑器 | 嵌入 RuleEditor |
| 操作栏 | "取消" 返回列表 / "创建" 调 API 后跳转详情 |

### 文件清单

```
web/src/pages/RulesPage.tsx        — 新
web/src/pages/StreamDetail.tsx     — 新
web/src/pages/TaskCreatePage.tsx   — 新
web/src/components/RuleEditor.tsx  — 新（从 TaskForm 抽取）
web/src/api/rules.ts               — 新增全局规则 API 方法
web/src/types/rule.ts              — 扩展类型（GlobalRuleItem）
web/src/App.tsx                    — 添加路由
web/src/components/Layout.tsx      — 添加导航链接
web/src/components/TaskForm.tsx    — 重构为使用 RuleEditor
```

## 不纳入范围

- Task Gantt 甘特图时间线（暂缓）
- 规则拖拽排序（后续迭代）
- 批量规则导入/导出
- 流详情页的活动日志时间线
