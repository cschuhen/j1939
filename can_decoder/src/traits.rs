use crate::types::{DecodeContext, DecodeError, DecodedMessage, RawFrame};
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::mpsc;

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

/// Decodes raw CAN frames into structured DecodedMessage.
/// Implementations handle protocol-specific decoding (J1939, ISO-11783, etc.).
#[allow(clippy::type_complexity)]
pub trait Decoder: Send {
    /// Human-readable name of this decoder implementation.
    fn name(&self) -> &str;

    /// Decode a single raw frame into a DecodedMessage.
    fn decode(
        &mut self,
        frame: RawFrame,
    ) -> Pin<
        Box<dyn Future<Output = Result<DecodedMessage, Box<dyn Error + Send + Sync>>> + Send + '_>,
    >;
}

/// A stateful, synchronous decoder for complex PGNs.
/// These are used for PGNs that require state (e.g. TP reassembly) or
/// have variable formats based on additional context.
pub trait ComplexDecoder: Send {
    /// Decodes a complete, reassembled message.
    /// `&mut self` allows the decoder to maintain internal state (e.g. for sequence tracking)
    /// Returns `Ok(None)` if the message is part of a sequence not yet complete.
    fn decode(
        &mut self,
        context: &DecodeContext,
        payload: &[u8],
    ) -> Result<Option<DecodedMessage>, DecodeError>;
}

/// Renders DecodedMessage items to a human-readable string format.
/// Implementations produce different output formats (console, JSON, CSV).
#[allow(clippy::type_complexity)]
pub trait Renderer: Send {
    /// Human-readable name of this renderer implementation.
    fn name(&self) -> &str;

    /// Render a decoded message into a formatted string.
    fn render<'a>(
        &'a mut self,
        message: &'a DecodedMessage,
    ) -> Pin<Box<dyn Future<Output = Result<String, Box<dyn Error + Send + Sync>>> + Send + '_>>;
}

/// Filters DecodedMessage items based on configurable rules.
/// Multiple filters can be chained to narrow down the output.
pub trait Filter: Send + Sync {
    /// Human-readable name of this filter implementation.
    fn name(&self) -> &str;

    /// Return true if the given DecodedMessage passes this filter.
    fn matches<'a>(
        &'a self,
        message: &'a DecodedMessage,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
}
