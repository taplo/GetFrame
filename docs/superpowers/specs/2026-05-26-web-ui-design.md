# Phase 7: Web UI — Stream & Task Management

## Overview

React SPA for managing video streams, extraction tasks, and system monitoring. Communicates with the existing Axum REST API (`/api/v1/*`).

## Tech Stack

- **Framework**: React 19 + TypeScript 5
- **Build**: Vite 6
- **UI**: shadcn/ui (Radix primitives + Tailwind CSS 4)
- **Routing**: React Router v7
- **HTTP**: fetch API (via Vite proxy → Axum backend)
- **State**: React hooks + context (no external state library)

## Page Structure

Three pages, SPA with React Router:

### 1. Dashboard (`/`)

- **4 stat cards**: 在线流数 / 活跃任务数 / 今日抽帧数 / 错误数
- **最近流状态**: compact list with Online/Offline/Error indicators + frame rate
- **最近任务**: compact list with Running/Paused/Stopped badges

### 2. Streams (`/streams`)

- **列表**: checkbox + status/name/URL/type/tags column, search, status filter
- **批量操作**: dropdown for batch delete, batch update tags
- **导出 CSV**: download current filter result as CSV
- **导入 CSV**: download template → upload file → preview validation → confirm
- **新建/编辑**: dialog form (name, URL, type auto-detect, tags, description)
- **行操作**: edit, rules (navigate to child view), delete

### 3. Tasks (`/tasks`)

- **列表**: checkbox + status/name/stream/rule/frames/actions, search, status filter
- **批量操作**: dropdown for batch start/pause/stop/delete
- **导出 CSV**: download current filter result as CSV
- **导入 CSV**: download template → upload file → preview validation → confirm
- **新建**: dialog form (name, stream select, rule type + params)
- **行操作**: start/pause/resume/stop/delete per row

## API Client Layer

- `src/api/streams.ts` — CRUD + test connection
- `src/api/rules.ts` — CRUD per stream
- `src/api/tasks.ts` — CRUD + lifecycle (start/pause/resume/stop)
- `src/api/health.ts` — health/ready endpoints

## Data Flow

1. Vite dev server proxies `/api/*` → backend dev server (configurable via `vite.config.ts` `server.proxy`)
2. Production: Axum `ServeDir` serves built frontend from `web/dist/` at `/` root
3. Each page fetches data on mount; mutations trigger list refresh

## Directory Structure

```
web/
├── index.html
├── package.json
├── tsconfig.json
├── vite.config.ts
├── src/
│   ├── main.tsx
│   ├── App.tsx            # Router setup
│   ├── pages/
│   │   ├── Dashboard.tsx
│   │   ├── Streams.tsx
│   │   └── Tasks.tsx
│   ├── components/
│   │   ├── Layout.tsx      # Nav + content wrapper
│   │   ├── StatCard.tsx
│   │   ├── StreamTable.tsx
│   │   ├── TaskTable.tsx
│   │   ├── StreamForm.tsx  # Create/edit dialog
│   │   ├── TaskForm.tsx    # Create dialog
│   │   ├── ImportDialog.tsx
│   │   └── ConfirmDialog.tsx
│   ├── api/
│   │   ├── client.ts       # Base fetch wrapper
│   │   ├── streams.ts
│   │   ├── rules.ts
│   │   ├── tasks.ts
│   │   └── health.ts
│   └── types/
│       ├── stream.ts
│       ├── task.ts
│       └── rule.ts
```

## Backend Dependencies

- **Batch import**: Frontend sends array of records to the existing `POST /api/v1/streams` and `POST /api/v1/tasks` endpoints (one request per item, with error aggregation). Dedicated batch endpoints can be added later if performance requires.
- **CORS / static serving**: Production build is served by Axum via `tower-http::services::ServeDir`. Dev uses Vite proxy (`server.proxy` in `vite.config.ts`).

## Notes

- shadcn/ui components installed via `npx shadcn@latest add`
- No i18n library — UI is Chinese-first
- CSV parsing via `papaparse` or manual `Blob.text()` + split
