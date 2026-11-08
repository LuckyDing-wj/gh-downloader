use crate::models::DownloadTask;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{watch, Mutex};

#[derive(Default)]
pub struct AppState {
    pub cancellations: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    pub tasks: Arc<Mutex<HashMap<String, DownloadTask>>>,
}
