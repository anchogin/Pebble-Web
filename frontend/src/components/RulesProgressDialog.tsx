import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { CheckCircle, X, XCircle } from "lucide-react";
import { cancelAllRulesRun, cancelRuleRun } from "@/lib/api";
import { wsClient } from "@/lib/websocket";

const RULE_EXEC_EVENTS = [
  "rules_exec_started",
  "rules_exec_progress",
  "rules_exec_completed",
  "rules_exec_cancelled",
  "rules_exec_error",
] as const;

type RuleExecEventType = (typeof RULE_EXEC_EVENTS)[number];
type RuleExecStatus = "running" | "completed" | "cancelled" | "error";

interface Props {
  readonly ruleId: string | null;
  readonly runId: string;
  readonly ruleName: string;
  readonly onReady?: (ruleId: string | null, runId: string) => void;
  readonly onClose: () => void;
}

interface RuleExecState {
  readonly total: number | null;
  readonly processed: number;
  readonly matched: number;
  readonly actionsApplied: number;
  readonly errors: number;
  readonly status: RuleExecStatus;
  readonly errorMessage: string | null;
}

interface RuleExecMessage {
  readonly type: RuleExecEventType;
  readonly ruleId: string | null;
  readonly runId: string | null;
  readonly data: Record<string, unknown>;
}

