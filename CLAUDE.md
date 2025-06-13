MouseShare ツール仕様書（Rust実装）

1. システム概要

目的: macOS と Linux（Wayland/Hyprland + NixOS）間でマウス操作を共有
ユーザー体験: 2つの物理画面を1つの大きな仮想画面として操作できる

特徴: 軽量・ローカルLAN専用・CLI + 設定ファイルベース・シームレスなマルチモニター体験

## 重要な設計原則

### マルチモニター統合体験
- ユーザーからは「大きな1枚の画面」として認識される
- macOS(2600x1440) + Linux(1920x1080) = 仮想画面(4520x1440)
- マウス位置は仮想座標系で管理し、各システムのローカル座標に変換

### 相互排他制御
- 片方の画面でマウス操作中は、もう片方の物理マウスは無効化
- エッジ越え時のみ制御権が移譲される
- 制御権を持つ側のみがマウスイベントをキャプチャ・送信

### 座標変換ロジック
- 送信側: ローカル座標 → 仮想座標 → ネットワーク送信
- 受信側: ネットワーク受信 → 仮想座標 → ローカル座標 → マウス注入

### 重要な設計原則：マウス制御の分離

**問題**: macOSの物理マウス位置をそのまま使うと、Linux側制御時に破綻する
- macOS側制御時: 物理マウス位置 = 仮想座標のmacOS部分
- Linux側制御時: 物理マウス位置 ≠ 仮想座標のLinux部分（物理的にはmacOS画面上）

**解決策**: プログラム内で独立した仮想マウス座標を管理
- **仮想マウス座標**: プログラムが管理する統一座標系(0,0)-(4519,1439)
- **物理マウス座標**: macOSから取得する実際のマウス位置
- **制御領域判定**: 仮想座標でどちら側を制御中かを管理

**実装方針**:
1. 仮想マウス座標を内部状態として管理
2. macOS側制御時: 物理マウス → 仮想座標更新
3. Linux側制御時: 物理マウス移動量のみ取得 → 仮想座標に加算
4. 各フレームで仮想座標から適切な画面にマウス注入

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
  width: 2560               # 自分の画面解像度
  height: 1440
remote_screen:
  width: 1920               # 相手の画面解像度  
  height: 1080
layout:
  position: left            # 仮想画面での自分の位置（left/right/top/bottom）
  remote_position: right    # 仮想画面での相手の位置
edge:
  sender_to_receiver: right # 制御権移譲する方向
  receiver_to_sender: left  # 制御権を戻す方向
protocol: udp               # udp or tcp
buffer_size: 4096

## 仮想画面レイアウト例
macOS(sender, position=left) + Linux(receiver, position=right)
┌─────────────────┬─────────────────┐
│   macOS 2600px  │  Linux 1920px   │
│     1440px      │    1080px       │
│    (0,0)-(2599,1439) │ (2600,0)-(4519,1079) │
└─────────────────┴─────────────────┘
仮想座標: (0,0) - (4519,1439)

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