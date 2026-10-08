//! Persistence boundary. Domain transitions stay in dashboard-core.
use dashboard_core::Store;
use std::path::Path;

pub trait Repository {
    /// Return one consistent committed snapshot without a writer lock.
    fn read(&self) -> Result<Store, String>;
    /// Serialize read-modify-write operations until commit or drop.
    fn begin(&self) -> Result<UnitOfWork, String>;
}

pub trait TransactionBackend {
    fn commit(&mut self, store: &Store) -> Result<(), String>;
}

pub struct UnitOfWork {
    pub store: Store,
    backend: Box<dyn TransactionBackend>,
    committed: bool,
}

impl UnitOfWork {
    pub fn new(store: Store, backend: Box<dyn TransactionBackend>) -> Self {
        Self {
            store,
            backend,
            committed: false,
        }
    }

    /// A unit of work can commit once. Dropping without commit rolls back.
    pub fn commit(&mut self) -> Result<(), String> {
        if self.committed {
            return Err("transaction already committed".into());
        }
        self.backend.commit(&self.store)?;
        self.committed = true;
        Ok(())
    }
}

/// Composition root: callers depend on Repository, not a storage format.
pub fn at(dir: &Path) -> Box<dyn Repository> {
    Box::new(crate::sqlite_repository::SqliteRepository(dir.to_owned()))
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    struct MemoryRepository(Rc<RefCell<Store>>);
    struct MemoryTransaction(Rc<RefCell<Store>>);
    impl TransactionBackend for MemoryTransaction {
        fn commit(&mut self, store: &Store) -> Result<(), String> {
            *self.0.borrow_mut() = store.clone();
            Ok(())
        }
    }
    impl Repository for MemoryRepository {
        fn read(&self) -> Result<Store, String> {
            Ok(self.0.borrow().clone())
        }
        fn begin(&self) -> Result<UnitOfWork, String> {
            Ok(UnitOfWork::new(
                self.read()?,
                Box::new(MemoryTransaction(self.0.clone())),
            ))
        }
    }

    #[test]
    fn host_request_lookup_uses_an_injected_repository_without_files() {
        use dashboard_core::{Action, ActionRequest, RequestState};
        let repo = MemoryRepository(Rc::new(RefCell::new(Store::default())));
        let mut tx = repo.begin().unwrap();
        let request = ActionRequest {
            request_id: "memory".into(),
            action: Action::Launch {
                session: "dev".into(),
                epoch: "epoch".into(),
                cwd: "/tmp".into(),
                tool: "codex".into(),
            },
        };
        tx.store.claim(&request, 10).unwrap();
        assert!(crate::actions::result(&repo, "memory").is_err());
        tx.commit().unwrap();
        assert_eq!(
            crate::actions::result(&repo, "memory").unwrap().state,
            RequestState::Pending
        );
        assert!(!repo.begin().unwrap().store.claim(&request, 20).unwrap());
    }
}
