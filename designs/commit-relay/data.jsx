const INITIAL_REPOS = [
  {
    id: "r1",
    name: "payment-service",
    path: "D:\\Projects\\payment-service",
    type: "git",
    branch: "main",
    lastUsed: "2 小时前",
  },
  {
    id: "r2",
    name: "legacy-billing",
    path: "D:\\SVN\\legacy-billing",
    type: "svn",
    branch: "trunk",
    lastUsed: "昨天",
    svnUser: "zhangwei",
  },
  {
    id: "r3",
    name: "mobile-gateway",
    path: "D:\\Projects\\mobile-gateway",
    type: "git",
    branch: "develop",
    lastUsed: "3 天前",
  },
  {
    id: "r4",
    name: "config-center",
    path: "\\\\fileserver\\svn\\config-center",
    type: "svn",
    branch: "branches/release-2.4",
    lastUsed: "1 周前",
    svnUser: "zhangwei",
  },
];

const COMMITS = [
  {
    id: "c1",
    hash: "a3f8c21",
    msg: "fix: 修复支付回调超时重试逻辑",
    author: "张伟",
    date: "2026-06-14 16:32",
    files: 3,
  },
  {
    id: "c2",
    hash: "7b2e9d0",
    msg: "feat: 新增支付宝沙箱环境配置项",
    author: "李娜",
    date: "2026-06-13 11:08",
    files: 5,
  },
  {
    id: "c3",
    hash: "e1c4f88",
    msg: "refactor: 抽取 PaymentClient 公共接口",
    author: "张伟",
    date: "2026-06-12 09:45",
    files: 8,
  },
  {
    id: "c4",
    hash: "9d0a1b2",
    msg: "chore: 升级 spring-boot 至 3.2.5",
    author: "王磊",
    date: "2026-06-10 14:20",
    files: 2,
  },
  {
    id: "c5",
    hash: "f5e6d7c",
    msg: "fix: 订单状态机并发竞态条件",
    author: "李娜",
    date: "2026-06-09 17:55",
    files: 4,
  },
];

const FILE_CHANGES = [
  {
    id: "f1",
    path: "src/main/java/com/pay/CallbackHandler.java",
    status: "mod",
    additions: 24,
    deletions: 8,
    diff: [
      { type: "ctx", old: 42, new: 42, text: "    public void onCallback(PaymentEvent event) {" },
      { type: "del", old: 43, new: null, text: "        retryOnce(event);" },
      { type: "add", old: null, new: 43, text: "        retryWithBackoff(event, MAX_RETRIES);" },
      { type: "ctx", old: 44, new: 44, text: "        metrics.record(event.getType());" },
      { type: "add", old: null, new: 45, text: "        log.info(\"callback processed: {}\", event.getId());" },
    ],
  },
  {
    id: "f2",
    path: "src/main/resources/application-sandbox.yml",
    status: "add",
    additions: 18,
    deletions: 0,
    diff: [
      { type: "add", old: null, new: 1, text: "alipay:" },
      { type: "add", old: null, new: 2, text: "  sandbox:" },
      { type: "add", old: null, new: 3, text: "    enabled: true" },
      { type: "add", old: null, new: 4, text: "    gateway: https://openapi.alipaydev.com/gateway.do" },
      { type: "add", old: null, new: 5, text: "    app-id: ${ALIPAY_SANDBOX_APP_ID}" },
    ],
  },
  {
    id: "f3",
    path: "src/main/java/com/pay/RetryPolicy.java",
    status: "add",
    additions: 42,
    deletions: 0,
    diff: [
      { type: "add", old: null, new: 1, text: "package com.pay;" },
      { type: "add", old: null, new: 2, text: "" },
      { type: "add", old: null, new: 3, text: "public class RetryPolicy {" },
      { type: "add", old: null, new: 4, text: "    private static final int MAX_RETRIES = 3;" },
      { type: "add", old: null, new: 5, text: "    // exponential backoff implementation" },
    ],
  },
  {
    id: "f4",
    path: "src/main/java/com/pay/LegacyCallback.java",
    status: "del",
    additions: 0,
    deletions: 67,
    diff: [
      { type: "del", old: 1, new: null, text: "package com.pay;" },
      { type: "del", old: 2, new: null, text: "" },
      { type: "del", old: 3, new: null, text: "@Deprecated" },
      { type: "del", old: 4, new: null, text: "public class LegacyCallback { ... }" },
    ],
  },
  {
    id: "f5",
    path: "pom.xml",
    status: "mod",
    additions: 2,
    deletions: 2,
    diff: [
      { type: "ctx", old: 18, new: 18, text: "    <properties>" },
      { type: "del", old: 19, new: null, text: "        <spring-boot.version>3.2.4</spring-boot.version>" },
      { type: "add", old: null, new: 19, text: "        <spring-boot.version>3.2.5</spring-boot.version>" },
      { type: "ctx", old: 20, new: 20, text: "    </properties>" },
    ],
  },
];

