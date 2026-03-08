import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { expandDateRange } from "./dateRange";
import "./App.css";

type VisitorInfo = {
  idCard: string;
  name: string;
  phone: string;
  photo: unknown;
  idPhoto: unknown;
};

type ReceptionInfo = {
  employeeId: string;
  name: string;
  department: string;
  phone: string;
};

type VisitorRow = {
  idCard: string;
  loading: boolean;
  info?: VisitorInfo;
  error?: string;
};

type BatchLogItem = {
  date?: string;
  result: string;
  reason?: string;
  waitSeconds?: number;
  responseRaw?: string;
};

type BatchLogPayload = {
  date?: unknown;
  result?: unknown;
  reason?: unknown;
  waitSeconds?: unknown;
  responseRaw?: unknown;
};

type HistoryRecord = {
  date: string;
  submittedAt: string;
};

type FormState = {
  account: string;
  visitorIdCards: string[];
  receptionId: string;
};

function normalizeLog(payload: BatchLogPayload): BatchLogItem {
  return {
    date: typeof payload.date === "string" ? payload.date : undefined,
    result: typeof payload.result === "string" ? payload.result : "unknown",
    reason: typeof payload.reason === "string" ? payload.reason : undefined,
    waitSeconds:
      typeof payload.waitSeconds === "number" ? payload.waitSeconds : undefined,
    responseRaw:
      typeof payload.responseRaw === "string" ? payload.responseRaw : undefined,
  };
}

function formatCountdown(totalSeconds: number): string {
  const safeSeconds = Math.max(0, totalSeconds);
  const minutes = Math.floor(safeSeconds / 60)
    .toString()
    .padStart(2, "0");
  const seconds = (safeSeconds % 60).toString().padStart(2, "0");
  return `${minutes}:${seconds}`;
}

function formatHistoryTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return iso;
  }
  return date.toLocaleString("zh-CN", { hour12: false });
}

