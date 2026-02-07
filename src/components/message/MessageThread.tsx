import type { DisplayMessage } from "../../types";
import { UserMessage } from "./UserMessage";
import { AssistantMessage } from "./AssistantMessage";
import { Wrench } from "lucide-react";
import { formatTime } from "./utils";

interface MessageThreadProps {
  messages: DisplayMessage[];
}

export function MessageThread({ messages }: MessageThreadProps) {
  return (
    <div className="max-w-4xl mx-auto py-6 px-6 space-y-4">
      {messages.map((msg, i) => {
        if (msg.role === "user") {
          return <UserMessage key={msg.uuid || i} message={msg} />;
        }
        if (msg.role === "tool") {
          return <ToolRoleMessage key={msg.uuid || i} message={msg} />;
        }
        return <AssistantMessage key={msg.uuid || i} message={msg} />;
      })}
    </div>
  );
}

function ToolRoleMessage({ message }: { message: DisplayMessage }) {
  return (
    <div className="flex gap-3">
      <div className="shrink-0 w-7 h-7 rounded-full bg-primary/10 flex items-center justify-center">
        <Wrench className="w-3.5 h-3.5 text-primary" />
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-1">
          <span className="text-sm font-medium">Tool</span>
          {message.timestamp && (
            <span className="text-xs text-muted-foreground">
              {formatTime(message.timestamp)}
            </span>
          )}
        </div>
        {message.content.map((block, i) => {
          if (block.type === "text") {
            return (
              <div key={i} className="text-sm whitespace-pre-wrap break-words">
                {block.text}
              </div>
            );
          }
          if (block.type === "function_call_output") {
            return (
              <div
                key={i}
                className="mt-2 text-xs rounded-md p-3 font-mono overflow-x-auto bg-muted text-muted-foreground"
              >
                <pre className="whitespace-pre-wrap break-all">
                  {block.output.length > 2000
                    ? block.output.slice(0, 2000) + "\n... (truncated)"
                    : block.output}
                </pre>
              </div>
            );
          }
          if (block.type === "tool_result") {
            return (
              <div
                key={i}
                className={`mt-2 text-xs rounded-md p-3 font-mono overflow-x-auto ${
                  block.isError
                    ? "bg-destructive/10 text-destructive border border-destructive/20"
                    : "bg-muted text-muted-foreground"
                }`}
              >
                <pre className="whitespace-pre-wrap break-all">
                  {block.content.length > 2000
                    ? block.content.slice(0, 2000) + "\n... (truncated)"
                    : block.content}
                </pre>
              </div>
            );
          }
          return null;
        })}
      </div>
    </div>
  );
}