const CONFLICTS = [
  {
    id: "cf1",
    path: "src/main/java/com/pay/CallbackHandler.java",
    status: "mod",
    reason: "目标仓库中该文件已被修改，行 43–48 存在重叠变更",
    overlapLines: [43, 48],
    before: [
      "    public void onCallback(PaymentEvent event) {",
      "        validateSignature(event);",
      "        retryOnce(event);",
      "        notifyDownstream(event);",
      "        metrics.record(event.getType());",
      "    }",
    ],
    after: [
      "    public void onCallback(PaymentEvent event) {",
      "        validateSignature(event);",
      "        retryWithBackoff(event, MAX_RETRIES);",
      "        notifyDownstream(event);",
      "        metrics.record(event.getType());",
      "        log.info(\"callback processed: {}\", event.getId());",
      "    }",
    ],
  },
  {
    id: "cf2",
    path: "src/main/resources/application-sandbox.yml",
    status: "add",
    reason: "目标仓库已存在同名文件，内容不一致",
    overlapLines: [1, 5],
    before: [
      "alipay:",
      "  sandbox:",
      "    enabled: false",
      "    gateway: https://openapi.alipay.com/gateway.do",
      "    app-id: ${ALIPAY_PROD_APP_ID}",
    ],
    after: [
      "alipay:",
      "  sandbox:",
      "    enabled: true",
      "    gateway: https://openapi.alipaydev.com/gateway.do",
      "    app-id: ${ALIPAY_SANDBOX_APP_ID}",
    ],
  },
];

function splitFilePath(path) {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (i < 0) return { dir: "", name: path };
  return { dir: path.slice(0, i + 1), name: path.slice(i + 1) };
}

function alignConflictLines(beforeLines, afterLines) {
  const max = Math.max(beforeLines.length, afterLines.length);
  const rows = [];
  for (let i = 0; i < max; i++) {
    const left = beforeLines[i] ?? null;
    const right = afterLines[i] ?? null;
    const kind = left === right ? "same" : left === null ? "add" : right === null ? "del" : "chg";
    rows.push({ left, right, kind, lineNo: i + 1 });
  }
  return rows;
}

const STEPS = [
  { id: "source", label: "源仓库", num: 1 },
  { id: "commits", label: "选择提交", num: 2 },
  { id: "preview", label: "变更预览", num: 3 },
  { id: "target", label: "目标仓库", num: 4 },
  { id: "migrate", label: "迁移", num: 5 },
];

const STATUS_LABELS = { add: "新增", mod: "修改", del: "删除" };

const EDITOR_PRESETS = [
  { id: "vscode", name: "VS Code", kind: "vscode", exe: "C:\\Users\\zhangwei\\AppData\\Local\\Programs\\Microsoft VS Code\\Code.exe" },
  { id: "vs", name: "Visual Studio", kind: "visualstudio", exe: "C:\\Program Files\\Microsoft Visual Studio\\2022\\Community\\Common7\\IDE\\devenv.exe" },
  { id: "cursor", name: "Cursor", kind: "cursor", exe: "C:\\Users\\zhangwei\\AppData\\Local\\Programs\\cursor\\Cursor.exe" },
  { id: "explorer", name: "File Explorer", kind: "explorer", exe: "explorer.exe" },
  { id: "terminal", name: "Terminal", kind: "terminal", exe: "wt.exe" },
  { id: "gitbash", name: "Git Bash", kind: "gitbash", exe: "C:\\Program Files\\Git\\git-bash.exe" },
  { id: "idea", name: "IntelliJ IDEA", kind: "idea", exe: "C:\\Program Files\\JetBrains\\IntelliJ IDEA 2024.1\\bin\\idea64.exe" },
  { id: "pycharm", name: "PyCharm", kind: "pycharm", exe: "C:\\Program Files\\JetBrains\\PyCharm 2024.1\\bin\\pycharm64.exe" },
];

Object.assign(window, {
  INITIAL_REPOS, COMMITS, FILE_CHANGES, CONFLICTS, STEPS, STATUS_LABELS, alignConflictLines,
  EDITOR_PRESETS, splitFilePath,
});
