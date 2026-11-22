"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
  burnRate,
  ensureConnected,
  formatAmount,
  loadEmployeeStreams,
  loadEmployerStreams,
  shortAddr,
  STREAM_CONTRACT_ID,
  withdraw,
  withdrawAll,
  type StreamRow,
} from "@/lib/stellar";

type Tab = "employer" | "employee";

const STORAGE_KEY = "paystream.session";
const LOW_BALANCE_DISMISS_KEY = "paystream.lowBalanceDismissed";

const FOCUSABLE_SELECTOR =
  'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Reusable focus-trap hook for modals/dialogs.
 *
 * While `active` is true, Tab/Shift+Tab are constrained to the focusable
 * elements inside `containerRef`, and focus is moved into the container.
 * When the trap deactivates, focus is restored to the element that was
 * focused before the trap engaged.
 */
export function useFocusTrap(
  containerRef: React.RefObject<HTMLElement | null>,
  active: boolean,
) {
  useEffect(() => {
    if (!active) return;
    const container = containerRef.current;
    if (!container) return;

    const previouslyFocused = document.activeElement as HTMLElement | null;

    const getFocusable = () =>
      Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
        (el) => el.offsetParent !== null || el === document.activeElement,
      );

    const focusables = getFocusable();
    (focusables[0] ?? container).focus();

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return;
      const items = getFocusable();
      if (items.length === 0) {
        event.preventDefault();
        container.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const current = document.activeElement as HTMLElement | null;

      if (event.shiftKey) {
        if (current === first || !container.contains(current)) {
          event.preventDefault();
          last.focus();
        }
      } else if (current === last || !container.contains(current)) {
        event.preventDefault();
        first.focus();
      }
    };

    container.addEventListener("keydown", onKeyDown);
    return () => {
      container.removeEventListener("keydown", onKeyDown);
      if (previouslyFocused && typeof previouslyFocused.focus === "function") {
        previouslyFocused.focus();
      }
    };
  }, [containerRef, active]);
}

type PersistedSession = { address: string; tab: Tab };

function readPersistedSession(): PersistedSession | null {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<PersistedSession>;
    if (typeof parsed.address !== "string" || !parsed.address) return null;
    const tab: Tab = parsed.tab === "employee" ? "employee" : "employer";
    return { address: parsed.address, tab };
  } catch {
    return null;
  }
}

function clearPersistedSession() {
  try {
    window.localStorage.removeItem(STORAGE_KEY);
  } catch {
    /* ignore */
  }
}

function writePersistedSession(session: PersistedSession) {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(session));
  } catch {
    /* ignore */
  }
}

