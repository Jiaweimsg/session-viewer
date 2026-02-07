import { format } from "date-fns";

export function formatTime(timestamp: string): string {
  try {
    return format(new Date(timestamp), "yyyy-MM-dd HH:mm:ss");
  } catch {
    return timestamp;
  }
}
