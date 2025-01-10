use std::pin::Pin;

use derive_more::derive::{Display, From};

#[derive(From, Debug, Display)]
pub enum Error {
    ToolCallMissingName,
    Serde(serde_json::Error),
    Uuid(uuid::Error),
}

pub type Result<A> = std::result::Result<A, Error>;
pub type BoxStream<A, E> =
    Pin<Box<dyn futures::Stream<Item = std::result::Result<A, E>> + Send>>;

pub type ResultStream<A, E> = std::result::Result<BoxStream<A, E>, E>;


pub fn convert_boxstream_to_impl_stream<A, E>(box_stream: BoxStream<A, E>) -> impl futures::Stream<Item = std::result::Result<A, E>> {
    box_stream
}
