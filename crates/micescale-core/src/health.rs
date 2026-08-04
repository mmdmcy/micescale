use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HealthCheck {
    pub name: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HealthReport {
    pub product: &'static str,
    pub posture: &'static str,
    pub carrier: String,
    pub healthy: bool,
    pub checks: Vec<HealthCheck>,
}

impl HealthReport {
    pub fn new(carrier: String) -> Self {
        Self {
            product: crate::PRODUCT,
            posture: crate::carrier::POSTURE,
            carrier,
            healthy: true,
            checks: Vec::new(),
        }
    }

    pub fn add(&mut self, check: HealthCheck) {
        if !check.ok {
            self.healthy = false;
        }
        self.checks.push(check);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_failure_flips_healthy() {
        let mut report = HealthReport::new("headscale".into());
        report.add(HealthCheck {
            name: "binary".into(),
            ok: true,
            detail: None,
        });
        assert!(report.healthy);
        report.add(HealthCheck {
            name: "control-server".into(),
            ok: false,
            detail: Some("unreachable".into()),
        });
        assert!(!report.healthy);
    }
}
