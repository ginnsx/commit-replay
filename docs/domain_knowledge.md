# Relay 跨仓库迁移 — 领域知识

> 从实现过程与用户反馈中提炼。每条规则对应真实 bug 或设计决策，后续改动应优先对照此处。

来源对话（agent transcripts）：

| 主题 | 对话 |
|------|------|
| SVN/Git 提交列表、在线 vs 本地 | [1b3f9f79-3124-48ea-a05b-76d8b95d75f3](1b3f9f79-3124-48ea-a05b-76d8b95d75f3) |
| 预览合并、跨版本、行号漂移 | [2d1135d0-d89d-4782-9793-d6e9348ccd9d](2d1135d0-d89d-4782-9793-d6e9348ccd9d) |
| 迁移 apply 顺序、blocked、source_after | [6915fb4c-c61c-4218-b5fb-721c989668f4](6915fb4c-c61c-4218-b5fb-721c989668f4) |
| patch apply / flickzeug 启发 | [f7da6ad4-8f8f-4807-8ec6-96bf8225ca38](f7da6ad4-8f8f-4807-8ec6-96bf8225ca38) |

---

## 1. 提交顺序

### 规则

**多 commit 迁移与预览合并，必须按时间正序（旧 → 新）处理；UI 列表默认新 → 旧，不能直接当作 apply 顺序。**

### 原因

- 前端 commit 列表按日期降序展示；`source_refs` 传入后端时若保持该顺序，migrate 逐条 commit 会在目标仓库产生**倒序**提交。
- 预览侧若按反序做 `merge_patches_last_wins`，较旧 patch 会覆盖较新 patch（例：50545 的 `rows="5"` 覆盖 50556 去掉 `style` 的改动）。
- patch 叠加语义依赖 baseline 链：后一次 apply 的输入是前一次的结果，顺序错了合并结果错误。

### 正确做法

- 后端对 `units` 按 revision / 日期**正序**排序后再 preview / migrate。
- 前端 `sourceRefs` 按 `date` 正序传给后端。
- 同路径多 commit 合并：按正序依次 apply patch，上一步 `after` 作为下一步 `before`。

### 相关代码

- `sort_units_chronological` / `chronological_source_refs`
- `migrate.rs` apply 循环

---

## 2. 获取提交记录：在线 vs 本地

### 规则

**提交列表应对齐服务器 HEAD，不能因本地工作副本落后而漏提交；读 diff/内容可走服务器，不必强制本地 update。**

### SVN

| 场景 | 行为 | 风险 |
|------|------|------|
| 对 **WC 路径** 跑 `svn log`（无 `-r`） | 默认范围 **BASE:1**，只到上次 update 的版本 | 看不到 HEAD 之上提交（TortoiseSVN 标 "not in work copy" 的那些） |
| 对 **仓库 URL** 跑 `svn log -r HEAD:1` | 列出服务器全部可达提交 | 正确的产品行为 |

**已采用方案**：`SvnReader` 通过 `svn info` 取 URL 拉 log；`svn diff -c` / `svn cat -r` 仍用 WC 路径定位仓库，数据来自服务器。

**选超出本地 WC 的 revision 仍可迁移**：合并基准是**目标** WC 当前内容，与源 WC 是否在 HEAD 无关；源 WC 全程只读，不需要先 `svn update`。

### Git

| 场景 | 行为 | 风险 |
|------|------|------|
| `git log` 未指定 `origin/<branch>` | 只列本地 HEAD 可达提交 | 未 fetch / 本地分支落后时缺最新提交 |
| `git show <sha>` | 必须本地已有该 commit | 选中后可能直接失败 |

**与 SVN 的差异**：没有 BASE 截断，但有「本地对象库不完整」问题。应对：`git fetch`；可选改为查 `origin/<branch>`（与 SVN 查 URL 同思路）。

### 设计原则

- 列表：看**在线/服务器**历史，不因 WC 旧而隐藏。
- 应用：读变更走 VCS 命令（服务器/对象库），不修改源 WC。
- 不自动 `svn update` / `git pull`：避免覆盖用户本地未提交改动。

---

## 3. 预览必须与迁移语义一致

### 规则

**第 4 步预览 = 所选 commit 按正序 apply 到目标 WC 后的净变更；不是「某一条 commit 的单次 diff」，也不是「源侧 patch 字符串合并」。**

### 错误模式

- 同路径多 commit：后者覆盖前者 → 只看到最后一次局部改动。
- 按 `source_ref` 字符串排序决定「谁赢」→ 不等于时间顺序。
- 用源 patch merge 展示 → 不是「目标当前 → 合并后」的净差异。

### 正确做法

- 对同 `target_path` 在内存中串行 fold：`before` = 目标 WC → 依次 derive_after → 最终 `after`。
- diff = `lines_to_diff(目标 WC before, 合并后 after)`，只显示有 `+`/`-` 的行。
- 合并后行数增减均为 0 的文件不展示（无净变更）。
- diff 随列表一次返回，避免点击文件再请求。

---

## 4. `source_after` 不能当写入内容

### 规则

**SVN `source_after`（`svn cat -r N`）是该 revision 的完整文件快照，包含该 revision 之前全部历史，不仅是本次选中的 commit。跨版本 / blocked 文件禁止直接 WriteAfter 写入 `source_after`。**

### 真实 bug

