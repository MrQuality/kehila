use task_contract::project::EstimateUnit;
use task_contract::project_create::*;
use task_contract::work_item::ProjectId;
fn json_string(value: &str) -> String {
    let mut quoted = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            character if character.is_control() => {
                quoted.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}
fn command_from_args(args: &[String]) -> Result<ProjectCreateCommand, &'static str> {
    if args.len() != 6 {
        return Err("Expected: project_id operation_id name prefix estimate_unit");
    }
    let estimate_unit = match args[5].as_str() {
        "hours" => EstimateUnit::Hours,
        "points" => EstimateUnit::Points,
        _ => return Err("estimate_unit must be hours or points"),
    };
    Ok(ProjectCreateCommand {
        project_id: ProjectId(args[1].clone()),
        operation_id: args[2].clone(),
        name: args[3].clone(),
        prefix: args[4].clone(),
        estimate_unit,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = command_from_args(&args).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let ProjectCreateDecision::Apply(result) = decide_project_create(false, None, true, &cmd)
    else {
        panic!("fixture rejected")
    };
    assert_eq!(result.configuration.validate(), Ok(()));
    let project = &result.project;
    let configuration = &result.configuration;
    let statuses = configuration
        .statuses
        .iter()
        .map(|status| {
            format!(
                "{{\"id\":{},\"name\":{},\"group_id\":null,\"phase\":{},\"archived\":{}}}",
                json_string(&status.id.0),
                json_string(&status.name),
                json_string(&format!("{:?}", status.phase).to_lowercase()),
                status.archived
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let workflows = configuration
        .workflows
        .iter()
        .map(|workflow| {
            let status_ids = workflow
                .status_ids
                .iter()
                .map(|status_id| json_string(&status_id.0))
                .collect::<Vec<_>>()
                .join(",");
            let phase_changes = workflow
                .permitted_phase_changes
                .iter()
                .map(|change| {
                    format!(
                        "[{},{}]",
                        json_string(&format!("{:?}", change.from).to_lowercase()),
                        json_string(&format!("{:?}", change.to).to_lowercase())
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                concat!(
                    "{{\"id\":{},\"initial_status_id\":{},\"archived\":{},",
                    "\"status_ids\":[{}],\"permitted_phase_changes\":[{}]}}"
                ),
                json_string(&workflow.id.0),
                json_string(&workflow.initial_status_id.0),
                workflow.archived,
                status_ids,
                phase_changes
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let types = configuration
        .types
        .iter()
        .map(|item_type| {
            let permitted_workflows = item_type
                .permitted_workflows
                .iter()
                .map(|workflow_id| json_string(&workflow_id.0))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                concat!(
                    "{{\"id\":{},\"default_workflow_id\":{},\"title_field_id\":{},",
                    "\"archived\":{},\"permitted_workflows\":[{}]}}"
                ),
                json_string(&item_type.id.0),
                json_string(&item_type.default_workflow_id.0),
                json_string(&item_type.title_field_id.as_ref().unwrap().0),
                item_type.archived,
                permitted_workflows
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let fields = configuration
        .fields
        .iter()
        .map(|field| {
            assert!(field.options.is_empty());
            format!(
                concat!(
                    "{{\"id\":{},\"owner_type\":{},\"name\":{},\"kind\":{},",
                    "\"origin\":{},\"usage\":{},\"archived\":{},\"options\":[]}}"
                ),
                json_string(&field.id.0),
                json_string(&field.owner_type.0),
                json_string(&field.name),
                json_string(&format!("{:?}", field.kind).to_lowercase()),
                json_string(&format!("{:?}", field.origin).to_lowercase()),
                json_string(&format!("{:?}", field.usage).to_lowercase()),
                field.archived
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    assert!(configuration.status_groups.is_empty() && configuration.relationship_types.is_empty());
    println!(
        concat!(
            "{{\"seed_profile\":\"m1_v1\"",
            ",\"project\":{{\"id\":{}",
            ",\"name\":{}",
            ",\"prefix\":{}",
            ",\"configuration_revision\":{}",
            ",\"next_sequence\":{}",
            ",\"archived\":{}",
            ",\"ever_estimated\":{}",
            ",\"estimate_unit\":{}}}",
            ",\"configuration\":{{\"project_id\":{}",
            ",\"revision\":{}",
            ",\"project_archived\":{}",
            ",\"statuses\":[{}]",
            ",\"status_groups\":[]",
            ",\"workflows\":[{}]",
            ",\"types\":[{}]",
            ",\"fields\":[{}]",
            ",\"relationship_types\":[]}}}}"
        ),
        json_string(&project.id.0),
        json_string(&project.name),
        json_string(&project.prefix),
        project.configuration_revision,
        project.next_sequence,
        project.archived,
        project.ever_estimated,
        json_string(&format!("{:?}", project.estimate_unit).to_lowercase()),
        json_string(&configuration.project_id.0),
        configuration.revision,
        configuration.project_archived,
        statuses,
        workflows,
        types,
        fields
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(unit: &str) -> Vec<String> {
        [
            "seed_oracle",
            "project",
            "operation",
            "QA Project",
            "QA",
            unit,
        ]
        .iter()
        .map(|value| value.to_string())
        .collect()
    }

    #[test]
    fn accepts_both_supported_units() {
        assert!(matches!(
            command_from_args(&args("hours")).unwrap().estimate_unit,
            EstimateUnit::Hours
        ));
        assert!(matches!(
            command_from_args(&args("points")).unwrap().estimate_unit,
            EstimateUnit::Points
        ));
    }

    #[test]
    fn rejects_unknown_unit_and_wrong_argument_count() {
        for unit in ["", "minutes", "Hours", "point"] {
            assert!(command_from_args(&args(unit)).is_err());
        }
        assert!(command_from_args(&[]).is_err());
        let mut extra = args("hours");
        extra.push("unexpected".to_string());
        assert!(command_from_args(&extra).is_err());
    }
}
