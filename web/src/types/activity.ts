export interface ActivityEvent {
  id: number;
  event_type: string;
  resource_type: string;
  resource_id: string | null;
  actor: string;
  description: string;
  details: Record<string, unknown> | null;
  recorded_at: string;
}

export interface ActivityQuery {
  event_type?: string;
  resource_type?: string;
  actor?: string;
  search?: string;
  since?: string;
  until?: string;
  page?: number;
  page_size?: number;
}

export interface ActivityListResponse {
  items: ActivityEvent[];
  total: number;
  page: number;
  page_size: number;
}
