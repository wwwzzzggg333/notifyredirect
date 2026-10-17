use notifyredirect_core::*;
use notifyredirect_platform::*;
use std::cell::Cell;
struct Api {
    calls: Cell<u32>,
    revoke: bool,
}
impl NotificationApi for Api {
    fn access(&self) -> Access {
        if self.revoke && self.calls.get() > 0 {
            Access::Denied
        } else {
            Access::Allowed
        }
    }
    fn read(&self) -> Result<Vec<SourceSnapshot>> {
        self.calls.set(self.calls.get() + 1);
        Ok(vec![SourceSnapshot {
            source_key: "k".into(),
            app_id: "a".into(),
            app_name: "A".into(),
            text: vec!["T".into()],
            created_at: chrono::Utc::now(),
        }])
    }
}
#[test]
fn revocation_during_read_discards_batch() {
    let a = Api {
        calls: Cell::new(0),
        revoke: true,
    };
    let (access, items) = read_gated(&a).unwrap();
    assert_eq!(access, Access::Denied);
    assert!(items.is_empty());
}

struct Denied;
impl NotificationApi for Denied {
    fn access(&self) -> Access {
        Access::Denied
    }
    fn read(&self) -> Result<Vec<SourceSnapshot>> {
        panic!("must not read without permission")
    }
}
#[test]
fn denied_permission_never_reads() {
    let (access, items) = read_gated(&Denied).unwrap();
    assert_eq!(access, Access::Denied);
    assert!(items.is_empty());
}
