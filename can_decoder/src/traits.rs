use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::types::{PrettyOutput, RawFrame};

/// Abstracts data sources that produce raw CAN frames.
/// Implementations handle different input methods (live SocketCAN or candump files).
#[allow(clippy::type_complexity)]
pub trait Source: Send + Sync {
    /// Human-readable name of this source implementation.
    fn name(&self) -> &str;

    /// Start the source and send RawFrames through the channel until stopped or errored.
    fn start(
        self: Arc<Self>,
        tx: mpsc::UnboundedSender<RawFrame>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send + 'static>>;
}

/// Decodes raw CAN frames into structured PrettyOutput items.
/// Implementations handle protocol-specific decoding (J1939, ISO11783, etc.).
#[allow(clippy::type_complexity)]
pub trait Decoder: Send {
    /// Human-readable name of this decoder implementation.
    fn name(&self) -> &str;

    /// Decode a single raw frame into zero or more PrettyOutput items.
    fn decode(
        &mut self,
        frame: RawFrame,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<PrettyOutput>, Box<dyn Error + Send + Sync>>>
                + Send
                + '_,
        >,
    >;
}

/// Renders PrettyOutput items to a human-readable string format.
/// Implementations produce different output formats (console, JSON, CSV).
#[allow(clippy::type_complexity)]
pub trait Renderer: Send {
    /// Human-readable name of this renderer implementation.
    fn name(&self) -> &str;

    /// Render a single PrettyOutput item into a formatted string.
    fn render(
        &mut self,
        output: PrettyOutput,
    ) -> Pin<Box<dyn Future<Output = Result<String, Box<dyn Error + Send + Sync>>> + Send + '_>>;
}

/// Filters PrettyOutput items based on configurable rules.
/// Multiple filters can be chained to narrow down the output.
pub trait Filter: Send + Sync {
    /// Human-readable name of this filter implementation.
    fn name(&self) -> &str;

    /// Return true if the given PrettyOutput item passes this filter.
    fn matches<'a>(
        &'a self,
        output: &'a PrettyOutput,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
}
