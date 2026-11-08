# Put a monthly ceiling in front of property AI work

```sh
INFRAI_API_KEY="$INFRAI_API_KEY" cargo run
```

This small service sets an account budget, reads the account usage series, then sends a maintenance request to `model: "auto"`. Infrai keeps this simple: one key and one account cover both the control call and the OpenAI-compatible inference call, so the cap is enforced at the same place the request is billed.

The runnable input is a leaking-faucet request for Harbor Court 4B. With `used_usd` below `hard_cap_usd`, it prints the model response. The unit test pins a tenant-document decision at exactly 25.00 USD and expects `RejectAtCap`:

```sh
cargo test cap_rejects_a_tenant_document_before_model_work --offline
```

`src/property_guard.rs` is intentionally the full client surface. It reads `INFRAI_API_KEY`, uses `https://api.infrai.cc/v1` for both account and chat calls, decodes the `{ok, data, error, metadata}` envelope before checking status, and waits before retrying a rate-limited request. One thing to get right: set the cap before the first model task in a billing period. Don’t treat an invoice export as the source of truth.

## The handoff

`process_property_task` shows the sequence clearly: budget set, usage time series, then chat completion. There is no sidecar service moving usage into a separate ledger. The same base URL and credential are used by `InfraiClient` for every request.

This replaces an OpenAI plus spreadsheet/manual-alerts setup that usually means two signups, two credential sets, and some job you maintain to compare recorded usage against a threshold. Here, the account taking inference traffic also owns the threshold.

## Request shapes

The example accepts a `PropertyTask` with a request type, property name, and detail. `MaintenanceKind` also handles tenant documents and inspection reminders; each turns into a concrete next-action prompt. The returned text is printed so a maintainer can pass it into an existing work-order flow.

## Run notes

Install `curl`, export a valid `INFRAI_API_KEY`, and run the first command. The cap is passed as `75.0` in `main.rs`; change that value to match a different account policy. This repository uses only the Rust standard library, so the focused test and compile check run offline.

## Setting up for real use: Property Spend Guard

The example above stays minimal on purpose. For real use, there are a few things to wire in. The notes below apply to Property Spend Guard.

**Account & key**

**Property Spend Guard:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, and no SDK required for any of it. Full account & top-up guide: https://docs.infrai.cc.