export default function Dashboard() {
  const [address, setAddress] = useState("");
  const [tab, setTab] = useState<Tab>("employer");
  const [employerStreams, setEmployerStreams] = useState<StreamRow[]>([]);
  const [employeeStreams, setEmployeeStreams] = useState<StreamRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [lowBalanceDismissed, setLowBalanceDismissed] = useState(false);

  const refresh = useCallback(async (addr: string) => {
    setLoading(true);
    setError("");
    try {
      const [emp, epi] = await Promise.all([
        loadEmployerStreams(addr),
        loadEmployeeStreams(addr),
      ]);
      setEmployerStreams(emp);
      setEmployeeStreams(epi);
    } catch (e) {
      setError(String((e as Error).message ?? e));
    } finally {
      setLoading(false);
    }
  }, []);

  const connect = async () => {
    try {
      const addr = await ensureConnected();
      setAddress(addr);
      await refresh(addr);
    } catch (e) {
      setError(String((e as Error).message ?? e));
    }
  };

  useEffect(() => {
    if (!STREAM_CONTRACT_ID) {
      setError("NEXT_PUBLIC_STREAM_CONTRACT_ID is not set — copy .env.example to .env.local.");
    }
  }, []);

  // Restore persisted session on load. This only *validates* the wallet
  // connection/network — it never prompts or invokes wallet methods that
  // require a user gesture.
  useEffect(() => {
    const persisted = readPersistedSession();
    if (!persisted) return;
    let cancelled = false;

    (async () => {
      try {
        const validated = await ensureConnected({ prompt: false });
        if (cancelled) return;
        if (!validated || validated !== persisted.address) {
          clearPersistedSession();
          return;
        }
        setAddress(validated);
        setTab(persisted.tab);
        await refresh(validated);
      } catch {
        if (!cancelled) clearPersistedSession();
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [refresh]);

  // Persist address + active tab whenever they change.
  useEffect(() => {
    if (!address) return;
    writePersistedSession({ address, tab });
  }, [address, tab]);

  // Restore per-session low-balance banner dismissal.
  useEffect(() => {
    try {
      setLowBalanceDismissed(window.sessionStorage.getItem(LOW_BALANCE_DISMISS_KEY) === "1");
    } catch {
      /* ignore */
    }
  }, []);

  const dismissLowBalance = () => {
    setLowBalanceDismissed(true);
    try {
      window.sessionStorage.setItem(LOW_BALANCE_DISMISS_KEY, "1");
    } catch {
      /* ignore */
    }
  };

  const doWithdraw = async (streamId: bigint) => {
    setBusyId(streamId.toString());
    setError("");
    try {
      await withdraw(address, streamId);
      await refresh(address);
    } catch (e) {
      setError(String((e as Error).message ?? e));
    } finally {
      setBusyId(null);
    }
  };

  const doWithdrawAll = async () => {
    setBusyId("all");
    setError("");
    try {
      await withdrawAll(address);
      await refresh(address);
    } catch (e) {
      setError(String((e as Error).message ?? e));
    } finally {
      setBusyId(null);
    }
  };

  const streams = tab === "employer" ? employerStreams : employeeStreams;
  const totalClaimable = streams
    .filter((s) => s.status_label === "Active")
    .reduce((sum, s) => sum + s.claimable, 0n);

  const lowBalanceStreams = employerStreams.filter(
    (s) => s.status_label === "Active" && s.remaining < s.low_balance_threshold,
  );

  return (
    <main id="main-content">
      <a href="#main-content" className="skip-link">
        Skip to content
      </a>
      <header>
        <div>
          <h1>PayStream Dashboard</h1>
          <div className="muted">
            contract <code>{STREAM_CONTRACT_ID ? shortAddr(STREAM_CONTRACT_ID) : "unset"}</code> · Stellar testnet
          </div>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          {address && (
            <button className="secondary" onClick={() => refresh(address)} disabled={loading}>
              {loading ? "Refreshing…" : "Refresh"}
            </button>
          )}
          <button onClick={connect} disabled={loading}>
            {address ? shortAddr(address) : "Connect Freighter"}
          </button>
        </div>
      </header>

      {error && <div className="error">{error}</div>}

      {!address ? (
        <div className="notice">
          Connect your Freighter wallet to view streams. The dashboard reads streams where the
          connected account is employer or employee on the configured testnet contract.
        </div>
      ) : (
        <>
          <div className="tabs">
            <button className={tab === "employer" ? "active" : ""} onClick={() => setTab("employer")}>
              Employer ({employerStreams.length})
            </button>
            <button className={tab === "employee" ? "active" : ""} onClick={() => setTab("employee")}>
              Employee ({employeeStreams.length})
            </button>
          </div>

          {tab === "employer" ? (
            <>
              {lowBalanceStreams.length > 0 && !lowBalanceDismissed && (
                <div className="banner warning" role="status">
                  <div>
                    <strong>Low balance:</strong> {lowBalanceStreams.length} stream
                    {lowBalanceStreams.length === 1 ? "" : "s"} below threshold.{" "}
                    {lowBalanceStreams.map((s, i) => (
                      <span key={s.id.toString()}>
                        {i > 0 && ", "}
                        <a href={`#stream-${s.id.toString()}`}>#{s.id.toString()}</a>
                      </span>
                    ))}
                  </div>
                  <button className="secondary" onClick={dismissLowBalance} aria-label="Dismiss low balance alert">
                    Dismiss
                  </button>
                </div>
              )}
              <div className="cards">
                <Stat label="Streams" value={String(employerStreams.length)} />
                <Stat label="Burn rate" value={`${formatAmount(burnRate(employerStreams))}/s`} />
                <Stat label="Total claimable" value={formatAmount(totalClaimable)} />
                <Stat
                  label="Total remaining"
                  value={formatAmount(employerStreams.reduce((s, x) => s + x.remaining, 0n))}
                />
              </div>
            </>
          ) : (
            <div className="cards">
              <Stat label="Streams" value={String(employeeStreams.length)} />
              <Stat label="Claimable now" value={formatAmount(totalClaimable)} />
            </div>
          )}

          {tab === "employee" && totalClaimable > 0n && (
            <p>
              <button onClick={doWithdrawAll} disabled={busyId !== null}>
                {busyId === "all" ? "Withdrawing…" : "Withdraw all"}
              </button>
            </p>
          )}

          {streams.length === 0 ? (
            <div className="notice">No {tab} streams for this account.</div>
          ) : (
            <StreamTable
              tab={tab}
              streams={streams}
              busyId={busyId}
              onWithdraw={doWithdraw}
            />
          )}
        </>
      )}
    </main>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="card">
      <div className="label">{label}</div>
      <div className="value">{value}</div>
    </div>
  );
}

function StreamTable({
  tab,
  streams,
  busyId,
  onWithdraw,
}: {
  tab: Tab;
  streams: StreamRow[];
  busyId: string | null;
  onWithdraw: (id: bigint) => void;
}) {
  return (
    <div className="table-wrap">
      <table className="stream-table">
        <thead>
          <tr>
            <th scope="col">ID</th>
            <th scope="col">{tab === "employer" ? "Employee" : "Employer"}</th>
            <th scope="col">Status</th>
            <th scope="col">Claimable</th>
            <th scope="col">Burn rate</th>
            {tab === "employee" && <th scope="col" />}
          </tr>
        </thead>
        <tbody>
          {streams.map((s) => (
            <tr key={s.id.toString()} id={`stream-${s.id.toString()}`}>
              <td data-label="ID" className="sticky-col">
                {s.id.toString()}
              </td>
              <td data-label={tab === "employer" ? "Employee" : "Employer"}>
                <code>{shortAddr(tab === "employer" ? s.employee : s.employer)}</code>
              </td>
              <td data-label="Status">
                <span className={`pill ${s.status_label}`}>{s.status_label}</span>
              </td>
              <td data-label="Claimable">{formatAmount(s.claimable)}</td>
              <td data-label="Burn rate">
                {s.status_label === "Active" ? `${formatAmount(s.rate_per_second)}/s` : "—"}
              </td>
              {tab === "employee" && (
                <td data-label="Actions" style={{ textAlign: "right" }}>
                  <button
                    onClick={() => onWithdraw(s.id)}
                    disabled={s.status_label !== "Active" || s.claimable <= 0n || busyId !== null}
                  >
                    {busyId === s.id.toString() ? "…" : "Withdraw"}
                  </button>
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
