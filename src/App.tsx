import { useEffect, useMemo, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
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
  socialProof: unknown;
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
  phone: string;
  loading: boolean;
  info?: VisitorInfo;
  error?: string;
};

type BatchLogItem = {
  date?: string;
  receptionId?: string;
  result: string;
  reason?: string;
  waitSeconds?: number;
  responseRaw?: string;
};

type BatchLogPayload = {
  date?: unknown;
  receptionId?: unknown;
  result?: unknown;
  reason?: unknown;
  waitSeconds?: unknown;
  responseRaw?: unknown;
};

type FormState = {
  account: string;
  visitorIdCards: string[];
  visitorPhones: string[];
  receptionIds: string[];
};

type SubmissionStatus = "pending" | "submitting" | "success" | "skipped" | "failed" | "stopped";

type SubmissionItem = {
  date: string;
  weekday: string;
  receptionId: string;
  receptionName: string;
  receptionDept: string;
  status: SubmissionStatus;
  existing: boolean;
};

type SubmissionTaskPayload = {
  date: string;
  receptionId: string;
};

type LogActionFeedback = {
  type: "success" | "error";
  message: string;
};

type BannerMessage = {
  type: "info" | "success";
  text: string;
};

type LoginStatus = "idle" | "logging-in" | "logged-in" | "failed";

type LoginResultPayload = {
  success?: boolean;
  phone?: string;
  obtainedAt?: string;
  error?: string;
  status?: string;
};

type TokenStatusPayload = {
  phone: string;
  obtainedAt: string;
};

type VisitorStatusRecord = {
  flowNum: string;
  visitorName: string;
  visitorPhone: string;
  visitCompany: string;
  visitPark: string;
  applyType: string;
  rPersonName: string;
  rPersonPhone: string;
  dateStart: string;
  dateEnd: string;
  flowStatus: string;
  createTime: string;
};

type RestoreProgress = {
  active: boolean;
  visitorDone: number;
  visitorTotal: number;
  receptionDone: number;
  receptionTotal: number;
};

type BatchImportProgress = {
  target: "visitor" | "reception";
  done: number;
  total: number;
  success: number;
  failed: number;
  duplicates: number;
};

const MAX_VISIBLE_LOGS = 200;
const MAX_LOG_TEXT_LENGTH = 4000;

function truncateLogText(text: string, maxLength: number = MAX_LOG_TEXT_LENGTH): string {
  if (text.length <= maxLength) {
    return text;
  }
  return `${text.slice(0, maxLength)}...[已截断 ${text.length - maxLength} 个字符]`;
}

function normalizeLog(payload: BatchLogPayload): BatchLogItem {
  return {
    date: typeof payload.date === "string" ? payload.date : undefined,
    receptionId:
      typeof payload.receptionId === "string" ? payload.receptionId : undefined,
    result: typeof payload.result === "string" ? payload.result : "unknown",
    reason: typeof payload.reason === "string" ? payload.reason : undefined,
    waitSeconds:
      typeof payload.waitSeconds === "number" ? payload.waitSeconds : undefined,
    responseRaw:
      typeof payload.responseRaw === "string"
        ? truncateLogText(payload.responseRaw)
        : undefined,
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

const WEEKDAY_NAMES = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"] as const;

function weekdayLabel(dateStr: string): string {
  const d = new Date(dateStr + "T00:00:00");
  if (Number.isNaN(d.getTime())) return "";
  return WEEKDAY_NAMES[d.getDay()];
}

function formatHistoryTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return iso;
  }
  return date.toLocaleString("zh-CN", { hour12: false });
}

function buildExistingSubmissionKey(date: string, receptionId: string): string {
  return `${date}::${receptionId}`;
}

function buildSubmissionItems(
  dates: string[],
  receptions: ReceptionInfo[],
  existingSubmissionKeySet: Set<string>,
  removedSubmissionKeySet: Set<string>
): SubmissionItem[] {
  const sortedDates = [...dates].sort();
  const items: SubmissionItem[] = [];

  for (const reception of receptions) {
    for (const date of sortedDates) {
      const submissionKey = buildExistingSubmissionKey(date, reception.employeeId);
      if (removedSubmissionKeySet.has(submissionKey)) {
        continue;
      }
      items.push({
        date,
        weekday: weekdayLabel(date),
        receptionId: reception.employeeId,
        receptionName: reception.name,
        receptionDept: reception.department,
        status: "pending",
        existing: existingSubmissionKeySet.has(submissionKey),
      });
    }
  }

  return items;
}

function formatDurationRange(secondsMin: number, secondsMax: number): string {
  const toText = (seconds: number) => {
    if (seconds < 60) {
      return `${seconds} 秒`;
    }
    const minutes = Math.floor(seconds / 60);
    const remain = seconds % 60;
    return remain === 0 ? `${minutes} 分钟` : `${minutes} 分 ${remain} 秒`;
  };

  if (secondsMin === secondsMax) {
    return toText(secondsMin);
  }
  return `${toText(secondsMin)} - ${toText(secondsMax)}`;
}

