# ClawX - Tauri 版本

<p align="center">
  <img src="src/assets/logo.svg" width="128" height="128" alt="ClawX Logo" />
</p>

ClawX 是一个基于 OpenClaw 的 AI 智能体桌面客户端。这是 ClawX 的 Tauri 版本，由个人开发者从原 Electron 版本迁移而来。

## 项目状态

⚠️ **早期开发阶段** - 这是一个正在进行中的项目，可能存在 bug 和未完善的功能。

## 快速开始

### 前置要求

- Node.js 22+
- pnpm 9+
- Rust (最新稳定版)

### 安装与运行

```bash
# 克隆仓库
git clone https://github.com/dydydd/ClawX.rs.git
cd ClawX.rs

# 安装依赖
pnpm install

# 开发模式
pnpm tauri dev

# 构建生产版本
pnpm tauri build
```

## 技术栈

- **前端**: React 19 + TypeScript + Tailwind CSS
- **后端**: Tauri 2 + Rust
- **状态管理**: Zustand
- **构建工具**: Vite

## 主要功能

- 💬 智能聊天界面
- 📡 多频道管理
- ⏰ 定时任务
- 🧩 技能系统
- 🔐 多 AI 供应商支持
- 🌙 深色/浅色主题

## 贡献

欢迎提交 Issue 和 Pull Request！

## 特别感谢

感谢原 ClawX 开发团队（ValueCell Team）的开源贡献。本项目基于原 ClawX 项目从 Electron 迁移到 Tauri。

- 原项目地址: https://github.com/ValueCell-ai/ClawX
- 原开发者: ValueCell Team

## 开源协议

MIT License