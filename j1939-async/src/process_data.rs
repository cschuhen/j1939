//const FILE_CODE: u8 = 0xFB;

#[derive(Copy, Clone, PartialEq, Eq, defmt::Format)]
pub enum Command {
    TechnicalCapabilities = 0,
    DDOPTransfer = 1,
    RequestValue = 2,
    Value = 3,
    MeasurementTimeInterval = 4,
    MeasuermentDistanceInterval = 5,
    MeasurementMinumum = 6,
    MeasurementMaximum = 7,
    MeasurementChange = 8,
    PeerControlAssignment = 9,
    SetValueWithAcknowledgment = 0xa,
    ProcessDataAcknowledgment = 0xd,
    ServerStatus = 0xe,
    ClientTask = 0xf, // Or Unknown if in PD ACK
}

pub type ElementNumber = u16;
pub type Ddi = u16;
type Value = i32;

#[derive(Copy, Clone, PartialEq, Eq, defmt::Format)]
pub struct ErrorCodes(i32);

impl ErrorCodes {
    pub fn new() -> Self {
        ErrorCodes(0xFFFF_FF00u32 as i32)
    }

    pub fn set_command_not_supported(self) -> Self {
        Self(self.0 | 0x01)
    }
    pub fn set_invalid_element_number(self) -> Self {
        Self(self.0 | 0x02)
    }
    pub fn set_ddi_not_supported(self) -> Self {
        Self(self.0 | 0x04)
    }
    pub fn set_trigger_not_supported(self) -> Self {
        Self(self.0 | 0x08)
    }
    pub fn set_not_settable(self) -> Self {
        Self(self.0 | 0x10)
    }
    pub fn set_invalid_threshold(self) -> Self {
        Self(self.0 | 0x20)
    }
    pub fn set_not_conform_to_ddi(self) -> Self {
        Self(self.0 | 0x40)
    }
    pub fn set_value_out_of_range(self) -> Self {
        Self(self.0 | 0x80)
    }
    pub fn set_command(self, command: Command) -> Self {
        Self((self.0 & 0xFFFF_00FFu32 as i32) | ((command as i32) & 0xFF) << 8)
    }
    pub fn get_command(&self) -> Command {
        match self.0 & 0xFF {
            0 => Command::TechnicalCapabilities,
            1 => Command::DDOPTransfer,
            2 => Command::RequestValue,
            3 => Command::Value,
            4 => Command::MeasurementTimeInterval,
            5 => Command::MeasuermentDistanceInterval,
            6 => Command::MeasurementMinumum,
            7 => Command::MeasurementMaximum,
            8 => Command::MeasurementChange,
            9 => Command::PeerControlAssignment,
            10 => Command::SetValueWithAcknowledgment,
            13 => Command::ProcessDataAcknowledgment,
            14 => Command::ServerStatus,
            15 => Command::ClientTask,
            _ => Command::TechnicalCapabilities,
        }
    }
    pub fn get_command_not_supported(&self) -> bool {
        (self.0 & 0x01) != 0
    }
    pub fn get_invalid_element_number(&self) -> bool {
        (self.0 & 0x02) != 0
    }
    pub fn get_ddi_not_supported(&self) -> bool {
        (self.0 & 0x04) != 0
    }
    pub fn get_trigger_not_supported(&self) -> bool {
        (self.0 & 0x08) != 0
    }
    pub fn get_not_settable(&self) -> bool {
        (self.0 & 0x10) != 0
    }
    pub fn get_invalid_threshold(&self) -> bool {
        (self.0 & 0x20) != 0
    }
    pub fn get_not_conform_to_ddi(&self) -> bool {
        (self.0 & 0x40) != 0
    }
    pub fn get_value_out_of_range(&self) -> bool {
        (self.0 & 0x80) != 0
    }
    pub fn get(&self) -> i32 {
        self.0
    }
    pub fn set(&mut self, code: i32) {
        self.0 = code;
    }
    pub fn get_error_code(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }
}

#[derive(Copy, Clone, PartialEq, Eq, defmt::Format)]
pub struct ProcessData {
    pub command: Command,
    pub element_number: ElementNumber,
    pub ddi: Ddi,
    pub value: Value,
}

impl From<&ProcessData> for [u8; 8] {
    fn from(pd: &ProcessData) -> Self {
        let mut ret = [0u8; 8];

        let cmd = (pd.command as u8) & 0x0f;
        let eno = pd.element_number & 0xfff;
        ret[0] = cmd | (((eno << 4) & 0xf0) as u8);
        ret[1] = (eno >> 4) as u8;
        ret[2] = (pd.ddi >> 0) as u8;
        ret[3] = (pd.ddi >> 8) as u8;

        //ret[4..8] = pd.value.to_ne_bytes();

        ret[4] = (pd.value >> 0) as u8;
        ret[5] = (pd.value >> 8) as u8;
        ret[6] = (pd.value >> 16) as u8;
        ret[7] = (pd.value >> 24) as u8;

        ret
    }
}

