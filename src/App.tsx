import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { expandDateRange } from "./dateRange";
import "./App.css";

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

function App() {
  const [startDate, setStartDate] = useState("");
  const [endDate, setEndDate] = useState("");
  const [dates, setDates] = useState<string[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [logs, setLogs] = useState<BatchLogItem[]>([]);
  const [countdownSeconds, setCountdownSeconds] = useState<number | null>(null);
  const [completedCount, setCompletedCount] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | undefined>();

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    listen<BatchLogPayload>("batch-log", (event) => {
      const item = normalizeLog(event.payload ?? {});
      setLogs((prev) => [...prev, item]);

      if (item.result === "success") {
        setCompletedCount((prev) => prev + 1);
      }

      if (typeof item.waitSeconds === "number" && item.waitSeconds > 0) {
        setCountdownSeconds(item.waitSeconds);
      } else if (item.result === "success") {
        setCountdownSeconds(null);
      }

      if (item.result === "failed" || item.result === "stopped") {
        setIsRunning(false);
        setCountdownSeconds(null);
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
        if (prev === null) {
          return null;
        }
        if (prev <= 1) {
          return 0;
        }
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
        const hasUnfinishedTask = isRunning && completedCount < dates.length;
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
  }, [isRunning, completedCount, dates.length]);

  const generateDates = () => {
    if (!startDate || !endDate) {
      setErrorMessage("请先选择开始和结束日期");
      return;
    }

    try {
      const expanded = expandDateRange(startDate, endDate);
      setDates(expanded);
      setErrorMessage(undefined);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "日期生成失败");
    }
  };

  const removeDate = (target: string) => {
    if (isRunning) {
      return;
    }
    setDates((prev) => prev.filter((item) => item !== target));
  };

  const startSubmit = async () => {
    if (dates.length === 0) {
      setErrorMessage("请至少保留一个日期");
      return;
    }

    try {
      setErrorMessage(undefined);
      setLogs([]);
      setCompletedCount(0);
      setCountdownSeconds(null);
      setIsRunning(true);
      await invoke("start_batch_submit", { dates });
      window.alert("任务已完成");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (!message.includes("batch stopped manually")) {
        setErrorMessage(message);
      }
    } finally {
      setIsRunning(false);
      setCountdownSeconds(null);
    }
  };

  const stopSubmit = async () => {
    if (!isRunning) {
      return;
    }

    const hasUnfinishedTask = completedCount < dates.length;
    if (hasUnfinishedTask) {
      const confirmed = window.confirm("任务未完成，确认停止任务吗？");
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

  return (
    <main className="page">
      <section className="panel">
        <h1 className="title">到访批量提交</h1>

        <div className="block">
          <h2>1. 选择日期区间</h2>
          <div className="date-fields">
            <label>
              开始日期
              <input
                type="date"
                value={startDate}
                disabled={isRunning}
                onChange={(event) => setStartDate(event.currentTarget.value)}
              />
            </label>
            <label>
              结束日期
              <input
                type="date"
                value={endDate}
                disabled={isRunning}
                onChange={(event) => setEndDate(event.currentTarget.value)}
              />
            </label>
            <button type="button" disabled={isRunning} onClick={generateDates}>
              生成日期列表
            </button>
          </div>
        </div>

        <div className="block">
          <h2>2. 待提交日期</h2>
          <div className="list-box">
            {dates.length === 0 ? <p className="empty">暂无日期</p> : null}
            {dates.map((date) => (
              <div key={date} className="date-row">
                <span>{date}</span>
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
        </div>

        <div className="block action-row">
          <h2>3. 批量执行</h2>
          <div className="actions">
            <button
              type="button"
              onClick={startSubmit}
              disabled={isRunning || dates.length === 0}
            >
              开始提交
            </button>
            <button type="button" onClick={stopSubmit} disabled={!isRunning}>
              停止提交
            </button>
          </div>
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

        <div className="block">
          <h2>4. 日志</h2>
          <div className="log-box">
            {logs.length === 0 ? <p className="empty">暂无日志</p> : null}
            {logs.map((log, index) => (
              <article
                key={`${log.date ?? "none"}-${log.result}-${index}`}
                className="log-item"
              >
                <p>
                  [{index + 1}] {log.date ?? "-"} | {log.result}
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
      </section>
    </main>
  );
}

export default App;
