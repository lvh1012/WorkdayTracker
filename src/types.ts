export type WorkdayStatus = "active" | "departure_pending" | "complete";

export interface WorkdaySummary {
  date: string;
  arrivalMs: number;
  departureMs: number | null;
  pendingDepartureMs: number | null;
  durationSeconds: number;
  status: WorkdayStatus;
}

export interface Dashboard {
  today: WorkdaySummary | null;
  history: WorkdaySummary[];
  autostartEnabled: boolean;
}

