use super::filesystem;
use super::{DoctorCheck, DoctorCheckStatus, DoctorReport};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DoctorOptions {
    pub live_safari: bool,
}

pub fn run_doctor(options: DoctorOptions) -> DoctorReport {
    let checks = collect_checks(options);
    DoctorReport {
        ok: report_ok(&checks),
        checks,
    }
}

fn collect_checks(_options: DoctorOptions) -> Vec<DoctorCheck> {
    filesystem::run_checks()
}

fn report_ok(checks: &[DoctorCheck]) -> bool {
    !checks
        .iter()
        .any(|check| check.required && check.status == DoctorCheckStatus::Fail)
}

#[cfg(test)]
mod tests {
    use super::{DoctorCheck, DoctorCheckStatus, DoctorOptions, report_ok, run_doctor};

    fn make_check(status: DoctorCheckStatus, required: bool) -> DoctorCheck {
        DoctorCheck {
            id: "test.check".to_string(),
            status,
            target: "target".to_string(),
            message: "message".to_string(),
            fix: Some("fix".to_string()),
            required,
        }
    }

    #[test]
    fn run_doctor_includes_filesystem_check_ids() {
        let report = run_doctor(DoctorOptions::default());

        let ids: Vec<&str> = report.checks.iter().map(|check| check.id.as_str()).collect();

        assert!(ids.contains(&"fda.messages.chat_db"));
        assert!(ids.contains(&"fda.safari.history_db"));
    }

    #[test]
    fn report_ok_is_false_when_required_check_fails() {
        let checks = vec![make_check(DoctorCheckStatus::Fail, true)];

        assert!(!report_ok(&checks));
    }

    #[test]
    fn report_ok_ignores_non_required_failures() {
        let checks = vec![make_check(DoctorCheckStatus::Fail, false)];

        assert!(report_ok(&checks));
    }

    #[test]
    fn report_ok_ignores_warn_and_skip_checks() {
        let checks = vec![
            make_check(DoctorCheckStatus::Warn, true),
            make_check(DoctorCheckStatus::Skip, true),
        ];

        assert!(report_ok(&checks));
    }
}
