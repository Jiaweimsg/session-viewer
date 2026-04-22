import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

interface ForceUpdatePayload {
  current: string;
  min_required: string;
}

export function ForceUpdateOverlay() {
  const [info, setInfo] = useState<ForceUpdatePayload | null>(null);
  const [updating, setUpdating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const unlistenBlock = listen<ForceUpdatePayload>("force-update", (e) => {
      setInfo(e.payload);
      setError(null);
    });
    const unlistenClear = listen("force-update-cleared", () => {
      setInfo(null);
      setError(null);
      setUpdating(false);
    });
    return () => {
      unlistenBlock.then((fn) => fn()).catch(() => {});
      unlistenClear.then((fn) => fn()).catch(() => {});
    };
  }, []);

  if (!info) return null;

  const handleUpdate = async () => {
    setUpdating(true);
    setError(null);
    try {
      await invoke("start_self_update");
      // success: the updater will relaunch the app; nothing else to do.
    } catch (e) {
      setError(String(e));
      setUpdating(false);
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
          disabled={updating}
          style={{
            width: "100%",
            padding: "12px",
            fontSize: "14px",
            fontWeight: 600,
            background: updating ? "#b8b0e8" : "#6c5ce7",
            color: "#fff",
            border: "none",
            borderRadius: "8px",
            cursor: updating ? "not-allowed" : "pointer",
            transition: "background 0.2s",
          }}
        >
          {updating ? "正在更新..." : "立即更新"}
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
            }}
          >
            {error}
          </div>
        )}
      </div>
    </div>
  );
}
