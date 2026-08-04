# copy-diff

一个桌面工具，用于将 SVN 或 Git 仓库中的提交迁移到 Git 或 SVN 目标仓库。

它可以选择提交、预览文件变更、配置路径映射、处理冲突，并保存迁移记录。

## 支持范围

- 源仓库和目标仓库均支持 Git、SVN
- 支持 Git ↔ Git、Git ↔ SVN、SVN ↔ Git、SVN ↔ SVN
- 迁移前会生成计划，未处理的冲突或待确认项不能执行迁移

## 环境要求

- Node.js 18+
- Rust stable
- Git
- SVN

Windows 还需安装 Visual Studio Build Tools，并使用 MSVC Rust 工具链；否则 Tauri 可能无法编译。

## 快速开始

```bash
git clone <repo-url> copy-diff
cd copy-diff
npm install
npm run tauri dev
```

首次启动需要编译 Rust，耗时较长属于正常现象。

## 常用命令

| 命令                           | 用途             |
| ------------------------------ | ---------------- |
| `npm run tauri dev`            | 启动桌面开发模式 |
| `npm run tauri build`          | 构建安装包       |
| `npm run dev`                  | 仅启动前端界面   |
| `npm test`                     | 运行前端测试     |
| `npm run lint`                 | 检查前端代码     |
| `npm run format:check`         | 检查前端格式     |
| `cd src-tauri && cargo test`   | 运行 Rust 测试   |
| `cd src-tauri && cargo clippy` | 检查 Rust 代码   |

开发前建议运行：

```bash
npm test && npm run lint && npm run format:check
cd src-tauri && cargo test && cargo clippy
```

更多测试说明见 [TESTING.md](TESTING.md)。
