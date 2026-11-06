//! Adapter-only historical checks. No serving-response or clock-race claim.
use super::*;
use crate::postgres::transaction::{
    ItemHistoryAuthorization, ItemHistoryPreparation, ItemReplayRequest,
};

async fn load(
    admin: &mut Admin,
    a: &Authority,
    deadline: Instant,
    request: ItemReplayRequest,
) -> ItemHistoryPreparation {
    let mut transaction = admin.read_snapshot(a, deadline).await.unwrap();
    let prepared = transaction
        .prepare_item_history(request)
        .await
        .unwrap_or_else(|_| panic!("historical preparation failed"));
    transaction.rollback().await.unwrap();
    prepared
}
async fn saved(
    admin: &mut Admin,
    a: &Authority,
    deadline: Instant,
    request: ItemReplayRequest,
    expected: &SavedResult,
) {
    let ItemHistoryPreparation::Ready(draft) = load(admin, a, deadline, request).await else {
        panic!("historical result not found")
    };
    // Decode is deliberately after confirmed snapshot rollback.
    let packet = draft.admit().unwrap();
    assert_eq!(
        &work_item_codec::decode_result(1, packet.result_bytes()).unwrap(),
        expected
    );
    let transaction = admin.item_replay_transaction(a, deadline).await.unwrap();
    let released = transaction
        .authorize_item_history(packet)
        .await
        .unwrap_or_else(|_| panic!("historical authorization failed"));
    let ItemHistoryAuthorization::Released(release) = released else {
        panic!("live history expired")
    };
    assert_eq!(release.authority().installation, a.installation);
    assert_eq!(release.authority().owner, a.owner);
    assert_eq!(release.authority().runtime, a.runtime);
    assert_eq!(release.authority().authentication, a.authentication);
    assert_eq!(release.authority().revision, a.revision);
}
pub(super) async fn check(
    admin: &mut Admin,
    a: &Authority,
    deadline: Instant,
    project: &str,
    configuration: &Configuration,
    clear_command: Arc<CanonicalItem>,
    created: &SavedResult,
    cleared: &SavedResult,
) {
    let before = evidence::snapshot(admin).await;
    saved(
        admin,
        a,
        deadline,
        ItemReplayRequest::Create {
            project: project.into(),
            handle: "a".repeat(32),
        },
        created,
    )
    .await;
    saved(
        admin,
        a,
        deadline,
        ItemReplayRequest::Edit(clear_command),
        cleared,
    )
    .await;
    // Same token, different exact command cannot load a saved body or execute.
    let changed = edit_command(
        a,
        configuration,
        'c',
        1,
        EstimateEdit::Keep,
        FieldEdit::Keep,
    );
    let ItemHistoryPreparation::Resolved(resolved) =
        load(admin, a, deadline, ItemReplayRequest::Edit(changed)).await
    else {
        panic!("changed exact echo was not resolved")
    };
    assert_eq!(resolved.outcome(), ItemDisposition::Conflict);
    // Unseen reconciliation must stop at metadata, even with impossible current
    // version expectations. It cannot load current state or grant fresh execution.
    let unknown = edit_command(
        a,
        configuration,
        'x',
        u64::MAX,
        EstimateEdit::Keep,
        FieldEdit::Keep,
    );
    let observe = admin.database_observer();
    let mut transaction = admin.read_snapshot(a, deadline).await.unwrap();
    let entered = observe();
    let prepared = transaction
        .prepare_item_history(ItemReplayRequest::Edit(unknown))
        .await
        .unwrap_or_else(|_| panic!("unseen history lookup failed"));
    assert!(matches!(prepared, ItemHistoryPreparation::Absent));
    assert_eq!(
        observe() - entered,
        1,
        "unseen history loaded mutable state"
    );
    transaction.rollback().await.unwrap();
    assert_eq!(
        evidence::snapshot(admin).await,
        before,
        "historical path mutated durable artifacts"
    );
}
