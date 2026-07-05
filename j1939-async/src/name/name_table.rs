type Error = crate::error::Error;

const FILE_CODE: u8 = 0xFB;

const MAX_NAMES: usize = 255; // Can customise, template?
const MAX_ADDRESSES: usize = 255; // Don't change
pub const NULL_ADDRESS: u8 = 0xFE; // 254
const NOT_ASSIGNED_INDEX: u8 = 0xFF; // 255
const TIMEOUT_NONE: u8 = 0;

#[derive(Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub(crate) enum NameState {
    INIT,
    CLAIMING,
    ACTIVE,
    DEAD,
}
impl From<u16> for NameState {
    fn from(val: u16) -> Self {
        match val {
            0 => NameState::INIT,
            1 => NameState::CLAIMING,
            2 => NameState::ACTIVE,
            3 => NameState::DEAD,
            _ => NameState::INIT,
        }
    }
}
impl From<NameState> for u16 {
    fn from(val: NameState) -> Self {
        match val {
            NameState::INIT => 0,
            NameState::CLAIMING => 1,
            NameState::ACTIVE => 2,
            NameState::DEAD => 3,
        }
    }
}

bitfield::bitfield! {
    #[derive(Copy, Clone)]
    /// Address and meta for a name entry.
    pub(crate) struct NameEntry(u16);
    impl Debug;
    u16;
    #[inline]
    pub u16, from into NameState, state, set_state: 1, 0;   // 2 bits
    pub u16, local, set_local: 2;                           // 1 bit
    pub u8, timeout, set_timeout: 7, 3;                     // 5 bits
    pub u8, address, set_address: 15, 8;                    // 8 bits
}

//use embedded_hal::can::Frame;

use crate::error::mkerr;
use crate::name::Name;
pub(crate) struct NameTable {
    names: [Name; MAX_NAMES],
    info: [NameEntry; MAX_NAMES],
    address_to_name_lookup: [u8; MAX_ADDRESSES],
    num_names: usize,
}

fn set_info_state(
    ni: &mut NameEntry,
    state: NameState,
    #[cfg_attr(not(feature = "defmt"), allow(unused_variables))] line: u32,
) {
    #[cfg(feature = "defmt")]
    defmt::println!(
        "Name@{} {} state={:?} -> {:?}",
        line,
        ni.address(),
        ni.state(),
        state
    );
    ni.set_state(state);
}

impl NameTable {
    pub fn new() -> NameTable {
        NameTable {
            names: [Name(0); MAX_NAMES],
            info: [NameEntry(0); MAX_NAMES],
            address_to_name_lookup: [255; MAX_ADDRESSES],
            num_names: 0x0,
        }
    }

    pub fn size(self: &Self) -> usize {
        return self.num_names;
    }

    pub fn reset_state(&mut self, line: u32) {
        for info in &mut self.info[0..self.num_names].iter_mut().into_iter() {
            set_info_state(info, NameState::INIT, line);
        }
    }

