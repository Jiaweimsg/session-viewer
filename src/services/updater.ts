// 应用内自动更新封装。
//
// 两处更新点（设置页"检查更新" + 服务端下发的强制更新遮罩）共用本模块：
// 通过 tauri-plugin-updater 拉取 GitHub Release 上的 latest.json，校验 minisign
// 签名后下载对应平台产物并就地安装，最后用 tauri-plugin-process 重启加载新版本。
// 全程在应用内完成，不再跳转浏览器。

import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type { Update };

/** 下载进度。`total` 为 null 表示服务端未返回 Content-Length（进度条走不确定态）。 */
export interface DownloadProgress {
  /** 已下载字节数 */
  downloaded: number;
  /** 总字节数，未知时为 null */
  total: number | null;
  /** 0–1 的完成比例，total 未知时为 null */
  fraction: number | null;
}

/**
 * 检查是否有新版本。
 * - 有更新 → 返回 `Update`（含 version / body 等）
 * - 已是最新 → 返回 `null`
 * - 网络 / 签名 / 解析错误 → 抛出，由调用方兜底处理
 */
export async function checkForUpdate(): Promise<Update | null> {
  return await check();
}

/**
 * 下载并安装给定更新，期间通过 `onProgress` 回报进度；安装完成后重启应用。
 * 注意：`relaunch()` 之后进程即被替换，函数正常不会返回。
 */
export async function installUpdate(
  update: Update,
  onProgress?: (p: DownloadProgress) => void,
): Promise<void> {
  let downloaded = 0;
  let total: number | null = null;

  await update.downloadAndInstall((event) => {
    switch (event.event) {
      case "Started":
        total = event.data.contentLength ?? null;
        downloaded = 0;
        onProgress?.({ downloaded, total, fraction: total ? 0 : null });
        break;
      case "Progress":
        downloaded += event.data.chunkLength;
        onProgress?.({
          downloaded,
          total,
          fraction: total ? Math.min(downloaded / total, 1) : null,
        });
        break;
      case "Finished":
        onProgress?.({ downloaded, total, fraction: total ? 1 : null });
        break;
    }
  });

  await relaunch();
}
