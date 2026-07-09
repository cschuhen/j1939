use crate::can::CanInterface;
use crate::can::Frame;
use crate::can::FrameData;
use crate::error::mkerr;
use crate::pgn;
type Error = crate::error::Error;
const FILE_CODE: u8 = 0xFC;

const BROADCAST_ADDRESS: u8 = 0xFF; // 255
const NAME_TIMEOUT_RESOLUTION: u8 = 10;
const NAME_TIMEOUT_RECALL: u8 = 1;
const NAME_CLAIM_TIMEOUT: u8 = 250 / NAME_TIMEOUT_RESOLUTION;
const CLAIM_PRIORITY: u8 = 5;
use crate::name::name_table::NULL_ADDRESS;

use crate::name::name_table::NameEntry;
use crate::name::name_table::NameState;
use crate::name::name_table::NameTable;

#[derive(Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
enum NameManagerState {
    Disabled,
    Init,
    WaitOtherNames,
    //LocalClaimsWaiting,
    Active,
}

impl NameManagerState {
    fn running(&self) -> bool {
        match self {
            NameManagerState::Disabled => false,
            NameManagerState::Init => false,
            NameManagerState::WaitOtherNames => false,
            //NameManagerState::LocalClaimsWaiting => true,
            NameManagerState::Active => true,
        }
    }
}

pub struct NameManager {
    table: NameTable,
    next_timeout: Option<u8>,
    time_delta_remander: usize,
    manager_state: NameManagerState,
    manager_state_timeout: u8,
}

impl NameManager {
    pub fn new() -> NameManager {
        NameManager {
            table: NameTable::new(),
            next_timeout: Some(0),
            time_delta_remander: 0,
            manager_state: NameManagerState::Init,
            manager_state_timeout: 0,
        }
    }

    pub fn disable(&mut self) {
        self.to_state(NameManagerState::Disabled);
        self.next_timeout = None;
    }

    pub fn enable(&mut self) {
        if self.manager_state != NameManagerState::Disabled {
            return;
        }
        self.to_state(NameManagerState::Init);
    }

    fn to_state(&mut self, state: NameManagerState) {
        #[cfg(feature = "defmt")]
        defmt::println!("NameManager state={:?} -> {:?}", self.manager_state, state);
        match state {
            NameManagerState::Disabled => {}
            NameManagerState::Init => {
                self.table.reset_state(line!());
                self.next_timeout = Some(250 / NAME_TIMEOUT_RESOLUTION);
            }
            NameManagerState::WaitOtherNames => {}
            NameManagerState::Active => {}
        }
        self.manager_state = state;
    }

    fn expect_enabled(&self) -> Result<(), Error> {
        if self.manager_state == NameManagerState::Disabled {
            return Err(mkerr(
                FILE_CODE,
                crate::error::ErrorCode::NotEnabled,
                line!(),
            ));
        }
        Ok(())
    }

    pub fn find_spare_address(&self, pref: u8) -> Option<u8> {
        let mut cur = pref;
        loop {
            match self.table.index_at_address(cur) {
                None => {
                    return Some(cur);
                }
                Some(_) => {
                    if cur >= NULL_ADDRESS - 1 {
                        cur = 0
                    } else {
                        cur += 1
                    }
                    if cur == pref {
                        return None;
                    }
                }
            }
        }
    }
    pub fn get_next_timeout_ms(&self) -> Option<u32> {
        match self.next_timeout {
            Some(x) => Some(x as u32 * NAME_TIMEOUT_RESOLUTION as u32),
            None => None,
        }
    }

