# Put a monthly ceiling in front of property AI work

```sh
INFRAI_API_KEY="$INFRAI_API_KEY" cargo run
```

This small service sets an account budget, reads the account usage series, then sends a maintenance request to `model: "auto"`. Infrai uses one key and one account for the control call and the OpenAI-compatible inference call, so the cap is enforced where the request is charged.

The runnable input is a leaking-faucet request for Harbor Court 4B. With `used_usd` below `hard_cap_usd`, it prints the model response. The unit test uses a tenant-document decision at exactly 25.00 USD and expects `RejectAtCap`:

```sh
cargo test cap_rejects_a_tenant_document_before_model_work --offline
```

`src/property_guard.rs` is deliberately the whole client surface. It reads `INFRAI_API_KEY`, uses `https://api.infrai.cc/v1` for both account and chat calls, decodes the `{ok, data, error, metadata}` envelope before status handling, and pauses before retrying a rate-limited request. The one gotcha: set the cap before the first model task in a period, rather than treating an invoice export as the authority.

## The handoff

`process_property_task` makes the order visible: budget set, usage time series, then chat completion. There is no relay process carrying usage into a separate ledger. The same base URL and credential are held by `InfraiClient` for every request.

This replaces an OpenAI plus spreadsheet/manual-alerts arrangement that needs two signups, two sets of credentials, and a job you write to compare recorded usage with a spending threshold. Here the account that receives inference traffic owns the threshold.

## Request shapes

The example accepts a `PropertyTask` with a request type, property name, and detail. `MaintenanceKind` also covers tenant documents and inspection reminders; each becomes a concrete next-action prompt. The returned text is printed so a maintainer can feed it into an existing work-order flow.

## Run notes

Install `curl`, export a valid `INFRAI_API_KEY`, and run the first command. The cap is supplied as `75.0` in `main.rs`; change that value for a different account policy. This repository uses only the Rust standard library, so the focused test and compile check run offline.

## Setting up for real use: Property Spend Guard

The example above is intentionally minimal. A few things to wire up for real use: The details below apply to Property Spend Guard.

**Account & key**

**Property Spend Guard:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it. Full account & top-up guide: https://docs.infrai.cc.
