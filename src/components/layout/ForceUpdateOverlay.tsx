import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

interface ForceUpdatePayload {
  current: string;
  min_required: string;
}

export function ForceUpdateOverlay() {
  const [info, setInfo] = useState<ForceUpdatePayload | null>(null);

  useEffect(() => {
    const unlistenPromise = listen<ForceUpdatePayload>("force-update", (e) => {
      setInfo(e.payload);
    });
    return () => {
      unlistenPromise.then((fn) => fn()).catch(() => {});
    };
  }, []);

  if (!info) return null;

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
          <strong style={{ color: "#e17055" }}>v{info.current}</strong>
          。请联系管理员获取最新安装包。
          <br />
          <br />
          在升级前，会话浏览功能仍可使用，但使用数据上报已停止。
        </p>
        <div
          style={{
            fontSize: "12px",
            color: "#999",
            padding: "8px 12px",
            background: "#f5f5f7",
            borderRadius: "6px",
            fontFamily: "ui-monospace, monospace",
          }}
        >
          session-viewer v{info.current} → v{info.min_required}
        </div>
      </div>
    </div>
  );
}
