import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { formatDate, formatDuration, formatTime } from "./format";
import type { Dashboard, WorkdayStatus, WorkdaySummary } from "./types";

const statusLabels: Record<WorkdayStatus, string> = {
  active: "Đang theo dõi",
  departure_pending: "Đang chờ 30 phút",
  complete: "Đã hoàn tất",
};

function SummaryCard({ label, value, hint }: { label: string; value: string; hint: string }) {
  return (
    <article className="summary-card">
      <p className="summary-label">{label}</p>
      <strong>{value}</strong>
      <p className="summary-hint">{hint}</p>
    </article>
  );
}

function HistoryRow({ workday }: { workday: WorkdaySummary }) {
  const departure =
    workday.status === "departure_pending"
      ? `Chờ từ ${formatTime(workday.pendingDepartureMs)}`
      : formatTime(workday.departureMs);

  return (
    <tr>
      <td>{formatDate(workday.date)}</td>
      <td>{formatTime(workday.arrivalMs)}</td>
      <td>{departure}</td>
      <td className="duration-cell">{formatDuration(workday.durationSeconds)}</td>
      <td>
        <span className={`status status-${workday.status}`}>{statusLabels[workday.status]}</span>
      </td>
    </tr>
  );
}

export default function App() {
  const [dashboard, setDashboard] = useState<Dashboard | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [savingSetting, setSavingSetting] = useState(false);

  const loadDashboard = useCallback(async () => {
    try {
      setDashboard(await invoke<Dashboard>("get_dashboard"));
      setError(null);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadDashboard();

    // Rust emits this event after session state or the 30-minute projection changes.
    const unlistenPromise = listen("workday-updated", () => void loadDashboard());
    const refreshTimer = window.setInterval(() => void loadDashboard(), 60_000);

    return () => {
      window.clearInterval(refreshTimer);
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [loadDashboard]);

  async function updateAutostart(enabled: boolean) {
    setSavingSetting(true);
    try {
      await invoke("set_autostart_enabled", { enabled });
      await loadDashboard();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSavingSetting(false);
    }
  }

  if (loading) {
    return <main className="center-state">Đang tải dữ liệu local…</main>;
  }

  if (!dashboard) {
    return (
      <main className="center-state error-state">
        <strong>Không thể đọc dữ liệu</strong>
        <p>{error}</p>
        <button onClick={() => void loadDashboard()}>Thử lại</button>
      </main>
    );
  }

  const today = dashboard.today;

  return (
    <main className="app-shell">
      <header className="page-header">
        <div>
          <p className="eyebrow">LOCAL • SINGLE USER</p>
          <h1>Workday Tracker</h1>
          <p className="subtitle">Theo dõi thời điểm đến và rời công ty từ Windows session.</p>
        </div>
        <span className={`status status-${today?.status ?? "active"}`}>
          {today ? statusLabels[today.status] : "Chưa có dữ liệu"}
        </span>
      </header>

      {error && <aside className="error-banner">{error}</aside>}

      <section className="summary-grid" aria-label="Tổng quan hôm nay">
        <SummaryCard
          label="Đến"
          value={formatTime(today?.arrivalMs ?? null)}
          hint="Lần app khởi động đầu tiên hôm nay"
        />
        <SummaryCard
          label="Rời"
          value={
            today?.status === "departure_pending"
              ? formatTime(today.pendingDepartureMs)
              : formatTime(today?.departureMs ?? null)
          }
          hint={today?.status === "departure_pending" ? "Candidate, chưa final" : "Lần rời cuối cùng"}
        />
        <SummaryCard
          label="Tổng thời gian"
          value={formatDuration(today?.durationSeconds ?? 0)}
          hint="Tính từ giờ đến tới hiện tại/giờ rời"
        />
      </section>

      <section className="panel">
        <div className="panel-heading">
          <div>
            <h2>Lịch sử</h2>
            <p>Daily projection được tính từ immutable event log.</p>
          </div>
        </div>

        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>Ngày</th>
                <th>Đến</th>
                <th>Rời</th>
                <th>Tổng</th>
                <th>Trạng thái</th>
              </tr>
            </thead>
            <tbody>
              {dashboard.history.length === 0 ? (
                <tr>
                  <td colSpan={5} className="empty-cell">Chưa có lịch sử.</td>
                </tr>
              ) : (
                dashboard.history.map((workday) => <HistoryRow key={workday.date} workday={workday} />)
              )}
            </tbody>
          </table>
        </div>
      </section>

      <section className="panel settings-panel">
        <div>
          <h2>Khởi động cùng Windows</h2>
          <p>App chạy sau khi user hiện tại đăng nhập; không cần quyền admin.</p>
        </div>
        <label className="switch">
          <input
            type="checkbox"
            checked={dashboard.autostartEnabled}
            disabled={savingSetting}
            onChange={(event) => void updateAutostart(event.target.checked)}
          />
          <span className="slider" aria-hidden="true" />
          <span className="sr-only">Khởi động cùng Windows</span>
        </label>
      </section>

      <footer>Data: %LOCALAPPDATA%\\com.lvh1012.workdaytracker\\workday-tracker.db</footer>
    </main>
  );
}