    fn add(self: &mut Self, name: &Name) -> Result<usize, Error> {
        if self.num_names == MAX_NAMES {
            return Err(mkerr(
                FILE_CODE,
                crate::error::ErrorCode::NameTableFull,
                line!(),
            ));
        }
        // List needs to stay sorted to start from top of stack.
        for i in (0..=self.num_names).rev() {
            // Place the item in the correct place
            if i == 0 || name > &self.names[i - 1] {
                self.names[i] = *name;
                self.info[i].set_address(NULL_ADDRESS);
                self.info[i].set_local(false);
                set_info_state(&mut self.info[i], NameState::INIT, line!());
                self.num_names += 1;
                return Ok(i);
            }

            // Else shift the entries up 1 to make space.
            self.names[i] = self.names[i - 1];
            self.info[i] = self.info[i - 1];
            if self.info[i].address() != NULL_ADDRESS {
                self.address_to_name_lookup[self.info[i].address() as usize] = i as u8;
            }
        }
        Err(mkerr(
            FILE_CODE,
            crate::error::ErrorCode::NameTableFull,
            line!(),
        ))
    }
    // Returns entry or
    pub fn entry(self: &mut Self, name: &Name) -> Result<usize, Error> {
        match self.names[0..self.num_names].binary_search(name) {
            Ok(index) => Ok(index),
            Err(_) => self.add(name),
        }
    }
    pub fn info_from_name(self: &Self, name: &Name) -> Result<NameEntry, Error> {
        match self.names[0..self.num_names].binary_search(name) {
            Ok(index) => Ok(self.info[index]),
            Err(_) => Err(mkerr(FILE_CODE, crate::error::ErrorCode::NoExist, line!())),
        }
    }
    pub fn info(self: &Self, index: usize) -> Result<NameEntry, Error> {
        if index >= self.num_names {
            return Err(mkerr(FILE_CODE, crate::error::ErrorCode::NoExist, line!()));
        }
        Ok(self.info[index])
    }
    pub fn set_state(
        self: &mut Self,
        index: usize,
        state: NameState,
        line: u32,
    ) -> Result<usize, Error> {
        if index >= self.num_names {
            return Err(mkerr(FILE_CODE, crate::error::ErrorCode::NoExist, line));
        }
        set_info_state(&mut self.info[index], state, line);
        Ok(index)
    }
    pub fn set_local(self: &mut Self, index: usize, local: bool) -> Result<usize, Error> {
        if index >= self.num_names {
            return Err(mkerr(FILE_CODE, crate::error::ErrorCode::NoExist, line!()));
        }
        self.info[index].set_local(local);
        Ok(index)
    }
    pub fn set_timeout(self: &mut Self, index: usize, timeout: u8) -> Result<usize, Error> {
        if index >= self.num_names {
            return Err(mkerr(FILE_CODE, crate::error::ErrorCode::NoExist, line!()));
        }
        self.info[index].set_timeout(timeout);
        Ok(index)
    }
    pub fn clear_timeout(self: &mut Self, index: usize) -> Result<usize, Error> {
        self.set_timeout(index, TIMEOUT_NONE)
    }
    pub fn set_address(self: &mut Self, index: usize, address: u8) -> Result<usize, Error> {
        if index >= self.num_names {
            return Err(mkerr(FILE_CODE, crate::error::ErrorCode::NoExist, line!()));
        }
        let info = &mut self.info[index];
        if info.address() != NULL_ADDRESS {
            self.address_to_name_lookup[info.address() as usize] = NOT_ASSIGNED_INDEX
        }
        info.set_address(address);
        if address != NULL_ADDRESS {
            self.address_to_name_lookup[address as usize] = index as u8;
        }
        Ok(index)
    }

    pub fn index_at_address(self: &Self, address: u8) -> Option<usize> {
        match self.address_to_name_lookup[address as usize] {
            NOT_ASSIGNED_INDEX => None,
            x if (x as usize) < self.num_names => Some(x as usize),
            _ => None,
        }
    }
    pub fn get_name(self: &Self, index: usize) -> Result<Name, Error> {
        if index >= self.num_names {
            return Err(mkerr(FILE_CODE, crate::error::ErrorCode::NoExist, line!()));
        }
        Ok(self.names[index])
    }
    pub fn name(self: &Self, index: usize) -> &Name {
        &self.names[index]
    }
}

impl Default for NameTable {
    fn default() -> Self {
        Self::new()
    }
}

#[test]
fn name_struct() {
    //use std::ops::Add;
    use std::mem;
    let mut entry = NameEntry(0);

    // Check bounds and not overlapping, 1/2 odd fields 1's
    entry.set_state(NameState::DEAD);
    entry.set_local(false);
    entry.set_timeout(31);
    entry.set_address(0);
    assert_eq!(entry.state(), NameState::DEAD);
    assert_eq!(entry.local(), false);
    assert_eq!(entry.timeout(), 31);
    assert_eq!(entry.address(), 0);

    // Check bounds and not overlapping, 2/2 even fields 1's
    entry.set_state(NameState::INIT);
    entry.set_local(true);
    entry.set_timeout(0);
    entry.set_address(255);
    assert_eq!(entry.state(), NameState::INIT);
    assert_eq!(entry.local(), true);
    assert_eq!(entry.timeout(), 0);
    assert_eq!(entry.address(), 255);

    assert_eq!(2, mem::size_of::<NameEntry>());
    assert_eq!(20, mem::size_of::<[NameEntry; 10]>());
    //print!("SIZE({} )", mem::size_of<NameEntry>())
}

#[test]
fn name_manager_table() {
    let mut table = NameTable::new();
    assert_eq!(table.size(), 0);
    //table.add(&Name(7));
    let test_cases: [u8; 10] = [7, 1, 4, 3, 2, 22, 77, 55, 23, 45];
    for i in test_cases {
        match table.add(&Name(i as u64)) {
            Ok(index) => table.set_address(index, i).unwrap(),
            _ => 0,
        };
    }
    assert_eq!(table.size(), test_cases.len());

    assert_eq!(table.entry(&Name(test_cases[2] as u64)).ok(), Some(3)); // 4 index=3
    assert_eq!(table.entry(&Name(test_cases[6] as u64)).ok(), Some(9));
    assert_eq!(table.entry(&Name(test_cases[1] as u64)).ok(), Some(0));

    for i in 1..test_cases.len() {
        assert_eq!(true, table.name(i - 1) < table.name(i));
    }
    // Implicit add
    assert_eq!(table.entry(&Name(33 as u64)).ok(), Some(7));
}
