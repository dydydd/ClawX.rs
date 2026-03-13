# ClawX - Tauri バージョン

<p align="center">
  <img src="src/assets/logo.svg" width="128" height="128" alt="ClawX Logo" />
</p>

ClawX は OpenClaw をベースにした AI エージェントデスクトップクライアントです。これは ClawX の Tauri バージョンで、個人開発者によって元の Electron バージョンから移行されました。

## プロジェクトステータス

⚠️ **早期開発段階** - 進行中のプロジェクトであり、バグや未完成の機能が存在する可能性があります。

## クイックスタート

### 前提条件

- Node.js 22+
- pnpm 9+
- Rust (最新安定版)

### インストールと実行

```bash
# リポジトリをクローン
git clone https://github.com/dydydd/ClawX.rs.git
cd ClawX.rs

# 依存関係をインストール
pnpm install

# 開発モード
pnpm tauri dev

# 本番ビルド
pnpm tauri build
```

## 技術スタック

- **フロントエンド**: React 19 + TypeScript + Tailwind CSS
- **バックエンド**: Tauri 2 + Rust
- **状態管理**: Zustand
- **ビルドツール**: Vite

## 主な機能

- 💬 インテリジェントチャットインターフェース
- 📡 マルチチャネル管理
- ⏰ スケジュールタスク
- 🧩 スキルシステム
- 🔐 複数 AI プロバイダー対応
- 🌙 ダーク/ライトテーマ

## コントリビューション

Issue や Pull Request を歓迎します！

## 特別な感謝

オリジナルの ClawX 開発チーム（ValueCell Team）のオープンソース貢献に感謝します。このプロジェクトは元の ClawX プロジェクトから Electron から Tauri に移行されました。

- 元のプロジェクト: https://github.com/ValueCell-ai/ClawX
- 元の開発者: ValueCell Team

## ライセンス

MIT License