function getLocalTodayDate(): string {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function parseDateValue(value: string): Date | null {
  if (!value) {
    return null;
  }
  const date = new Date(`${value}T00:00:00`);
  return Number.isNaN(date.getTime()) ? null : date;
}

function formatDateValue(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

function addMonths(date: Date, months: number): Date {
  return new Date(date.getFullYear(), date.getMonth() + months, 1);
}

function getMonthAnchor(value?: string, min?: string): Date {
  const source = value || min || getLocalTodayDate();
  const parsed = parseDateValue(source);
  if (!parsed) {
    const today = parseDateValue(getLocalTodayDate());
    return new Date(today?.getFullYear() ?? 1970, today?.getMonth() ?? 0, 1);
  }
  return new Date(parsed.getFullYear(), parsed.getMonth(), 1);
}

function formatMonthTitle(month: Date): string {
  return `${month.getFullYear()}年${String(month.getMonth() + 1).padStart(2, "0")}月`;
}

const MAX_DATE_RANGE_DAYS = 10;

type CalendarDay = {
  value: string;
  dayNumber: number;
  inCurrentMonth: boolean;
  disabled: boolean;
  isToday: boolean;
};

function buildCalendarDays(month: Date, min?: string): CalendarDay[] {
  const monthStart = new Date(month.getFullYear(), month.getMonth(), 1);
  const gridStart = addDays(monthStart, -monthStart.getDay());
  const today = getLocalTodayDate();

  return Array.from({ length: 42 }, (_, index) => {
    const current = addDays(gridStart, index);
    const value = formatDateValue(current);
    return {
      value,
      dayNumber: current.getDate(),
      inCurrentMonth: current.getMonth() === month.getMonth(),
      disabled: Boolean(min && value < min),
      isToday: value === today,
    };
  });
}

type DatePickerFieldProps = {
  label: string;
  value: string;
  min?: string;
  disabled?: boolean;
  onChange: (value: string) => void;
};

function DatePickerField({
  label,
  value,
  min,
  disabled = false,
  onChange,
}: DatePickerFieldProps) {
  const [open, setOpen] = useState(false);
  const [visibleMonth, setVisibleMonth] = useState<Date>(() => getMonthAnchor(value, min));
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) {
      setVisibleMonth(getMonthAnchor(value, min));
    }
  }, [min, open, value]);

  useEffect(() => {
    if (!open) {
      return;
    }

    const handlePointerDown = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleEscape);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleEscape);
    };
  }, [open]);

  const calendarDays = buildCalendarDays(visibleMonth, min);
  const canGoPrev = !min || formatDateValue(addMonths(visibleMonth, -1)) >= min.slice(0, 7) + "-01";

  return (
    <div className="date-picker-field" ref={rootRef}>
      <label>{label}</label>
      <button
        type="button"
        className={`date-picker-trigger${open ? " date-picker-trigger-open" : ""}`}
        onClick={() => {
          if (disabled) {
            return;
          }
          setVisibleMonth(getMonthAnchor(value, min));
          setOpen((previous) => !previous);
        }}
        disabled={disabled}
        aria-haspopup="dialog"
        aria-expanded={open}
      >
        <span className={value ? "date-picker-value" : "date-picker-placeholder"}>
          {value || "请选择日期"}
        </span>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <rect x="3" y="4" width="18" height="18" rx="2" ry="2" />
          <line x1="16" y1="2" x2="16" y2="6" />
          <line x1="8" y1="2" x2="8" y2="6" />
          <line x1="3" y1="10" x2="21" y2="10" />
        </svg>
      </button>
      {open ? (
        <div className="date-picker-popover" role="dialog" aria-label={`${label}选择器`}>
          <div className="date-picker-header">
            <button
              type="button"
              className="date-picker-nav"
              onClick={() => setVisibleMonth((previous) => addMonths(previous, -1))}
              disabled={!canGoPrev}
              aria-label="上个月"
            >
              ‹
            </button>
            <strong>{formatMonthTitle(visibleMonth)}</strong>
            <button
              type="button"
              className="date-picker-nav"
              onClick={() => setVisibleMonth((previous) => addMonths(previous, 1))}
              aria-label="下个月"
            >
              ›
            </button>
          </div>
          <div className="date-picker-weekdays">
            {["日", "一", "二", "三", "四", "五", "六"].map((weekday) => (
              <span key={weekday}>{weekday}</span>
            ))}
          </div>
          <div className="date-picker-grid">
            {calendarDays.map((day) => {
              const isSelected = day.value === value;
              return (
                <button
                  key={day.value}
                  type="button"
                  className={[
                    "date-picker-day",
                    day.inCurrentMonth ? "" : "date-picker-day-outside",
                    day.isToday ? "date-picker-day-today" : "",
                    isSelected ? "date-picker-day-selected" : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  disabled={day.disabled}
                  onClick={() => {
                    onChange(day.value);
                    setOpen(false);
                  }}
                  aria-pressed={isSelected}
                >
                  {day.dayNumber}
                </button>
              );
            })}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function serializeLogs(logs: BatchLogItem[]): string {
  return logs
    .map((log, index) => {
      const target =
        log.result === "visitor_query"
          ? "[访客查询]"
          : log.result === "reception_query"
            ? "[接待人查询]"
            : log.result === "status_query"
              ? "[预约记录查询]"
              : log.result === "status_query_failed"
                ? "[预约记录查询失败]"
                : log.result === "check_token"
                  ? "[登录校验]"
                  : log.result === "check_token_failed"
                    ? "[登录校验失败]"
                    : log.result === "login_send_code"
                      ? "[发送验证码]"
                      : log.result === "login_send_code_failed"
                        ? "[发送验证码失败]"
                        : log.result === "login_visitor_login"
                          ? "[登录]"
                          : log.result === "login_visitor_login_failed"
                            ? "[登录失败]"
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

function serializeTaskItems(items: SubmissionItem[]): string {
  return items
    .map(
      (item, index) =>
        `${index + 1}. ${item.date} | ${item.weekday} | ${item.receptionName}(${item.receptionDept}) | ${item.receptionId}`
    )
    .join("\n");
}

function App() {
  const [account, setAccount] = useState("");
  const [visitors, setVisitors] = useState<VisitorRow[]>([
    { idCard: "", phone: "", loading: false },
  ]);
  const [receptions, setReceptions] = useState<ReceptionRow[]>([
    { employeeId: "", loading: false },
  ]);

  const [startDate, setStartDate] = useState("");
  const [endDate, setEndDate] = useState("");
  const [dates, setDates] = useState<string[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [logs, setLogs] = useState<BatchLogItem[]>([]);
  const [existingSubmissionKeys, setExistingSubmissionKeys] = useState<string[]>([]);
  const [countdownSeconds, setCountdownSeconds] = useState<number | null>(null);
  const [processedCount, setProcessedCount] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | undefined>();
  const [bannerMessage, setBannerMessage] = useState<BannerMessage | null>(null);
  const [logActionFeedback, setLogActionFeedback] = useState<LogActionFeedback | null>(null);
  const [factoryInfo, setFactoryInfo] = useState<{ company: string; part: string; applyType: string } | null>(null);
  const [loginStatus, setLoginStatus] = useState<LoginStatus>("idle");
  const [loginError, setLoginError] = useState("");
  const [loginObtainedAt, setLoginObtainedAt] = useState("");
  const [statusRecords, setStatusRecords] = useState<VisitorStatusRecord[]>([]);
  const [statusModalOpen, setStatusModalOpen] = useState(false);
  const [statusLoading, setStatusLoading] = useState(false);

  const [logModalOpen, setLogModalOpen] = useState(false);
  const [completionModalOpen, setCompletionModalOpen] = useState(false);
  const [submissionItems, setSubmissionItems] = useState<SubmissionItem[]>([]);
  const [removedSubmissionKeys, setRemovedSubmissionKeys] = useState<string[]>([]);
  const [showOnlyActionableTasks, setShowOnlyActionableTasks] = useState(false);
  const [restoreProgress, setRestoreProgress] = useState<RestoreProgress>({
    active: false,
    visitorDone: 0,
    visitorTotal: 0,
    receptionDone: 0,
    receptionTotal: 0,
  });
  const [batchImportProgress, setBatchImportProgress] = useState<BatchImportProgress | null>(null);
  const tableWrapRef = useRef<HTMLDivElement>(null);

  const [batchVisitorModalOpen, setBatchVisitorModalOpen] = useState(false);
  const [batchVisitorText, setBatchVisitorText] = useState("");
  const [batchReceptionModalOpen, setBatchReceptionModalOpen] = useState(false);
  const [batchReceptionText, setBatchReceptionText] = useState("");
  const [appVersion, setAppVersion] = useState("");

  const startupLoginPhoneRef = useRef<string>("");
  const todayDate = useMemo(() => getLocalTodayDate(), []);
  const existingSubmissionKeySet = useMemo(
    () => new Set(existingSubmissionKeys),
    [existingSubmissionKeys]
  );
  const removedSubmissionKeySet = useMemo(
    () => new Set(removedSubmissionKeys),
    [removedSubmissionKeys]
  );

  const allVisitorsReady = visitors.length > 0 && visitors.every((v) => v.info && !v.loading);
  const allReceptionsReady = receptions.length > 0 && receptions.every((r) => r.info && !r.loading);

  // 自动生成申请列表：dates × receptions 笛卡尔积
  const confirmedVisitors = useMemo(
    () => visitors.filter((v) => v.info).map((v) => v.info!),
    [visitors]
  );
  const confirmedReceptions = useMemo(
    () => receptions.filter((r) => r.info).map((r) => r.info!),
    [receptions]
  );
  const confirmedVisitorIdCards = useMemo(() => {
    const ids = confirmedVisitors
      .map((visitor) => visitor.idCard.trim())
      .filter((idCard) => idCard.length > 0);
    ids.sort();
    return ids;
  }, [confirmedVisitors]);
  const confirmedVisitorIdKey = useMemo(
    () => confirmedVisitorIdCards.join(","),
    [confirmedVisitorIdCards]
  );
  const executionOrderKeys = useMemo(() => {
    return buildSubmissionItems(
      dates,
      confirmedReceptions,
      new Set<string>(),
      removedSubmissionKeySet
    ).map((item) => buildExistingSubmissionKey(item.date, item.receptionId));
  }, [dates, confirmedReceptions, removedSubmissionKeySet]);
  const actionableSubmissionItems = useMemo(
    () =>
      showOnlyActionableTasks
        ? submissionItems.filter((item) => !(item.existing && item.status === "pending"))
        : submissionItems,
    [showOnlyActionableTasks, submissionItems]
  );
  const skippedPreviewCount = useMemo(
    () => submissionItems.filter((item) => item.existing).length,
    [submissionItems]
  );
  const requestPreviewCount = useMemo(
    () => submissionItems.filter((item) => !item.existing).length,
    [submissionItems]
  );
  const waitSlotCount = Math.max(requestPreviewCount - 1, 0);
  const estimatedWaitText = useMemo(
    () => formatDurationRange(waitSlotCount * 30, waitSlotCount * 50),
    [waitSlotCount]
  );
  const currentTaskIndex = useMemo(
    () => submissionItems.findIndex((item) => item.status === "submitting"),
    [submissionItems]
  );
  const currentTask = currentTaskIndex >= 0 ? submissionItems[currentTaskIndex] : null;
  const nextTask =
    currentTaskIndex >= 0
      ? submissionItems
          .slice(currentTaskIndex + 1)
          .find((item) => item.status === "pending" || item.status === "submitting") ?? null
      : submissionItems.find((item) => item.status === "pending") ?? null;

  useEffect(() => {
    if (isRunning) return; // 提交中不重新生成
    if (dates.length === 0 || confirmedReceptions.length === 0) {
      setSubmissionItems([]);
      return;
    }
    setSubmissionItems(
      buildSubmissionItems(
        dates,
        confirmedReceptions,
        existingSubmissionKeySet,
        removedSubmissionKeySet
      )
    );
  }, [
    dates,
    confirmedReceptions,
    existingSubmissionKeySet,
    isRunning,
    removedSubmissionKeySet,
  ]);
  const canSubmit =
    account.trim().length > 0 &&
    allVisitorsReady &&
    allReceptionsReady &&
    submissionItems.length > 0 &&
    !isRunning;

  const syncExistingSubmissionKeys = async (
    targetDates: string[],
    targetVisitorIdCards: string[] = confirmedVisitorIdCards,
    targetReceptions: ReceptionInfo[] = confirmedReceptions
  ) => {
    if (
      targetDates.length === 0 ||
      targetVisitorIdCards.length === 0 ||
      targetReceptions.length === 0
    ) {
      setExistingSubmissionKeys([]);
      return [] as string[];
    }
    try {
      const existingGroups = await Promise.all(
        targetReceptions.map(async (reception) => {
          const existingDates = await invoke<string[]>("get_existing_keys", {
            dates: targetDates,
            receptionId: reception.employeeId,
            visitorIdCards: targetVisitorIdCards,
          });
          return existingDates.map((date) =>
            buildExistingSubmissionKey(date, reception.employeeId)
          );
        })
      );
      const existingKeys = Array.from(new Set(existingGroups.flat()));
      setExistingSubmissionKeys(existingKeys);
      return existingKeys;
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
      return [] as string[];
    }
  };

  const handleStartDateChange = (value: string) => {
    const nextEnd = !value || !endDate || endDate < value ? value : endDate;
    applyDateRangeSelection(value, nextEnd);
  };

  const handleEndDateChange = (value: string) => {
    applyDateRangeSelection(startDate, value);
  };

  const applyPresetDateRange = (days: number, offsetDays: number = 1) => {
    const startBase = parseDateValue(todayDate) ?? new Date();
    const start = formatDateValue(addDays(startBase, offsetDays));
    const end = formatDateValue(addDays(parseDateValue(start) ?? startBase, days - 1));
    applyDateRangeSelection(start, end);
  };

  const applyDateRangeSelection = (nextStart: string, nextEnd: string) => {
    if (!nextStart || !nextEnd) {
      setStartDate(nextStart);
      setEndDate(nextEnd);
      setDates([]);
      setRemovedSubmissionKeys([]);
      setErrorMessage(undefined);
      setBannerMessage(null);
      return;
    }

    const start = parseDateValue(nextStart);
    if (!start) {
      setErrorMessage("日期格式无效");
      return;
    }

    let normalizedEnd = nextEnd < nextStart ? nextStart : nextEnd;
    const maxAllowedEnd = formatDateValue(addDays(start, MAX_DATE_RANGE_DAYS - 1));

    if (normalizedEnd > maxAllowedEnd) {
      normalizedEnd = maxAllowedEnd;
      setBannerMessage({
        type: "info",
        text: `日期区间不能超过 ${MAX_DATE_RANGE_DAYS} 天，结束日期已自动调整`,
      });
    } else {
      setBannerMessage(null);
      setErrorMessage(undefined);
    }

    try {
      const expanded = expandDateRange(nextStart, normalizedEnd);
      setStartDate(nextStart);
      setEndDate(normalizedEnd);
      setDates(expanded);
      setRemovedSubmissionKeys([]);
      setErrorMessage(undefined);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "日期生成失败");
    }
  };

  useEffect(() => {
    invoke<Record<string, string>>("get_factory_info")
      .then((info) => {
        setFactoryInfo({
          company: info.company,
          part: info.part,
          applyType: info.applyType,
        });
      })
      .catch((error) => {
        setErrorMessage(`加载厂区信息失败: ${String(error)}`);
      });
  }, []);

  // 启动时恢复登录状态，直接写入 logs 状态确保日志可见
  useEffect(() => {
    let disposed = false;

    const restoreLoginState = async () => {
      try {
        const status = await invoke<TokenStatusPayload | null>("get_token_status");
        if (disposed || !status) {
          return;
        }

        const savedPhone = status.phone.trim();
        startupLoginPhoneRef.current = savedPhone;
        setAccount(savedPhone);
        setLoginObtainedAt(status.obtainedAt);
        setLoginError("");
        setLoginStatus("logging-in");

        const valid = await invoke<boolean>("check_token");
        if (disposed) {
          return;
        }

        if (valid) {
          setLoginStatus("logged-in");
          return;
        }

        await invoke("clear_token");
        if (disposed) {
          return;
        }

        setLoginStatus("idle");
        setLoginObtainedAt("");
        setLoginError("登录已失效，请重新登录");
      } catch (error) {
        if (disposed) {
          return;
        }

        // 网络或服务端异常，保留 token 不清除，允许用户稍后重试
        setLoginStatus("logged-in");
        setLoginError(
          error instanceof Error ? error.message : "登录状态校验失败，已保留登录信息"
        );
      }
    };

    void restoreLoginState();

    return () => {
      disposed = true;
    };
  }, []);

  // Listen for login-result events
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    listen<LoginResultPayload>("login-result", (event) => {
      if (disposed) return;
      const payload = event.payload ?? {};

      if (payload.status === "sending_code") {
        setLoginStatus("logging-in");
        return;
      }

      if (payload.status === "progress") {
        setLoginStatus("logging-in");
        return;
      }

      if (payload.success) {
        setLoginStatus("logged-in");
        setLoginError("");
        if (typeof payload.phone === "string") {
          setAccount(payload.phone);
          startupLoginPhoneRef.current = payload.phone;
        }
        setLoginObtainedAt(
          typeof payload.obtainedAt === "string" ? payload.obtainedAt : ""
        );
      } else {
        setLoginStatus("failed");
        setLoginError(
          typeof payload.error === "string" ? payload.error : "未知错误"
        );
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
        setErrorMessage(`登录事件监听失败: ${String(error)}`);
      });

    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, []);

  const triggerLogin = async () => {
    const phone = account.trim();
    if (!phone) {
      setErrorMessage("请先填写申请人手机号");
      return;
    }
    setLoginStatus("logging-in");
    setLoginError("");
    try {
      await invoke("start_login", { account: phone });
    } catch (error) {
      setLoginStatus("failed");
      setLoginError(error instanceof Error ? error.message : String(error));
    }
  };

  const switchLoginAccount = async () => {
    setLoginError("");
    setLoginObtainedAt("");
    setLoginStatus("idle");
    startupLoginPhoneRef.current = "";
    setAccount("");

    try {
      await invoke("clear_token");
    } catch (error) {
      setErrorMessage(
        `清理登录状态失败: ${error instanceof Error ? error.message : String(error)}`
      );
    }
  };

  const queryStatus = async (idCard: string, visitorPhone?: string) => {
    if (!idCard.trim()) {
      setErrorMessage("请先填写身份证号");
      return;
    }
    if (loginStatus !== "logged-in") {
      setErrorMessage("请先登录后再查询预约记录");
      return;
    }
    setStatusLoading(true);
    try {
      const phone = visitorPhone?.trim() || null;
      const records = await invoke<VisitorStatusRecord[]>(
        "query_visitor_status",
        { visitorPhone: phone, idCard: idCard.trim() }
      );
      setStatusRecords(records);
      setStatusModalOpen(true);
    } catch (error) {
      setErrorMessage(
        `查询预约记录失败: ${error instanceof Error ? error.message : String(error)}`
      );
    } finally {
      setStatusLoading(false);
    }
  };

  // Auto-save form state on change (debounced)
  const initialLoadDone = useRef(false);
  useEffect(() => {
    if (!initialLoadDone.current) {
      return;
    }
    const timer = window.setTimeout(() => {
      const validVisitors = visitors.filter((v) => v.idCard.trim().length > 0);
      const visitorIdCards = validVisitors.map((v) => v.idCard.trim());
      const visitorPhones = validVisitors.map((v) => v.phone.trim());
      const receptionIds = receptions
        .map((r) => r.employeeId.trim())
        .filter((id) => id.length > 0);
      invoke("save_form_state", {
        account: account.trim(),
        visitorIdCards,
        visitorPhones,
        receptionIds,
      }).catch(() => {});
    }, 500);
    return () => window.clearTimeout(timer);
  }, [account, visitors, receptions]);

  useEffect(() => {
    let disposed = false;

    // Load saved form state and auto-query
    invoke<FormState | null>("load_form_state")
      .then(async (saved) => {
        if (disposed || !saved) return;
        const savedAccount = saved.account || "";
        const savedReceptionIds = saved.receptionIds ?? [];
        const savedIdCards = saved.visitorIdCards ?? [];
        const savedPhones = saved.visitorPhones ?? [];
        const hasRestoreData = savedIdCards.length > 0 || savedReceptionIds.length > 0;

        if (hasRestoreData) {
          setRestoreProgress({
            active: true,
            visitorDone: 0,
            visitorTotal: savedIdCards.length,
            receptionDone: 0,
            receptionTotal: savedReceptionIds.length,
          });
        }

        if (savedAccount && !startupLoginPhoneRef.current) {
          setAccount(savedAccount);
        }

        // Auto-query visitors
        if (savedAccount && savedIdCards.length > 0) {
          const rows: VisitorRow[] = savedIdCards.map((idCard, i) => ({
            idCard,
            phone: savedPhones[i] || "",
            loading: true,
          }));
          setVisitors(rows);

          for (let i = 0; i < savedIdCards.length; i++) {
            if (disposed) return;
            const idCard = savedIdCards[i].trim();
            const visitorPhone = savedPhones[i]?.trim() || null;
            if (!idCard) continue;
            try {
              const info = await invoke<VisitorInfo>("fetch_visitor_info", {
                visitorPhone,
                account: savedAccount.trim(),
                idCard,
              });
              if (!disposed) {
                setVisitors((prev) =>
                  prev.map((v, idx) =>
                    idx === i ? { ...v, loading: false, info, error: undefined } : v
                  )
                );
                setRestoreProgress((prev) => ({ ...prev, visitorDone: i + 1 }));
              }
            } catch (error) {
              if (!disposed) {
                const msg = error instanceof Error ? error.message : String(error);
                setVisitors((prev) =>
                  prev.map((v, idx) =>
                    idx === i ? { ...v, loading: false, info: undefined, error: msg } : v
                  )
                );
                setRestoreProgress((prev) => ({ ...prev, visitorDone: i + 1 }));
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
                setRestoreProgress((prev) => ({ ...prev, receptionDone: i + 1 }));
              }
            } catch (error) {
              if (!disposed) {
                const msg = error instanceof Error ? error.message : String(error);
                setReceptions((prev) =>
                  prev.map((r, idx) =>
                    idx === i ? { ...r, loading: false, info: undefined, error: msg } : r
                  )
                );
                setRestoreProgress((prev) => ({ ...prev, receptionDone: i + 1 }));
              }
            }
          }
        }

        if (!disposed && hasRestoreData) {
          setRestoreProgress((prev) => ({ ...prev, active: false }));
          setBannerMessage({
            type: "success",
            text: `已恢复上次会话：访客 ${savedIdCards.length} 条，接待人 ${savedReceptionIds.length} 条`,
          });
        }
      })
      .catch((error) => {
        if (!disposed) {
          setErrorMessage(
            `加载表单状态失败: ${error instanceof Error ? error.message : String(error)}`
          );
          setRestoreProgress((prev) => ({ ...prev, active: false }));
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
      setLogs((prev) => [...prev, item].slice(-MAX_VISIBLE_LOGS));

      if (item.result === "success" || item.result === "skipped" || item.result === "failed") {
        setProcessedCount((prev) => prev + 1);
        if (item.date && item.receptionId) {
          const currentKey = buildExistingSubmissionKey(item.date, item.receptionId);
          const currentExecutionIndex = executionOrderKeys.indexOf(currentKey);
          const nextKey =
            currentExecutionIndex >= 0
              ? executionOrderKeys[currentExecutionIndex + 1]
              : undefined;

          setSubmissionItems((prev) => {
            const currentIndex = prev.findIndex(
              (submissionItem) =>
                buildExistingSubmissionKey(
                  submissionItem.date,
                  submissionItem.receptionId
                ) === currentKey
            );

            if (currentIndex < 0) {
              return prev;
            }

            const wrap = tableWrapRef.current;
            if (wrap) {
              const row = wrap.querySelector(`tr[data-submission-key="${currentKey}"]`);
              if (row) {
                row.scrollIntoView({ behavior: "smooth", block: "center" });
              }
            }

            return prev.map((submissionItem, index) => {
              const submissionKey = buildExistingSubmissionKey(
                submissionItem.date,
                submissionItem.receptionId
              );

              if (index === currentIndex) {
                return {
                  ...submissionItem,
                  status: item.result as SubmissionStatus,
                };
              }

              if (
                nextKey &&
                submissionKey === nextKey &&
                submissionItem.status === "pending"
              ) {
                return { ...submissionItem, status: "submitting" };
              }

              return submissionItem;
            });
          });
        }
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

      if (item.result === "stopped") {
        // 所有 pending 改为 stopped
        setSubmissionItems((prev) =>
          prev.map((si) =>
            si.status === "pending" || si.status === "submitting"
              ? { ...si, status: "stopped" }
              : si
          )
        );
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
  }, [executionOrderKeys]);

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
    if (!errorMessage) return;
    const timer = window.setTimeout(() => {
      setErrorMessage(undefined);
    }, 5000);
    return () => window.clearTimeout(timer);
  }, [errorMessage]);

  useEffect(() => {
    if (!bannerMessage) return;
    const timer = window.setTimeout(() => {
      setBannerMessage(null);
    }, 4000);
    return () => window.clearTimeout(timer);
  }, [bannerMessage]);

  const isRunningRef = useRef(isRunning);
  useEffect(() => {
    isRunningRef.current = isRunning;
  }, [isRunning]);

  useEffect(() => {
    let disposed = false;

    getVersion()
      .then((version) => {
        if (!disposed) {
          setAppVersion(version);
        }
      })
      .catch((error) => {
        console.warn("读取软件版本失败", error);
      });

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlistenClose: (() => void) | undefined;

    getCurrentWindow()
      .onCloseRequested(async (event) => {
        event.preventDefault();

        if (isRunningRef.current) {
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

        try {
          await invoke("clear_log");
        } catch {
          // 清理失败不阻止关闭
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
  }, []);

  const queryVisitor = async (index: number) => {
    const row = visitors[index];
    if (!row || !row.idCard.trim()) {
      setErrorMessage("请先填写访客身份证号");
      return;
    }
    if (!account.trim() && !row.phone.trim()) {
      setErrorMessage("请先填写申请人手机号或访客手机号");
      return;
    }

    setVisitors((prev) =>
      prev.map((v, i) =>
        i === index ? { ...v, loading: true, error: undefined } : v
      )
    );

    try {
      const visitorPhone = row.phone.trim() || null;
      const info = await invoke<VisitorInfo>("fetch_visitor_info", {
        visitorPhone,
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
    setVisitors((prev) => [...prev, { idCard: "", phone: "", loading: false }]);
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

  const updateVisitorPhone = (index: number, value: string) => {
    setVisitors((prev) =>
      prev.map((v, i) =>
        i === index ? { ...v, phone: value, info: undefined, error: undefined } : v
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

  const removeSubmissionItem = (date: string, receptionId: string) => {
    if (isRunning) return;
    const submissionKey = buildExistingSubmissionKey(date, receptionId);
    setRemovedSubmissionKeys((prev) =>
      prev.includes(submissionKey) ? prev : [...prev, submissionKey]
    );
    setSubmissionItems((prev) =>
      prev.filter(
        (item) =>
          buildExistingSubmissionKey(item.date, item.receptionId) !== submissionKey
      )
    );
    setProcessedCount(0);
    setCountdownSeconds(null);
  };

  const removeExistingSubmissionItems = () => {
    if (isRunning) return;
    const existingKeys = submissionItems
      .filter((item) => item.existing)
      .map((item) => buildExistingSubmissionKey(item.date, item.receptionId));

    if (existingKeys.length === 0) {
      setBannerMessage({ type: "info", text: "当前没有可批量移除的已存在任务" });
      return;
    }

    setRemovedSubmissionKeys((prev) => Array.from(new Set([...prev, ...existingKeys])));
    setBannerMessage({ type: "success", text: `已移除 ${existingKeys.length} 条已存在任务` });
  };

  const restoreAllSubmissionItems = () => {
    if (isRunning) return;
    if (removedSubmissionKeys.length === 0) {
      setBannerMessage({ type: "info", text: "当前没有隐藏任务" });
      return;
    }
    setRemovedSubmissionKeys([]);
    setShowOnlyActionableTasks(false);
    setBannerMessage({ type: "success", text: "已恢复全部任务" });
  };

  const copyFailedSubmissionItems = async () => {
    const failedItems = submissionItems.filter((item) => item.status === "failed");
    if (failedItems.length === 0) {
      setBannerMessage({ type: "info", text: "当前没有失败任务可复制" });
      return;
    }
    try {
      await copyText(serializeTaskItems(failedItems));
      setBannerMessage({ type: "success", text: `已复制 ${failedItems.length} 条失败任务` });
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  };

  const keepOnlyFailedSubmissionItems = () => {
    const failedItems = submissionItems.filter((item) => item.status === "failed");
    if (failedItems.length === 0) {
      setBannerMessage({ type: "info", text: "当前没有失败任务可保留" });
      return;
    }

    const failedKeySet = new Set(
      failedItems.map((item) => buildExistingSubmissionKey(item.date, item.receptionId))
    );
    const allKeys = buildSubmissionItems(
      dates,
      confirmedReceptions,
      existingSubmissionKeySet,
      new Set<string>()
    ).map((item) => buildExistingSubmissionKey(item.date, item.receptionId));

    setRemovedSubmissionKeys(allKeys.filter((key) => !failedKeySet.has(key)));
    setSubmissionItems(
      failedItems.map((item) => ({
        ...item,
        status: "pending" as const,
        existing: false,
      }))
    );
    setProcessedCount(0);
    setCountdownSeconds(null);
    setCompletionModalOpen(false);
    setShowOnlyActionableTasks(false);
    setBannerMessage({ type: "success", text: `已保留 ${failedItems.length} 条失败任务，可直接重试` });
  };

  useEffect(() => {
    if (isRunning) return;
    if (!allVisitorsReady || !allReceptionsReady) {
      setExistingSubmissionKeys([]);
      return;
    }
    void syncExistingSubmissionKeys(dates, confirmedVisitorIdCards, confirmedReceptions);
  }, [
    allReceptionsReady,
    allVisitorsReady,
    confirmedReceptions,
    confirmedVisitorIdCards,
    confirmedVisitorIdKey,
    dates,
    isRunning,
  ]);

  const startSubmit = async () => {
    if (!canSubmit) return;

    if (confirmedVisitors.length === 0 || confirmedReceptions.length === 0) {
      setErrorMessage("请先查询所有访客和接待人信息");
      return;
    }

    const tasks: SubmissionTaskPayload[] = submissionItems.map((item) => ({
      date: item.date,
      receptionId: item.receptionId,
    }));
    if (tasks.length === 0) {
      setErrorMessage("请至少保留一条任务");
      return;
    }

    try {
      setErrorMessage(undefined);
      await syncExistingSubmissionKeys(dates);

      // 检测是否有已提交的任务，提示用户是否强制重新提交
      const existingItems = submissionItems.filter((item) => item.existing);
      let forceResubmit = false;
      if (existingItems.length > 0) {
        const confirmed = await confirm(
          `当前有 ${existingItems.length} 条已提交的任务，是否强制重新提交？`,
          { title: "确认重新提交", kind: "warning" }
        );
        if (!confirmed) return;
        forceResubmit = true;
      }

      setLogs([]);
      setProcessedCount(0);
      setCountdownSeconds(null);
      // 重置所有行为 pending
      setSubmissionItems((prev) =>
        prev.map((si) => ({ ...si, status: "pending" as const }))
      );
      const firstExecutionKey = buildExistingSubmissionKey(
        tasks[0].date,
        tasks[0].receptionId
      );
      if (firstExecutionKey) {
        setSubmissionItems((prev) =>
          prev.map((submissionItem) =>
            buildExistingSubmissionKey(
              submissionItem.date,
              submissionItem.receptionId
            ) === firstExecutionKey
              ? { ...submissionItem, status: "submitting" as const }
              : submissionItem
          )
        );
      }
      setIsRunning(true);
      await invoke("start_batch_submit", {
        account: account.trim(),
        visitors: confirmedVisitors,
        receptions: confirmedReceptions,
        tasks,
        forceResubmit,
      });
      setCompletionModalOpen(true);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (message.includes("批量提交已手动停止")) {
        // 手动停止不显示错误
      } else if (message.includes("条失败")) {
        // 部分失败，显示完成弹窗（弹窗内有失败统计）
        setCompletionModalOpen(true);
      } else {
        setErrorMessage(message);
      }
    } finally {
      setIsRunning(false);
      setCountdownSeconds(null);
      await syncExistingSubmissionKeys(dates);
    }
  };

  const stopSubmit = async () => {
    if (!isRunning) return;
    const hasUnfinishedTask = processedCount < submissionItems.length;
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

  const clearForm = async () => {
    const confirmed = await confirm("确认清空所有表单数据吗？清空后无法恢复。", {
      title: "确认清除表单",
      kind: "warning",
      okLabel: "确认清除",
      cancelLabel: "取消",
    });
    if (!confirmed) return;

    // 清空表单字段
    setAccount("");
    setVisitors([{ idCard: "", phone: "", loading: false }]);
    setReceptions([{ employeeId: "", loading: false }]);
    setStartDate("");
    setEndDate("");
    setDates([]);
    setRemovedSubmissionKeys([]);
    setExistingSubmissionKeys([]);
    setSubmissionItems([]);
    setShowOnlyActionableTasks(false);
    setProcessedCount(0);
    setCountdownSeconds(null);
    setErrorMessage(undefined);
    setBannerMessage(null);
    setLogs([]);
    setStatusRecords([]);
    setLogActionFeedback(null);
    setBatchImportProgress(null);
    setRestoreProgress({
      active: false,
      visitorDone: 0,
      visitorTotal: 0,
      receptionDone: 0,
      receptionTotal: 0,
    });

    // 重置登录状态
    setLoginStatus("idle");
    setLoginError("");
    setLoginObtainedAt("");
    startupLoginPhoneRef.current = "";
    try {
      await invoke("clear_token");
    } catch {
      // 忽略清理失败
    }

    // 立即保存空状态，确保关闭后重开也是空的
    try {
      await invoke("save_form_state", {
        account: "",
        visitorIdCards: [],
        visitorPhones: [],
        receptionIds: [],
      });
    } catch {
      // 忽略保存失败
    }
  };

  const confirmBatchVisitors = async () => {
    const lines = batchVisitorText
      .split(/[\n,;，；]/)
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    if (lines.length === 0) return;

    // 解析每行：支持 "身份证号 手机号" 格式，手机号可选
    const parsed = lines.map((line) => {
      const parts = line.split(/\s+/);
      const idCard = parts[0] || "";
      const phone = parts[1] || "";
      return { idCard, phone };
    });

    const existingIds = new Set(visitors.map((v) => v.idCard.trim()).filter((id) => id.length > 0));
    const newItems = parsed.filter((p) => p.idCard && !existingIds.has(p.idCard));
    if (newItems.length === 0) {
      setBannerMessage({ type: "info", text: "所有身份证号已存在于列表中" });
      setBatchVisitorModalOpen(false);
      setBatchVisitorText("");
      return;
    }

    // 移除空行，追加新行
    const kept = visitors.filter((v) => v.idCard.trim().length > 0);
    const newRows: VisitorRow[] = newItems.map(({ idCard, phone }) => ({ idCard, phone, loading: true }));
    const allRows = [...kept, ...newRows];
    setVisitors(allRows);
    setBatchVisitorModalOpen(false);
    setBatchVisitorText("");
    let successCount = 0;
    let failedCount = 0;
    const duplicateCount = lines.length - newItems.length;
    setBatchImportProgress({
      target: "visitor",
      done: 0,
      total: newItems.length,
      success: 0,
      failed: 0,
      duplicates: duplicateCount,
    });

    // 逐个查询
    const startIdx = kept.length;
    for (let i = 0; i < newItems.length; i++) {
      const idx = startIdx + i;
      const { idCard, phone } = newItems[i];
      if (!account.trim() && !phone.trim()) {
        setVisitors((prev) =>
          prev.map((v, j) =>
            j === idx ? { ...v, loading: false, error: "请先填写申请人手机号或访客手机号" } : v
          )
        );
        setBatchImportProgress((prev) =>
          prev && prev.target === "visitor"
            ? { ...prev, done: i + 1, failed: prev.failed + 1 }
            : prev
        );
        failedCount += 1;
        continue;
      }
      try {
        const visitorPhone = phone.trim() || null;
        const info = await invoke<VisitorInfo>("fetch_visitor_info", {
          visitorPhone,
          account: account.trim(),
          idCard,
        });
        setVisitors((prev) =>
          prev.map((v, j) =>
            j === idx ? { ...v, loading: false, info, error: undefined } : v
          )
        );
        setBatchImportProgress((prev) =>
          prev && prev.target === "visitor"
            ? { ...prev, done: i + 1, success: prev.success + 1 }
            : prev
        );
        successCount += 1;
      } catch (error) {
        const msg = error instanceof Error ? error.message : String(error);
        setVisitors((prev) =>
          prev.map((v, j) =>
            j === idx ? { ...v, loading: false, info: undefined, error: msg } : v
          )
        );
        setBatchImportProgress((prev) =>
          prev && prev.target === "visitor"
            ? { ...prev, done: i + 1, failed: prev.failed + 1 }
            : prev
        );
        failedCount += 1;
      }
    }

    setBannerMessage({
      type: "success",
      text: `批量导入访客完成：成功 ${successCount} 条，失败 ${failedCount} 条，重复 ${duplicateCount} 条`,
    });
  };

  const confirmBatchReceptions = async () => {
    const lines = batchReceptionText
      .split(/[\n,;，；]/)
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    if (lines.length === 0) return;

    const existingIds = new Set(receptions.map((r) => r.employeeId.trim()).filter((id) => id.length > 0));
    const newIds = lines.filter((id) => !existingIds.has(id));
    if (newIds.length === 0) {
      setBannerMessage({ type: "info", text: "所有员工号已存在于列表中" });
      setBatchReceptionModalOpen(false);
      setBatchReceptionText("");
      return;
    }

    const kept = receptions.filter((r) => r.employeeId.trim().length > 0);
    const newRows: ReceptionRow[] = newIds.map((employeeId) => ({ employeeId, loading: true }));
    const allRows = [...kept, ...newRows];
    setReceptions(allRows);
    setBatchReceptionModalOpen(false);
    setBatchReceptionText("");
    let successCount = 0;
    let failedCount = 0;
    const duplicateCount = lines.length - newIds.length;
    setBatchImportProgress({
      target: "reception",
      done: 0,
      total: newIds.length,
      success: 0,
      failed: 0,
      duplicates: duplicateCount,
    });

    const startIdx = kept.length;
    for (let i = 0; i < newIds.length; i++) {
      const idx = startIdx + i;
      const employeeId = newIds[i];
      try {
        const info = await invoke<ReceptionInfo>("fetch_reception_info", { employeeId });
        setReceptions((prev) =>
          prev.map((r, j) =>
            j === idx ? { ...r, loading: false, info, error: undefined } : r
          )
        );
        setBatchImportProgress((prev) =>
          prev && prev.target === "reception"
            ? { ...prev, done: i + 1, success: prev.success + 1 }
            : prev
        );
        successCount += 1;
      } catch (error) {
        const msg = error instanceof Error ? error.message : String(error);
        setReceptions((prev) =>
          prev.map((r, j) =>
            j === idx ? { ...r, loading: false, info: undefined, error: msg } : r
          )
        );
        setBatchImportProgress((prev) =>
          prev && prev.target === "reception"
            ? { ...prev, done: i + 1, failed: prev.failed + 1 }
            : prev
        );
        failedCount += 1;
      }
    }

    setBannerMessage({
      type: "success",
      text: `批量导入接待人完成：成功 ${successCount} 条，失败 ${failedCount} 条，重复 ${duplicateCount} 条`,
    });
  };

  const clearLogs = async () => {
    setLogs([]);
    try {
      await invoke("clear_log");
    } catch {
      // 磁盘日志清理失败不影响前端
    }
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
      <h1 className="title">
        <span className="title-main">批量入场申请</span>
        {appVersion ? <span className="title-version">v{appVersion}</span> : null}
      </h1>
      {factoryInfo ? (
        <div className="factory-banner">
          <span className="factory-company">{factoryInfo.company}</span>
          <span className="factory-divider" />
          <span className="factory-tag">{factoryInfo.part}</span>
          <span className="factory-tag">{factoryInfo.applyType}</span>
        </div>
      ) : null}
      {errorMessage ? (
        <div className="error-banner">
          <span>{errorMessage}</span>
          <button type="button" className="error-banner-close" onClick={() => setErrorMessage(undefined)}>&times;</button>
        </div>
      ) : null}
      {bannerMessage ? (
        <div className={`message-banner message-banner-${bannerMessage.type}`}>
          <span>{bannerMessage.text}</span>
          <button type="button" className="message-banner-close" onClick={() => setBannerMessage(null)}>&times;</button>
        </div>
      ) : null}
      {restoreProgress.active ? (
        <div className="restore-banner">
          正在恢复上次会话：访客 {restoreProgress.visitorDone}/{restoreProgress.visitorTotal}，
          接待人 {restoreProgress.receptionDone}/{restoreProgress.receptionTotal}
        </div>
      ) : null}
      <div className="layout">
      <section className="panel panel-left">

        <div className="block">
          <h2>1. 申请人</h2>
          <label>
            申请人手机号
            <div className="account-row">
              <input
                type="text"
                placeholder="申请人手机号"
                value={account}
                disabled={isRunning || loginStatus === "logged-in"}
                onChange={(e) => {
                  setAccount(e.currentTarget.value);
                  if (loginStatus === "failed") {
                    setLoginStatus("idle");
                    setLoginError("");
                    setLoginObtainedAt("");
                  }
                }}
              />
              <button
                type="button"
                className={loginStatus === "logged-in" ? "btn-login-done" : ""}
                disabled={!account.trim() || isRunning || loginStatus === "logging-in"}
                onClick={
                  loginStatus === "logged-in" ? switchLoginAccount : triggerLogin
                }
              >
                {loginStatus === "logging-in"
                  ? "记录登录中..."
                  : loginStatus === "logged-in"
                    ? "切换申请账号"
                    : "记录查询登录"}
              </button>
            </div>
          </label>
          {loginStatus === "logged-in" && loginObtainedAt ? (
            <p className="login-info">
              <span className="login-status-dot login-dot-success" />
              预约记录登录时间: {formatHistoryTime(loginObtainedAt)}
            </p>
          ) : null}
          {loginStatus === "failed" && loginError ? (
            <p className="field-error">{loginError}</p>
          ) : null}
          <p className="hint">提交申请只使用手机号作为申请人信息，登录仅用于“查询记录”，不会影响提交流程。</p>
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
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && !isRunning && !visitor.loading && visitor.idCard.trim()) {
                        queryVisitor(index);
                      }
                    }}
                  />
                  <input
                    type="text"
                    placeholder="手机号(选填)"
                    value={visitor.phone}
                    disabled={isRunning || visitor.loading}
                    onChange={(e) =>
                      updateVisitorPhone(index, e.currentTarget.value)
                    }
                  />
                  <button
                    type="button"
                    disabled={
                      isRunning ||
                      visitor.loading ||
                      !visitor.idCard.trim()
                    }
                    onClick={() => queryVisitor(index)}
                  >
                    {visitor.loading ? "查询中..." : "确认"}
                  </button>
                  <button
                    type="button"
                    className="btn-secondary"
                    disabled={
                      !visitor.idCard.trim() ||
                      loginStatus !== "logged-in" ||
                      statusLoading
                    }
                    onClick={() => queryStatus(visitor.idCard, visitor.phone)}
                  >
                    {statusLoading ? "查询中..." : "查询记录"}
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
          <div className="btn-add-row">
            <button
              type="button"
              className="btn-add"
              disabled={isRunning}
              onClick={addVisitor}
            >
              + 添加访客
            </button>
            <button
              type="button"
              className="btn-add"
              disabled={isRunning}
              onClick={() => { setBatchVisitorText(""); setBatchVisitorModalOpen(true); }}
            >
              批量添加
            </button>
          </div>
          {batchImportProgress?.target === "visitor" ? (
            <p className="hint">
              批量导入进度：{batchImportProgress.done}/{batchImportProgress.total}，成功 {batchImportProgress.success}，失败 {batchImportProgress.failed}，重复 {batchImportProgress.duplicates}
            </p>
          ) : null}
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
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && !isRunning && !reception.loading && reception.employeeId.trim()) {
                        queryReception(index);
                      }
                    }}
                  />
                  <button
                    type="button"
                    disabled={
                      isRunning || reception.loading || !reception.employeeId.trim()
                    }
                    onClick={() => queryReception(index)}
                  >
                    {reception.loading ? "查询中..." : "确认"}
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
          <div className="btn-add-row">
            <button
              type="button"
              className="btn-add"
              disabled={isRunning}
              onClick={addReception}
            >
              + 添加接待人
            </button>
            <button
              type="button"
              className="btn-add"
              disabled={isRunning}
              onClick={() => { setBatchReceptionText(""); setBatchReceptionModalOpen(true); }}
            >
              批量添加
            </button>
          </div>
          {batchImportProgress?.target === "reception" ? (
            <p className="hint">
              批量导入进度：{batchImportProgress.done}/{batchImportProgress.total}，成功 {batchImportProgress.success}，失败 {batchImportProgress.failed}，重复 {batchImportProgress.duplicates}
            </p>
          ) : null}
        </div>

        <div className="block">
          <h2>4. 到访日期</h2>
          <div className="date-fields">
            <DatePickerField
              label="开始日期"
              value={startDate}
              min={todayDate}
              disabled={isRunning}
              onChange={handleStartDateChange}
            />
            <DatePickerField
              label="结束日期"
              value={endDate}
              min={startDate || todayDate}
              disabled={isRunning}
              onChange={handleEndDateChange}
            />
          </div>
          <div className="date-shortcuts">
            <button type="button" className="btn-secondary" disabled={isRunning} onClick={() => applyPresetDateRange(1, 1)}>
              明天
            </button>
            <button type="button" className="btn-secondary" disabled={isRunning} onClick={() => applyPresetDateRange(3)}>
              未来 3 天
            </button>
            <button type="button" className="btn-secondary" disabled={isRunning} onClick={() => applyPresetDateRange(5)}>
              未来 5 天
            </button>
            <button type="button" className="btn-secondary" disabled={isRunning} onClick={() => applyPresetDateRange(10)}>
              未来 10 天
            </button>
          </div>
        </div>


      </section>

      <aside className="panel panel-right">
        {/* 顶部汇总 */}
        <div className="submission-summary">
          {allVisitorsReady && dates.length > 0 ? (
            <p>
              <strong>访客：</strong>
              {visitors.filter((v) => v.info).map((v) => v.info!.name).join("、")}
            </p>
          ) : null}
          {allReceptionsReady && dates.length > 0 ? (
            <p>
              <strong>接待人：</strong>
              {confirmedReceptions.map((r) => `${r.name}(${r.department})`).join("、")}
            </p>
          ) : null}
          <p className="submission-count">
            日期范围 {dates.length} 天 | 接待人 {confirmedReceptions.length || 0} 人 | 待执行{" "}
            <strong>{submissionItems.length}</strong> 条任务
          </p>
          {submissionItems.length > 0 ? (
            <div className="preflight-summary">
              <span>实际提交 {requestPreviewCount} 条</span>
              <span>本地跳过 {skippedPreviewCount} 条</span>
              <span>预计等待 {estimatedWaitText}</span>
            </div>
          ) : null}
          {submissionItems.length > 0 ? (
            <div className="submission-tools">
              <button
                type="button"
                className="btn-secondary"
                disabled={isRunning}
                onClick={() => setShowOnlyActionableTasks((prev) => !prev)}
              >
                {showOnlyActionableTasks ? "显示全部任务" : "仅看待提交"}
              </button>
              <button
                type="button"
                className="btn-secondary"
                disabled={isRunning || skippedPreviewCount === 0}
                onClick={removeExistingSubmissionItems}
              >
                删除已存在任务
              </button>
              <button
                type="button"
                className="btn-secondary"
                disabled={isRunning || removedSubmissionKeys.length === 0}
                onClick={restoreAllSubmissionItems}
              >
                恢复全部任务
              </button>
            </div>
          ) : null}
          {isRunning && currentTask ? (
            <div className="current-task-card">
              <p><strong>当前任务：</strong>{currentTask.date} / {currentTask.receptionName} / {currentTask.receptionDept}</p>
              <p><strong>下一条：</strong>{nextTask ? `${nextTask.date} / ${nextTask.receptionName}` : "无"}</p>
              <p><strong>剩余：</strong>{Math.max(submissionItems.length - processedCount, 0)} 条</p>
            </div>
          ) : null}
        </div>

        {/* 中部申请列表 */}
        <div className="submission-table-wrap" ref={tableWrapRef}>
          {actionableSubmissionItems.length === 0 ? (
            <p className="empty">
              {submissionItems.length === 0
                ? dates.length === 0 || confirmedReceptions.length === 0
                  ? "请先选择日期并确认接待人"
                  : "当前没有待执行任务，请保留至少一条任务"
                : "当前筛选结果为空"}
            </p>
          ) : (
            <table className="submission-table">
              <thead>
                <tr>
                  <th>#</th>
                  <th>日期</th>
                  <th>星期</th>
                  <th>接待人</th>
                  <th>部门</th>
                  <th>状态</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {actionableSubmissionItems.map((item, idx) => (
                  <tr
                    key={`${item.date}-${item.receptionName}-${idx}`}
                    data-submission-key={buildExistingSubmissionKey(item.date, item.receptionId)}
                    className={`submission-row-${item.status}`}
                  >
                    <td>{idx + 1}</td>
                    <td>{item.date}</td>
                    <td>{item.weekday}</td>
                    <td>{item.receptionName}</td>
                    <td>{item.receptionDept}</td>
                    <td>
                      <span className={`badge-status badge-${item.status}`}>
                        {item.status === "pending" && (item.existing ? "已存在" : "待提交")}
                        {item.status === "submitting" && "提交中"}
                        {item.status === "success" && "已提交"}
                        {item.status === "skipped" && "已跳过"}
                        {item.status === "failed" && "失败"}
                        {item.status === "stopped" && "已停止"}
                      </span>
                    </td>
                    <td>
                      <button
                        type="button"
                        className="icon-action-btn icon-action-btn-danger submission-delete-btn"
                        onClick={() => removeSubmissionItem(item.date, item.receptionId)}
                        disabled={isRunning}
                        title="删除任务"
                        aria-label={`删除 ${item.date} ${item.receptionName} 任务`}
                      >
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                          <polyline points="3 6 5 6 21 6" />
                          <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                          <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
                          <line x1="10" y1="11" x2="10" y2="17" />
                          <line x1="14" y1="11" x2="14" y2="17" />
                        </svg>
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        {/* 底部操作栏 */}
        <div className="submission-actions">
          {!allVisitorsReady || !allReceptionsReady ? (
            <p className="hint" style={{ margin: 0 }}>
              请先完成所有访客和接待人查询后再提交。
            </p>
          ) : null}
          {isRunning && (
            <p className="countdown" style={{ margin: 0 }}>
              {countdownSeconds === null
                ? "正在提交中，等待服务端返回..."
                : countdownSeconds > 0
                  ? `下一次请求倒计时：${formatCountdown(countdownSeconds)}`
                  : "即将提交下一条..."}
            </p>
          )}
          <div className="submission-action-row">
            <div className="actions">
              <button type="button" className="btn-start" onClick={startSubmit} disabled={!canSubmit}>
                开始提交
              </button>
              <button type="button" className="btn-stop" onClick={stopSubmit} disabled={!isRunning}>
                停止提交
              </button>
              <button type="button" className="btn-clear" onClick={clearForm} disabled={isRunning}>
                清除表单
              </button>
            </div>
            <div className="submission-meta">
              {submissionItems.length > 0 && (
                <span className="submission-progress">
                  {processedCount}/{submissionItems.length}
                </span>
              )}
              <button
                type="button"
                className="btn-secondary"
                onClick={() => setLogModalOpen(true)}
              >
                查看日志
              </button>
            </div>
          </div>
        </div>
      </aside>
      </div>

      {/* 任务完成弹窗 */}
      {completionModalOpen ? (() => {
        const successCount = submissionItems.filter((i) => i.status === "success").length;
        const skippedCount = submissionItems.filter((i) => i.status === "skipped").length;
        const failedCount = submissionItems.filter((i) => i.status === "failed").length;
        return (
          <div className="modal-overlay" onClick={() => setCompletionModalOpen(false)}>
            <div className="completion-modal" onClick={(e) => e.stopPropagation()}>
              <div className={failedCount > 0 ? "completion-icon completion-icon-warning" : "completion-icon completion-icon-success"}>
                {failedCount > 0 ? "!" : "✓"}
              </div>
              <h3 className="completion-title">
                {failedCount > 0 ? "任务已完成（部分失败）" : "任务已完成"}
              </h3>
              <div className="completion-stats">
                {successCount > 0 && <span className="stat-item stat-success">提交成功 {successCount}</span>}
                {skippedCount > 0 && <span className="stat-item stat-skipped">已跳过 {skippedCount}</span>}
                {failedCount > 0 && <span className="stat-item stat-failed">失败 {failedCount}</span>}
              </div>
              {failedCount > 0 ? (
                <div className="completion-actions">
                  <button
                    type="button"
                    className="btn-secondary completion-btn"
                    onClick={copyFailedSubmissionItems}
                  >
                    复制失败项
                  </button>
                  <button
                    type="button"
                    className="btn-start completion-btn"
                    onClick={keepOnlyFailedSubmissionItems}
                  >
                    仅保留失败项重试
                  </button>
                </div>
              ) : null}
              <button
                type="button"
                className="btn-primary completion-btn"
                onClick={() => setCompletionModalOpen(false)}
              >
                关闭
              </button>
            </div>
          </div>
        );
      })() : null}

      {/* 日志弹窗 */}
      {logModalOpen ? (
        <div className="modal-overlay" onClick={() => setLogModalOpen(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>提交日志</h3>
              <div className="icon-actions">
                <button
                  type="button"
                  className="icon-action-btn"
                  onClick={copyAllLogs}
                  disabled={logs.length === 0}
                  title="复制全部日志"
                  aria-label="复制全部日志"
                >
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
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
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <polyline points="3 6 5 6 21 6" />
                    <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
                    <line x1="10" y1="11" x2="10" y2="17" />
                    <line x1="14" y1="11" x2="14" y2="17" />
                  </svg>
                </button>
                <button
                  type="button"
                  className="modal-close"
                  onClick={() => setLogModalOpen(false)}
                >
                  &times;
                </button>
              </div>
            </div>
            {logActionFeedback ? (
              <p className={`log-action-feedback log-action-feedback-${logActionFeedback.type}`} style={{ margin: "0 20px" }}>
                {logActionFeedback.message}
              </p>
            ) : null}
            <div className="modal-log-body">
              {logs.length === 0 ? <p className="empty">暂无日志</p> : null}
              {[...logs].reverse().map((log, rIndex) => {
                const index = logs.length - 1 - rIndex;
                return (
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
                          : log.result === "status_query"
                            ? "[预约记录查询]"
                            : log.result === "status_query_failed"
                              ? "[预约记录查询失败]"
                              : log.date ?? "-"}{" "}
                      | {log.result}
                      {typeof log.waitSeconds === "number"
                        ? ` | 等待 ${log.waitSeconds}s`
                        : ""}
                    </p>
                    {log.reason ? <p>原因: {log.reason}</p> : null}
                    {log.responseRaw ? <pre>{log.responseRaw}</pre> : null}
                  </article>
                );
              })}
            </div>
          </div>
        </div>
      ) : null}

      {statusModalOpen ? (
        <div className="modal-overlay" onClick={() => setStatusModalOpen(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>预约记录</h3>
              <button
                type="button"
                className="modal-close"
                onClick={() => setStatusModalOpen(false)}
              >
                &times;
              </button>
            </div>
            {statusRecords.length === 0 ? (
              <p className="empty">暂无预约记录</p>
            ) : (
              <div className="modal-table-wrap">
                <table className="status-table">
                  <thead>
                    <tr>
                      <th>单号</th>
                      <th>访客姓名</th>
                      <th>访客电话</th>
                      <th>接待人</th>
                      <th>接待人联系方式</th>
                      <th>权限生效时间</th>
                      <th>权限截止时间</th>
                      <th>权限状态</th>
                      <th>申请时间</th>
                    </tr>
                  </thead>
                  <tbody>
                    {statusRecords.map((r) => (
                      <tr key={r.flowNum}>
                        <td>{r.flowNum}</td>
                        <td>{r.visitorName}</td>
                        <td>{r.visitorPhone}</td>
                        <td>{r.rPersonName}</td>
                        <td>{r.rPersonPhone}</td>
                        <td>{r.dateStart}</td>
                        <td>{r.dateEnd}</td>
                        <td>{r.flowStatus}</td>
                        <td>{r.createTime}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </div>
      ) : null}

      {/* 批量添加访客弹窗 */}
      {batchVisitorModalOpen ? (
        <div className="modal-overlay" onClick={() => setBatchVisitorModalOpen(false)}>
          <div className="batch-modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>批量添加访客</h3>
              <button
                type="button"
                className="modal-close"
                onClick={() => setBatchVisitorModalOpen(false)}
              >
                &times;
              </button>
            </div>
            <div className="batch-modal-body">
              <p className="batch-hint">请输入身份证号和手机号，每行一个，手机号选填（空格分隔）：</p>
              <textarea
                className="batch-textarea"
                rows={8}
                placeholder={"310101199001011234 13800138000\n320102199002022345\n330103199003033456 15912345678"}
                value={batchVisitorText}
                onChange={(e) => setBatchVisitorText(e.currentTarget.value)}
                autoFocus
              />
            </div>
            <div className="batch-modal-actions">
              <button type="button" className="btn-secondary" onClick={() => setBatchVisitorModalOpen(false)}>
                取消
              </button>
              <button
                type="button"
                disabled={!batchVisitorText.trim()}
                onClick={confirmBatchVisitors}
              >
                确认添加
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {/* 批量添加接待人弹窗 */}
      {batchReceptionModalOpen ? (
        <div className="modal-overlay" onClick={() => setBatchReceptionModalOpen(false)}>
          <div className="batch-modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>批量添加接待人</h3>
              <button
                type="button"
                className="modal-close"
                onClick={() => setBatchReceptionModalOpen(false)}
              >
                &times;
              </button>
            </div>
            <div className="batch-modal-body">
              <p className="batch-hint">请输入员工号，每行一个：</p>
              <textarea
                className="batch-textarea"
                rows={8}
                placeholder={"E001\nE002\nE003"}
                value={batchReceptionText}
                onChange={(e) => setBatchReceptionText(e.currentTarget.value)}
                autoFocus
              />
            </div>
            <div className="batch-modal-actions">
              <button type="button" className="btn-secondary" onClick={() => setBatchReceptionModalOpen(false)}>
                取消
              </button>
              <button
                type="button"
                disabled={!batchReceptionText.trim()}
                onClick={confirmBatchReceptions}
              >
                确认添加
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </main>
  );
}

export default App;
