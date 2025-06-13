MouseShare ツール仕様書（Rust実装）

1. システム概要

目的: macOS と Linux（Wayland/Hyprland + NixOS）間でマウス操作を共有

特徴: 軽量・ローカルLAN専用・CLI + 設定ファイルベース

2. 開発言語・主要技術

言語: Rust（静的型付け、メモリ安全、クロスプラットフォーム）

クレート例:

イベント取得: core-graphics（mac）, libevdev（Linux）

仮想イベント注入: core-graphics, uinput（Rustラッパー）

非同期IO: tokio（UDP/TCP）

設定管理: serde + serde_yaml / toml

CLI: clap

3. 動作モード

sender: マウスイベント取得 → 送信

receiver: イベント受信 → 仮想マウス注入

4. 主な機能

端越え検知: 設定ファイルで指定した画面端を検出

マウス移動／クリック（左・中・右）／スクロールを双方向透過

単一設定ファイルで IP, ポート, 画面解像度, エッジ設定

CLI: start/stop/status, 設定ファイルテンプレート出力, ログレベル指定

5. 設定ファイル構造（YAML/TOML）

mode: sender                # sender or receiver
remote_ip: 192.168.0.42     # 相手側IP
remote_port: 5000           # ポート
screen:
  width: 2560
  height: 1440
edge:
  sender_to_receiver: right  # left/right/top/bottom
  receiver_to_sender: left
protocol: udp               # udp or tcp
buffer_size: 4096

6. ネットワーク

プロトコル: UDP or TCP（設定）

動作環境: 同一LANまたは直接接続

認証・暗号化: 省略

7. モジュール構成

Capturer（mac/Linux兼用）

mac: Quartz Event Tap

Linux: /dev/input/event* → libevdev

Sender/Receiver ネットワーク層

tokio::net::{UdpSocket, TcpStream}

シリアライズ: JSON or bincode

Injector（mac/Linux兼用）

mac: CGEventCreateMouseEvent → CGEventPost

Linux: uinput 仮想デバイス

CLI & 設定読み込み

clap + serde

8. 実装ステップ

Rustプロジェクト初期化、依存登録

CLI 基盤（clap） + 設定読込実装

mac Capturer → ローカルで CGEventPost（ループバック）テスト

Linux Injector → /dev/uinput単体テスト

ネットワーク送受信（sender→receiver 単方向）

端越え判定ロジック実装

双方向＆receiver→sender 実装

テスト（ユニット & 実機）

# 開発環境
cargoはinstall済み。
git init実施ずみ