    fn send_claim(
        &mut self,
        iface: &mut dyn CanInterface,
        index: usize,
        info: &NameEntry,
        address: u8,
        destination: u8,
    ) -> Result<(), Error> {
        let id = crate::can::new_id_unchecked(
            pgn::ADDRESS_CLAIMED,
            address,
            destination,
            CLAIM_PRIORITY,
        );
        /*defmt::println!(
            "Send claim {} {} {} {:?} {:?}",
            line!(),
            index,
            address,
            destination,
            info.state(),
        );*/
        if iface
            .transmit(Frame::from_iter(
                id.as_raw(),
                self.table.name(index).bytes_iter(),
            ))
            .is_ok()
        {
            if info.address() == NULL_ADDRESS {
                // Finally sent our death notice
                self.table.set_state(index, NameState::DEAD, line!())?;
                self.clear_timeout(index)?;

            // If in active, just sending due to a request.
            } else if info.state() != NameState::ACTIVE {
                // Sending our claim, now need to wait 250ms
                self.table.set_state(index, NameState::CLAIMING, line!())?;
                self.set_name_timeout(index, NAME_CLAIM_TIMEOUT)?;
            }
        } else {
            // Queue full, recall in 10ms?
            self.set_name_timeout(index, NAME_TIMEOUT_RECALL)?;
        }
        Ok(())
    }
    fn kill_claim(&mut self, iface: &mut dyn CanInterface, index: usize) -> Result<usize, Error> {
        self.table.set_address(index, NULL_ADDRESS)?;

        let info = self.table.info(index)?;
        if info.local() {
            self.send_claim(iface, index, &info, NULL_ADDRESS, BROADCAST_ADDRESS)?;
        } else {
            self.table.set_state(index, NameState::DEAD, line!())?;
            self.table.clear_timeout(index)?;
        }

        Ok(index)
    }

    pub fn request_names(&mut self, iface: &mut dyn CanInterface, src: u8) -> Result<(), Error> {
        self.expect_enabled()?;
        let id = crate::can::new_id(pgn::REQUEST, src, 0xff, 5).unwrap();
        #[cfg(feature = "defmt")]
        defmt::println!("Request names");
        match iface.transmit(crate::can::Frame::from_slice(
            id.as_raw(),
            &pgn::encode(pgn::ADDRESS_CLAIMED), //&data
        )) {
            Ok(_) => Ok(()),
            Err(_) => Err(mkerr(FILE_CODE, crate::error::ErrorCode::NoSpace, line!())),
        }
    }

    fn set_name_timeout(self: &mut Self, index: usize, timeout: u8) -> Result<usize, Error> {
        self.next_timeout = match self.next_timeout {
            None => Some(timeout),
            Some(x) => Some(core::cmp::min(x, timeout)),
        };
        self.table.set_timeout(index, timeout)
    }
    fn set_manager_timeout(self: &mut Self, timeout: u8) {
        self.next_timeout = match self.next_timeout {
            None => Some(timeout),
            Some(x) => Some(core::cmp::min(x, timeout)),
        };
        self.manager_state_timeout = timeout;
    }
    fn clear_timeout(self: &mut Self, index: usize) -> Result<usize, Error> {
        self.table.clear_timeout(index)
    }
    fn accept_claim(
        &mut self,
        iface: &mut dyn CanInterface,
        index: usize,
        address: u8,
    ) -> Result<usize, Error> {
        self.table.set_address(index, address)?;
        //print!("Claim Accept New {} {} {}\n", index, address, self.table.info(index).unwrap().address());

        let info = self.table.info(index)?;
        if info.local() {
            match self.manager_state {
                NameManagerState::Disabled => {
                    return Err(mkerr(
                        FILE_CODE,
                        crate::error::ErrorCode::NotEnabled,
                        line!(),
                    ));
                }
                NameManagerState::Init => {
                    self.table.set_state(index, NameState::INIT, line!())?;
                    //self.set_name_timeout(index, NAME_CLAIM_TIMEOUT)?;
                }
                NameManagerState::WaitOtherNames => {
                    self.table.set_state(index, NameState::INIT, line!())?;
                    //self.set_name_timeout(index, self.manager_state_timeout)?;
                }
                NameManagerState::Active => {
                    self.send_claim(iface, index, &info, address, BROADCAST_ADDRESS)?;
                }
            }
        } else {
            self.table.set_state(index, NameState::CLAIMING, line!())?;
            self.set_name_timeout(index, NAME_CLAIM_TIMEOUT)?;
        }

        Ok(index)
    }