impl From<&[u8]> for ProcessData {
    fn from(data: &[u8]) -> Self {
        //let mut ret = [0u8; 8];

        let command = match data[0] & 0x0f {
            0 => Command::TechnicalCapabilities,
            1 => Command::DDOPTransfer,
            2 => Command::RequestValue,
            3 => Command::Value,
            4 => Command::MeasurementTimeInterval,
            5 => Command::MeasuermentDistanceInterval,
            6 => Command::MeasurementMinumum,
            7 => Command::MeasurementMaximum,
            8 => Command::MeasurementChange,
            9 => Command::PeerControlAssignment,
            10 => Command::SetValueWithAcknowledgment,
            13 => Command::ProcessDataAcknowledgment,
            14 => Command::ServerStatus,
            15 => Command::ClientTask,
            _ => Command::TechnicalCapabilities,
        };

        let element_number: ElementNumber = ((data[0] as u16) >> 4) | ((data[1] as u16) << 4);

        let ddi = Ddi::from_le_bytes(data[2..4].try_into().unwrap());
        let value = i32::from_le_bytes(data[4..8].try_into().unwrap());

        ProcessData {
            command,
            element_number,
            ddi,
            value,
        }
    }
}

impl ProcessData {
    pub fn new(command: Command, element_number: ElementNumber, ddi: Ddi, value: Value) -> Self {
        ProcessData {
            command,
            element_number,
            ddi,
            value,
        }
    }

    pub fn ack(&self, errors: ErrorCodes) -> Self {
        ProcessData {
            command: Command::ProcessDataAcknowledgment,
            element_number: self.element_number,
            ddi: self.ddi,
            value: errors.set_command(self.command).get(),
        }
    }

    pub fn response(&self, value: i32) -> Self {
        ProcessData {
            command: Command::ProcessDataAcknowledgment,
            element_number: self.element_number,
            ddi: self.ddi,
            value,
        }
    }

    pub fn from_data(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        Some(data.into())
    }

    pub fn data(&self) -> [u8; 8] {
        self.into()
    }
}

/*/
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, defmt::Format)]
pub enum HandlePdValue {
    NotHandled = 0,
    Handled = 1,
    HandledWithResponse(ProcessData),
}

impl HandlePdValue {
    pub fn is_handled(&self) -> bool {
        match self {
            HandlePdValue::Handled | HandlePdValue::HandledWithResponse(_) => true,
            _ => false,
        }
    }
}
*/

#[derive(Copy, Clone, PartialEq, Eq, defmt::Format)]
pub struct PdReturn {
    handled: bool,
    changed: bool,
    response: Option<ProcessData>,
}

impl PdReturn {
    pub fn new_not_handled() -> Self {
        PdReturn {
            handled: false,
            changed: false,
            response: None,
        }
    }
    pub fn new_unchanged() -> Self {
        PdReturn {
            handled: true,
            changed: false,
            response: None,
        }
    }

    pub fn new_changed() -> Self {
        PdReturn {
            handled: true,
            changed: true,
            response: None,
        }
    }

    pub fn new_with_response(handled: bool, changed: bool, response: ProcessData) -> Self {
        PdReturn {
            handled,
            changed,
            response: Some(response),
        }
    }
    pub fn handled(&self) -> bool {
        self.handled
    }
    pub fn changed(&self) -> bool {
        self.changed
    }
    pub fn response(&self) -> Option<ProcessData> {
        self.response
    }
}

pub type HandlePdResult = Result<PdReturn, crate::error::Error>;

// Read only
pub fn handle_pd_value(pd: &ProcessData, value: &i32) -> HandlePdResult {
    match pd.command {
        Command::Value | Command::SetValueWithAcknowledgment => Ok(PdReturn::new_with_response(
            true,
            false,
            pd.ack(ErrorCodes::new().set_command_not_supported()),
        )),
        Command::RequestValue => Ok(PdReturn::new_with_response(
            true,
            false,
            pd.response(*value),
            //ProcessData::new(Command::Value, pd.element_number, pd.ddi, *value),
        )),
        _ => Ok(PdReturn::new_with_response(
            true,
            false,
            pd.ack(ErrorCodes::new().set_command_not_supported()),
        )),
    }
}

// Read only
pub fn handle_settable_pd_value(pd: &ProcessData, value: &mut i32) -> HandlePdResult {
    match pd.command {
        Command::Value => {
            defmt::println!("Value");
            if *value == pd.value {
                Ok(PdReturn::new_unchanged())
            } else {
                *value = pd.value;
                Ok(PdReturn::new_changed())
            }
        }
        Command::SetValueWithAcknowledgment => {
            defmt::println!("SetValue");
            let changed: bool;
            if *value == pd.value {
                changed = false;
            } else {
                *value = pd.value;
                changed = true;
            }
            Ok(PdReturn::new_with_response(
                true,
                changed,
                pd.ack(ErrorCodes::new()),
            ))
        }
        Command::RequestValue => Ok(PdReturn::new_with_response(
            true,
            false,
            pd.response(*value),
            //ProcessData::new(Command::Value, pd.element_number, pd.ddi, *value),
        )),
        _ => handle_pd_value(pd, value),
    }
}

pub const DDI_PROP_TOTAL_CHARGE: Ddi = 0xe000; // micro-amp-hours
pub const DDI_PROP_CURRENT: Ddi = 0xe001; // micro-amp
pub const DDI_PROP_VOLTAGE: Ddi = 0xe002; // micro-volts
pub const DDI_PROP_SHUNT_RESISTANCE: Ddi = 0xe003; // micro-ohms
pub const DDI_PROP_SHUNT_OFFSET: Ddi = 0xe004; // ADC counts (2.5 μV)

pub const DDI_PROP_TEMPERATURE: Ddi = 0xe010; // micro-degrees
pub const DDI_PROP_HUMIDITY: Ddi = 0xe011; // ppm
pub const DDI_PROP_PRESSURE: Ddi = 0xe012; // milli-pascals
