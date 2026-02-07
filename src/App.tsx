import { Routes, Route, Navigate } from "react-router-dom";
import { AppLayout } from "./components/layout/AppLayout";
import { ProjectsPage } from "./components/project/ProjectsPage";
import { SessionsPage } from "./components/session/SessionsPage";
import { MessagesPage } from "./components/message/MessagesPage";
import { SearchPage } from "./components/search/SearchPage";
import { StatsPage } from "./components/stats/StatsPage";

function App() {
  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route path="/" element={<Navigate to="/claude/projects" replace />} />
        <Route path="/:tool/projects" element={<ProjectsPage />} />
        <Route path="/:tool/projects/:projectKey" element={<SessionsPage />} />
        <Route
          path="/:tool/projects/:projectKey/session/:sessionKey"
          element={<MessagesPage />}
        />
        <Route path="/:tool/search" element={<SearchPage />} />
        <Route path="/:tool/stats" element={<StatsPage />} />
      </Route>
    </Routes>
  );
}

export default App;
