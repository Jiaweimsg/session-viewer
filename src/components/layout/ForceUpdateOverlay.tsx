import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";

interface ForceUpdatePayload {
  current: string;
  min_required: string;
}

const RELEASES_URL = "https://github.com/Jiaweimsg/session-viewer/releases";

export function ForceUpdateOverlay() {
  const [info, setInfo] = useState<ForceUpdatePayload | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const unlistenBlock = listen<ForceUpdatePayload>("force-update", (e) => {
      setInfo(e.payload);
      setError(null);
    });
    const unlistenClear = listen("force-update-cleared", () => {
      setInfo(null);
      setError(null);
    });
    return () => {
      unlistenBlock.then((fn) => fn()).catch(() => {});
      unlistenClear.then((fn) => fn()).catch(() => {});
    };
  }, []);

  if (!info) return null;

  const handleUpdate = async () => {
    setError(null);
    try {
      await open(RELEASES_URL);
    } catch (e) {
      setError(`打开浏览器失败：${e}。请手动访问 ${RELEASES_URL}`);
    }
  };

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

        <button
          onClick={handleUpdate}
          style={{
            width: "100%",
            padding: "12px",
            fontSize: "14px",
            fontWeight: 600,
            background: "#6c5ce7",
            color: "#fff",
            border: "none",
            borderRadius: "8px",
            cursor: "pointer",
            transition: "background 0.2s",
          }}
        >
          立即更新
        </button>

        {error && (
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
            {error}
          </div>
        )}
      </div>
    </div>
  );
}