- blocked 文件 merge 失败时回退 `source_after` → diff 出现**未选中 commit** 的代码。
- finalize 用 `source_after` 写入 → 目标文件混入无关历史变更。

### 正确做法

- blocked / 跨版本：在**目标 baseline** 上用 `apply_patches_sequential` 只应用**本次选中 commit 的 patch**。
- 展示 diff：用所选 commit 的合并 patch（`merge_patches_for_display`），不用整文件 snapshot 做对比。
- 需要完整内容时从 reader 取 `source_before` / 按 commit 范围取，不用 patch 反推（`reconstruct_old/new_from_patch` 不可靠）。

---

## 5. 跨版本与 Blocked（ManualMerge）

### 规则

**源 patch 的 baseline 与目标 WC 不一致时（行号漂移、内容对不上），必须标为需人工确认，禁止 AutoOk 盲 apply。**

### 识别条件

- hunk `@@` 行号与目标文件实际行不一致，且 context 逐字匹配失败。
- `apply_unified_patch` 失败 → `merge_context_ok = false` → `conflict_risk: High`。
- `patch` 为空或 `conflict_risk: High` → 分类为 Review / ManualMerge，不能 AutoOk。

### 迁移写入

- `ManualMerge` 在 apply 阶段**跳过写入**；用户「标记已解决」后须升级为 `WriteAfter`（`resolved_blocked`）。
- squash：`WriteAfter` 在 unit 循环中 Skip，finalize 阶段统一写入 aggregated 内容。
- 非 squash：`resolved_blocked` 文件在全部 unit 之后做 aggregated finalize 并提交。
- 仅含 blocked 文件的 commit：apply 全 Skip 后工作区无变更 → 须 `git commit --allow-empty` 或 finalize 保证文件落盘。

### 典型场景

- 目标文件在 271 行，patch hunk 指向 272 行 → 旧逻辑强行 apply 产生语法错误。
- `pickingApplyInfoDetail.html` 类跨版本文件：必须手动确认，不能自动合并。

---

## 6. 目标仓库前置条件

### 规则

**迁移前只要求目标工作区干净（`git status` / SVN equivalent 无未提交变更），不要求与 `origin` 同步。**

### 易混淆报错

`Your branch is ahead of 'origin/master' by N commits` 常伴随 `nothing to commit, working tree clean` —— 前者只是状态提示，真正失败原因是 **apply 后内容与 HEAD 相同，无内容可提交**。与 origin 领先无关，不必 reset 无关 commit。

### 空 commit 处理

- apply 后工作区相对 HEAD 无 diff → 跳过 commit 或 `--allow-empty`（保留 commit 数量与源一致时）。
- squash：blocked 的 `WriteAfter` 延后 finalize，避免 unit 阶段空 commit。

---

## 7. Patch apply 实现约束

### 规则

| 约束 | 说明 |
|------|------|
| 行号是 hint，不是硬约束 | 多 commit 叠加后行号漂移；应 context 搜索定位，不能盲信 `@@` 行号 |
| preview 与 write 同一套 apply 语义 | 否则「预览对、写入挂」 |
| 不用 `content_equal(disk, prev)` 猜 apply 成功 | 应用结果应显式判定（如 ApplyStats） |
| fuzzy apply 有风险 | HTML/JS 大量相似行，误匹配可能引入新 bug；跨 baseline 优先 ManualMerge + 3-way |
| 顺序叠 patch ≠ 3-way merge | 每个 patch baseline 是源侧上一版，不是当前 target disk；跨 baseline 应用语义错误 |

### 可选演进（f7da6ad4）

- 算法层可引入 flickzeug（context 搜索、fuzz、3-way merge）。
- 业务层（多 commit replay、路径映射、integration 分类）必须保留自研。
- 只换 apply 不换 `source_after` 策略 / ManualMerge 流程 → 易修一错一。

---

## 8. Squash 模式

### 规则

- 勾选「合并为单次提交」：各 unit 的 `WriteAfter` 在循环中 Skip，最后 finalize 一次性写入并单次 commit。
- commit message：用户自定义（非自动生成摘要）。
- finalize 须写入全部 aggregated 路径；`content_equal(after, before)` 过滤会导致 blocked 文件丢失。

---

## 9. 集成策略速查

| 策略 | apply 阶段 | 说明 |
|------|------------|------|
| ApplyPatch | 打 patch 到目标当前文件 | patch 须能在目标 baseline 上应用 |
| WriteAfter | 写 `after` 全文 | squash 下延后到 finalize |
| ManualMerge | 跳过 | 用户解决后 → WriteAfter |
| Skip | 跳过 | 目标已含期望内容 |

`resolved_blocked`：`ManualMerge` + 已标记解决 → `WriteAfter`。

---

## 10. 检查清单（改 preview / migrate 前）

1. `units` / `source_refs` 是否**旧 → 新**？
2. 提交列表是否来自**服务器 HEAD**（SVN URL / Git origin）？
3. 预览 diff 是否为**目标 WC → 累积合并后**的净差异？
4. 是否误用 **`source_after` 整文件** 写入或展示？
5. patch 失败是否标 **ManualMerge**，而非 AutoOk 或 silent fallback？
6. **`resolved_blocked`** 是否在 migrate 中升级为 WriteAfter + finalize？
7. squash / 非 squash 下 **WriteAfter 时机**是否正确？
8. preview 与 write 的 **apply 路径**是否一致？
