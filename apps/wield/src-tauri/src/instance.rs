use std::sync::Arc;

use zbus::fdo::{RequestNameFlags, RequestNameReply};

pub const BUS_NAME: &str = "io.github.DhanushSantosh.Wield";
pub const OBJECT_PATH: &str = "/io/github/DhanushSantosh/Wield";

#[async_trait::async_trait]
pub trait ShellHandle: Send + Sync + 'static {
    fn show_palette(&self);
    async fn run_tool(&self, id: &str, args_json: &str) -> String;
}

pub enum Acquired {
    /// `Some` keeps the owned bus name and object server alive. `None` means
    /// the session bus was unavailable, so startup is intentionally degraded.
    Primary(Option<zbus::Connection>),
    Secondary,
}

struct WieldService {
    handle: Arc<dyn ShellHandle>,
}

#[zbus::interface(name = "io.github.DhanushSantosh.Wield")]
impl WieldService {
    async fn show_palette(&self) {
        self.handle.show_palette();
    }

    async fn run_tool(&self, id: &str, args_json: &str) -> String {
        self.handle.run_tool(id, args_json).await
    }
}

pub async fn acquire(handle: Arc<dyn ShellHandle>, run: Option<(String, String)>) -> Acquired {
    let connection = match zbus::Connection::session().await {
        Ok(connection) => connection,
        Err(error) => {
            tracing::warn!(%error, "session bus unavailable; running without single-instance service");
            return Acquired::Primary(None);
        }
    };
    acquire_on(connection, handle, run).await
}

async fn acquire_on(
    connection: zbus::Connection,
    handle: Arc<dyn ShellHandle>,
    run: Option<(String, String)>,
) -> Acquired {
    if let Err(error) = connection
        .object_server()
        .at(OBJECT_PATH, WieldService { handle })
        .await
    {
        tracing::warn!(%error, "could not register single-instance object; running without service");
        return Acquired::Primary(None);
    }

    let reply = connection
        .request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into())
        .await;
    match reply {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => {
            Acquired::Primary(Some(connection))
        }
        Ok(RequestNameReply::Exists | RequestNameReply::InQueue) => {
            relay(&connection, run).await;
            Acquired::Secondary
        }
        Err(zbus::Error::NameTaken) => {
            relay(&connection, run).await;
            Acquired::Secondary
        }
        Err(error) => {
            tracing::warn!(%error, "could not acquire single-instance name; running without service");
            Acquired::Primary(None)
        }
    }
}

async fn relay(connection: &zbus::Connection, run: Option<(String, String)>) {
    let proxy = match zbus::Proxy::new(connection, BUS_NAME, OBJECT_PATH, BUS_NAME).await {
        Ok(proxy) => proxy,
        Err(error) => {
            tracing::warn!(%error, "could not connect to running Wield instance");
            return;
        }
    };

    if let Some((id, args_json)) = run {
        match proxy
            .call::<_, _, String>("RunTool", &(id.as_str(), args_json.as_str()))
            .await
        {
            Ok(outcome) => println!("{outcome}"),
            Err(error) => tracing::warn!(%error, "could not relay tool run"),
        }
    } else if let Err(error) = proxy.call::<_, _, ()>("ShowPalette", &()).await {
        tracing::warn!(%error, "could not relay palette activation");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct FakeShell {
        shown: Mutex<u32>,
        last_run: Mutex<Option<(String, String)>>,
    }

    #[async_trait::async_trait]
    impl ShellHandle for FakeShell {
        fn show_palette(&self) {
            *self.shown.lock().unwrap() += 1;
        }

        async fn run_tool(&self, id: &str, args: &str) -> String {
            *self.last_run.lock().unwrap() = Some((id.into(), args.into()));
            "{\"Cancelled\":null}".into()
        }
    }

    #[tokio::test]
    async fn second_acquire_relays_show_palette_to_the_first() {
        let Some(bus) = crate::test_support::PrivateBus::launch() else {
            eprintln!("skipping: dbus-daemon not available");
            return;
        };
        let first_connection = bus.connect().await.unwrap();
        let second_connection = bus.connect().await.unwrap();

        let first = Arc::new(FakeShell {
            shown: Mutex::new(0),
            last_run: Mutex::new(None),
        });
        let Acquired::Primary(Some(_connection)) =
            acquire_on(first_connection, first.clone(), None).await
        else {
            panic!("first should be primary");
        };

        let second = Arc::new(FakeShell {
            shown: Mutex::new(0),
            last_run: Mutex::new(None),
        });
        let acquired = acquire_on(second_connection, second, None).await;
        match acquired {
            Acquired::Secondary => {}
            Acquired::Primary(Some(_)) => panic!("second unexpectedly acquired the name"),
            Acquired::Primary(None) => panic!("second acquire degraded without a connection"),
        }
        assert_eq!(*first.shown.lock().unwrap(), 1);
    }
}
