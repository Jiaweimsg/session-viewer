import { create } from "zustand";
import type { ToolType, DisplayMessage } from "../types";
import * as api from "../services/tauriApi";

interface AppState {
  // Tool selection
  activeTool: ToolType;

  // Projects
  projects: any[];
  projectsLoading: boolean;
  selectedProject: string | null;

  // Sessions
  sessions: any[];
  sessionsLoading: boolean;
  selectedSession: string | null;

  // Messages
  messages: DisplayMessage[];
  messagesLoading: boolean;
  messagesTotal: number;
  messagesPage: number;
  messagesHasMore: boolean;

  // Search
  searchQuery: string;
  searchResults: any[];
  searchLoading: boolean;

  // Stats
  stats: any;
  tokenSummary: any;
  statsLoading: boolean;

  // Actions
  setActiveTool: (tool: ToolType) => void;
  loadProjects: () => Promise<void>;
  selectProject: (projectKey: string) => Promise<void>;
  selectSession: (sessionKey: string, projectKey?: string) => Promise<void>;
  loadMoreMessages: () => Promise<void>;
  loadAllMessages: () => Promise<void>;
  search: (query: string) => Promise<void>;
  loadStats: () => Promise<void>;
  clearSelection: () => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  activeTool: "claude",

  projects: [],
  projectsLoading: false,
  selectedProject: null,

  sessions: [],
  sessionsLoading: false,
  selectedSession: null,

  messages: [],
  messagesLoading: false,
  messagesTotal: 0,
  messagesPage: 0,
  messagesHasMore: false,

  searchQuery: "",
  searchResults: [],
  searchLoading: false,

  stats: null,
  tokenSummary: null,
  statsLoading: false,

  setActiveTool: (tool: ToolType) => {
    set({
      activeTool: tool,
      projects: [],
      projectsLoading: false,
      selectedProject: null,
      sessions: [],
      sessionsLoading: false,
      selectedSession: null,
      messages: [],
      messagesLoading: false,
      messagesTotal: 0,
      messagesPage: 0,
      messagesHasMore: false,
      searchQuery: "",
      searchResults: [],
      searchLoading: false,
      stats: null,
      tokenSummary: null,
      statsLoading: false,
    });
  },

  loadProjects: async () => {
    const tool = get().activeTool;
    set({ projectsLoading: true });
    try {
      const projects = await api.getProjects(tool);
      set({ projects, projectsLoading: false });
    } catch (e) {
      console.error("Failed to load projects:", e);
      set({ projectsLoading: false });
    }
  },

  selectProject: async (projectKey: string) => {
    const tool = get().activeTool;
    set({
      selectedProject: projectKey,
      sessionsLoading: true,
      selectedSession: null,
      messages: [],
      messagesTotal: 0,
      messagesPage: 0,
    });
    try {
      const sessions = await api.getSessions(tool, projectKey);
      set({ sessions, sessionsLoading: false });
    } catch (e) {
      console.error("Failed to load sessions:", e);
      set({ sessionsLoading: false });
    }
  },

  selectSession: async (sessionKey: string, projectKey?: string) => {
    const tool = get().activeTool;
    const pKey = projectKey || get().selectedProject;
    set({
      selectedSession: sessionKey,
      messagesLoading: true,
      messages: [],
      messagesTotal: 0,
      messagesPage: 0,
    });
    try {
      const result = await api.getMessages(
        tool,
        sessionKey,
        pKey ?? null,
        0,
        50
      );
      set({
        messages: result.messages,
        messagesTotal: result.total,
        messagesPage: 0,
        messagesHasMore: result.hasMore,
        messagesLoading: false,
      });
    } catch (e) {
      console.error("Failed to load messages:", e);
      set({ messagesLoading: false });
    }
  },

  loadMoreMessages: async () => {
    const state = get();
    if (
      !state.selectedSession ||
      !state.messagesHasMore ||
      state.messagesLoading
    ) {
      return;
    }

    const nextPage = state.messagesPage + 1;
    set({ messagesLoading: true });
    try {
      const result = await api.getMessages(
        state.activeTool,
        state.selectedSession,
        state.selectedProject,
        nextPage,
        50
      );
      set({
        messages: [...state.messages, ...result.messages],
        messagesPage: nextPage,
        messagesHasMore: result.hasMore,
        messagesLoading: false,
      });
    } catch (e) {
      console.error("Failed to load more messages:", e);
      set({ messagesLoading: false });
    }
  },

  loadAllMessages: async () => {
    const state = get();
    if (!state.selectedSession || !state.messagesHasMore || state.messagesLoading) return;

    set({ messagesLoading: true });
    let currentPage = state.messagesPage;
    let allMessages = [...state.messages];
    let hasMore = true;

    while (hasMore) {
      currentPage += 1;
      try {
        const result = await api.getMessages(
          state.activeTool,
          state.selectedSession,
          state.selectedProject,
          currentPage,
          50
        );
        allMessages = [...allMessages, ...result.messages];
        hasMore = result.hasMore;
      } catch (e) {
        console.error("Failed to load all messages:", e);
        hasMore = false;
      }
    }

    set({
      messages: allMessages,
      messagesPage: currentPage,
      messagesHasMore: false,
      messagesLoading: false,
    });
  },

  search: async (query: string) => {
    const tool = get().activeTool;
    set({ searchQuery: query, searchLoading: true });
    if (!query.trim()) {
      set({ searchResults: [], searchLoading: false });
      return;
    }
    try {
      const results = await api.globalSearch(tool, query, 50);
      set({ searchResults: results, searchLoading: false });
    } catch (e) {
      console.error("Failed to search:", e);
      set({ searchLoading: false });
    }
  },

  loadStats: async () => {
    const tool = get().activeTool;
    set({ statsLoading: true });
    try {
      const [stats, tokenSummary] = await Promise.all([
        api.getStats(tool),
        api.getTokenSummary(tool),
      ]);
      set({ stats, tokenSummary, statsLoading: false });
    } catch (e) {
      console.error("Failed to load stats:", e);
      set({ statsLoading: false });
    }
  },

  clearSelection: () => {
    set({
      selectedProject: null,
      selectedSession: null,
      sessions: [],
      messages: [],
    });
  },
}));
