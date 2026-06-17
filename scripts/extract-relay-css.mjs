import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const html = fs.readFileSync(
  path.join(root, "designs/commit-relay/index.html"),
  "utf8",
);
const m = html.match(/<style>([\s\S]*)<\/style>/);
if (!m) throw new Error("style block not found");
const outDir = path.join(root, "src/styles");
fs.mkdirSync(outDir, { recursive: true });
fs.writeFileSync(path.join(outDir, "relay.css"), m[1]);
