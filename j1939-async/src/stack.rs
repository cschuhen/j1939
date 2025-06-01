#![deny(warnings)]

use crate::can::ApiId;
use crate::can::Id;
use crate::error::*;
const FILE_CODE: u8 = 0xFD;

#[cfg(feature = "embassy")]
use crate::os::*;

type Error = crate::error::Error;

pub struct Stack {
    pub name_manager: crate::name::Manager,
    suspended: bool,
} // struct Stack

impl Stack {
    pub fn new() -> Stack {
        Stack {
            name_manager: crate::name::Manager::new(),
            suspended: false,
        }
    }

    pub fn suspend(&mut self) {
        self.name_manager.disable();
        self.suspended = true;
    }

    pub fn resume(&mut self) {
        self.name_manager.enable();
        self.suspended = false;
    }

    pub fn is_suspended(&self) -> bool {
        self.suspended
    }

    pub fn register_local_name(
        &mut self,
        iface: &mut dyn crate::can::CanInterface,
        address: u8,
        name: &crate::name::Name,
    ) -> Result<bool, Error> {
        self.name_manager.register_local_name(iface, address, name)
    }

    pub fn on_can_frame(
        &mut self,
        iface: &mut dyn crate::can::CanInterface,
        id: ApiId,
        data: &crate::can::FrameData,
    ) -> Result<bool, Error> {
        match id.pgn() {
            crate::pgn::ADDRESS_CLAIMED => self.name_manager.on_can_frame(iface, id, data),
            crate::pgn::REQUEST => self.name_manager.on_can_frame(iface, id, data),
            _ => Ok(false),
        }
    }

    pub fn advance_time(
        &mut self,
        iface: &mut dyn crate::can::CanInterface,
        time_delta_ms: usize,
    ) -> Result<bool, Error> {
        self.name_manager.advance_time(iface, time_delta_ms)
    }

    pub fn get_next_timeout_ms(&self) -> Option<u32> {
        self.name_manager.get_next_timeout_ms()
    }

    pub fn send_to_address(
        &mut self,
        iface: &mut dyn crate::can::CanInterface,
        src: &crate::name::Name,
        dest: u8,
        pgn: u32,
        priority: u8,
        data: &[u8],
    ) -> Result<(), Error> {
        let address = self.name_manager.resolve_local(&src)?;

        let id = match crate::can::new_id(pgn, address, dest, priority) {
            Some(id) => id,
            None => {
                return Err(mkerr(
                    FILE_CODE,
                    crate::error::ErrorCode::InvalidId,
                    line!(),
                ))
            }
        };

        match iface.transmit(crate::can::Frame::from_slice(id.as_raw(), data)) {
            Ok(_) => {}
            Err(_) => {
                return Err(mkerr(FILE_CODE, ErrorCode::NoSpace, line!()));
            }
        }

        Ok(())
    }

    pub fn send_broadcast(
        &mut self,
        iface: &mut dyn crate::can::CanInterface,
        src: &crate::name::Name,
        pgn: u32,
        priority: u8,
        data: &[u8],
    ) -> Result<(), Error> {
        self.send_to_address(iface, src, 0xFF, pgn, priority, data)
    }
}

#[cfg(feature = "embassy")]
pub struct SharedStack {
    stack: StackInMutex,
}

#[cfg(feature = "embassy")]
impl SharedStack {
    pub fn new() -> SharedStack {
        SharedStack {
            stack: StackInMutex::new(crate::stack::Stack::new()),
        }
    }

    pub async fn suspend(&mut self) {
        let mut guard = self.stack.lock().await;
        guard.suspend();
    }

    pub async fn resume(&mut self) {
        let mut guard = self.stack.lock().await;
        guard.resume();
    }

    pub async fn is_suspended(&self) -> bool {
        let guard = self.stack.lock().await;
        guard.is_suspended()
    }

    pub async fn register_local_name(
        &self,
        iface: &mut dyn crate::can::CanInterface,
        address: u8,
        name: &crate::name::Name,
    ) -> Result<bool, Error> {
        let mut guard = self.stack.lock().await;
        guard.register_local_name(iface, address, name)
    }

    pub async fn get_next_timeout_ms(&self) -> Option<u32> {
        let guard = self.stack.lock().await;
        guard.get_next_timeout_ms()
    }

    pub async fn on_can_frame(
        &self,
        iface: &mut dyn crate::can::CanInterface,
        id: ApiId,
        data: &crate::can::FrameData,
    ) -> Result<bool, Error> {
        let mut guard = self.stack.lock().await;
        guard.on_can_frame(iface, id, data)
    }

    pub async fn advance_time(
        &self,
        iface: &mut dyn crate::can::CanInterface,
        time_delta_ms: usize,
    ) -> Result<bool, Error> {
        let mut guard = self.stack.lock().await;
        guard.advance_time(iface, time_delta_ms)
    }

    pub async fn send_to_address(
        &self,
        iface: &mut dyn crate::can::CanInterface,
        src: &crate::name::Name,
        dest: u8,
        pgn: u32,
        priority: u8,
        data: &[u8],
    ) -> Result<(), Error> {
        let mut guard = self.stack.lock().await;
        guard.send_to_address(iface, src, dest, pgn, priority, data)
    }

    pub async fn send_broadcast(
        &self,
        iface: &mut dyn crate::can::CanInterface,
        src: &crate::name::Name,
        pgn: u32,
        priority: u8,
        data: &[u8],
    ) -> Result<(), Error> {
        let mut guard = self.stack.lock().await;
        guard.send_broadcast(iface, src, pgn, priority, data)
    }
}
