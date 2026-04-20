# Revive

Revive は、隣接する複数の Rust 製エミュレーター core を 1 つの SDL2 フロントエンドから起動する統合ランチャーです。

同時に動かすタイトルは 1 本だけです。ROM の拡張子または `--system` 指定から対象システムを判定し、各 core の API 差分を `revive-core` が吸収します。

## 対応システム

- NES / Famicom
- SNES / Super Famicom
- Mega Drive / Genesis
- PC Engine / TurboGrafx-16
- Game Boy
- Game Boy Color
- Game Boy Advance

## 構成

この workspace は既存 emulator core を path dependency として参照します。Revive の checkout と同じ階層に以下の repository が必要です。

```text
../nes-rust/crates/core
../snes-rust/crates/core
../megadrive/crates/core
../pce/crates/core
../gameboy/crates/core
../gameboy/crates/gb
../gameboy/crates/gba
```

Revive 側の crate は責務ごとに分かれています。

- `crates/revive-core`: システム判定、各 emulator core の adapter、共通 API
- `crates/revive-cheat`: UI 非依存のチート検索、チート定義、JSON 保存/読み込み
- `crates/revive-cli`: SDL2 + OpenGL + egui フロントエンド

## 必要なもの

- Rust toolchain
- C/C++ toolchain
- CMake
- macOS の場合は Apple Silicon native build を推奨

SDL2 は `sdl2` crate の `bundled` / `static-link` feature でビルドします。通常は system SDL2 の事前インストールは不要です。

## 起動方法

ROM を指定しない場合、ローカルファイル選択ダイアログが開きます。

```sh
cargo run
cargo run -- --select
cargo run -- <rom>
```

システムを明示する場合:

```sh
cargo run -- run <rom> --system nes
cargo run -- run <rom> --system snes
cargo run -- run <rom> --system megadrive
cargo run -- run <rom> --system pce
cargo run -- run <rom> --system gb
cargo run -- run <rom> --system gbc
cargo run -- run <rom> --system gba
```

音声なしで起動する場合:

```sh
cargo run -- <rom> --no-audio
```

チートファイルを明示する場合:

```sh
cargo run -- <rom> --cheats cheats/custom.json
```

## Apple Silicon 向けビルド

Apple Silicon では `aarch64-apple-darwin` と `target-cpu=native` を使う alias を用意しています。

```sh
cargo run-apple -- --select
cargo run-apple -- <rom>
cargo build-apple
```

通常の `cargo run` でも core crate は dev profile で `opt-level = 3` になっていますが、実際に遊ぶ場合は release alias の方が軽くなります。

## ROM 判定

拡張子から自動判定します。

- `.nes`: NES
- `.sfc`, `.smc`: SNES
- `.md`, `.gen`: Mega Drive
- `.pce`: PC Engine
- `.gb`: Game Boy
- `.gbc`: Game Boy Color
- `.gba`: Game Boy Advance
- `.bin`: `SEGA` header がある場合のみ Mega Drive

判定できない場合は `--system` を指定してください。

## 操作方法

共通操作:

- `Esc`: 終了
- `Tab`: チートパネルの表示/非表示
- `Ctrl/Cmd + 0..9`: ステートセーブ
- `0..9`: ステートロード

NES:

- 矢印: D-pad
- `Z` / `J`: A
- `X` / `K`: B
- `Return` / `Space`: Start
- `Backspace` / Shift: Select

SNES:

- 矢印: D-pad
- `D`: A
- `S`: B
- `W`: X
- `A`: Y
- `E`: L
- `Q`: R
- `Return` / `Space`: Start
- `Backspace` / Shift: Select

Mega Drive:

- 矢印: D-pad
- `A`: A
- `Z`: B
- `X`: C
- `S`: X
- `D`: Y
- `F`: Z
- `Q`: Mode
- `Return` / `Space`: Start

PC Engine:

- 矢印: D-pad
- `Z` / `J`: I
- `X` / `K`: II
- `Return` / `Space`: Run
- `Backspace` / Shift: Select

Game Boy / Game Boy Color:

- 矢印: D-pad
- `X` / `J`: A
- `Z` / `K`: B
- `Return` / `Space`: Start
- `Backspace` / Shift: Select

Game Boy Advance:

- 矢印: D-pad
- `X` / `J`: A
- `Z` / `K`: B
- `A`: L
- `S`: R
- `Return` / `Space`: Start
- `Backspace` / Shift: Select

## チートパネル

`Tab` で `../snes-rust` と同系統の右サイドチートパネルを開きます。パネル表示中はゲーム入力を解除し、テキスト入力や UI 操作を優先します。

パネルには 2 つのタブがあります。

- `Hex Viewer`: メモリ閲覧、アドレスジャンプ、値の直接書き換え
- `Cheat Search`: snapshot、条件フィルタ、候補一覧、チート追加、Active Cheats 管理

Active Cheats では以下ができます。

- enabled の ON/OFF
- 書き込む値の編集
- ラベル編集
- 削除
- Save / Load

