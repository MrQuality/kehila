use project_create_codec::*;
use task_contract::project::EstimateUnit;
use task_contract::project_create::*;
use task_contract::work_item::ProjectId;

fn result() -> ProjectCreateResult {
    let command = ProjectCreateCommand {
        operation_id: "snapshot-test".into(),
        project_id: ProjectId("snapshot-test".into()),
        name: "Snapshot test".into(),
        prefix: "ST".into(),
        estimate_unit: EstimateUnit::Hours,
    };
    match decide_project_create(false, None, true, &command) {
        ProjectCreateDecision::Apply(result) => result,
        _ => panic!("fixture must be an accepted result"),
    }
}

#[test]
fn revision_pair_checks_fixed_relationships_separately_from_decode() {
    let r = result();
    let project = encode_project_snapshot(&r.project).unwrap();
    let configuration = encode_snapshot(&r.configuration).unwrap();
    assert_eq!(
        verify_revision_snapshots(&r.project.id, 1, 1, &project, &configuration),
        Ok((r.project.clone(), r.configuration.clone()))
    );
    assert!(
        verify_revision_snapshots(&ProjectId("wrong".into()), 1, 1, &project, &configuration)
            .is_err()
    );
    assert!(verify_revision_snapshots(&r.project.id, 2, 1, &project, &configuration).is_err());
    assert_eq!(
        verify_revision_snapshots(&r.project.id, 1, 2, &project, &configuration),
        Err(Error::UnsupportedVersion(2))
    );
    for change in 0..5 {
        let mut bad = r.clone();
        match change {
            0 => bad.project.id.0 = "wrong".into(),
            1 => bad.configuration.project_id.0 = "wrong".into(),
            2 => bad.project.configuration_revision = 2,
            3 => bad.configuration.revision = 2,
            _ => bad.configuration.project_archived = true,
        }
        let p = encode_project_snapshot(&bad.project).unwrap();
        let c = encode_snapshot(&bad.configuration).unwrap();
        assert!(decode_project_snapshot(1, &p).is_ok());
        assert!(decode_snapshot(1, &c).is_ok());
        assert!(verify_revision_snapshots(&r.project.id, 1, 1, &p, &c).is_err());
        assert!(encode_native_revision_snapshots(&bad).is_err());
    }
    let mut old = r.clone();
    old.project.id.0.clear();
    old.configuration.project_id.0.clear();
    old.project.configuration_revision = 0;
    old.configuration.revision = 0;
    old.project.next_sequence = 999;
    old.project.ever_estimated = true;
    assert!(verify_revision_snapshots(
        &old.project.id,
        0,
        1,
        &encode_project_snapshot(&old.project).unwrap(),
        &encode_snapshot(&old.configuration).unwrap()
    )
    .is_ok());
}

#[test]
fn native_revision_admission_rejects_nul_without_changing_historical_codecs() {
    let r = result();
    let admitted = encode_native_revision_snapshots(&r).unwrap();
    assert_eq!(
        verify_revision_snapshots(
            &r.project.id,
            1,
            admitted.codec_version(),
            admitted.project(),
            admitted.configuration()
        ),
        Ok((r.project.clone(), r.configuration.clone()))
    );
    for change in 0..5 {
        let mut bad = r.clone();
        match change {
            0 => bad.project.name.push('\0'),
            1 => bad.project.prefix.push('\0'),
            2 => {
                bad.project.id.0.push('\0');
                bad.configuration.project_id.0.push('\0');
            }
            3 => bad.configuration.statuses[0].name.push('\0'),
            _ => bad.configuration.fields[0].id.0.push('\0'),
        }
        assert!(encode_project_snapshot(&bad.project).is_ok());
        assert!(encode_snapshot(&bad.configuration).is_ok());
        assert_eq!(
            encode_native_revision_snapshots(&bad).err(),
            Some(Error::InvalidNativeSnapshot("JSONB cannot store NUL"))
        );
    }
    let mut literal = r;
    literal.project.name = r"literal \u0000 is not NUL".into();
    assert!(encode_native_revision_snapshots(&literal).is_ok());
}

#[test]
fn native_admission_reserves_postgresql_formatting_space_at_the_byte_boundary() {
    let mut r = result();
    // Enough strings to approach 1 MiB without violating individual string or node bounds.
    r.configuration.statuses = vec![r.configuration.statuses[0].clone(); 16];
    for status in &mut r.configuration.statuses {
        status.name = "x".repeat(65_500);
    }
    // This frozen 16-status shape adds 283 structural JSONB spaces. The native
    // bridge independently checks PostgreSQL's actual text output at the ceiling.
    r.configuration.statuses[15].name.clear();
    let base = encode_snapshot(&r.configuration).unwrap().len();
    let padding = MAX_RECORD_BYTES - base - 283;
    assert!(padding <= MAX_STRING_BYTES);
    r.configuration.statuses[15].name = "x".repeat(padding);
    assert!(encode_native_revision_snapshots(&r).is_ok());
    r.configuration.statuses[15].name.push('x');
    assert!(encode_snapshot(&r.configuration).is_ok());
    assert_eq!(
        encode_native_revision_snapshots(&r).err(),
        Some(Error::LimitExceeded)
    );
}
