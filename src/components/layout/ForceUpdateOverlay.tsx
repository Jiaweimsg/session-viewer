import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";
import { checkForUpdate, installUpdate } from "../../services/updater";

interface ForceUpdatePayload {
  current: string;
  min_required: string;
}

const RELEASES_URL = "https://github.com/Jiaweimsg/session-viewer/releases";

type Status =
  | { kind: "prompt" }
  | { kind: "checking" }
  | { kind: "downloading"; fraction: number | null }
  | { kind: "error"; message: string };

export function ForceUpdateOverlay() {
  const [info, setInfo] = useState<ForceUpdatePayload | null>(null);
  const [status, setStatus] = useState<Status>({ kind: "prompt" });

  useEffect(() => {
    const unlistenBlock = listen<ForceUpdatePayload>("force-update", (e) => {
      setInfo(e.payload);
      setStatus({ kind: "prompt" });
    });
    const unlistenClear = listen("force-update-cleared", () => {
      setInfo(null);
      setStatus({ kind: "prompt" });
    });
    return () => {
      unlistenBlock.then((fn) => fn()).catch(() => {});
      unlistenClear.then((fn) => fn()).catch(() => {});
    };
  }, []);

  if (!info) return null;

  const openReleases = async () => {
    try {
      await open(RELEASES_URL);
    } catch (e) {
      setStatus({
        kind: "error",
        message: `打开浏览器失败：${e}。请手动访问 ${RELEASES_URL}`,
      });
    }
  };

  // 应用内自动更新：检查 → 下载安装 → 重启。任何失败都退回手动下载。
  const handleUpdate = async () => {
    setStatus({ kind: "checking" });
    try {
      const update = await checkForUpdate();
      if (!update) {
        // 服务端抬高了最低版本，但发布渠道暂无更高版本的更新包 —— 退回手动下载。
        setStatus({
          kind: "error",
          message: "暂无可用的更新包，请前往 GitHub 手动下载安装最新版本。",
        });
        await openReleases();
        return;
      }
      setStatus({ kind: "downloading", fraction: 0 });
      // 安装完成后会自动重启，正常不会返回到这里。
      await installUpdate(update, (p) => {
        setStatus({ kind: "downloading", fraction: p.fraction });
      });
    } catch (e) {
      setStatus({
        kind: "error",
        message: `${e instanceof Error ? e.message : String(e)}`,
      });
    }
  };

  const busy = status.kind === "checking" || status.kind === "downloading";

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(26, 26, 46, 0.88)",
        zIndex: 99999,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        backdropFilter: "blur(6px)",
      }}
    >
      <div
        style={{
          background: "#fff",
          borderRadius: "12px",
          padding: "32px",
          maxWidth: "440px",
          width: "90%",
          boxShadow: "0 20px 60px rgba(0,0,0,0.4)",
          textAlign: "center",
        }}
      >
        <div style={{ fontSize: "44px", marginBottom: "12px" }}>⚠️</div>
        <h2 style={{ fontSize: "20px", fontWeight: 600, marginBottom: "8px", color: "#1a1a2e" }}>
          需要升级到新版本
        </h2>
        <p style={{ fontSize: "13px", color: "#666", marginBottom: "20px", lineHeight: 1.6 }}>
          服务端要求的最低客户端版本为{" "}
          <strong style={{ color: "#6c5ce7" }}>v{info.min_required}</strong>
          ，当前版本{" "}
          <strong style={{ color: "#e17055" }}>v{info.current}</strong>。
        </p>
        <div
          style={{
            fontSize: "12px",
            color: "#999",
            padding: "8px 12px",
            background: "#f5f5f7",
            borderRadius: "6px",
            fontFamily: "ui-monospace, monospace",
            marginBottom: "20px",
          }}
        >
          session-viewer v{info.current} → v{info.min_required}
        </div>

        {status.kind === "downloading" ? (
          <div>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                fontSize: "12px",
                color: "#666",
                marginBottom: "6px",
              }}
            >
              <span>正在下载并安装…</span>
              {status.fraction != null && <span>{Math.round(status.fraction * 100)}%</span>}
            </div>
            <div
              style={{
                height: "6px",
                width: "100%",
                background: "#eee",
                borderRadius: "999px",
                overflow: "hidden",
              }}
            >
              <div
                style={{
                  height: "100%",
                  background: "#6c5ce7",
                  borderRadius: "999px",
                  width: status.fraction != null ? `${status.fraction * 100}%` : "33%",
                  transition: "width 0.2s",
                }}
              />
            </div>
            <p style={{ fontSize: "12px", color: "#999", marginTop: "10px", lineHeight: 1.5 }}>
              更新完成后将自动重启，请勿手动关闭应用。
            </p>
          </div>
        ) : (
          <button
            onClick={handleUpdate}
            disabled={busy}
            style={{
              width: "100%",
              padding: "12px",
              fontSize: "14px",
              fontWeight: 600,
              background: busy ? "#a29bfe" : "#6c5ce7",
              color: "#fff",
              border: "none",
              borderRadius: "8px",
              cursor: busy ? "default" : "pointer",
              transition: "background 0.2s",
            }}
          >
            {status.kind === "checking" ? "检查中…" : "立即更新"}
          </button>
        )}

        {status.kind === "error" && (
          <div
            style={{
              marginTop: "12px",
              padding: "10px 12px",
              background: "#fff4e6",
              border: "1px solid #ffb88a",
              borderRadius: "6px",
              fontSize: "12px",
              color: "#b85400",
              textAlign: "left",
              lineHeight: 1.5,
              wordBreak: "break-all",
            }}
          >
            <div style={{ marginBottom: "8px" }}>更新失败：{status.message}</div>
            <button
              onClick={openReleases}
              style={{
                fontSize: "12px",
                fontWeight: 600,
                color: "#6c5ce7",
                background: "none",
                border: "none",
                padding: 0,
                cursor: "pointer",
                textDecoration: "underline",
              }}
            >
              前往 GitHub 手动下载
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
