import { request } from "./client"

export const healthApi = {
  health: () => request<{ status: string }>("/health"),
  ready: () => request<{ ready: boolean }>("/ready"),
}
