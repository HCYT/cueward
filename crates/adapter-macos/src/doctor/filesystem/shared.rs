use std::fs;
use std::io::ErrorKind;

use rusqlite::{Connection, OpenFlags};

use crate::doctor::{DoctorCheck, DoctorCheckStatus};
use crate::error::{FULL_DISK_ACCESS_GUIDANCE, FULL_DISK_ACCESS_SETTINGS_PATH};

use super::targets::{ProbeKind, ProbeTarget};

pub(super) fn probe_target(target: ProbeTarget) -> DoctorCheck {
    match target.kind {
        ProbeKind::Directory => probe_directory(target),
        ProbeKind::Sqlite => probe_sqlite(target),
    }
}

fn probe_directory(target: ProbeTarget) -> DoctorCheck {
    let path = target.absolute_path();
    if !path.exists() {
        return missing_check(target);
    }

    match fs::read_dir(&path) {
        Ok(_) => pass_check(target, "readable"),
        Err(err) if is_permission_denied_io_error(&err) => permission_denied_check(target),
        Err(err) => warn_check(target, format!("failed to read directory: {err}")),
    }
}

fn probe_sqlite(target: ProbeTarget) -> DoctorCheck {
    let path = target.absolute_path();
    if !path.exists() {
        return missing_check(target);
    }

    match Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(conn) => match conn.prepare("SELECT 1") {
            Ok(_) => pass_check(target, "readable"),
            Err(err) if is_permission_denied_sqlite_error(&err.to_string()) => {
                permission_denied_check(target)
            }
            Err(err) => warn_check(target, format!("failed to prepare sqlite probe: {err}")),
        },
        Err(err) if is_permission_denied_sqlite_error(&err.to_string()) => {
            permission_denied_check(target)
        }
        Err(err) => warn_check(target, format!("failed to open sqlite target: {err}")),
    }
}

fn pass_check(target: ProbeTarget, message: &str) -> DoctorCheck {
    DoctorCheck {
        id: target.id.to_string(),
        status: DoctorCheckStatus::Pass,
        target: target.display_path(),
        message: message.to_string(),
        fix: None,
        required: target.required,
    }
}

fn missing_check(target: ProbeTarget) -> DoctorCheck {
    DoctorCheck {
        id: target.id.to_string(),
        status: target.missing_status,
        target: target.display_path(),
        message: "target not found".to_string(),
        fix: None,
        required: target.required,
    }
}

fn permission_denied_check(target: ProbeTarget) -> DoctorCheck {
    DoctorCheck {
        id: target.id.to_string(),
        status: DoctorCheckStatus::Fail,
        target: target.display_path(),
        message: "permission denied".to_string(),
        fix: Some(full_disk_access_fix()),
        required: target.required,
    }
}

fn warn_check(target: ProbeTarget, message: String) -> DoctorCheck {
    DoctorCheck {
        id: target.id.to_string(),
        status: DoctorCheckStatus::Warn,
        target: target.display_path(),
        message,
        fix: None,
        required: target.required,
    }
}

fn full_disk_access_fix() -> String {
    format!("{FULL_DISK_ACCESS_GUIDANCE}: {FULL_DISK_ACCESS_SETTINGS_PATH}")
}

fn is_permission_denied_io_error(err: &std::io::Error) -> bool {
    err.kind() == ErrorKind::PermissionDenied
}

fn is_permission_denied_sqlite_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("unable to open")
        || normalized.contains("permission denied")
        || normalized.contains("operation not permitted")
        || normalized.contains("access denied")
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::doctor::DoctorCheckStatus;

    use super::super::targets::{ProbeKind, ProbeTarget};
    use super::{is_permission_denied_sqlite_error, probe_target};

    fn temp_target(
        absolute_path: &std::path::Path,
        kind: ProbeKind,
        missing_status: DoctorCheckStatus,
    ) -> ProbeTarget {
        let relative_path = absolute_path.display().to_string();
        let leaked = Box::leak(relative_path.into_boxed_str());
        ProbeTarget {
            id: "test.target",
            relative_path: leaked,
            kind,
            required: true,
            missing_status,
        }
    }

    #[test]
    fn sqlite_probe_reports_pass_for_readable_database() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("sample.db");
        Connection::open(&db_path).expect("create sqlite db");
        let target = temp_target(&db_path, ProbeKind::Sqlite, DoctorCheckStatus::Warn);

        let check = probe_target(target);

        assert_eq!(check.status, DoctorCheckStatus::Pass);
    }

    #[test]
    fn directory_probe_reports_missing_status_when_target_is_absent() {
        let temp = tempfile::tempdir().expect("tempdir");
        let target = temp_target(
            &temp.path().join("missing"),
            ProbeKind::Directory,
            DoctorCheckStatus::Skip,
        );

        let check = probe_target(target);

        assert_eq!(check.status, DoctorCheckStatus::Skip);
        assert_eq!(check.message, "target not found");
    }

    #[test]
    fn sqlite_permission_detection_matches_expected_errors() {
        assert!(is_permission_denied_sqlite_error(
            "unable to open database file"
        ));
        assert!(is_permission_denied_sqlite_error("Operation not permitted"));
        assert!(!is_permission_denied_sqlite_error("file is not a database"));
    }
}
