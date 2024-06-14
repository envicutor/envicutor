use rusqlite::Connection;

use crate::globals::DB_PATH;

pub struct Transaction<F>
where
    F: FnOnce(Connection) + Clone + Send + 'static,
{
    pub rollback_fn: F,
}

impl<T> Drop for Transaction<T>
where
    T: FnOnce(Connection) + Clone + Send + 'static,
{
    fn drop(&mut self) {
        let rollback_fn = self.rollback_fn.clone();
        tokio::spawn(async move {
            let res = tokio::task::spawn_blocking(move || {
                let connection = Connection::open(DB_PATH);
                match connection {
                    Ok(connection) => {
                        rollback_fn(connection);
                    }
                    Err(e) => {
                        eprintln!("Failed to open SQLite connection: {e}");
                    }
                }
            })
            .await;
            if let Err(e) = res {
                eprintln!("Failed to spawn async task: {e}");
            }
        });
    }
}