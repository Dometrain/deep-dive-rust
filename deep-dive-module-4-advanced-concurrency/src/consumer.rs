//! A background **async** event consumer -- the `tokio::spawn` sibling of
//! `audit::AuditLogger`'s background *thread*.
//!
//! Real systems often publish an event onto a message topic (Kafka,
//! RabbitMQ, SQS, NATS, ...) whenever something happens, and let one or
//! more consumers react to it off the request path, asynchronously.
//! `routes::tasks::create_task` calls [`EventPublisher::publish`] after a
//! task is created -- a plain, non-blocking, non-`async` channel send, the
//! same shape as `audit::AuditLogger::record`. [`run`] is spawned once,
//! with `tokio::spawn` instead of `thread::spawn`, in `main`, and consumes
//! events for as long as the app runs -- the async counterpart of
//! `audit::AuditWorker`'s background thread.
//!
//! See `examples::module_4::background_async_worker` for this exact shape
//! taught as a standalone lesson sample, and the "Swapping in a real
//! message broker" section below for what changes (and, just as
//! importantly, what *doesn't*) if the channel this module simulates were
//! a real Kafka topic instead.
//!
//! ## Swapping in a real message broker
//!
//! This module models "messages arriving on a topic" with an in-process
//! `tokio::sync::mpsc` channel -- no broker to install, nothing new to run,
//! which is what keeps [`run`]'s tests fast and Docker-free. A real
//! deployment would swap [`EventPublisher`]'s sender for a Kafka producer
//! (or an equivalent for RabbitMQ, SQS, NATS, ...) and [`run`]'s
//! `receiver.recv().await` loop for a consumer's own async stream. Using
//! the `rdkafka` crate, that loop looks like this instead -- same shape,
//! different source:
//!
//! ```ignore
//! use futures::StreamExt;
//! use rdkafka::consumer::{Consumer, StreamConsumer};
//! use rdkafka::Message;
//!
//! let consumer: StreamConsumer = client_config.create()?;
//! consumer.subscribe(&["task-events"])?;
//!
//! let mut stream = consumer.stream();
//! while let Some(message) = stream.next().await {
//!     let message = message?;
//!     process(decode(message.payload())).await;
//!     consumer.commit_message(&message, CommitMode::Async)?;
//! }
//! ```
//!
//! Starting this loop is identical either way --
//! `tokio::spawn(run(receiver))`, once, at startup, running for as long as
//! the app does. Nothing about *how you start a background async worker*
//! changes based on what's actually on the other end of the channel.

use crate::models::TaskId;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::Duration;
use tracing::info;

/// One event published onto the (simulated) topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskEvent {
    pub task_id: TaskId,
    pub kind: &'static str,
}

/// Cheap to clone -- cloning copies the channel's `Sender`, not any
/// queued events, so every Actix worker's clone still funnels into the one
/// background task [`channel`] paired it with.
#[derive(Clone)]
pub struct EventPublisher(UnboundedSender<TaskEvent>);

impl EventPublisher {
    /// Publishes an event without waiting for anything downstream to
    /// process it. Deliberately not `async fn`: an unbounded channel's
    /// `send` never blocks, so publishing never sits on the request's
    /// critical path -- the same trade `audit::AuditLogger::record` makes,
    /// for the same reason.
    ///
    /// A failed send only means the consumer task has already exited (e.g.
    /// mid-shutdown); dropping the event at that point is the right call
    /// here, same as it is for the audit trail.
    pub fn publish(&self, task_id: TaskId, kind: &'static str) {
        let _ = self.0.send(TaskEvent { task_id, kind });
    }
}

/// Builds the channel this module uses in place of a real topic. Returns
/// `(publisher, receiver)`: `publisher` is cheap to clone into every
/// worker; `receiver` is handed to [`run`] exactly once, in `main`.
pub fn channel() -> (EventPublisher, UnboundedReceiver<TaskEvent>) {
    let (sender, receiver) = mpsc::unbounded_channel();
    (EventPublisher(sender), receiver)
}

/// The background consumer loop, meant to be handed straight to
/// `tokio::spawn` once, in `main`. Runs for as long as the app does, and
/// ends on its own once every [`EventPublisher`] clone has been dropped --
/// the async counterpart of `audit::AuditWorker`'s `std::sync::mpsc`
/// version, with no explicit "stop" signal needed either way.
pub async fn run(events: UnboundedReceiver<TaskEvent>) {
    drain(events).await;
}

/// The actual consumer logic, factored out from [`run`] so it can be
/// tested directly: feed it a few events, drop the sender, and check what
/// it processed -- rather than testing an infinite loop that never returns
/// in production.
async fn drain(mut events: UnboundedReceiver<TaskEvent>) -> Vec<TaskEvent> {
    let mut processed = Vec::new();
    while let Some(event) = events.recv().await {
        process(&event).await;
        processed.push(event);
    }
    processed
}

/// Stands in for whatever a real consumer would do with a message: call a
/// webhook, update a search index, publish a derived event elsewhere. The
/// `sleep` represents that real, I/O-bound work -- and is exactly why this
/// is a `tokio::spawn`ed *task*, not a `thread::spawn`ed *thread*: I/O-bound
/// waiting is what async is for (see `examples::module_4`'s "Threads vs.
/// Async" note).
async fn process(event: &TaskEvent) {
    tokio::time::sleep(Duration::from_millis(5)).await;
    info!(task_id = %event.task_id, kind = event.kind, "processed task event");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn processes_every_event_in_publish_order_and_stops_once_the_sender_is_dropped() {
        let (publisher, receiver) = channel();

        publisher.publish(TaskId(1), "task.created");
        publisher.publish(TaskId(2), "task.created");

        // Dropping the only `EventPublisher` (and, with it, its `Sender`)
        // lets `drain`'s `while let Some(...)` loop end once both events
        // above are drained -- the exact async counterpart of
        // `examples::module_4::channel_iteration`'s "drop the sender" step.
        drop(publisher);

        let processed = drain(receiver).await;
        assert_eq!(
            processed,
            vec![
                TaskEvent {
                    task_id: TaskId(1),
                    kind: "task.created"
                },
                TaskEvent {
                    task_id: TaskId(2),
                    kind: "task.created"
                },
            ]
        );
    }

    #[tokio::test]
    async fn a_clone_publishes_into_the_same_consumer() {
        let (publisher, receiver) = channel();
        let clone = publisher.clone();

        clone.publish(TaskId(7), "task.created");

        // Both handles hold a `Sender` clone -- both have to go before
        // `drain`'s loop can end.
        drop(publisher);
        drop(clone);

        let processed = drain(receiver).await;
        assert_eq!(processed[0].task_id, TaskId(7));
    }
}
