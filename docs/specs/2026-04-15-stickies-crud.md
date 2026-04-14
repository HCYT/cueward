# Stickies CRUD

## Goal

處理 issue #93，為 `cueward` 新增 macOS Stickies（便條紙）的基本 CRUD。

需求重點：

- 先支援 list / create / update / delete
- 以 AppleScript 直接控制 Stickies app
- 第一版只處理純文字 title / body
- 先不處理顏色、字型、視窗位置、富文字格式

## CLI

新增 top-level `stickies` 子命令：

```bash
cueward stickies list
cueward stickies create --title "臨時待辦" --body "記得回覆客戶"
cueward stickies update --id <sticky-id> [--title "新標題"] [--body "更新後內容"]
cueward stickies delete --id <sticky-id>
```

語意：

- `list`
  - 列出目前所有便條紙
  - 每張輸出 `id`、`title`、`body`
  - 若 title 缺失，回傳可讀的 fallback label
- `create`
  - 需要 `--title` 與 `--body`
  - 會建立一張新的便條紙
- `update`
  - 需要 `--id`
  - `--title`、`--body` 至少一個
- `delete`
  - 需要 `--id`

## Identifier 策略

Stickies 不保證提供像 Notes DB 那樣的穩定 UUID。

第一版採「AppleScript 可取到的最穩定 key」策略：

- 若 app object 有可直接讀取的 unique id / name 組合，可直接使用
- 若沒有，使用 list 結果中可穩定重建的 fingerprint
- `id` 必須能在同一次 session 與一般日常操作中穩定對應到同一張便條紙

若發現 AppleScript 沒有足夠穩定的 key，第一版可退一步定義為：

- `id = normalized(title + body prefix + index)` 類型的應用層 fingerprint

但必須保證：

- `list` 回來的 `id` 能直接餵給 `update` / `delete`
- 找不到目標時回傳明確錯誤，不默默操作錯誤便條紙

## 輸出格式

`list`：

```json
[
  {
    "id": "sticky-1",
    "title": "臨時待辦",
    "body": "記得回覆客戶"
  }
]
```

`create`：

```json
{
  "created": true,
  "sticky": {
    "id": "sticky-1",
    "title": "臨時待辦",
    "body": "記得回覆客戶"
  }
}
```

`update`：

```json
{
  "updated": true,
  "sticky": {
    "id": "sticky-1",
    "title": "新標題",
    "body": "更新後內容"
  }
}
```

`delete`：

```json
{
  "deleted": true,
  "id": "sticky-1"
}
```

## 架構

新增 adapter 模組：

- `crates/adapter-macos/src/stickies.rs`

責任：

- 建 AppleScript
- 執行 Stickies CRUD
- 將 Stickies app 輸出 decode 成 Rust struct

CLI 端：

- `crates/cli/src/commands/stickies.rs`
- `crates/cli/src/commands/mod.rs`

不把 Stickies 混進 `notes.rs` 或 `quick_notes.rs`：

- 它是不同 app
- 生命週期和資料模型不同
- repo 規則偏向每個功能領域一個模組

## AppleScript 介面

第一版預期模式：

- `tell application "Stickies"`
- 讀取每張 note / window 的 title 與 body
- 針對 create / update / delete 走同一條 app automation 路徑

共用 pattern：

- 使用 `applescript.rs` 裡現有 `escape` / `escape_body` / `run` / `run_capture`
- 若需要 timestamp/date formatting，再抽共用 helper，不複製邏輯

## 錯誤處理

明確區分：

- `stickies not found`
- `sticky not found`
- `sticky id ambiguous`
- `invalid sticky id`
- `create failed`
- `update failed`
- `delete failed`

若 Stickies 沒開啟但 AppleScript 可喚起 app，允許自動喚起。
若系統權限阻擋，錯誤訊息要能讓使用者知道是 Automation / Apple Events 權限問題。

## 測試

至少包含：

- CLI parsing tests：
  - `stickies list`
  - `stickies create`
  - `stickies update`
  - `stickies delete`
- AppleScript builder / parser tests：
  - list script 會抓 title/body
  - update script 至少要求一個更新欄位
  - delete / update 用 `id` 對應目標
- 若實作中有 fallback id/fingerprint，補 unit test 驗證穩定性

第一版不要求：

- 真機整合測試自動化
- 顏色 / 視窗 frame round-trip

## 驗收條件

- `cueward stickies list` 可成功列出至少一張便條紙
- `create / update / delete` 都可用
- 無標題便條紙不會讓流程失敗
- `list` 回來的 `id` 可直接用於 `update` / `delete`
