use task_contract::project::EstimateUnit;
use task_contract::project_create::*;
use task_contract::work_item::ProjectId;
fn q(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if c.is_control() => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let cmd = ProjectCreateCommand {
        project_id: ProjectId(a[1].clone()),
        operation_id: a[2].clone(),
        name: a[3].clone(),
        prefix: a[4].clone(),
        estimate_unit: if a[5] == "hours" {
            EstimateUnit::Hours
        } else {
            EstimateUnit::Points
        },
    };
    let ProjectCreateDecision::Apply(r) = decide_project_create(false, None, true, &cmd) else {
        panic!("fixture rejected")
    };
    assert_eq!(r.configuration.validate(), Ok(()));
    let p = &r.project;
    let c = &r.configuration;
    let statuses = c
        .statuses
        .iter()
        .map(|s| {
            format!(
                "{{\"id\":{},\"name\":{},\"group_id\":null,\"phase\":{},\"archived\":{}}}",
                q(&s.id.0),
                q(&s.name),
                q(&format!("{:?}", s.phase).to_lowercase()),
                s.archived
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let workflows=c.workflows.iter().map(|w|format!("{{\"id\":{},\"initial_status_id\":{},\"archived\":{},\"status_ids\":[{}],\"permitted_phase_changes\":[{}]}}",q(&w.id.0),q(&w.initial_status_id.0),w.archived,w.status_ids.iter().map(|x|q(&x.0)).collect::<Vec<_>>().join(","),w.permitted_phase_changes.iter().map(|e|format!("[{},{}]",q(&format!("{:?}",e.from).to_lowercase()),q(&format!("{:?}",e.to).to_lowercase()))).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(",");
    let types=c.types.iter().map(|t|format!("{{\"id\":{},\"default_workflow_id\":{},\"title_field_id\":{},\"archived\":{},\"permitted_workflows\":[{}]}}",q(&t.id.0),q(&t.default_workflow_id.0),q(&t.title_field_id.as_ref().unwrap().0),t.archived,t.permitted_workflows.iter().map(|x|q(&x.0)).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(",");
    let fields=c.fields.iter().map(|f|{assert!(f.options.is_empty());format!("{{\"id\":{},\"owner_type\":{},\"name\":{},\"kind\":{},\"origin\":{},\"usage\":{},\"archived\":{},\"options\":[]}}",q(&f.id.0),q(&f.owner_type.0),q(&f.name),q(&format!("{:?}",f.kind).to_lowercase()),q(&format!("{:?}",f.origin).to_lowercase()),q(&format!("{:?}",f.usage).to_lowercase()),f.archived)}).collect::<Vec<_>>().join(",");
    assert!(c.status_groups.is_empty() && c.relationship_types.is_empty());
    println!("{{\"seed_profile\":\"m1_v1\",\"project\":{{\"id\":{},\"name\":{},\"prefix\":{},\"configuration_revision\":{},\"next_sequence\":{},\"archived\":{},\"ever_estimated\":{},\"estimate_unit\":{}}},\"configuration\":{{\"project_id\":{},\"revision\":{},\"project_archived\":{},\"statuses\":[{}],\"status_groups\":[],\"workflows\":[{}],\"types\":[{}],\"fields\":[{}],\"relationship_types\":[]}}}}",q(&p.id.0),q(&p.name),q(&p.prefix),p.configuration_revision,p.next_sequence,p.archived,p.ever_estimated,q(&format!("{:?}",p.estimate_unit).to_lowercase()),q(&c.project_id.0),c.revision,c.project_archived,statuses,workflows,types,fields);
}
