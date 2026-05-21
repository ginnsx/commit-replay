import { useEffect, useState } from "react";
import { BrowserRouter, Link, Route, Routes } from "react-router-dom";
import { ping } from "./lib/invoke";
import CommitList from "./pages/CommitList";

function AppShell() {
  const [bridgeStatus, setBridgeStatus] = useState<"checking" | "ok" | "error">("checking");

  useEffect(() => {
    ping()
      .then(() => setBridgeStatus("ok"))
      .catch(() => setBridgeStatus("error"));
  }, []);

  return (
    <div className="min-h-screen bg-gray-950 text-gray-100">
      <nav className="flex items-center gap-4 border-b border-gray-800 px-6 py-3">
        <span className="font-semibold text-gray-100">copy-diff</span>
        <Link to="/" className="text-sm text-gray-400 hover:text-gray-200">
          提交列表
        </Link>
        <span className="ml-auto text-xs text-gray-500">
          bridge: {bridgeStatus === "checking" && <span className="text-yellow-400">…</span>}
          {bridgeStatus === "ok" && <span className="text-green-400">ok</span>}
          {bridgeStatus === "error" && <span className="text-red-400">error</span>}
        </span>
      </nav>
      <Routes>
        <Route path="/" element={<CommitList />} />
      </Routes>
    </div>
  );
}

export default function App() {
  return (
    <BrowserRouter>
      <AppShell />
    </BrowserRouter>
  );
}
