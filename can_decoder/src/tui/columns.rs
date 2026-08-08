use ratatui::layout::Constraint;

pub use crate::columns::Column;

impl Column {
    pub fn constraint(self) -> Constraint {
        match self {
            Self::Detail | Self::DetailCondensed => Constraint::Min(self.base_width()),
            _ => Constraint::Length(self.base_width()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColumnState {
    pub column: Column,
    pub enabled: bool,
}

impl ColumnState {
    pub fn new(column: Column) -> Self {
        Self {
            column,
            enabled: column.default_enabled(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColumnConfig {
    pub states: Vec<ColumnState>,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        let mut states = Vec::new();
        for col in Column::all() {
            states.push(ColumnState::new(*col));
        }
        Self { states }
    }
}

impl ColumnConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enabled_columns(&self) -> Vec<Column> {
        self.states
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.column)
            .collect()
    }

    pub fn toggle(&mut self, column: Column) {
        if let Some(state) = self.states.iter_mut().find(|s| s.column == column) {
            state.enabled = !state.enabled;
        }
    }

    pub fn get_constraints(&self) -> Vec<Constraint> {
        self.enabled_columns()
            .iter()
            .map(|c| c.constraint())
            .collect()
    }

    pub fn header_cells(&self) -> Vec<&str> {
        self.enabled_columns().iter().map(|c| c.label()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_labels() {
        assert_eq!(Column::AbsTime.label(), "Abs Time");
        assert_eq!(Column::Time.label(), "Time");
        assert_eq!(Column::Src.label(), "Src");
        assert_eq!(Column::Dest.label(), "Dst");
        assert_eq!(Column::Pgn.label(), "PGN");
        assert_eq!(Column::PgnName.label(), "PGN Name");
        assert_eq!(Column::Title.label(), "Title");
        assert_eq!(Column::Data.label(), "Data");
        assert_eq!(Column::Detail.label(), "Detail");
        assert_eq!(Column::DetailCondensed.label(), "Detail Condensed");
    }

    #[test]
    fn test_default_enabled() {
        let config = ColumnConfig::new();
        for col in &[
            Column::Time,
            Column::Src,
            Column::Dest,
            Column::Pgn,
            Column::Title,
            Column::Detail,
        ] {
            assert!(
                config
                    .states
                    .iter()
                    .find(|s| s.column == *col)
                    .unwrap()
                    .enabled
            );
        }
        for col in &[
            Column::AbsTime,
            Column::PgnName,
            Column::Data,
            Column::DetailCondensed,
        ] {
            assert!(
                !config
                    .states
                    .iter()
                    .find(|s| s.column == *col)
                    .unwrap()
                    .enabled
            );
        }
    }

    #[test]
    fn test_toggle() {
        let mut config = ColumnConfig::new();
        config.toggle(Column::PgnName);
        assert!(
            config
                .states
                .iter()
                .find(|s| s.column == Column::PgnName)
                .unwrap()
                .enabled
        );

        config.toggle(Column::Time);
        assert!(
            !config
                .states
                .iter()
                .find(|s| s.column == Column::Time)
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn test_enabled_columns() {
        let mut config = ColumnConfig::new();
        config.toggle(Column::PgnName);
        config.toggle(Column::Data);

        let enabled = config.enabled_columns();
        assert_eq!(enabled.len(), 8); // 6 default + 2 toggled

        config.toggle(Column::Time);
        let enabled = config.enabled_columns();
        assert_eq!(enabled.len(), 7);
    }

    #[test]
    fn test_all_columns_count() {
        assert_eq!(Column::all().len(), 10);
    }

    #[test]
    fn test_constraints_match_enabled() {
        let mut config = ColumnConfig::new();
        let constraints = config.get_constraints();
        assert_eq!(constraints.len(), 6); // default enabled count

        config.toggle(Column::PgnName);
        config.toggle(Column::Data);
        let constraints = config.get_constraints();
        assert_eq!(constraints.len(), 8);
    }

    #[test]
    fn test_header_cells() {
        let mut config = ColumnConfig::new();
        config.toggle(Column::PgnName);

        let cells = config.header_cells();
        assert_eq!(cells.len(), 7);
        assert!(cells.contains(&"PGN Name"));
        assert!(!cells.contains(&"Data"));
    }

    #[test]
    fn test_base_widths() {
        assert_eq!(Column::AbsTime.base_width(), 14);
        assert_eq!(Column::Time.base_width(), 12);
        assert_eq!(Column::Src.base_width(), 3);
        assert_eq!(Column::Dest.base_width(), 3);
        assert_eq!(Column::Pgn.base_width(), 6);
        assert_eq!(Column::PgnName.base_width(), 20);
        assert_eq!(Column::Title.base_width(), 15);
        assert_eq!(Column::Data.base_width(), 24);
        assert_eq!(Column::Detail.base_width(), 30);
        assert_eq!(Column::DetailCondensed.base_width(), 30);
    }
}
