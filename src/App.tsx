import { useEffect, useState } from "react";
import { ping } from "./lib/invoke";

function App() {
  const [bridgeStatus, setBridgeStatus] = useState<"checking" | "ok" | "error">("checking");

  useEffect(() => {
    ping()
      .then(() => setBridgeStatus("ok"))
      .catch(() => setBridgeStatus("error"));
  }, []);

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-4 bg-gray-950 text-gray-100">
      <h1 className="text-3xl font-bold tracking-tight">copy-diff</h1>
      <p className="text-gray-400 text-sm">
        Rust bridge:{" "}
        {bridgeStatus === "checking" && <span className="text-yellow-400">checking…</span>}
        {bridgeStatus === "ok" && <span className="text-green-400">connected</span>}
        {bridgeStatus === "error" && <span className="text-red-400">error</span>}
      </p>
    </main>
  );
}

export default App;