    // Sent at bootup after waiting for other names.
    fn send_local_claims(
        &mut self,
        iface: &mut dyn CanInterface,
        for_addr: u8,
    ) -> Result<bool, Error> {
        if for_addr == 255 {
            for index in 0..self.table.size() {
                let info = self.table.info(index).unwrap();
                if !info.local() {
                    continue;
                }
                #[cfg(feature = "defmt")]
                defmt::println!(
                    "Send local claim {} {} {:?}\n",
                    line!(),
                    index,
                    info.address()
                );
                self.send_claim(iface, index, &info, info.address(), BROADCAST_ADDRESS)?;
            }
        } else {
            match self.table.index_at_address(for_addr) {
                Some(index) => {
                    let info = self.table.info(index)?;
                    if info.local() {
                        self.send_claim(iface, index, &info, info.address(), BROADCAST_ADDRESS)?;
                    }
                }
                None => {}
            }
        }
        return Ok(true);
    }

    pub fn advance_time(
        &mut self,
        iface: &mut dyn CanInterface,
        mut time_delta_ms: usize,
    ) -> Result<bool, Error> {
        time_delta_ms += self.time_delta_remander;
        if time_delta_ms > 255 {
            time_delta_ms = 250;
        }

        let time_delta = (time_delta_ms / (NAME_TIMEOUT_RESOLUTION as usize)) as u8;
        self.time_delta_remander = time_delta_ms % (NAME_TIMEOUT_RESOLUTION as usize);
        let cur_timeout = self.next_timeout;
        self.next_timeout = None;

        if self.manager_state == NameManagerState::Disabled {
            return Ok(false);
        }

        match self.manager_state {
            NameManagerState::Disabled => {
                return Ok(false);
            }
            NameManagerState::Init => {
                self.to_state(NameManagerState::WaitOtherNames);
                self.set_manager_timeout(NAME_CLAIM_TIMEOUT); // FIXME
                self.request_names(iface, NULL_ADDRESS)?;
            }
            NameManagerState::WaitOtherNames => {
                if self.manager_state_timeout <= time_delta {
                    self.manager_state_timeout = 0;
                    self.to_state(NameManagerState::Active);
                    self.send_local_claims(iface, BROADCAST_ADDRESS)?;
                } else {
                    self.set_manager_timeout(self.manager_state_timeout - time_delta);
                }
            }
            //NameManagerState::LocalClaimsWaiting => {}
            NameManagerState::Active => {
                // Fixme: This can probably be after the match.
                if cur_timeout.is_none() {
                    return Ok(true);
                }
            }
        }

        for index in 0..self.table.size() {
            let info = self.table.info(index).unwrap();
            if 0 == info.timeout() {
                continue;
            } // No timeout
            if info.timeout() > time_delta {
                // Decrement timeout by the full amount, but ensure it doesn't wrap below 0
                let new_timeout = info.timeout().saturating_sub(time_delta);
                self.set_name_timeout(index, new_timeout)?;
                continue;
            }

            // Expired
            self.clear_timeout(index)?;

            if info.state() == NameState::CLAIMING {
                // Promote to active state
                self.table.set_state(index, NameState::ACTIVE, line!())?;
                //print!("CLAIM ACTIVE address={}\n", info.address());
            } else if info.local() {
                // Fixme; Needed? shouldn't set timeout until running.
                if !self.manager_state.running() {
                    self.set_name_timeout(index, 10)?;
                    // Wait for state to change.
                    continue;
                }
                self.send_claim(iface, index, &info, info.address(), BROADCAST_ADDRESS)?;
            }
        }
        return Ok(true);
    }