function App() {
  const [account, setAccount] = useState("");
  const [visitors, setVisitors] = useState<VisitorRow[]>([
    { idCard: "", loading: false },
  ]);
  const [receptionId, setReceptionId] = useState("");
  const [receptionLoading, setReceptionLoading] = useState(false);
  const [receptionInfo, setReceptionInfo] = useState<ReceptionInfo | undefined>();
  const [receptionError, setReceptionError] = useState<string | undefined>();

  const [startDate, setStartDate] = useState("");
  const [endDate, setEndDate] = useState("");
  const [dates, setDates] = useState<string[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [logs, setLogs] = useState<BatchLogItem[]>([]);
  const [historyRecords, setHistoryRecords] = useState<HistoryRecord[]>([]);
  const [existingDates, setExistingDates] = useState<string[]>([]);
  const [countdownSeconds, setCountdownSeconds] = useState<number | null>(null);
  const [processedCount, setProcessedCount] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | undefined>();

  const existingDateSet = useMemo(() => new Set(existingDates), [existingDates]);

  const allVisitorsReady = visitors.length > 0 && visitors.every((v) => v.info && !v.loading);
  const receptionReady = !!receptionInfo && !receptionLoading;
  const canSubmit =
    account.trim().length > 0 &&
    allVisitorsReady &&
    receptionReady &&
    dates.length > 0 &&
    !isRunning;

  const loadRecentHistory = async () => {
    try {
      const records = await invoke<HistoryRecord[]>("get_recent_history");
      setHistoryRecords(records);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  };

  const syncExistingDates = async (targetDates: string[]) => {
    if (targetDates.length === 0) {
      setExistingDates([]);
      return [] as string[];
    }
    try {
      const existing = await invoke<string[]>("get_existing_dates", {
        dates: targetDates,
      });
      setExistingDates(existing);
      return existing;
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
      return [] as string[];
    }
  };

  useEffect(() => {
    void loadRecentHistory();
    // Load saved form state
    invoke<FormState | null>("load_form_state")
      .then((saved) => {
        if (!saved) return;
        if (saved.account) setAccount(saved.account);
        if (saved.receptionId) setReceptionId(saved.receptionId);
        if (saved.visitorIdCards && saved.visitorIdCards.length > 0) {
          const rows: VisitorRow[] = saved.visitorIdCards.map((idCard) => ({
            idCard,
            loading: false,
          }));
          setVisitors(rows);
        }
      })
      .catch((error) => {
        setErrorMessage(
          `加载表单状态失败: ${error instanceof Error ? error.message : String(error)}`
        );
      });
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    listen<BatchLogPayload>("batch-log", (event) => {
      const item = normalizeLog(event.payload ?? {});
      setLogs((prev) => [...prev, item]);

      if (item.result === "success" || item.result === "skipped") {
        setProcessedCount((prev) => prev + 1);
      }

      if (typeof item.waitSeconds === "number" && item.waitSeconds > 0) {
        setCountdownSeconds(item.waitSeconds);
      } else if (
        item.result === "success" ||
        item.result === "skipped" ||
        item.result === "failed" ||
        item.result === "stopped"
      ) {
        setCountdownSeconds(null);
      }

      if (item.result === "failed" || item.result === "stopped") {
        setIsRunning(false);
      }
    })
      .then((unlistenFn) => {
        if (disposed) {
          unlistenFn();
          return;
        }
        unlisten = unlistenFn;
      })
      .catch((error) => {
        setErrorMessage(`日志监听失败: ${String(error)}`);
      });

    return () => {
      disposed = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  useEffect(() => {
    if (!isRunning) {
      return;
    }
    const timer = window.setInterval(() => {
      setCountdownSeconds((prev) => {
        if (prev === null) return null;
        if (prev <= 1) return 0;
        return prev - 1;
      });
    }, 1000);
    return () => {
      window.clearInterval(timer);
    };
  }, [isRunning]);

  useEffect(() => {
    let disposed = false;
    let unlistenClose: (() => void) | undefined;

    getCurrentWindow()
      .onCloseRequested(async (event) => {
        const hasUnfinishedTask = isRunning && processedCount < dates.length;
        const message = hasUnfinishedTask
          ? "当前任务未完成，确认关闭软件吗？"
          : "确认关闭软件吗？";
        const confirmed = window.confirm(message);
        if (!confirmed) {
          event.preventDefault();
        }
      })
      .then((unlistenFn) => {
        if (disposed) {
          unlistenFn();
          return;
        }
        unlistenClose = unlistenFn;
      })
      .catch((error) => {
        setErrorMessage(`关闭事件监听失败: ${String(error)}`);
      });

    return () => {
      disposed = true;
      if (unlistenClose) {
        unlistenClose();
      }
    };
  }, [isRunning, processedCount, dates.length]);

  const queryVisitor = async (index: number) => {
    const row = visitors[index];
    if (!row || !row.idCard.trim() || !account.trim()) {
      setErrorMessage("请先填写申请人手机号和访客身份证号");
      return;
    }

    setVisitors((prev) =>
      prev.map((v, i) =>
        i === index ? { ...v, loading: true, error: undefined } : v
      )
    );

    try {
      const info = await invoke<VisitorInfo>("fetch_visitor_info", {
        account: account.trim(),
        idCard: row.idCard.trim(),
      });
      setVisitors((prev) =>
        prev.map((v, i) =>
          i === index ? { ...v, loading: false, info, error: undefined } : v
        )
      );
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      setVisitors((prev) =>
        prev.map((v, i) =>
          i === index
            ? { ...v, loading: false, info: undefined, error: msg }
            : v
        )
      );
    }
  };

  const addVisitor = () => {
    setVisitors((prev) => [...prev, { idCard: "", loading: false }]);
  };

  const removeVisitor = (index: number) => {
    if (visitors.length <= 1) return;
    setVisitors((prev) => prev.filter((_, i) => i !== index));
  };

  const updateVisitorIdCard = (index: number, value: string) => {
    setVisitors((prev) =>
      prev.map((v, i) =>
        i === index ? { ...v, idCard: value, info: undefined, error: undefined } : v
      )
    );
  };

  const queryReception = async () => {
    if (!receptionId.trim()) {
      setErrorMessage("请填写接待人员工号");
      return;
    }

    setReceptionLoading(true);
    setReceptionError(undefined);

    try {
      const info = await invoke<ReceptionInfo>("fetch_reception_info", {
        employeeId: receptionId.trim(),
      });
      setReceptionInfo(info);
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      setReceptionError(msg);
      setReceptionInfo(undefined);
    } finally {
      setReceptionLoading(false);
    }
  };

  useEffect(() => {
    if (!startDate || !endDate || isRunning) return;
    try {
      const expanded = expandDateRange(startDate, endDate);
      setDates(expanded);
      setErrorMessage(undefined);
      void syncExistingDates(expanded);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "日期生成失败");
    }
  }, [startDate, endDate]);

  const removeDate = (target: string) => {
    if (isRunning) return;
    setDates((prev) => {
      const next = prev.filter((item) => item !== target);
      setExistingDates((existing) => existing.filter((item) => item !== target));
      return next;
    });
  };

  const startSubmit = async () => {
    if (!canSubmit) return;

    const confirmedVisitors = visitors
      .filter((v) => v.info)
      .map((v) => v.info as VisitorInfo);

    if (confirmedVisitors.length === 0 || !receptionInfo) {
      setErrorMessage("请先查询所有访客和接待人信息");
      return;
    }

    try {
      setErrorMessage(undefined);
      await syncExistingDates(dates);
      setLogs([]);
      setProcessedCount(0);
      setCountdownSeconds(null);
      setIsRunning(true);
      await invoke("start_batch_submit", {
        account: account.trim(),
        visitors: confirmedVisitors,
        reception: receptionInfo,
        dates,
      });
      window.alert("任务已完成");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (!message.includes("batch stopped manually")) {
        setErrorMessage(message);
      }
    } finally {
      setIsRunning(false);
      setCountdownSeconds(null);
      await loadRecentHistory();
      await syncExistingDates(dates);
    }
  };

  const stopSubmit = async () => {
    if (!isRunning) return;
    const hasUnfinishedTask = processedCount < dates.length;
    if (hasUnfinishedTask) {
      const confirmed = window.confirm("任务未完成，确认停止任务吗？");
      if (!confirmed) return;
    }
    try {
      await invoke("stop_batch_submit");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <main className="page">
      <h1 className="title">批量入场申请</h1>
      <div className="layout">
      <section className="panel panel-left">

        <div className="block">
          <h2>1. 申请人信息</h2>
          <label>
            手机号
            <input
              type="text"
              placeholder="申请人手机号"
              value={account}
              disabled={isRunning}
              onChange={(e) => setAccount(e.currentTarget.value)}
            />
          </label>
        </div>

        <div className="block">
          <h2>2. 访客管理</h2>
          <div className="visitor-list">
            {visitors.map((visitor, index) => (
              <div key={index} className="visitor-row">
                <div className="visitor-input-group">
                  <input
                    type="text"
                    placeholder="身份证号"
                    value={visitor.idCard}
                    disabled={isRunning || visitor.loading}
                    onChange={(e) =>
                      updateVisitorIdCard(index, e.currentTarget.value)
                    }
                  />
                  <button
                    type="button"
                    disabled={
                      isRunning ||
                      visitor.loading ||
                      !visitor.idCard.trim() ||
                      !account.trim()
                    }
                    onClick={() => queryVisitor(index)}
                  >
                    {visitor.loading ? "查询中..." : "查询"}
                  </button>
                  {visitors.length > 1 ? (
                    <button
                      type="button"
                      className="btn-danger"
                      disabled={isRunning}
                      onClick={() => removeVisitor(index)}
                    >
                      删除
                    </button>
                  ) : null}
                </div>
                {visitor.info ? (
                  <div className="visitor-info">
                    <span className="badge-success">已查询</span>
                    <span>
                      {visitor.info.name} | {visitor.info.phone}
                    </span>
                  </div>
                ) : null}
                {visitor.error ? (
                  <p className="field-error">{visitor.error}</p>
                ) : null}
              </div>
            ))}
          </div>
          <button
            type="button"
            className="btn-add"
            disabled={isRunning}
            onClick={addVisitor}
          >
            + 添加访客
          </button>
        </div>

        <div className="block">
          <h2>3. 接待人信息</h2>
          <div className="reception-row">
            <input
              type="text"
              placeholder="员工号"
              value={receptionId}
              disabled={isRunning || receptionLoading}
              onChange={(e) => {
                setReceptionId(e.currentTarget.value);
                setReceptionInfo(undefined);
                setReceptionError(undefined);
              }}
            />
            <button
              type="button"
              disabled={
                isRunning || receptionLoading || !receptionId.trim()
              }
              onClick={queryReception}
            >
              {receptionLoading ? "查询中..." : "查询"}
            </button>
          </div>
          {receptionInfo ? (
            <div className="reception-info">
              <span className="badge-success">已查询</span>
              <span>
                {receptionInfo.name} | {receptionInfo.department} |{" "}
                {receptionInfo.phone}
              </span>
            </div>
          ) : null}
          {receptionError ? (
            <p className="field-error">{receptionError}</p>
          ) : null}
        </div>

        <div className="block">
          <h2>4. 到访日期</h2>
          <div className="date-fields">
            <label>
              开始日期
              <input
                type="date"
                value={startDate}
                disabled={isRunning}
                onChange={(e) => setStartDate(e.currentTarget.value)}
              />
            </label>
            <label>
              结束日期
              <input
                type="date"
                value={endDate}
                disabled={isRunning}
                onChange={(e) => setEndDate(e.currentTarget.value)}
              />
            </label>
          </div>
        </div>

        <div className="block">
          <h2>5. 待提交日期</h2>
          <div className="list-box">
            {dates.length === 0 ? <p className="empty">暂无日期</p> : null}
            {dates.map((date) => (
              <div key={date} className="date-row">
                <div className="date-main">
                  <span>{date}</span>
                  {existingDateSet.has(date) ? (
                    <span className="badge-existing">已存在</span>
                  ) : null}
                </div>
                <button
                  type="button"
                  onClick={() => removeDate(date)}
                  disabled={isRunning}
                >
                  删除
                </button>
              </div>
            ))}
          </div>
          {existingDates.length > 0 ? (
            <p className="hint">
              检测到 {existingDates.length} 条日期已有申请记录，提交时将自动跳过。
            </p>
          ) : null}
        </div>

        <div className="block action-row">
          <h2>6. 批量执行</h2>
          <div className="actions">
            <button
              type="button"
              onClick={startSubmit}
              disabled={!canSubmit}
            >
              开始提交
            </button>
            <button type="button" onClick={stopSubmit} disabled={!isRunning}>
              停止提交
            </button>
          </div>
          {!allVisitorsReady || !receptionReady ? (
            <p className="hint">
              请先完成所有访客查询和接待人查询后再提交。
            </p>
          ) : null}
          {isRunning ? (
            <p className="countdown">
              {countdownSeconds === null
                ? "正在提交中，等待服务端返回..."
                : countdownSeconds > 0
                  ? `下一次请求倒计时：${formatCountdown(countdownSeconds)}`
                  : "即将提交下一条..."}
            </p>
          ) : null}
        </div>

        {errorMessage ? <p className="error">{errorMessage}</p> : null}
      </section>

      <aside className="panel panel-right">
        <div className="block">
          <h2>7. 日志</h2>
          <div className="log-box">
            {logs.length === 0 ? <p className="empty">暂无日志</p> : null}
            {logs.map((log, index) => (
              <article
                key={`${log.date ?? "none"}-${log.result}-${index}`}
                className="log-item"
              >
                <p>
                  [{index + 1}]{" "}
                  {log.result === "visitor_query"
                    ? "[访客查询]"
                    : log.result === "reception_query"
                      ? "[接待人查询]"
                      : log.date ?? "-"}{" "}
                  | {log.result}
                  {typeof log.waitSeconds === "number"
                    ? ` | 等待 ${log.waitSeconds}s`
                    : ""}
                </p>
                {log.reason ? <p>原因: {log.reason}</p> : null}
                {log.responseRaw ? <pre>{log.responseRaw}</pre> : null}
              </article>
            ))}
          </div>
        </div>

        <div className="block">
          <h2>8. 最近一个月申请历史（本地记录）</h2>
          <p className="history-note">仅保存在当前设备，不会上传到服务器。</p>
          <div className="list-box">
            {historyRecords.length === 0 ? (
              <p className="empty">暂无历史记录</p>
            ) : (
              historyRecords.map((record) => (
                <div
                  key={`${record.date}-${record.submittedAt}`}
                  className="history-row"
                >
                  <span>{record.date}</span>
                  <span className="history-time">
                    {formatHistoryTime(record.submittedAt)}
                  </span>
                </div>
              ))
            )}
          </div>
        </div>
      </aside>
      </div>
    </main>
  );
}

export default App;
