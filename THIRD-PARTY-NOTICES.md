# Third-Party Notices

このリポジトリには以下のサードパーティソフトウェアが同梱されています。
それぞれのライセンス条項に従って再配布されます。

---

## NoSleep.js v0.12.0

- ファイル: `static/nosleep.js`
- 作者: Rich Tibbett
- 配布元: https://github.com/richtr/NoSleep.js
- ライセンス: MIT

**改変について**: 同梱の `static/nosleep.js` は上流のミニファイ済み配布物
(`NoSleep.min.js` v0.12.0) を無改変で収録しています。ただし本プロジェクトの
`static/index.html` は、NoSleep が生成する keep-awake 用 `<video>` 要素を DOM に
接続し直すパッチを実行時に当てています（近年の iOS Safari が detach された
`<video>` では自動ロックを抑止しないため）。上流のコード自体は変更していません。

### ライセンス全文

```
The MIT License (MIT)

Copyright (c) Rich Tibbett

Permission is hereby granted, free of charge, to any person obtaining
a copy of this software and associated documentation files (the
"Software"), to deal in the Software without restriction, including
without limitation the rights to use, copy, modify, merge, publish,
distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so, subject to
the following conditions:

The above copyright notice and this permission notice shall be
included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

---

## Rust クレート依存

`Cargo.toml` / `Cargo.lock` に記載の依存クレート（tokio, axum, tower-http, wasapi,
serde, serde_json, bytes, anyhow およびその推移的依存）はソースコードとして本
リポジトリに同梱していません。いずれも MIT または Apache-2.0（もしくはそのデュアル
ライセンス）で配布されています。

ビルド済みバイナリを配布する場合は、それらの帰属表記を添付してください。
一覧は次のコマンドで生成できます。

```bash
cargo install cargo-license
cargo license
```
