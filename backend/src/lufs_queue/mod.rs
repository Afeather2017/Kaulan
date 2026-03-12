//! Single-threaded LUFS calculation queue.
//!
//! This module provides a queue-based system for LUFS calculations that ensures
//! only one calculation runs at a time, preventing CPU overload from concurrent
//! operations.
//!
//! Jobs are submitted via `LufsQueue::submit_job()` and processed sequentially
//! by a background worker task. Results are sent back via oneshot channels.

use sea_orm::{DatabaseConnection, EntityTrait, ActiveModelTrait, Set};
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn, error};

use crate::entities::music::{Entity as MusicEntity, ActiveModel as MusicActiveModel};
use crate::file_ops::get_file_reader;
use lufsgen::LufsCalculator;

/// LUFS calculation job submitted to the queue
pub struct LufsJob {
    pub music_id: i32,
    pub file_path: String,
    pub respond_to: oneshot::Sender<Option<f64>>,
}

/// LUFS queue service - manages job queue and worker task
pub struct LufsQueue {
    job_sender: mpsc::UnboundedSender<LufsJob>,
}

impl LufsQueue {
    /// Creates a new LUFS queue and spawns the worker task
    ///
    /// # Arguments
    /// * `db_conn` - Database connection for updating LUFS values
    ///
    /// # Returns
    /// A `LufsQueue` instance that can be used to submit jobs
    pub fn new(db_conn: DatabaseConnection) -> Self {
        let (job_sender, job_receiver) = mpsc::unbounded_channel();

        // Spawn the worker task
        tokio::spawn(run_worker(job_receiver, db_conn));

        Self { job_sender }
    }

    /// Submits a LUFS calculation job to the queue
    ///
    /// # Arguments
    /// * `music_id` - The ID of the music entry in the database
    /// * `file_path` - The path to the audio file (or content URI on Android)
    ///
    /// # Returns
    /// A `oneshot::Receiver` that will receive the LUFS value when calculation completes
    ///
    /// # Returns
    /// - `Some(f64)` - LUFS value calculated successfully
    /// - `None` - Unsupported audio format
    pub fn submit_job(&self, music_id: i32, file_path: String) -> oneshot::Receiver<Option<f64>> {
        let (tx, rx) = oneshot::channel();

        let job = LufsJob {
            music_id,
            file_path,
            respond_to: tx,
        };

        // Send job to queue (unbounded, so this won't fail)
        let _ = self.job_sender.send(job);

        rx
    }
}

/// Worker task that processes LUFS calculation jobs sequentially
async fn run_worker(
    mut job_receiver: mpsc::UnboundedReceiver<LufsJob>,
    db_conn: DatabaseConnection,
) {
    info!("[LUFS QUEUE] Worker started");

    while let Some(job) = job_receiver.recv().await {
        let music_id = job.music_id;
        let file_path = job.file_path.clone();

        info!("[LUFS QUEUE] Processing music ID: {}", music_id);

        // Calculate LUFS
        let result = calculate_lufs(&file_path).await;

        match result {
            Ok(Some(lufs_value)) => {
                // Update database with calculated LUFS
                match MusicEntity::find_by_id(music_id).one(&db_conn).await {
                    Ok(Some(music)) => {
                        let mut active_model: MusicActiveModel = music.into();
                        active_model.lufs = Set(Some(lufs_value));

                        match active_model.update(&db_conn).await {
                            Ok(_) => {
                                info!("[LUFS QUEUE] SUCCESS: music ID: {}, LUFS: {}", music_id, lufs_value);
                                let _ = job.respond_to.send(Some(lufs_value));
                            }
                            Err(e) => {
                                error!("[LUFS QUEUE] ERROR: Failed to update DB for music ID {}: {}", music_id, e);
                                let _ = job.respond_to.send(None);
                            }
                        }
                    }
                    Ok(None) => {
                        error!("[LUFS QUEUE] ERROR: Music not found for ID: {}", music_id);
                        let _ = job.respond_to.send(None);
                    }
                    Err(e) => {
                        error!("[LUFS QUEUE] ERROR: Database error for music ID {}: {}", music_id, e);
                        let _ = job.respond_to.send(None);
                    }
                }
            }
            Ok(None) => {
                // Unsupported format
                warn!("[LUFS QUEUE] Unsupported format: music ID: {}", music_id);
                let _ = job.respond_to.send(None);
            }
            Err(e) => {
                // Calculation error
                error!("[LUFS QUEUE] ERROR: Failed to calculate LUFS for music ID {}: {}", music_id, e);
                let _ = job.respond_to.send(None);
            }
        }
    }

    info!("[LUFS QUEUE] Worker stopped");
}

/// Calculates LUFS for a file path using seekable reader
async fn calculate_lufs(file_path: &str) -> Result<Option<f64>, String> {
    let reader = get_file_reader()
        .open_seekable_reader(file_path)
        .await
        .map_err(|e| format!("Failed to open seekable reader: {}", e))?;

    let file_label = file_path.to_string();

    tokio::task::spawn_blocking(move || {
        let calc = LufsCalculator::default();
        match calc.calculate_from_reader(reader) {
            Ok(Some(lufs)) => {
                info!("[LUFS] SUCCESS: {} - LUFS: {}", file_label, lufs);
                Some(lufs)
            }
            Ok(None) => {
                warn!("[LUFS] FAILED: Unsupported format for: {}", file_label);
                None
            }
            Err(e) => {
                error!("[LUFS] ERROR: Failed to calculate LUFS for {}: {}", file_label, e);
                None
            }
        }
    })
    .await
    .map_err(|e| format!("LUFS task execution failed: {}", e))
}