    fn process_claim(
        &mut self,
        iface: &mut dyn CanInterface,
        source: u8,
        name: &crate::name::Name,
        local: bool,
    ) -> Result<bool, Error> {
        let index = self.table.entry(name)?;
        if local {
            self.table.set_local(index, local)?;
        }
        let info = self.table.info(index)?; // Copy

        // Dead NAME
        if source == NULL_ADDRESS || source == BROADCAST_ADDRESS {
            #[cfg(feature = "defmt")]
            defmt::println!("Claim dead {} {}\n", line!(), source);
            self.kill_claim(iface, index)?;
            return Ok(true);
        }

        if info.state() == NameState::DEAD
            || info.state() == NameState::INIT
            || info.address() != source
        {
            let current_clam = self.table.index_at_address(source);
            if current_clam.is_some() && current_clam.unwrap() != index {
                // Existing entry, who wins?
                let current_claim_index = current_clam.unwrap();
                let current_claim_name = self.table.get_name(current_claim_index);
                let current_claim_info = self.table.info(current_claim_index)?; // Copy
                let local_not_started =
                    current_claim_info.state() == NameState::INIT && current_claim_info.local();
                if local_not_started
                    || (current_claim_name.is_ok() && current_claim_name.unwrap() < *name)
                {
                    //print!("Claim New Win\n");
                    // New Claim wins

                    if current_claim_info.local() {
                        // Local claim, try another address
                        match self.find_spare_address(128) {
                            Some(new_add) => {
                                #[cfg(feature = "defmt")]
                                defmt::println!(
                                    "Claim local move {} {}->{}\n",
                                    line!(),
                                    source,
                                    new_add
                                );
                                self.accept_claim(iface, current_claim_index, new_add)?;
                            }
                            None => {
                                //  Couldn't find one
                                #[cfg(feature = "defmt")]
                                defmt::println!("Claim dead {} {}\n", line!(), source);
                                self.kill_claim(iface, current_claim_index)?;
                            }
                        }
                    } else {
                        #[cfg(feature = "defmt")]
                        defmt::println!("Claim dead {} {}\n", line!(), source);
                        self.kill_claim(iface, current_claim_index)?;
                    }
                    self.accept_claim(iface, index, source)?;
                } else {
                    //print!("Claim New Loose\n");
                    // Existing claim wins
                    if local {
                        // Local claim, try another address
                        match self.find_spare_address(128) {
                            Some(new_add) => {
                                #[cfg(feature = "defmt")]
                                defmt::println!(
                                    "Claim local move {} {}->{}\n",
                                    line!(),
                                    source,
                                    new_add
                                );
                                self.accept_claim(iface, index, new_add)?;
                            }
                            None => {
                                //  Couldn't find one
                                #[cfg(feature = "defmt")]
                                defmt::println!("Claim dead {} {}\n", line!(), source);
                                self.kill_claim(iface, index)?;
                            }
                        }
                    } else {
                        #[cfg(feature = "defmt")]
                        defmt::println!("Claim dead {} {}->{}\n", line!(), info.address(), source);
                        self.kill_claim(iface, index)?;
                    }
                }
            } else {
                //print!("Claim New {}\n", source);
                self.accept_claim(iface, index, source)?;
            }
        }

        // Nothing changed?

        Ok(true)
    }

    pub fn resolve(&self, name: &crate::name::Name) -> Result<crate::can::Address, Error> {
        self.expect_enabled()?;
        let info = self.table.info_from_name(name)?;
        if info.state() != NameState::ACTIVE {
            return Err(mkerr(
                FILE_CODE,
                crate::error::ErrorCode::NotResolved,
                line!(),
            ));
        }

        Ok(info.address())
    }

    pub fn resolve_local(&self, name: &crate::name::Name) -> Result<crate::can::Address, Error> {
        self.expect_enabled()?;
        let info = self.table.info_from_name(name)?;
        if !info.local() {
            return Err(mkerr(FILE_CODE, crate::error::ErrorCode::NotLocal, line!()));
        }
        if info.state() != NameState::ACTIVE {
            return Err(crate::error::mkerr_u32(
                FILE_CODE,
                crate::error::ErrorCode::NotResolved,
                line!(),
                info.state() as u32,
            ));
        }

        Ok(info.address())
    }

