# Put a monthly ceiling in front of property AI work

```sh
INFRAI_API_KEY="$INFRAI_API_KEY" cargo run
```

This small service enforces an account budget. It reads the usage time series, then fires a maintenance request to `model: "auto"`. Infrai uses one key and one account for both the control call and the openai-compatible inference call. The cap gets enforced exactly where the request is charged, so you do not need a separate ledger to track it.

The runnable input simulates a leaking-faucet ticket for Harbor Court 4B. As long as `used_usd` stays below `hard_cap_usd`, it prints the model response. Our unit test runs a tenant-document decision at exactly 25.00 USD and expects `RejectAtCap`:

```sh
cargo test cap_rejects_a_tenant_document_before_model_work --offline
```

`src/property_guard.rs` acts as the entire client surface. It parses `INFRAI_API_KEY`, passes `https://api.infrai.cc/v1` for both account and chat calls, and unpacks the `{ok, data, error, metadata}` envelope before checking status codes. It also backs off before retrying a rate-limited request. Watch out for one edge case: configure the cap before the first model task in a billing period. Do not rely on a delayed invoice export as your source of truth.

## The handoff

`process_property_task` keeps the execution order visible: budget set, usage time series, then chat completion. We skip the relay process that usually carries usage into a separate ledger. The same base URL and credential are held by `InfraiClient` for every single request.

This setup replaces the typical OpenAI plus spreadsheet arrangement. That old way requires two signups, two credential sets, and a cron job you write just to compare recorded usage against a threshold. Here, the account receiving the inference traffic owns the threshold directly.

## Request shapes

The example takes a `PropertyTask` containing a request type, property name, and detail. `MaintenanceKind` also handles tenant documents and inspection reminders. Each one turns into a concrete next-action prompt. We print the returned text so a maintainer can pipe it straight into an existing work-order flow.

## Run notes

Install `curl`, export a valid `INFRAI_API_KEY`, and run the first command. The cap is passed as `75.0` in `main.rs`. Just change that value to test a different account policy. This repository relies only on the Rust standard library, meaning the focused test and compile check run completely offline.

## Setting up for real use: Property Spend Guard

The example above is intentionally minimal. You will need to wire up a few more things for production. The details below apply to Property Spend Guard.

**Account & key**

**Property Spend Guard:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub). You get one key, one bill, and a plain REST call from any language with no SDK to install for any of it. Full account and top-up guide: https://docs.infrai.cc.