import { useRef, useEffect } from "react";
import { Check } from "lucide-react";
import { useTheme } from "../../hooks/useTheme";
import type { ThemeOption } from "../../hooks/useTheme";

interface Props {
  onClose: () => void;
}

export function ThemePicker({ onClose }: Props) {
  const { theme, setTheme, themes } = useTheme();
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [onClose]);

  const handleSelect = (t: ThemeOption) => {
    setTheme(t.id);
    onClose();
  };

  return (
    <div
      ref={ref}
      className="absolute bottom-12 left-2 right-2 bg-card border border-border rounded-lg shadow-xl p-2 z-50"
    >
      <p className="text-xs font-medium text-muted-foreground px-2 py-1 mb-1">
        主题
      </p>
      <div className="grid grid-cols-2 gap-1">
        {themes.map((t) => (
          <button
            key={t.id}
            onClick={() => handleSelect(t)}
            className={`flex items-center gap-2 px-2 py-1.5 rounded-md text-xs transition-colors ${
              theme === t.id
                ? "bg-accent text-accent-foreground"
                : "hover:bg-accent/50 text-muted-foreground hover:text-foreground"
            }`}
          >
            <div className="flex gap-0.5 shrink-0">
              <div
                className="w-3 h-3 rounded-sm border border-border/50"
                style={{ backgroundColor: t.colors.bg }}
              />
              <div
                className="w-3 h-3 rounded-sm border border-border/50"
                style={{ backgroundColor: t.colors.accent }}
              />
            </div>
            <span className="truncate">{t.name}</span>
            {theme === t.id && <Check className="w-3 h-3 shrink-0 ml-auto" />}
          </button>
        ))}
      </div>
    </div>
  );
}
