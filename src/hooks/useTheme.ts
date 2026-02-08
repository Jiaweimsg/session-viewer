import { useState, useCallback, useEffect } from "react";

export interface ThemeOption {
  id: string;
  name: string;
  isDark: boolean;
  colors: { bg: string; accent: string };
}

export const themes: ThemeOption[] = [
  { id: "dark", name: "Dark", isDark: true, colors: { bg: "#0a0e1a", accent: "#253347" } },
  { id: "light", name: "Light", isDark: false, colors: { bg: "#ffffff", accent: "#e9e9ef" } },
  { id: "nord", name: "Nord", isDark: true, colors: { bg: "#2e3440", accent: "#3b4252" } },
  { id: "dracula", name: "Dracula", isDark: true, colors: { bg: "#282a36", accent: "#bd93f9" } },
  { id: "monokai", name: "Monokai", isDark: true, colors: { bg: "#272822", accent: "#a6e22e" } },
  { id: "solarized-light", name: "Solarized", isDark: false, colors: { bg: "#fdf6e3", accent: "#268bd2" } },
];

const DARK_THEMES = new Set(["dark", "nord", "dracula", "monokai"]);

function applyTheme(id: string) {
  const el = document.documentElement;
  // Clear all theme classes
  el.classList.remove("dark", "theme-nord", "theme-dracula", "theme-monokai", "theme-solarized-light");
  // Apply new
  if (DARK_THEMES.has(id)) el.classList.add("dark");
  if (id !== "dark" && id !== "light") el.classList.add(`theme-${id}`);
}

export function useTheme() {
  const [theme, setThemeState] = useState<string>(() => {
    return localStorage.getItem("theme") || "dark";
  });

  // Apply theme on mount to ensure classes are correct
  useEffect(() => {
    applyTheme(theme);
  }, []);

  const setTheme = useCallback((id: string) => {
    applyTheme(id);
    localStorage.setItem("theme", id);
    setThemeState(id);
  }, []);

  return { theme, setTheme, themes };
}
