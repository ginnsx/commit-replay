# 测试说明

copy-diff 采用分层测试：**解析器单元测试（必跑）**、**集成测试（可选）**、**前端组件测试（必跑）**。

业务级黑盒验收测试集见 [`docs/black_box_test_suite.md`](docs/black_box_test_suite.md)，用于验证跨仓库迁移不会漏文件、多文件、误写路径或错误处理冲突。自动回归方案见 [`docs/regression_automation_plan.md`](docs/regression_automation_plan.md)。

## 快速命令

```bash
# 默认：Rust 单元测试 + 前端 Vitest（CI / 日常提交前）
npm test
cd src-tauri && cargo test

# 前端监听模式
npm run test:watch

# Git -> Git 黑盒自动回归（临时仓库 + 文件哈希/提交数校验）
npm run test:regression:smoke
npm run test:regression:safety
npm run test:regression:release

# 可选：对真实 SVN 仓库做集成测试（需网络与凭据）
npm run test:integration
```

---

## 1. Rust 单元测试（fixtures，无需 SVN）

**位置**

| 模块 | 测试内容 | Fixtures |
|------|----------|----------|
| `vcs/svn/log_parser.rs` | 解析 `svn log --xml` | `src-tauri/tests/fixtures/svn/log_sample.xml` |
| `vcs/svn/diff_parser.rs` | 解析 `svn diff -c` | `diff_modify.txt`, `diff_add_delete.txt`, `diff_binary.txt` |
| `vcs/svn/ref_util.rs` | `svn:12345` 引用格式 | 无 |
| `mapper.rs` | 路径映射 | 无 |
| `preview/patch_apply.rs` | 内存应用 unified diff | 无 |
| `preview/service.rs` | 多 unit 按 target_path 聚合 | 无 |

**运行**

```bash
cd src-tauri
cargo test
cargo test log_parser   # 单模块
cargo test diff_parser
```

**新增 fixture 时**

1. 将样例输出保存到 `src-tauri/tests/fixtures/svn/<name>.xml|.txt`
2. 在对应 `#[cfg(test)] mod tests` 中用 `CARGO_MANIFEST_DIR` 读取
3. 断言字段与边界情况（空 diff、二进制、非法 XML）

---

## 2. Rust 集成测试（真实 SVN，默认忽略）

**位置**：`src-tauri/tests/svn_integration.rs`

**环境变量**

| 变量 | 说明 |
|------|------|
| `COPY_DIFF_SVN_URL` | SVN 仓库 URL（必填） |
| `COPY_DIFF_SVN_USER` | 用户名（可选） |
| `COPY_DIFF_SVN_PASS` | 密码（可选） |

**运行**

```powershell
$env:COPY_DIFF_SVN_URL = "https://your-svn-server/repo/trunk"
$env:COPY_DIFF_SVN_USER = "user"
$env:COPY_DIFF_SVN_PASS = "pass"
npm run test:integration
```

等价于：

```bash
cd src-tauri && cargo test --test svn_integration -- --ignored
```

未设置 `COPY_DIFF_SVN_URL` 时测试会 `expect` 失败；日常 `cargo test` 不会执行这些用例（`#[ignore]`）。

**覆盖场景**

- `list_recent_from_live_repo` — `svn log --xml -l 5`
- `load_changeset_from_live_repo` — 对最新一条 revision 执行 `svn diff -c` + 解析

---

## 3. 前端测试（Vitest + Testing Library）

**位置**

- `src/lib/types.test.ts` — 类型与模型约定
- `src/pages/CommitList.test.tsx` — 提交列表 UI
- `src/pages/Preview.test.tsx` — 预览页（mock `build_preview` + Monaco）

**策略**

- 使用 `vi.mock("@tauri-apps/api/core")` 模拟 `invoke`，不启动 Tauri
- 每个用例前重置 `useConnectionStore` / `useSelectionStore` 状态

**运行**

```bash
npm test
npm run test:watch
```

**CommitList 用例说明**

| 用例 | 验证点 |
|------|--------|
| URL 为空 | 不调用 `list_commits`，显示校验文案 |
| 拉取成功 | 表格展示 revision / 作者 / 说明 |
| 勾选行 | 已选计数变化 |
| invoke 失败 | 错误信息展示 |

**Preview 用例说明**

| 用例 | 验证点 |
|------|--------|
| 未填工作区路径 | 不调用 `build_preview` |
| 生成成功 | stats、Monaco diff、`build_preview` 参数含 mappings |

---

## 4. 手动端到端验证（步骤二 + 三）

### 提交列表（步骤二）

在已安装 `svn` 且可访问目标仓库的机器上：

```bash
npm run tauri dev
```

1. 打开「提交列表」，填写 SVN URL、凭据、条数  
2. 点击「拉取提交」，应出现 revision 列表  
3. 勾选若干条，点击「进入预览」

### 变更预览（步骤三）

1. 在预览页填写**目标 Git 工作区**本地路径（已 `git clone` 的仓库）  
2. 配置路径映射（默认 `/trunk` → `.`，按实际仓库调整）  
3. 点击「生成预览」：左侧文件树，右侧 Monaco before/after diff  
4. 橙色文件名表示 `git apply --check` 可能冲突  

---

## 5. 提交前检查清单

```bash
npm test && npm run lint && npm run format:check
cd src-tauri && cargo test && cargo clippy
```

有 SVN 测试环境时额外执行：`npm run test:integration`