デフォルトのチート保存先はゲームごとに分かれます。

```text
cheats/<system>/<rom>/cheats.json
```

例:

```text
cheats/snes/Super F1 Circus Gaiden (Japan)/cheats.json
cheats/megadrive/Sonic The Hedgehog/cheats.json
```

`--cheats <path>` を指定した場合は、そのパスを使用します。旧形式の `cheats/<rom>.json` が存在する場合は読み込み fallback します。

## チートJSON形式

`revive-cheat` は以下の JSON を読み書きします。

```json
[
  {
    "region": "wram",
    "offset": 4660,
    "value": 153,
    "enabled": true,
    "label": "Example"
  }
]
```

主な region id:

- NES: `cpu_ram`, `prg_ram`
- SNES: `wram`, `sram`
- Mega Drive: `wram`
- PC Engine: `wram`, `cart_ram`, `bram`
- Game Boy / Game Boy Color: `cart_ram`
- Game Boy Advance: 現在の core API ではチート対象メモリ未公開

Game Boy / Game Boy Color の `cart_ram` は現在の core API では read-only です。

## ステート保存

ステートデータもゲームごとに分けます。

```text
states/<system>/<rom>/slot<N>.<ext>
```

例:

```text
states/snes/Super F1 Circus Gaiden (Japan)/slot1.sns
states/megadrive/Sonic The Hedgehog/slot1.mdst
states/nes/Super Mario Bros/slot1.sav
states/pce/Adventure Island/slot1.pcst
states/gba/Example/slot1.gbas
```

旧形式の保存ファイルがある場合、ロード時のみ fallback します。

```text
states/<system>/<rom>.slot<N>.<ext>
states/<rom>.slot<N>.sav  # NES 旧形式
```

制限:

- GB/GBC のステート保存は現在の `../gameboy` core API では未公開です。
- GBA のステート保存は対応しています。

## 永続セーブ

SRAM や backup RAM は各 core の API に合わせて扱います。

- SNES: `.srm`
- PC Engine: `.sav`, `.brm`
- Game Boy / Game Boy Color: `.sav`
- Game Boy Advance: `.sav`
- NES: core 側の SRAM 保存処理

永続セーブは通常終了時に flush します。クラッシュ時は保存されない可能性があります。

## 設計

Revive は「複数 core を同時実行する emulator」ではなく、「1 本の ROM に対して適切な core を選び、共通 UI で操作する frontend」です。

### `revive-core`

`revive-core` は各 emulator core の違いを adapter で隠します。`CoreInstance` enum が NES / SNES / Mega Drive / PC Engine / Game Boy / GBA を包み、CLI には以下の共通操作だけを見せます。

- `load_rom`
- `step_frame`
- `frame`
- `audio_spec`
- `drain_audio_i16`
- `set_button`
- `memory_regions`
- `read_memory`
- `write_memory_byte`
- `save_state_to_slot`
- `load_state_from_slot`
- `flush_persistent_save`

追加システムを入れる場合は、`SystemKind`、`CoreInstance`、ROM 判定、入力 mapping、state path、memory region を増やします。

### `revive-cheat`

`revive-cheat` は UI に依存しません。チート検索とチート定義の保存だけを担当します。

- `CheatSearch`: RAM snapshot と filter による候補絞り込み
- `SearchFilter`: equal / changed / increased などの検索条件
- `CheatManager`: `CheatEntry` の追加、削除、JSON 保存/読み込み

チートは `region + offset + value` で表現します。実際の書き込み先は `revive-core` の adapter が解決します。

### `revive-cli`

`revive-cli` は SDL2 + OpenGL + egui の frontend です。

主な流れ:

1. ROM path を CLI 引数またはファイル選択ダイアログから取得
2. 拡張子または `--system` から system を判定
3. `CoreInstance::load_rom` で対象 core を起動
4. イベントループで入力、ステート操作、チートパネル操作を処理
5. 毎フレーム `apply_cheats -> step_frame -> apply_cheats`
6. audio queue にサンプルを供給
7. RGB24 frame を OpenGL texture にアップロード
8. egui のチートパネルを右サイドに描画

パネル表示中は game viewport の右側に panel width を確保し、ゲーム画面は残り領域に aspect ratio を維持して表示します。

## 開発コマンド

```sh
cargo fmt
cargo check -p revive-cli
cargo check --target aarch64-apple-darwin -p revive-cli
cargo test -p revive-core
cargo test -p revive-cheat
cargo build -p revive-cli
```

隣接 core に変更が入った場合は、その repository 側の test も実行してください。

例:

```sh
cd ../nes-rust
cargo test -p nes-emulator save_state

cd ../snes-rust
cargo test -p snes-core
```

## 既知の制限

- 同時に実行できる ROM は 1 本だけです。
- GB/GBC のステート保存は未対応です。
- GBA の cheat memory region は現在未公開です。
- `--cheats` 未指定時の Save は `cheats/<system>/<rom>/cheats.json` に保存します。
- path dependency の sibling repository がないとビルドできません。
