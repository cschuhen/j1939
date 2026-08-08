use iso11783_data::strings::pgn as pgn_titles;

use crate::types::DecodedMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Column {
    AbsTime,
    Time,
    Src,
    Dest,
    Pgn,
    PgnName,
    Title,
    Data,
    Detail,
    DetailCondensed,
}

impl Column {
    pub fn all() -> &'static [Self] {
        &[
            Self::AbsTime,
            Self::Time,
            Self::Src,
            Self::Dest,
            Self::Pgn,
            Self::PgnName,
            Self::Title,
            Self::Data,
            Self::Detail,
            Self::DetailCondensed,
        ]
    }

    pub fn format(
        self,
        msg: &DecodedMessage,
        max_width: u16,
        global_start_time: core::option::Option<u64>,
    ) -> String {
        match self {
            Column::AbsTime => crate::formats::format_timestamp(msg.timestamp()),
            Column::Time => crate::formats::format_elapsed_time(msg.timestamp(), global_start_time),
            Column::Src => format!("{:02X}", msg.source_address()),
            Column::Dest => format!("{:02X}", msg.dest_address()),
            Column::Pgn => format!("{:X}", msg.pgn()),
            Column::PgnName => {
                if let Some(pgn_name) = pgn_titles::lookup(msg.pgn()) {
                    pgn_name.to_string()
                } else {
                    String::new()
                }
            }
            Column::Title => msg.title.clone(),
            Column::Data => crate::formats::format_data_hex(msg.data_bytes()),
            Column::Detail => crate::formats::build_detail_string(msg, max_width),
            Column::DetailCondensed => {
                crate::formats::build_detail_condensed_string(msg, max_width)
            }
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::AbsTime => "Abs Time",
            Self::Time => "Time",
            Self::Src => "Src",
            Self::Dest => "Dst",
            Self::Pgn => "PGN",
            Self::PgnName => "PGN Name",
            Self::Title => "Title",
            Self::Data => "Data",
            Self::Detail => "Detail",
            Self::DetailCondensed => "Detail Condensed",
        }
    }

    pub fn default_enabled(self) -> bool {
        match self {
            Self::Time | Self::Src | Self::Dest | Self::Pgn | Self::Title | Self::Detail => true,
            Self::AbsTime | Self::PgnName | Self::Data | Self::DetailCondensed => false,
        }
    }

    pub fn base_width(self) -> u16 {
        match self {
            Self::AbsTime => 14,
            Self::Time => 12,
            Self::Src | Self::Dest => 3,
            Self::Pgn => 6,
            Self::PgnName => 20,
            Self::Title => 15,
            Self::Data => 24,
            Self::Detail => 30,
            Self::DetailCondensed => 30,
        }
    }

    pub fn required_sizs(self) -> (u16, bool) {
        // returns (basewidth, expand)
        match self {
            Self::Detail | Self::DetailCondensed => (self.base_width(), true),
            _ => (self.base_width(), false),
        }
    }
}