    pub fn resolve_local_else_null(&self, name: &crate::name::Name) -> u8 {
        match self.resolve_local(name) {
            Ok(addr) => addr,
            Err(_) => 254,
        }
    }

    pub fn register_local_name(
        &mut self,
        iface: &mut dyn CanInterface,
        address: u8,
        name: &crate::name::Name,
    ) -> Result<bool, Error> {
        #[cfg(feature = "defmt")]
        defmt::println!("Register local name {} {}\n", line!(), address);
        self.process_claim(iface, address, name, true)
    }

    pub fn on_can_frame(
        &mut self,
        iface: &mut dyn CanInterface,
        id: crate::can::ApiId,
        data: &FrameData,
    ) -> Result<bool, Error> {
        if self.manager_state == NameManagerState::Disabled {
            return Ok(false);
        }
        //print!("Name Got Frame pgn={}\n", id.pgn());
        #[cfg(feature = "defmt")]
        defmt::println!(
            "Name Got Frame pgn={}/{} len={}\n",
            id.pgn(),
            pgn::REQUEST,
            data.len()
        );
        use crate::can::Id;
        return match id.pgn() {
            pgn::ADDRESS_CLAIMED => match crate::name::Name::from_bytes(&data) {
                Ok(name) => {
                    #[cfg(feature = "defmt")]
                    defmt::println!("Remote claim {} {}\n", line!(), id.source());
                    self.process_claim(iface, id.source(), &name, false)
                }
                Err(e) => Err(e),
            },
            pgn::REQUEST if data.len() >= 3 && data[0..3] == pgn::encode(pgn::ADDRESS_CLAIMED) => {
                #[cfg(feature = "defmt")]
                defmt::println!("PR @{}", line!());

                self.send_local_claims(iface, id.destination())?;
                Ok(true)
            }
            _ => Ok(false),
        };
    }
}

/*fn send_frames(iface : & mut dyn CanInterface, man: & mut NameManager) {
    use crate::Id;
    loop {
        if let Some(pkt) = iface.get_packet() {
            let id = Id::new(pkt.id());
            match man.on_can_frame(iface, id, &pkt.data()) {
                Ok(true)    => print!("Packet handled\n"),
                Ok(false)   => print!("Packet NOT handled\n"),
                Err(err)    => print!("Packet error: {}\n",err),
            }
        } else {
            break;
        }
    }
}*/

/*fn j1939_frame(_pgn: u32, source: u8, destination:u8, priority:u8, data: &[u8]) -> Frame {
    let id = crate::Id::new_with_fields(_pgn, source, destination, priority);
    //print!("ID {:x} pgn={:x}->{:x}\n", id.as_raw(), _pgn, id.pgn());
    Frame::from_iter(id.as_raw(), data)
}*/

#[cfg(test)]
pub mod name_manager_tests {
    use crate::can::CanInterface;
    use crate::can::DummyCanInerface;
    use crate::can::FrameData;
    use crate::name::name_manager::NameManager;
    use crate::name::Name;

