# audio2ios-fast

Windowsのシステム音声をリアルタイムでiPhoneのSafariに飛ばして再生するツールです。
[../audio2ios](https://github.com/Shannon-toppo/audio2ios) のPythonサーバーを **Rust** に書き換えた高速版です。

RustサーバーがWASAPIループバックで音声をキャプチャし、生PCMをWebSocketでLAN配信。
SafariクライアントはAudioWorkletで再生します（iOS 18対策のScriptProcessorNodeフォールバック付き）。
クライアント（`static/`）は元のPython版から流用したもので、ワイヤープロトコルは
バイト互換です（iOS の画面スリープ対策まわりのみ追加修正が入っています）。

## 必要環境

- Windows（WASAPIループバック対応）
- Rust ツールチェーン（`cargo`） — https://rustup.rs
- 同一LAN上のiPhone（Safari）

## セットアップと起動

```bash
cargo run --release      # ビルドして起動（0.0.0.0:8080、PORT環境変数で変更可）
```

または `start.bat` をダブルクリック（初回のみ自動でリリースビルドします）。

起動するとコンソールにURL（例: `http://192.168.1.5:8080`）が表示されます。
iPhoneのSafariでそのURLを開き、「再生開始」をタップしてください。

## 使い方

1. PCで `cargo run --release`（または `start.bat`）を実行
2. 表示されたURLをiPhoneのSafariで開く
3. 「再生開始」をタップ（iOSの自動再生制限のため必ずタップ操作が必要）
4. 音が出ない／途切れる場合は「互換モードで再生」を試す

再生中は画面下部に `buf / underrun / overflow` のカウンタが表示され、状態を確認できます。

- **underrun が増える** → バッファ不足。`static/pcm-worklet.js` の `preRoll` を増やす
- **overflow が増え続ける** → 送出側が速いクロックドリフト（微小ドロップで吸収中）

## Python版との違い

| | audio2ios (Python) | audio2ios-fast (Rust) |
|---|---|---|
| 音声キャプチャ | pyaudiowpatch | `wasapi` クレート（イベント駆動ループバック） |
| HTTP/WebSocket | aiohttp | axum + tower-http |
| 音声→配信の橋渡し | asyncio.Queue | 専用スレッド + `tokio::broadcast` |
| 起動 | `python server.py` | `cargo run --release` |

ネイティブコンパイルによりCPU負荷とレイテンシが下がります。配信プロトコル・チャンクサイズ・
`/meta` のJSONはPython版とバイト互換なので、どちらのサーバーでも同じクライアントが動作します。

## 仕組み

```
WASAPIループバック → キャプチャスレッド → 960フレーム分割 → broadcastチャンネル
                                                                  ↓
iPhone Safari ← AudioWorklet（リングバッファ＋プリロール） ← WebSocketクライアント
```

- 配信プロトコルは生16-bit PCM、960フレーム（20ms @ 48kHz）チャンク
- クライアントは150msぶん貯めてから再生開始し、WiFiのジッタを吸収
- 詳細な設計は [CLAUDE.md](CLAUDE.md) を参照

## ライセンス

MIT License — 詳細は [LICENSE](LICENSE) を参照してください。

同梱のサードパーティコード（`static/nosleep.js` = NoSleep.js, MIT）の帰属表記は
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) にまとめています。
