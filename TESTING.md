# 测试说明

copy-diff 的测试分三层：

1. **单元测试**：Rust 核心逻辑和 TypeScript 类型约定，提交前必跑。
2. **自动回归**：用临时 Git/SVN 仓库做黑盒验收，验证迁移结果、提交数和回滚。
3. **人工验收**：覆盖桌面 UI、配置管理、人工冲突处理和迁移历史等尚未完全自动化的流程。

自动回归方案的设计细节见 [`docs/regression_automation_plan.md`](docs/regression_automation_plan.md)。验收测试标准已集中在本文的[验收测试](#验收测试)章节。

---

## 快速命令

日常提交前：

```bash
npm test
npm run lint
npm run format:check
cd src-tauri && cargo test
cd src-tauri && cargo clippy
```

按需运行：

```bash
# 前端测试
npm test
npm run test:watch
npm run test:coverage

# Rust 单元测试
cd src-tauri && cargo test
cd src-tauri && cargo test mapper

# 可选：真实 SVN 工作副本集成测试
npm run test:integration

# 黑盒自动回归
npm run test:regression:smoke
npm run test:regression:safety
npm run test:regression:release
npm run test:regression:ui
npm run test:regression:all
```

`test:regression:release` 使用 `svnadmin` 和 `svn checkout file://...` 创建本地 SVN 仓库，不访问网络。缺少 `svn` 或 `svnadmin` 时 SVN case 会记录为 skipped；正式 release 环境应安装 SVN 工具并要求 SVN case 零 skipped。

---

## 单元测试

### Rust

位置：

| 模块 | 覆盖点 |
|------|--------|
| `src-tauri/src/error.rs` | `AppError` 序列化 |
| `src-tauri/src/model.rs` | 核心模型约定 |
| `src-tauri/src/mapper.rs` | 路径映射、最长前缀、无匹配路径 |
| `src-tauri/src/diff/line_diff.rs` | 行级 diff |
| `src-tauri/src/preview/` | patch apply、集成计划、预览聚合、目标工作区检查 |
| `src-tauri/src/store/` | 密码加密、编辑器路径 |
| `src-tauri/src/vcs/git/` | Git log/ref 解析 |
| `src-tauri/src/vcs/svn/` | SVN CLI、diff/log/info/ref 解析 |
| `src-tauri/src/commands/reader.rs` | reader 入参和分页约定 |

Fixtures：

| 文件 | 用途 |
|------|------|
| `src-tauri/tests/fixtures/svn/log_sample.xml` | `svn log --xml` 解析 |
| `src-tauri/tests/fixtures/svn/diff_modify.txt` | SVN 修改 diff |
| `src-tauri/tests/fixtures/svn/diff_add_delete.txt` | SVN 新增/删除 diff |
| `src-tauri/tests/fixtures/svn/diff_binary.txt` | SVN 二进制 diff |

运行：

```bash
cd src-tauri
cargo test
cargo test log_parser
cargo test diff_parser
```

新增 fixture 时：

1. 将样例输出保存到 `src-tauri/tests/fixtures/svn/<name>.xml|.txt`。
2. 在对应 `#[cfg(test)] mod tests` 中用 `CARGO_MANIFEST_DIR` 读取。
3. 断言字段和边界情况，例如空 diff、二进制、非法 XML。

### TypeScript

位置：

| 文件 | 覆盖点 |
|------|--------|
| `src/lib/types.test.ts` | 前端类型与 Rust 共享模型的字段约定 |

当前没有 `src/pages/*.test.tsx` 组件测试；如果后续补 UI 组件测试，应放在对应组件附近或 `src/**/*.test.tsx`，并通过 `npm test` 运行。

运行：

```bash
npm test
npm run test:watch
npm run test:coverage
```

---

## SVN 集成测试

位置：`src-tauri/tests/svn_integration.rs`

这组测试默认 `#[ignore]`，用于验证真实 SVN 工作副本或 URL 的读取能力。

环境变量：

| 变量 | 说明 |
|------|------|
| `COPY_DIFF_SVN_WC_PATH` | SVN 工作副本路径，优先使用 |
| `COPY_DIFF_SVN_URL` | SVN 仓库 URL；未设置工作副本路径时作为 fallback |
| `COPY_DIFF_SVN_USER` | 用户名，可选 |
| `COPY_DIFF_SVN_PASS` | 密码，可选 |

运行：

```powershell
$env:COPY_DIFF_SVN_WC_PATH = "C:\path\to\svn\wc"
$env:COPY_DIFF_SVN_USER = "user"
$env:COPY_DIFF_SVN_PASS = "pass"
npm run test:integration
```

等价于：

```bash
cd src-tauri && cargo test --test svn_integration -- --ignored
```

覆盖场景：

| 用例 | 覆盖点 |
|------|--------|
| `list_recent_from_live_repo` | `svn log --xml -l 5` |
| `load_changeset_from_live_repo` | 对最新 revision 执行 `svn diff -c` 并解析为 `ChangeSet` |

---

## 自动回归

入口脚本：`scripts/regression/run-regression.ps1`

实际 runner：`src-tauri/src/bin/regression_runner.rs`

产物目录：

```text
artifacts/regression/<run_id>/
  summary.json
  summary.md
  cases/<case_id>/
    case.json
    result.json 或 error.json
    refs.json
    before/manifest.json
    after/manifest.json
    after/target.diff
```

套件：

| 命令 | 覆盖 |
|------|------|
| `npm run test:regression:smoke` | Git -> Git 的 `I01-GG`、`I03-GG`、`I04-GG` |
| `npm run test:regression:safety` | `smoke` + Git -> Git 的生产安全、复杂文件、路径映射、迁移模式、squash、历史记录用例 |
| `npm run test:regression:release` | `safety` + 本地 SVN 的 SVN -> Git、Git -> SVN、SVN -> SVN 核心矩阵 |
| `npm run test:regression:ui` | 当前记录为 skipped，等待 tauri-driver/WebDriver 接入 |
| `npm run test:regression:all` | `release` + `ui-smoke` |

当前自动化用例：

| 套件 | 用例 |
|------|------|
| `smoke` | `I01-GG`, `I03-GG`, `I04-GG` |
| `production-safety` | `I01-GG` - `I08-GG`, `D01-GG`, `D03-GG`, `D04-GG-rename`, `D05-GG-binary`, `D06-GG-crlf`, `D07-GG-empty`, `D08-GG-no-final-newline`, `D09-GG-chinese`, `D10-GG-longest-prefix`, Windows 下另含 `D11-GG-case-only-rename`, `E01-GG-partial-unmapped`, `E02-GG-directory-conflict`, `M01-GG-commit-result`, `M02-GG-strict-replay`, `M03-GG-squash`, `M04-GG-squash-empty-message`, `F10-GG-already-expected`, `F11-GG-context-drift-auto-merge`, `F12-GG-already-contained-skip`, `F13-GG-same-region-conflict`, `F14-GG-multiple-candidates-blocked` |
| `release` | `production-safety` + `SG01-SVN-Git-add`, `SG02-SVN-Git-modify`, `SG03-SVN-Git-move`, `SG04-SVN-Git-multi`, `GS01-Git-SVN-add`, `GS02-Git-SVN-delete`, `GS03-Git-SVN-dirty`, `SS01-SVN-SVN-add`, `SS02-SVN-SVN-delete` |

失败时优先查看：

```text
artifacts/regression/<run_id>/summary.md
artifacts/regression/<run_id>/cases/<case_id>/error.json
artifacts/regression/<run_id>/cases/<case_id>/after/target.diff
artifacts/regression/<run_id>/cases/<case_id>/before/manifest.json
artifacts/regression/<run_id>/cases/<case_id>/after/manifest.json
```

---

## 验收测试

验收测试只通过桌面应用可见能力、Git/SVN 命令行结果和目标仓库最终状态判断通过与否，不依赖 Rust/TypeScript 内部函数。自动回归可以调用测试 runner，但验收口径仍必须落在外部结果：目标文件集合、内容哈希、提交记录、工作区状态和迁移记录。

### 目标

核心目标是验证提交迁移不会出现：

- 漏迁文件、漏迁行、漏提交。
- 多迁文件、多改行、错误路径写入。
- 冲突未拦截、错误自动合并、未确认即执行。
- 失败后目标工作区残留脏状态。
- 迁移记录与实际结果不一致。

### 通用验收口径

每个迁移场景都必须验证以下结果：

| 检查项 | 通过标准 |
|---|---|
| 目标工作区状态 | 执行完成后工作区干净；失败或取消后回到执行前状态 |
| 文件集合 | 目标实际变更文件集合等于预期集合，不多不少 |
| 文件内容 | 目标文件内容等于预期内容，文本和二进制都按 SHA256 校验 |
| 提交数量 | 非 squash 模式下目标提交数等于实际应用的源提交数；squash 模式下等于 1 |
| 提交信息 | 每个 Relay 生成的目标提交都包含 `使用 Relay v{version} 合并`；squash 提交保留用户填写的首行 message，并在正文列出原 commit |
| 集成计划 | review / blocked 项未处理完时不可执行迁移 |
| 迁移记录 | 源仓库、目标仓库、提交、文件数、状态与实际执行结果一致 |

推荐判定方式：

```powershell
# Git 目标：检查工作区干净
git -C <target> status --porcelain

# Git 目标：检查最近提交
git -C <target> log --oneline -n 10

# Git 目标：检查文件变更集合
git -C <target> diff --name-status <baseline> HEAD

# 文本和二进制内容校验
Get-FileHash <target-file> -Algorithm SHA256

# SVN 目标：检查工作区干净
svn status <target>

# SVN 目标：检查最近提交
svn log -l 10 <target>
```

### 测试数据集

准备四类本地仓库组合：

| 数据集 | 源 | 目标 | 用途 |
|---|---|---|
| `DS-GG` | Git | Git | 覆盖最稳定、最高频的端到端迁移 |
| `DS-SG` | SVN | Git | 覆盖 SVN revision 到 Git commit |
| `DS-GS` | Git | SVN | 覆盖 Git commit 到 SVN revision |
| `DS-SS` | SVN | SVN | 覆盖 SVN 到 SVN |

每个数据集都准备相同语义的源提交序列，目标仓库从可记录的 baseline 开始。源提交 ID 可以不同，但语义必须一致。

### 源提交序列

| 编号 | 提交内容 | 预期用途 |
|---|---|---|
| `C01-add-text` | 新增 `src/new_file.txt`，内容 3 行 | 新增文本文件 |
| `C02-modify-text` | 修改 `src/existing.txt` 的中间一行 | 普通补丁应用 |
| `C03-delete-text` | 删除 `src/delete_me.txt` | 删除文件 |
| `C04-rename-text` | `src/move_me.txt` 移动到 `moved/move_me.txt` | 移动/重命名 |
| `C05-binary-add` | 新增或修改 `assets/blob.bin` | 二进制文件处理 |
| `C06-crlf-add` | 新增或修改包含 CRLF 的文件 | 换行兼容 |
| `C07-same-file-step1` | 修改 `src/chain.txt` 第 2 行为 `step1` | 多提交同文件顺序 |
| `C08-same-file-step2` | 修改 `src/chain.txt` 第 2 行为 `step2` | 多提交同文件最终结果 |
| `C09-add-then-delete-a` | 新增 `tmp/transient.txt` | 净变更抵消 |
| `C10-add-then-delete-b` | 删除 `tmp/transient.txt` | 净变更应为空 |
| `C11-empty-add` | 新增空文件 | 空文件 |
| `C12-no-final-newline` | 新增或修改无末尾换行文件 | 无末尾换行 |
| `C13-chinese-add` | 新增中文路径/内容文件 | 非 ASCII 路径与内容 |
| `C14-longest-prefix` | 新增模块内文件 | 多路径映射最长前缀 |
| `C15-partial-unmapped` | 同一提交含可映射和不可映射路径 | 不允许部分静默跳过 |
| `C16-case-only-rename` | 仅大小写变化的重命名 | Windows 大小写风险 |

### 目标仓库初始文件

目标 baseline 至少包含：

| 路径 | 内容要求 |
|---|---|
| `src/existing.txt` | 与源基线一致 |
| `src/delete_me.txt` | 与源基线一致 |
| `src/move_me.txt` | 与源基线一致 |
| `src/chain.txt` | 与源基线一致 |
| `src/target_only.txt` | 仅目标存在，用于证明不会被误删 |
| `README.md` | 仅目标存在，用于证明迁移范围外文件不变 |

SVN 目标使用 `trunk/` 作为目标根时，以上路径位于 `trunk/` 下。

### 场景矩阵

#### A. 仓库与配置

| ID | 场景 | 输入 | 预期 |
|---|---|---|---|
| A01 | 添加 Git 仓库 | 选择本地 Git 工作区 | 自动识别当前分支和分支列表，可保存 |
| A02 | 添加 SVN 仓库 | 选择本地 SVN 工作副本，填凭据 | 自动识别检出点和服务器信息，可保存 |
| A03 | 编辑 SVN 仓库但密码留空 | 修改名称或分支，密码为空 | 原密码保持，不影响后续拉取提交 |
| A04 | 删除仓库 | 删除已保存仓库 | 仓库列表移除，相关仓库对路径映射不再可用 |
| A05 | 源和目标相同 | 源/目标选同一仓库 | 不允许进入迁移执行 |
| A06 | 目标工作区脏 | 目标有未提交修改 | 执行迁移前失败，目标修改保持原样 |

#### B. 提交列表与选择

| ID | 场景 | 输入 | 预期 |
|---|---|---|---|
| B01 | 拉取最近提交 | 源仓库选择 `DS-*` | 显示 hash/revision、作者、时间、说明、文件数 |
| B02 | 分页加载更早提交 | 提交数超过第一页大小 | 加载更早提交后无重复、顺序稳定 |
| B03 | 搜索提交 | 搜索提交说明、作者或 hash | 只显示匹配项，已选状态不丢失 |
| B04 | 多选不连续提交 | 选择 `C01`、`C03`、`C08` | 预览只包含所选提交影响 |
| B05 | 全选过滤结果 | 搜索后全选 | 只选择当前过滤结果，不误选隐藏项 |

#### C. 路径映射

| ID | 场景 | 输入 | 预期 |
|---|---|---|---|
| C01 | 默认 Git 映射 | Git 源，默认映射 | `src/a.txt` 写入目标 `src/a.txt` |
| C02 | 默认 SVN trunk 映射 | SVN 源分支为 trunk | `/trunk/src/a.txt` 写入目标 `src/a.txt` 或 SVN 目标 `trunk/src/a.txt` |
| C03 | 自定义模块映射 | `/trunk/module-a` -> `packages/module-a` | 文件写入 `packages/module-a/...` |
| C04 | 多规则最长前缀 | 同时有 `/trunk` 和 `/trunk/module-a` | `/trunk/module-a/x` 命中更长规则 |
| C05 | 无匹配路径 | 所选提交包含未覆盖路径 | 预览或迁移失败，不能静默跳过 |
| C06 | 目标路径已有同名文件 | 新增文件映射到已存在目标路径 | 集成计划标为 blocked 或执行被拒绝 |

#### D. 预览准确性

| ID | 场景 | 输入 | 预期 |
|---|---|---|---|
| D01 | 单新增文件 | 选择 `C01` | 预览新增 1 个文件，新增行数正确 |
| D02 | 单修改文件 | 选择 `C02` | 预览修改 1 个文件，只显示实际增删行 |
| D03 | 单删除文件 | 选择 `C03` | 预览删除 1 个文件，目标内容将为空或删除 |
| D04 | 重命名/移动 | 选择 `C04` | 不遗留旧路径，目标只出现预期新路径变更 |
| D05 | 二进制文件 | 选择 `C05` | 预览标出文件变更，不生成错误文本 diff |
| D06 | CRLF 文件 | 选择 `C06` | 内容哈希匹配，换行处理稳定 |
| D07 | 多提交同文件 | 选择 `C07` + `C08` | 预览净结果为 `step2`，不是 `step1` |
| D08 | 增后删抵消 | 选择 `C09` + `C10` | 预览无 `tmp/transient.txt` 净变更 |
| D09 | 中文路径/内容 | 选择 `C13` | 目标路径和内容保持正确编码 |
| D10 | 多规则最长前缀 | 选择 `C14` | 命中最具体映射 |
| D11 | 仅大小写重命名 | Windows 下选择 `C16` | 目标文件名大小写符合预期 |

#### E. 集成计划与冲突

| ID | 场景 | 目标初始状态 | 模式 | 预期 |
|---|---|---|---|---|
| E01 | 干净补丁 | 目标基线与源一致 | 增量优先 | auto_ok + apply_patch |
| E02 | 目标独立修改但不重叠 | 修改同文件其他区域 | 增量优先 | 可自动或 review，不应 blocked |
| E03 | 目标独立修改且重叠 | 修改同一行 | 增量优先 | blocked + manual_merge |
| E04 | 补丁上下文漂移 | 目标同语义但行号变化 | 增量优先 | 标记 context_drift，review + write_after，确认后生成正确预览 |
| E05 | 严格模式上下文不匹配 | 目标和源基线不同 | strict_replay | blocked |
| E06 | 提交结果模式 | 目标和源基线不同 | commit_result | review + write_after，需确认 |
| E07 | 删除确认 | 选择 `C03` | 任意 | review，未确认不能执行 |
| E08 | 二进制写入确认 | 选择 `C05` | 任意 | review，未确认不能执行 |
| E09 | blocked 未处理 | 有 blocked 项直接执行 | 任意 | 执行按钮不可用或后端拒绝 |
| E10 | review 未确认 | 有 review 项直接执行 | 任意 | 执行按钮不可用或后端拒绝 |
| E11 | 目标已包含 | 目标提前包含源提交结果 | 增量优先 | already_contains + skip，不重复改文件 |
| E12 | 多候选定位 | 同一 old block 在目标出现多次 | 增量优先 | multiple_candidates + blocked，不自动选择 |

#### F. 迁移执行

| ID | 场景 | 输入 | 预期 |
|---|---|---|---|
| F01 | Git -> Git 单提交 | `DS-GG` 选择 `C02` | 目标新增 1 个 Relay 提交，文件内容完全匹配 |
| F02 | SVN -> Git 单 revision | `DS-SG` 选择 `C02` | 目标新增 1 个 Git 提交 |
| F03 | Git -> SVN 单提交 | `DS-GS` 选择 `C02` | 目标新增 1 个 SVN revision |
| F04 | SVN -> SVN 单 revision | `DS-SS` 选择 `C02` | 目标新增 1 个 SVN revision |
| F05 | 多提交逐条迁移 | 选择 `C01`、`C02`、`C03` | 目标提交数为 3，最终文件树符合预期 |
| F06 | 多提交 squash | 选择 `C01`、`C02`、`C03`，开启 squash | 目标提交数为 1，commit message 首行为用户填写内容，正文包含 Relay 版本和原 commit 列表 |
| F07 | squash 未填 message | 开启 squash，message 为空 | 不允许执行 |
| F08 | 失败回滚 | 人为制造一个执行中必失败文件 | 失败后目标回到 baseline，无半应用文件 |
| F09 | 空净变更 | 选择 `C09` + `C10` | 不产生多余文件；提交行为需符合产品定义并稳定 |
| F10 | 目标工作区已有期望内容 | 目标提前应用同样内容 | 不重复写入，不新增 Relay 提交 |
| F11 | 上下文漂移自动合并 | 目标文件行号漂移但 old block 唯一 | 用户确认 review 后目标内容为合并结果 |
| F12 | 目标已包含跳过 | 目标提前应用同样内容 | 目标文件保持不变，不新增 Relay 提交 |
| F13 | 同区域冲突阻断 | 目标同一行已有独立修改 | 执行前 blocked，目标保持不变 |
| F14 | 多候选阻断 | 同一 old block 出现多次 | 执行前 blocked，目标保持不变 |

#### G. 人工处理流程

| ID | 场景 | 输入 | 预期 |
|---|---|---|---|
| G01 | 用默认编辑器打开 blocked 文件 | blocked 项选择“用编辑器打开” | 打开目标文件真实路径 |
| G02 | 切换编辑器 | 从下拉选择另一个编辑器 | 后续打开使用新默认编辑器 |
| G03 | 添加自定义编辑器 | 添加可执行程序路径 | 编辑器列表出现新项，可设为默认 |
| G04 | 标记已解决前执行 | blocked 项未标记 | 不可执行 |
| G05 | 标记已解决后执行 | 人工改好目标文件并标记 | 可执行，最终提交包含人工解决后的结果 |
| G06 | review 全部接受 | 多个 review 项点击全部接受 | 所有 review 项进入已确认状态 |

#### H. 迁移记录

| ID | 场景 | 输入 | 预期 |
|---|---|---|---|
| H01 | 成功记录 | 任一成功迁移 | 记录出现于迁移历史 |
| H02 | 记录详情 | 打开本次记录 | 显示源/目标、提交、文件、冲突数量、状态 |
| H03 | 文件 diff 回看 | 在记录详情选择文件 | diff 与迁移时预览一致 |
| H04 | 失败迁移 | 执行失败 | 不应写入 success 记录；如有失败记录，状态必须明确为失败 |

#### I. 生产安全回归

这些用例优先级最高，任何一个失败都不能发布。

| ID | 场景 | 预期 |
|---|---|---|
| I01 | 所选提交只改 1 个文件 | 目标只出现这 1 个文件变更 |
| I02 | 目标有未提交业务改动 | Relay 拒绝执行，不覆盖业务改动 |
| I03 | 路径映射错误 | Relay 报错，不写入错误目录 |
| I04 | 同文件多提交链 | 最终内容等于源所选提交链的最终内容 |
| I05 | add 后 delete | 目标不出现临时文件 |
| I06 | delete 文件 | 只删除预期文件，不影响同目录其他文件 |
| I07 | blocked 未解决 | 不能执行迁移 |
| I08 | 执行中失败 | 目标仓库回滚到执行前 baseline |

### 每个用例的记录模板

```markdown
### <ID> <标题>

- 数据集：DS-GG / DS-SG / DS-GS / DS-SS
- 源 baseline：
- 目标 baseline：
- 选择提交：
- 路径映射：
- 迁移模式：
- 是否 squash：
- 操作步骤：
- 预期集成计划：
- 预期目标变更文件：
- 预期目标内容哈希：
- 预期提交数量：
- 预期迁移记录：
- 实际结果：
- 结论：通过 / 失败
```

### 最小发布准入集

日常发布前至少执行：

| 范围 | 必跑用例 |
|---|---|
| 自动回归 | `npm run test:regression:release`，且正式 release 环境 SVN case 零 skipped |
| Git -> Git 人工抽检 | A05, A06, B01, C01, D01-D11, E01-E10, F01, F05-F10, G04-G05, H01-H03, I01-I08 |
| SVN -> Git 人工抽检 | A02, B01, C02, D01-D08, F02, F05, H01 |
| Git -> SVN 人工抽检 | A01, B01, C01, F03, F05, H01 |
| SVN -> SVN 人工抽检 | A02, B01, C02, F04, F05, H01 |

如果时间不足，不能省略 `I01-I08`。

---

## 手动端到端验证

在已安装 `git`、`svn` 且准备好源/目标仓库的机器上：

```bash
npm run tauri dev
```

建议按五步向导验证：

1. 在源仓库步骤选择或新增 Git/SVN 源仓库。
2. 在选择提交步骤拉取提交列表，勾选一个或多个提交。
3. 在变更预览步骤生成源提交 base -> after 预览，核对文件树和行级 diff。
4. 在目标仓库步骤选择目标仓库，并配置路径映射。
5. 在确认迁移步骤核对集成计划，处理 review / blocked 项，执行迁移后检查目标仓库和迁移历史。

手动验证必须额外用 Git/SVN CLI 检查目标工作区，不只看 UI 成功提示。

---

## 提交前检查清单

普通文档或前端类型改动：

```bash
npm test
npm run lint
npm run format:check
```

涉及 Rust 核心、预览、VCS、迁移执行：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo clippy
npm run test:regression:safety
```

发布前：

```bash
npm test
npm run lint
npm run format:check
cd src-tauri && cargo test
cd src-tauri && cargo clippy
npm run test:regression:release
```