    #[test]
    fn name_manager() {
        let mut man = NameManager::new();
        let mut can = DummyCanInerface::new();

        assert_eq!(man.get_next_timeout_ms(), Some(0));
        assert_eq!(0, can.tx_queue.len());
        assert_eq!(Some(true), man.advance_time(&mut can, 0).ok());

        // Should be a request all names message
        assert_eq!(1, can.tx_queue.len());
        let frame = can.tx_queue.dequeue().unwrap();
        assert_eq!(frame.id().as_raw(), 0x14EAFFFE);
        assert_eq!(frame.data().as_slice(), &[0u8, 0xEEu8, 0u8]);
        assert_eq!(0, can.tx_queue.len());

        // Name manager needs to wait 250ms to init
        assert_eq!(man.get_next_timeout_ms(), Some(250));
        assert_eq!(Some(true), man.advance_time(&mut can, 250).ok());
        assert_eq!(0, can.tx_queue.len());

        print!(
            "Name SC {:x}\n",
            Name::create(true, 0, 0, 0, 0, 0, 0, 0, 0).raw()
        );
        let local_name = Name::from_bytes_iter([0u8, 0, 0, 0, 0, 0, 0, 0, 0].into_iter());

        print!("Register local name on 130\n");
        assert_eq!(
            Some(true),
            man.register_local_name(&mut can, 0x82, &local_name).ok()
        );
        assert_eq!(0xFE, man.resolve_local_else_null(&local_name)); // Not resolved yet

        // Expect address claim message
        assert_eq!(1, can.tx_queue.len());
        let frame = can.tx_queue.dequeue().unwrap();
        print!(
            "FRAME: {:x} {:?}\n",
            frame.id().as_raw(),
            frame.data().as_slice()
        );
        assert_eq!(frame.id().as_raw(), 0x14EEFF82);
        assert_eq!(frame.data().as_slice(), &[0, 0, 0, 0, 0, 0, 0, 0,]);
        assert_eq!(0, can.tx_queue.len());

        print!("Advance time by 200\n");
        assert_eq!(Some(true), man.advance_time(&mut can, 200).ok());
        assert_eq!(0xFE, man.resolve_local_else_null(&local_name)); // Not resolved yet
        print!("Advance time by 49\n");
        assert_eq!(Some(true), man.advance_time(&mut can, 49).ok());
        assert_eq!(0xFE, man.resolve_local_else_null(&local_name)); // Not resolved yet
        print!("Advance time by 1\n");
        assert_eq!(Some(true), man.advance_time(&mut can, 1).ok());
        assert_eq!(0x82, man.resolve_local_else_null(&local_name)); // Resolved

        assert_eq!(
            Some(true),
            man.on_can_frame(
                &mut can,
                crate::can::new_id(crate::pgn::ADDRESS_CLAIMED, 28, 255, 3).unwrap(),
                &FrameData::from_iter(Name::create(true, 0, 0, 0, 0, 0, 0, 0, 123).bytes_iter())
            )
            .ok()
        );
        print!("Advance time by 249\n");
        assert_eq!(Some(true), man.advance_time(&mut can, 249).ok());
        print!("Advance time by 1\n");
        assert_eq!(Some(true), man.advance_time(&mut can, 1).ok());
        assert_eq!(0, can.tx_queue.len());
    }

