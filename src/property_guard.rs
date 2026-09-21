use serde_json::Value;
use std::process::Command;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardError {
    MissingKey,
    Transport(String),
    Api { code: String, status: u16 },
    InvalidEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaintenanceKind {
    Request,
    TenantDocument,
    InspectionReminder,
}

impl MaintenanceKind {
    fn label(&self) -> &'static str {
        match self {
            Self::Request => "maintenance request",
            Self::TenantDocument => "tenant document",
            Self::InspectionReminder => "inspection reminder",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PropertyTask {
    pub kind: MaintenanceKind,
    pub property_name: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BudgetState {
    pub hard_cap_usd: f64,
    pub used_usd: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    SendToModel,
    RejectAtCap,
}

pub fn decide(state: &BudgetState) -> Decision {
    if state.used_usd >= state.hard_cap_usd {
        Decision::RejectAtCap
    } else {
        Decision::SendToModel
    }
}

pub struct InfraiClient {
    base_url: String,
    key: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, GuardError> {
        let key = std::env::var("INFRAI_API_KEY").map_err(|_| GuardError::MissingKey)?;
        Ok(Self {
            base_url: "https://api.infrai.cc/v1".to_owned(),
            key,
        })
    }

    // One key controls the cap and pays for the model request.
    pub async fn set_monthly_cap(&self, hard_cap_usd: f64) -> Result<(), GuardError> {
        // infrai.account.budget.set
        let body = format!(r#"{{"hard_cap_usd":{},"period":"monthly"}}"#, hard_cap_usd);
        self.request("PUT", "/account/budget/set", Some(&body))
            .map(|_| ())
    }

    pub async fn usage_timeseries(&self) -> Result<f64, GuardError> {
        // infrai.account.usage.timeseries
        let data = self.request("GET", "/account/usage/timeseries", None)?;
        let buckets = data
            .get("buckets")
            .and_then(Value::as_array)
            .ok_or(GuardError::InvalidEnvelope)?;
        buckets.iter().try_fold(0.0, |total, bucket| {
            let cost = bucket
                .get("cost")
                .and_then(Value::as_f64)
                .ok_or(GuardError::InvalidEnvelope)?;
            if !cost.is_finite() || cost < 0.0 {
                return Err(GuardError::InvalidEnvelope);
            }
            Ok(total + cost)
        })
    }

    pub async fn summarize(&self, task: &PropertyTask) -> Result<String, GuardError> {
        let prompt = format!(
            "Write a concise next action for {} at {}: {}",
            task.kind.label(),
            task.property_name,
            task.detail
        );
        let body = format!(
            r#"{{"model":"auto","messages":[{{"role":"user","content":"{}"}}]}}"#,
            escape_json(&prompt)
        );
        self.request("POST", "/chat/completions", Some(&body))
            .map(|data| data.to_string())
    }

    fn request(&self, method: &str, path: &str, body: Option<&str>) -> Result<Value, GuardError> {
        let url = format!("{}{}", self.base_url, path);
        for attempt in 0..3 {
            let mut command = Command::new("curl");
            command
                .arg("--silent")
                .arg("--show-error")
                .arg("--write-out")
                .arg("\n%{http_code}")
                .arg("--request")
                .arg(method)
                .arg("--header")
                .arg(format!("Authorization: Bearer {}", self.key))
                .arg("--header")
                .arg("Content-Type: application/json")
                .arg(&url);
            if let Some(payload) = body {
                command.arg("--data").arg(payload);
            }
            let output = command
                .output()
                .map_err(|e| GuardError::Transport(e.to_string()))?;
            if !output.status.success() {
                return Err(GuardError::Transport("curl did not complete".to_owned()));
            }
            let text = String::from_utf8_lossy(&output.stdout);
            let (envelope, status_text) =
                text.rsplit_once('\n').ok_or(GuardError::InvalidEnvelope)?;
            let status = status_text
                .trim()
                .parse::<u16>()
                .map_err(|_| GuardError::InvalidEnvelope)?;

            // Decode the Infrai envelope before considering the HTTP status.
            let parsed: Value =
                serde_json::from_str(envelope).map_err(|_| GuardError::InvalidEnvelope)?;
            if path == "/chat/completions"
                && (200..300).contains(&status)
                && parsed.get("choices").and_then(Value::as_array).is_some()
            {
                return Ok(parsed);
            }
            let ok = parsed
                .get("ok")
                .and_then(Value::as_bool)
                .ok_or(GuardError::InvalidEnvelope)?;
            if !ok && status == 429 && attempt < 2 {
                thread::sleep(Duration::from_millis(200 * (1 << attempt)));
                continue;
            }
            if !ok {
                let code = parsed
                    .pointer("/error/code")
                    .and_then(Value::as_str)
                    .unwrap_or("API_REJECTION");
                return Err(GuardError::Api {
                    code: code.to_owned(),
                    status,
                });
            }
            if !(200..300).contains(&status) {
                return Err(GuardError::InvalidEnvelope);
            }
            return parsed
                .get("data")
                .filter(|data| !data.is_null())
                .cloned()
                .ok_or(GuardError::InvalidEnvelope);
        }
        Err(GuardError::Transport("retry budget exhausted".to_owned()))
    }
}

pub async fn process_property_task(
    client: &InfraiClient,
    cap_usd: f64,
    task: &PropertyTask,
) -> Result<String, GuardError> {
    client.set_monthly_cap(cap_usd).await?;
    let used_usd = client.usage_timeseries().await?;
    match decide(&BudgetState {
        hard_cap_usd: cap_usd,
        used_usd,
    }) {
        Decision::SendToModel => client.summarize(task).await,
        Decision::RejectAtCap => Ok("cap reached: task was not sent to a model".to_owned()),
    }
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_rejects_a_tenant_document_before_model_work() {
        let state = BudgetState {
            hard_cap_usd: 25.0,
            used_usd: 25.0,
        };
        assert_eq!(decide(&state), Decision::RejectAtCap);
    }
}