const initialState: RuleExecState = {
  total: null,
  processed: 0,
  matched: 0,
  actionsApplied: 0,
  errors: 0,
  status: "running",
  errorMessage: null,
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isRuleExecEventType(value: unknown): value is RuleExecEventType {
  return value === "rules_exec_started"
    || value === "rules_exec_progress"
    || value === "rules_exec_completed"
    || value === "rules_exec_cancelled"
    || value === "rules_exec_error";
}

function readRuleId(value: unknown): string | null | undefined {
  if (typeof value === "string") return value;
  if (value === null) return null;
  return undefined;
}

function readRunId(value: unknown): string | null | undefined {
  if (typeof value === "string") return value;
  if (value === null) return null;
  return undefined;
}

function readNumber(data: Record<string, unknown>, key: string): number | undefined {
  const value = data[key];
  return typeof value === "number" ? value : undefined;
}

function parseRuleExecMessage(message: unknown): RuleExecMessage | null {
  if (!isRecord(message) || !isRuleExecEventType(message.type)) return null;

  const ruleId = readRuleId(message.rule_id);
  if (ruleId === undefined) return null;
  const runId = readRunId(message.run_id);
  if (runId === undefined) return null;

  return {
    type: message.type,
    ruleId,
    runId,
    data: isRecord(message.data) ? message.data : {},
  };
}

export default function RulesProgressDialog({ ruleId, runId, ruleName, onReady, onClose }: Props) {
  const { t } = useTranslation();
  const [state, setState] = useState<RuleExecState>(initialState);

  useEffect(() => {
    setState(initialState);
  }, [ruleId, runId]);

  useEffect(() => {
    const handleMessage = (message: unknown) => {
      const event = parseRuleExecMessage(message);
      if (!event) return;
      if (event.ruleId !== ruleId || event.runId !== runId) {
        console.log(`[执行规则]${ruleName}:忽略非当前执行事件`, {
          expectedRuleId: ruleId,
          expectedRunId: runId,
          eventRuleId: event.ruleId,
          eventRunId: event.runId,
          type: event.type,
        });
        return;
      }

      const processed = readNumber(event.data, "processed");
      const total = readNumber(event.data, "total");
      if (event.type !== "rules_exec_progress" || processed === undefined || processed % 100 === 0 || processed === total) {
        console.log(`[执行规则]${ruleName}:收到执行事件`, {
          ruleId,
          runId,
          type: event.type,
          data: event.data,
        });
      }

      if (event.type === "rules_exec_started") {
        setState((current) => ({
          ...current,
          total: total ?? null,
          processed: 0,
          matched: 0,
          actionsApplied: 0,
          errors: 0,
          status: "running",
          errorMessage: null,
        }));
        return;
      }

      if (event.type === "rules_exec_progress") {
        setState((current) => ({
          ...current,
          processed: readNumber(event.data, "processed") ?? current.processed,
          matched: readNumber(event.data, "matched") ?? current.matched,
          actionsApplied: readNumber(event.data, "actions_applied") ?? current.actionsApplied,
        }));
        return;
      }

      if (event.type === "rules_exec_completed") {
        setState((current) => ({
          ...current,
          total: total ?? current.total,
          processed: readNumber(event.data, "processed") ?? total ?? current.processed,
          matched: readNumber(event.data, "matched") ?? current.matched,
          actionsApplied: readNumber(event.data, "actions_applied") ?? current.actionsApplied,
          errors: readNumber(event.data, "errors") ?? current.errors,
          status: "completed",
          errorMessage: null,
        }));
        return;
      }

      if (event.type === "rules_exec_cancelled") {
        setState((current) => ({
          ...current,
          total: total ?? current.total,
          processed: readNumber(event.data, "processed") ?? current.processed,
          matched: readNumber(event.data, "matched") ?? current.matched,
          actionsApplied: readNumber(event.data, "actions_applied") ?? current.actionsApplied,
          errors: readNumber(event.data, "errors") ?? current.errors,
          status: "cancelled",
          errorMessage: null,
        }));
        return;
      }

      const messageValue = event.data.message;
      setState((current) => ({
        ...current,
        status: "error",
        errorMessage: typeof messageValue === "string" ? messageValue : t("rules.progressUnknownError", "Unknown error"),
      }));
    };

    RULE_EXEC_EVENTS.forEach((eventName) => wsClient.on(eventName, handleMessage));
    let cancelled = false;
    console.log(`[执行规则]${ruleName}:等待WebSocket就绪`, { ruleId, runId });
    wsClient.connect();
    wsClient.waitUntilAuthenticated().then((ready) => {
      if (cancelled) return;
      if (ready) {
        console.log(`[执行规则]${ruleName}:WebSocket已就绪`, { ruleId, runId });
        onReady?.(ruleId, runId);
        return;
      }
      console.warn(`[执行规则]${ruleName}:WebSocket不可用`, { ruleId, runId });
      setState((current) => ({
        ...current,
        status: "error",
        errorMessage: t("rules.progressWsUnavailable", "WebSocket connection unavailable. Please refresh and try again."),
      }));
    });

    return () => {
      cancelled = true;
      RULE_EXEC_EVENTS.forEach((eventName) => wsClient.off(eventName, handleMessage));
    };
  }, [onReady, ruleId, runId, ruleName, t]);

  const hasTotal = state.total !== null;
  const percent = hasTotal
    ? state.total === 0
      ? 100
      : Math.min(100, Math.round((state.processed / state.total) * 100))
    : 0;
  const isRunning = state.status === "running";
  const isCompleted = state.status === "completed";
  const isCancelled = state.status === "cancelled";
  const isError = state.status === "error";

  const handleClose = () => {
    if (isRunning) {
      const cancel = ruleId ? cancelRuleRun(ruleId, runId) : cancelAllRulesRun(runId);
      void cancel
        .then((result) => {
          console.log(`[执行规则]${ruleName}:取消接口返回`, { ruleId, runId, result });
        })
        .catch((err: unknown) => {
          console.warn(`[执行规则]${ruleName}:取消接口失败`, { ruleId, runId, err });
        });
    }
    console.log(`[执行规则]${ruleName}:关闭进度弹窗`, { ruleId, runId, status: state.status });
    onClose();
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="rules-progress-dialog-title"
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1100,
      }}
      >
        <style>{`
          @keyframes rules-progress-indeterminate {
            0% { transform: translateX(-100%) scaleX(0.4); }
            50% { transform: translateX(0%) scaleX(0.6); }
            100% { transform: translateX(100%) scaleX(0.4); }
          }
          .rules-progress-indeterminate {
            animation: rules-progress-indeterminate 1.4s ease-in-out infinite;
            transform-origin: left center;
          }
        `}</style>
      <div
        style={{
          width: "min(520px, calc(100vw - 32px))",
          backgroundColor: "var(--color-bg)",
          color: "var(--color-text-primary)",
          border: "1px solid var(--color-border)",
          borderRadius: "10px",
          boxShadow: "0 20px 60px rgba(0,0,0,0.3)",
          padding: "20px",
          display: "flex",
          flexDirection: "column",
          gap: "16px",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "12px" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            {isCompleted && <CheckCircle size={18} color="#22c55e" />}
            {isCancelled && <XCircle size={18} color="#f59e0b" />}
            {isError && <XCircle size={18} color="#ef4444" />}
            <h3
              id="rules-progress-dialog-title"
              style={{
                margin: 0,
                fontSize: "15px",
                fontWeight: 600,
                color: isCompleted ? "#22c55e" : isCancelled ? "#f59e0b" : isError ? "#ef4444" : "var(--color-text-primary)",
              }}
            >
              {ruleId ? t("rules.progressTitle") : t("rules.progressTitleAll")}
            </h3>
          </div>
          <button
            onClick={handleClose}
            aria-label={t("common.close")}
            style={{
              background: "none",
              border: "none",
              cursor: "pointer",
              padding: "4px",
              borderRadius: "4px",
              color: "var(--color-text-secondary)",
              display: "flex",
              alignItems: "center",
            }}
          >
            <X size={18} />
          </button>
        </div>

        <p style={{ margin: 0, fontSize: "13px", color: "var(--color-text-secondary)", lineHeight: 1.5 }}>
            {isRunning ? t("rules.executing") : isCompleted ? t("rules.progressComplete", {
              matched: state.matched,
              applied: state.actionsApplied,
            }) : isCancelled ? t("rules.progressCancelled", "Execution cancelled") : state.errorMessage}
        </p>

        <div
          style={{
            width: "100%",
            height: "8px",
            borderRadius: "999px",
            backgroundColor: "var(--color-border)",
            overflow: "hidden",
          }}
        >
          <div
            className={!hasTotal && isRunning ? "rules-progress-indeterminate" : undefined}
            style={{
              width: hasTotal ? `${percent}%` : "45%",
              height: "100%",
              borderRadius: "999px",
              backgroundColor: isError ? "#ef4444" : isCancelled ? "#f59e0b" : isCompleted ? "#22c55e" : "var(--color-accent)",
              transition: "width 180ms ease-out",
            }}
          />
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: "8px 12px" }}>
          <span style={{ fontSize: "13px", color: "var(--color-text-secondary)" }}>
            {t("rules.progressProcessed")}: <strong style={{ color: "var(--color-text-primary)" }}>{state.processed}</strong>
            {hasTotal && <span> / {state.total}</span>}
          </span>
          <span style={{ fontSize: "13px", color: "var(--color-text-secondary)" }}>
            {t("rules.progressMatched")}: <strong style={{ color: "var(--color-text-primary)" }}>{state.matched}</strong>
          </span>
          <span style={{ fontSize: "13px", color: "var(--color-text-secondary)" }}>
            {t("rules.progressActions")}: <strong style={{ color: "var(--color-text-primary)" }}>{state.actionsApplied}</strong>
          </span>
          <span style={{ fontSize: "13px", color: state.errors > 0 ? "#ef4444" : "var(--color-text-secondary)" }}>
            {t("rules.progressErrors")}: <strong>{state.errors}</strong>
          </span>
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end" }}>
          <button
            onClick={handleClose}
            style={{
              padding: "7px 16px",
              borderRadius: "6px",
              border: "none",
              backgroundColor: "var(--color-accent)",
              color: "#fff",
              fontSize: "13px",
              fontWeight: 600,
              cursor: "pointer",
            }}
          >
            {t("common.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
