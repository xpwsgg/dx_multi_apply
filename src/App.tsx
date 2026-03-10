import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { confirm } from "@tauri-apps/plugin-dialog";
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

type ReceptionRow = {
  employeeId: string;
  loading: boolean;
  info?: ReceptionInfo;
  error?: string;
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
  receptionIds: string[];
};

type LogActionFeedback = {
  type: "success" | "error";
  message: string;
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

function serializeLogs(logs: BatchLogItem[]): string {
  return logs
    .map((log, index) => {
      const target =
        log.result === "visitor_query"
          ? "[访客查询]"
          : log.result === "reception_query"
            ? "[接待人查询]"
            : log.date ?? "-";
      const wait =
        typeof log.waitSeconds === "number" ? ` | 等待 ${log.waitSeconds}s` : "";
      const lines = [`[${index + 1}] ${target} | ${log.result}${wait}`];
      if (log.reason) {
        lines.push(`原因: ${log.reason}`);
      }
      if (log.responseRaw) {
        lines.push("响应:");
        lines.push(log.responseRaw);
      }
      return lines.join("\n");
    })
    .join("\n\n");
}

async function copyText(text: string): Promise<void> {
  if (navigator.clipboard && window.isSecureContext) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  textarea.style.pointerEvents = "none";
  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();
  const copied = document.execCommand("copy");
  document.body.removeChild(textarea);
  if (!copied) {
    throw new Error("浏览器不支持自动复制");
  }
}

function App() {
  const [account, setAccount] = useState("");
  const [visitors, setVisitors] = useState<VisitorRow[]>([
    { idCard: "", loading: false },
  ]);
  const [receptions, setReceptions] = useState<ReceptionRow[]>([
    { employeeId: "", loading: false },
  ]);

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
  const [logActionFeedback, setLogActionFeedback] = useState<LogActionFeedback | null>(null);
  const [factoryInfo, setFactoryInfo] = useState<{ company: string; part: string; applyType: string } | null>(null);

  const existingDateSet = useMemo(() => new Set(existingDates), [existingDates]);

  const allVisitorsReady = visitors.length > 0 && visitors.every((v) => v.info && !v.loading);
  const allReceptionsReady = receptions.length > 0 && receptions.every((r) => r.info && !r.loading);
  const canSubmit =
    account.trim().length > 0 &&
    allVisitorsReady &&
    allReceptionsReady &&
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
    invoke<Record<string, string>>("get_factory_info").then((info) => {
      setFactoryInfo({
        company: info.company,
        part: info.part,
        applyType: info.applyType,
      });
    });
  }, []);

  // Auto-save form state on change (debounced)
  const initialLoadDone = useRef(false);
  useEffect(() => {
    if (!initialLoadDone.current) {
      return;
    }
    const timer = window.setTimeout(() => {
      const visitorIdCards = visitors
        .map((v) => v.idCard.trim())
        .filter((id) => id.length > 0);
      const receptionIds = receptions
        .map((r) => r.employeeId.trim())
        .filter((id) => id.length > 0);
      invoke("save_form_state", {
        account: account.trim(),
        visitorIdCards,
        receptionIds,
      }).catch(() => {});
    }, 500);
    return () => window.clearTimeout(timer);
  }, [account, visitors, receptions]);

  useEffect(() => {
    let disposed = false;

    void loadRecentHistory();
    // Load saved form state and auto-query
    invoke<FormState | null>("load_form_state")
      .then(async (saved) => {
        if (disposed || !saved) return;
        const savedAccount = saved.account || "";
        const savedReceptionIds = saved.receptionIds ?? [];
        const savedIdCards = saved.visitorIdCards ?? [];

        if (savedAccount) setAccount(savedAccount);

        // Auto-query visitors
        if (savedAccount && savedIdCards.length > 0) {
          const rows: VisitorRow[] = savedIdCards.map((idCard) => ({
            idCard,
            loading: true,
          }));
          setVisitors(rows);

          for (let i = 0; i < savedIdCards.length; i++) {
            if (disposed) return;
            const idCard = savedIdCards[i].trim();
            if (!idCard) continue;
            try {
              const info = await invoke<VisitorInfo>("fetch_visitor_info", {
                account: savedAccount.trim(),
                idCard,
              });
              if (!disposed) {
                setVisitors((prev) =>
                  prev.map((v, idx) =>
                    idx === i ? { ...v, loading: false, info, error: undefined } : v
                  )
                );
              }
            } catch (error) {
              if (!disposed) {
                const msg = error instanceof Error ? error.message : String(error);
                setVisitors((prev) =>
                  prev.map((v, idx) =>
                    idx === i ? { ...v, loading: false, info: undefined, error: msg } : v
                  )
                );
              }
            }
          }
        }

        // Auto-query receptions
        if (savedAccount && savedReceptionIds.length > 0) {
          const rows: ReceptionRow[] = savedReceptionIds.map((employeeId) => ({
            employeeId,
            loading: true,
          }));
          setReceptions(rows);

          for (let i = 0; i < savedReceptionIds.length; i++) {
            if (disposed) return;
            const employeeId = savedReceptionIds[i].trim();
            if (!employeeId) continue;
            try {
              const info = await invoke<ReceptionInfo>("fetch_reception_info", {
                employeeId,
              });
              if (!disposed) {
                setReceptions((prev) =>
                  prev.map((r, idx) =>
                    idx === i ? { ...r, loading: false, info, error: undefined } : r
                  )
                );
              }
            } catch (error) {
              if (!disposed) {
                const msg = error instanceof Error ? error.message : String(error);
                setReceptions((prev) =>
                  prev.map((r, idx) =>
                    idx === i ? { ...r, loading: false, info: undefined, error: msg } : r
                  )
                );
              }
            }
          }
        }
      })
      .catch((error) => {
        if (!disposed) {
          setErrorMessage(
            `加载表单状态失败: ${error instanceof Error ? error.message : String(error)}`
          );
        }
      })
      .finally(() => {
        if (!disposed) {
          initialLoadDone.current = true;
        }
      });

    return () => { disposed = true; };
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
    if (!logActionFeedback) {
      return;
    }
    const timer = window.setTimeout(() => {
      setLogActionFeedback(null);
    }, 2200);
    return () => {
      window.clearTimeout(timer);
    };
  }, [logActionFeedback]);

  useEffect(() => {
    let disposed = false;
    let unlistenClose: (() => void) | undefined;

    getCurrentWindow()
      .onCloseRequested(async (event) => {
        event.preventDefault();

        if (isRunning) {
          const confirmed = await confirm("当前任务未完成，确认关闭软件吗？", {
            title: "确认关闭软件",
            kind: "warning",
            okLabel: "确认关闭",
            cancelLabel: "继续运行",
          });
          if (!confirmed) {
            return;
          }
        }

        await getCurrentWindow().destroy();
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
  }, [isRunning]);

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

  const queryReception = async (index: number) => {
    const row = receptions[index];
    if (!row || !row.employeeId.trim()) {
      setErrorMessage("请填写接待人员工号");
      return;
    }

    setReceptions((prev) =>
      prev.map((r, i) =>
        i === index ? { ...r, loading: true, error: undefined } : r
      )
    );

    try {
      const info = await invoke<ReceptionInfo>("fetch_reception_info", {
        employeeId: row.employeeId.trim(),
      });
      setReceptions((prev) =>
        prev.map((r, i) =>
          i === index ? { ...r, loading: false, info, error: undefined } : r
        )
      );
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      setReceptions((prev) =>
        prev.map((r, i) =>
          i === index
            ? { ...r, loading: false, info: undefined, error: msg }
            : r
        )
      );
    }
  };

  const addReception = () => {
    setReceptions((prev) => [...prev, { employeeId: "", loading: false }]);
  };

  const removeReception = (index: number) => {
    if (receptions.length <= 1) return;
    setReceptions((prev) => prev.filter((_, i) => i !== index));
  };

  const updateReceptionEmployeeId = (index: number, value: string) => {
    setReceptions((prev) =>
      prev.map((r, i) =>
        i === index ? { ...r, employeeId: value, info: undefined, error: undefined } : r
      )
    );
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

    const confirmedReceptions = receptions
      .filter((r) => r.info)
      .map((r) => r.info as ReceptionInfo);

    if (confirmedVisitors.length === 0 || confirmedReceptions.length === 0) {
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
        receptions: confirmedReceptions,
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
      const confirmed = await confirm("任务未完成，确认停止任务吗？", {
        title: "确认停止任务",
        kind: "warning",
        okLabel: "确认停止",
        cancelLabel: "继续运行",
      });
      if (!confirmed) {
        return;
      }
    }
    try {
      await invoke("stop_batch_submit");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  };

  const clearLogs = () => {
    setLogs([]);
    setLogActionFeedback({ type: "success", message: "日志已清空" });
  };

  const copyAllLogs = async () => {
    if (logs.length === 0) {
      return;
    }
    try {
      await copyText(serializeLogs(logs));
      setLogActionFeedback({ type: "success", message: "日志已复制到剪贴板" });
    } catch (error) {
      setLogActionFeedback({
        type: "error",
        message: `复制日志失败: ${error instanceof Error ? error.message : String(error)}`,
      });
    }
  };

  return (
    <main className="page">
      <h1 className="title">批量入场申请</h1>
      {factoryInfo ? (
        <div className="factory-banner">
          <span className="factory-company">{factoryInfo.company}</span>
          <span className="factory-divider" />
          <span className="factory-tag">{factoryInfo.part}</span>
          <span className="factory-tag">{factoryInfo.applyType}</span>
        </div>
      ) : null}
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
                    {visitor.loading ? "查询中..." : "确认"}
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
          <h2>3. 接待人管理</h2>
          <div className="reception-list">
            {receptions.map((reception, index) => (
              <div key={index} className="reception-row">
                <div className="reception-input-group">
                  <input
                    type="text"
                    placeholder="员工号"
                    value={reception.employeeId}
                    disabled={isRunning || reception.loading}
                    onChange={(e) =>
                      updateReceptionEmployeeId(index, e.currentTarget.value)
                    }
                  />
                  <button
                    type="button"
                    disabled={
                      isRunning || reception.loading || !reception.employeeId.trim()
                    }
                    onClick={() => queryReception(index)}
                  >
                    {reception.loading ? "查询中..." : "查询"}
                  </button>
                  {receptions.length > 1 ? (
                    <button
                      type="button"
                      className="btn-danger"
                      disabled={isRunning}
                      onClick={() => removeReception(index)}
                    >
                      删除
                    </button>
                  ) : null}
                </div>
                {reception.info ? (
                  <div className="reception-info">
                    <span className="badge-success">已查询</span>
                    <span>
                      {reception.info.name} | {reception.info.department} |{" "}
                      {reception.info.phone}
                    </span>
                  </div>
                ) : null}
                {reception.error ? (
                  <p className="field-error">{reception.error}</p>
                ) : null}
              </div>
            ))}
          </div>
          <button
            type="button"
            className="btn-add"
            disabled={isRunning}
            onClick={addReception}
          >
            + 添加接待人
          </button>
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
          {!allVisitorsReady || !allReceptionsReady ? (
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
          <div className="block-header">
            <h2>7. 日志</h2>
            <div className="icon-actions">
              <button
                type="button"
                className="icon-action-btn"
                onClick={copyAllLogs}
                disabled={logs.length === 0}
                title="复制全部日志"
                aria-label="复制全部日志"
              >
                <svg
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  aria-hidden="true"
                >
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
              </button>
              <button
                type="button"
                className="icon-action-btn icon-action-btn-danger"
                onClick={clearLogs}
                disabled={logs.length === 0}
                title="清空日志"
                aria-label="清空日志"
              >
                <svg
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  aria-hidden="true"
                >
                  <polyline points="3 6 5 6 21 6" />
                  <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                  <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
                  <line x1="10" y1="11" x2="10" y2="17" />
                  <line x1="14" y1="11" x2="14" y2="17" />
                </svg>
              </button>
            </div>
          </div>
          {logActionFeedback ? (
            <p className={`log-action-feedback log-action-feedback-${logActionFeedback.type}`}>
              {logActionFeedback.message}
            </p>
          ) : null}
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