    #[test]
    fn conflict() {
        let our_name = Name::create(true, 0, 0, 0, 0, 0, 0, 0, 123);
        let them_name = Name::create(true, 0, 0, 0, 0, 0, 0, 0, 124);
        let mut man = NameManager::new();
        let mut can = DummyCanInerface::new();
        print!("US   Name SC {:x}\n", our_name.raw());
        print!("Then Name SC {:x}\n", them_name.raw());

        assert_eq!(man.get_next_timeout_ms(), Some(0));
        assert_eq!(0, can.tx_queue.len());
        assert_eq!(Some(true), man.advance_time(&mut can, 0).ok());

        // Should be a request all names message
        assert_eq!(1, can.tx_queue.len());
        let frame = can.tx_queue.dequeue().unwrap();
        assert_eq!(frame.id().as_raw(), 0x14EAFFFE);
        assert_eq!(frame.data().as_slice(), &[0u8, 0xEEu8, 0u8]);
        assert_eq!(0, can.tx_queue.len());

        // Name manager needs to wait 250ms to init
        assert_eq!(man.get_next_timeout_ms(), Some(250));
        assert_eq!(Some(true), man.advance_time(&mut can, 250).ok());
        assert_eq!(0, can.tx_queue.len());

        assert_eq!(
            Some(true),
            man.register_local_name(&mut can, 130, &our_name).ok()
        );

        // Expect address claim message
        assert_eq!(1, can.tx_queue.len());
        let frame = can.tx_queue.dequeue().unwrap();
        print!(
            "FRAME: {:x} {:?}\n",
            frame.id().as_raw(),
            frame.data().as_slice()
        );
        assert_eq!(frame.id().as_raw(), 0x14EEFF82);
        assert_eq!(
            frame.data().as_slice(),
            &our_name.bytes_iter().collect::<Vec<u8>>()
        );
        assert_eq!(0, can.tx_queue.len());

        print!(
            "XXXXX Advance time by 200 {:?}\n",
            man.get_next_timeout_ms()
        );
        assert_eq!(Some(true), man.advance_time(&mut can, 200).ok());
        assert_eq!(0xFE, man.resolve_local_else_null(&our_name)); // Not resolved yet
        print!("Advance time by 49\n");
        assert_eq!(Some(true), man.advance_time(&mut can, 49).ok());
        assert_eq!(0xFE, man.resolve_local_else_null(&our_name)); // Not resolved yet
        print!("Advance time by 1\n");
        assert_eq!(Some(true), man.advance_time(&mut can, 1).ok());
        assert_eq!(0x82, man.resolve_local_else_null(&our_name)); // Not resolved yet

        assert_eq!(
            Some(true),
            man.on_can_frame(
                &mut can,
                crate::can::new_id(crate::pgn::ADDRESS_CLAIMED, 130, 255, 3).unwrap(),
                &FrameData::from_iter(them_name.bytes_iter())
            )
            .ok()
        );
        print!("Advance time by 249\n");
        assert_eq!(Some(true), man.advance_time(&mut can, 249).ok());
        print!("Advance time by 1\n");
        assert_eq!(Some(true), man.advance_time(&mut can, 1).ok());
        assert_eq!(0x82, man.resolve_local_else_null(&our_name)); // Not resolved yet
        assert_eq!(0, can.tx_queue.len());
    }

    #[test]
    fn name_manager_junk() {
        use crate::can::Id;
        let mut _man = NameManager::new();
        let mut _can_dummy = DummyCanInerface::new();
        let _can: &mut dyn CanInterface = &mut _can_dummy;
        assert_eq!(
            crate::pgn::ADDRESS_CLAIMED,
            crate::can::new_id(crate::pgn::ADDRESS_CLAIMED, 1, 2, 3)
                .unwrap()
                .pgn()
        );
        //let name_1 = Name(0x1234_1234_1234_1234u64);

        //can.transmit(j1939_frame( crate::pgn::ADDRESS_CLAIMED, 2, 3, 4, &[0, 1, 2, 3, 4, 5, 6, 7])).unwrap();
        //send_frames(can, &mut man);
        //use crate::name_manager::NameManager;
        //let frame: Box<dyn embedded_hal::can::Frame> = Frame::new(0x123456, [0, 1, 2, 3, 4, 5, 6, 7]);
        //let frame  = bxcan::Frame::new_data(bxcan::ExtendedId::new(0x123456).unwrap(), [0, 1, 2, 3, 4, 5, 6, 7]);

        //let frame = bxcan::Frame::new(embedded_hal::can::ExtendedId::new(0).unwrap(), &[0, 1, 2, 3, 4, 5, 6, 7]);

        //fn do_something(frame: Box<dyn embedded_hal::can::Frame>){
        //
        //}
        //do_something(frame.unwrap());
        //struct Frame {id: RawCanId, len: FrameDataLen, data: [u8; 8]};

        //let mut sent_frames = Vec<Frame>::new();
        /*let mut sent_frames = Vec::new();

        let mut man = NameManager::new(
            Box::new(|id, len, data| {
                sent_frames.push(Frame{id, len, data});
                true})
        );
        */
    }
} // mod name_manager_tests
