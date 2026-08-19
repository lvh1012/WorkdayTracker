import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { formatDate, formatDurationBetween, formatTime } from "./format";
import { ALL_FILTER_VALUE, filterHistory, getHistoryYears } from "./history";
import {
  parseThemePreference,
  resolveTheme,
  THEME_STORAGE_KEY,
  type ThemePreference,
} from "./theme";
import type { Dashboard, WorkdayStatus, WorkdaySummary } from "./types";

const statusLabels: Record<WorkdayStatus, string> = {
  active: "Đang theo dõi",
  departure_pending: "Đang chờ 30 phút",
  complete: "Đã hoàn tất",
};

const months = Array.from({ length: 12 }, (_, index) => ({
  value: (index + 1).toString().padStart(2, "0"),
  label: `Tháng ${index + 1}`,
}));

const themeLabels: Record<ThemePreference, string> = {
  light: "Sáng",
  dark: "Tối",
  system: "Hệ thống",
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

function getDisplayedDuration(workday: WorkdaySummary, nowTimestampMs: number): string {
  return formatDurationBetween(workday.arrivalMs, workday.departureMs ?? nowTimestampMs);
}

function HistoryRow({ workday, nowTimestampMs }: { workday: WorkdaySummary; nowTimestampMs: number }) {
  const departure =
    workday.status === "departure_pending"
      ? `Chờ từ ${formatTime(workday.pendingDepartureMs)}`
      : formatTime(workday.departureMs);

  return (
    <tr>
      <td>{formatDate(workday.date)}</td>
      <td>{formatTime(workday.arrivalMs)}</td>
      <td>{departure}</td>
      <td className="duration-cell">{getDisplayedDuration(workday, nowTimestampMs)}</td>
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
  const [selectedYear, setSelectedYear] = useState(ALL_FILTER_VALUE);
  const [selectedMonth, setSelectedMonth] = useState(ALL_FILTER_VALUE);
  const [themePreference, setThemePreference] = useState<ThemePreference>(() =>
    parseThemePreference(window.localStorage.getItem(THEME_STORAGE_KEY)),
  );
  const latestDashboardRequestId = useRef(0);

  const loadDashboard = useCallback(async () => {
    const requestId = ++latestDashboardRequestId.current;
    try {
      const nextDashboard = await invoke<Dashboard>("get_dashboard");
      if (requestId === latestDashboardRequestId.current) {
        setDashboard(nextDashboard);
        setError(null);
      }
    } catch (reason) {
      if (requestId === latestDashboardRequestId.current) {
        setError(String(reason));
      }
    } finally {
      if (requestId === latestDashboardRequestId.current) {
        setLoading(false);
      }
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

  useLayoutEffect(() => {
    const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");

    const applyTheme = () => {
      const resolvedTheme = resolveTheme(themePreference, systemTheme.matches);
      document.documentElement.dataset.theme = resolvedTheme;
      document.documentElement.style.colorScheme = resolvedTheme;
    };

    // Persist the preference, not the resolved value, so "system" keeps following Windows.
    window.localStorage.setItem(THEME_STORAGE_KEY, themePreference);
    applyTheme();
    systemTheme.addEventListener("change", applyTheme);
    return () => systemTheme.removeEventListener("change", applyTheme);
  }, [themePreference]);

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
  const nowTimestampMs = Date.now();
  const historyYears = getHistoryYears(dashboard.history);
  const filteredHistory = filterHistory(dashboard.history, selectedYear, selectedMonth);

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
          value={today ? getDisplayedDuration(today, nowTimestampMs) : "00:00"}
          hint="Tính theo minute precision của giờ hiển thị"
        />
      </section>

      <section className="panel">
        <div className="panel-heading">
          <div>
            <h2>Lịch sử</h2>
            <p>
              Hiển thị {filteredHistory.length}/{dashboard.history.length} ngày từ immutable event log.
            </p>
          </div>
          <div className="history-filters" aria-label="Lọc lịch sử">
            <label className="filter-field">
              <span>Năm</span>
              <select value={selectedYear} onChange={(event) => setSelectedYear(event.target.value)}>
                <option value={ALL_FILTER_VALUE}>Tất cả</option>
                {historyYears.map((year) => (
                  <option key={year} value={year}>{year}</option>
                ))}
              </select>
            </label>
            <label className="filter-field">
              <span>Tháng</span>
              <select value={selectedMonth} onChange={(event) => setSelectedMonth(event.target.value)}>
                <option value={ALL_FILTER_VALUE}>Tất cả</option>
                {months.map((month) => (
                  <option key={month.value} value={month.value}>{month.label}</option>
                ))}
              </select>
            </label>
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
              {filteredHistory.length === 0 ? (
                <tr>
                  <td colSpan={5} className="empty-cell">Không có dữ liệu phù hợp bộ lọc.</td>
                </tr>
              ) : (
                filteredHistory.map((workday) => (
                  <HistoryRow key={workday.date} workday={workday} nowTimestampMs={nowTimestampMs} />
                ))
              )}
            </tbody>
          </table>
        </div>
      </section>

      <section className="panel settings-panel">
        <div className="settings-row">
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
        </div>
        <div className="settings-divider" />
        <div className="settings-row">
          <div>
            <h2>Giao diện</h2>
            <p>Theme được lưu local; chế độ hệ thống tự theo Windows.</p>
          </div>
          <div className="theme-switch" role="group" aria-label="Chọn theme">
            {(Object.keys(themeLabels) as ThemePreference[]).map((theme) => (
              <button
                key={theme}
                type="button"
                className={themePreference === theme ? "active" : undefined}
                aria-pressed={themePreference === theme}
                onClick={() => setThemePreference(theme)}
              >
                {themeLabels[theme]}
              </button>
            ))}
          </div>
        </div>
      </section>

      <footer>Data: %LOCALAPPDATA%\\com.lvh1012.workdaytracker\\workday-tracker.db</footer>
    </main>
  );
}
