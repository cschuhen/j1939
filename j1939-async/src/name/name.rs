use crate::error::mkerr;

type Error = crate::error::Error;
type ErrorCode = crate::error::ErrorCode;
const FILE_CODE: u8 = 0xFE;

bitfield::bitfield! {
    #[derive(Copy, Clone, Eq, PartialOrd, Ord, PartialEq, defmt::Format)]
    /// J1938 / ISO11783 / NMEA2000 NAME
    pub struct Name(u64);
    impl Debug;
    u64;
    #[inline]
    pub u64, raw, set_raw: 63, 0;
    pub u32, identity, set_identity: 20, 0;
    pub u32, manufacturer, set_manufacturer: 31, 21;
    pub u32, ecu_instance, set_ecu_instance: 34, 32;
    pub u64, function_instance, set_function_instance: 39, 35;
    pub u64, function, set_function: 47, 40;
    //pub _, _: 48; // reserved
    pub u64, vehicle_system, set_vehicle_system: 55, 49;
    pub u32, vehicle_system_instance, set_vehicle_system_instance: 59, 56;
    pub u32, industry_group, set_industry_group: 62, 60;
    pub self_configurable, set_self_configurable: 63;

}

pub struct NameBytesIter<'a> {
    nameref: &'a Name,
    index: u8,
}

impl Iterator for NameBytesIter<'_> {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        if self.index >= 64 {
            return None;
        } else {
            let result = self.nameref.raw() >> self.index;
            self.index += 8;
            Some(result as u8)
        }
    }
}

impl Name {
    pub fn create(
        self_configurable: bool,
        industry_group: u32,
        vehicle_system_instance: u32,
        vehicle_system: u64,
        function: u64,
        function_instance: u64,
        ecu_instance: u32,
        manufacturer: u32,
        identity: u32,
    ) -> Name {
        let mut ret = Name(0);

        ret.set_self_configurable(self_configurable);
        ret.set_industry_group(industry_group);
        ret.set_vehicle_system_instance(vehicle_system_instance);
        ret.set_vehicle_system(vehicle_system);
        ret.set_function(function);
        ret.set_function_instance(function_instance);
        ret.set_ecu_instance(ecu_instance);
        ret.set_manufacturer(manufacturer);
        ret.set_identity(identity);
        ret
    }
    pub fn from_bytes(buf: &[u8]) -> Result<Name, Error> {
        if buf.len() < 8 {
            return Err(mkerr(FILE_CODE, ErrorCode::NotEnoughBytes, line!()));
        }
        let mut raw: u64 = 0;
        for n in 0..8 {
            raw |= (buf[n] as u64) << (8 * n);
        }
        Ok(Name(raw))
    }
    pub fn set_bytes(&mut self, buf: &[u8; 8]) {
        let mut raw: u64 = 0;
        for n in 0..8 {
            raw |= (buf[n] as u64) << (8 * n);
        }
        self.set_raw(raw);
    }
    pub fn bytes_iter(&self) -> NameBytesIter {
        NameBytesIter {
            nameref: self,
            index: 0,
        }
    }
}

/// Generate a identity, sutiable for a NAME by xoring the bytes suppled to get it down to 21 bits.
pub fn identiy_from_bytes(idbuf: &[u8]) -> u32 {
    let mut ret: u32 = 0;

    let mask: u32 = 0x1F_FF_FF;

    let mut shift: u32 = 0;

    for byte in idbuf {
        ret ^= (*byte as u32) << shift;
        shift += 8;
        if shift >= 21 {
            let remain = ret & !mask;
            ret = (ret & mask) ^ (remain >> 21);
            shift -= 21;
        }
    }

    ret
}

#[cfg(test)]
mod name_tests {
    use crate::name::Name;

    #[test]
    fn name_struct() {
        let mut name = Name(0);
        let bytes: [u8; 8] = [0x6e, 0x83, 0x0d, 0x46, 0x00, 0x3e, 0x00, 0x80];
        name.set_bytes(&bytes);

        let bytes_from_iter: Vec<u8> = name.bytes_iter().collect();
        assert_eq!(bytes.to_vec(), bytes_from_iter);

        assert_eq!(name.raw(), 0x8000_3e00_460d_836e);
        assert_eq!(name.identity(), 885614);
        assert_eq!(name.manufacturer(), 560);
        assert_eq!(name.ecu_instance(), 0);
        assert_eq!(name.function_instance(), 0);
        assert_eq!(name.function(), 62);
        assert_eq!(name.vehicle_system(), 0);
        assert_eq!(name.vehicle_system_instance(), 0);
        assert_eq!(name.industry_group(), 0);
        assert_eq!(name.self_configurable(), true);

        assert_eq!(name, Name::from_bytes(&bytes).unwrap());

        let all_on = Name::create(true, 0x7, 0xf, 0x7f, 0xff, 0x1f, 0x7, 0x7ff, 0xffffffff);
        //all_on.set_reserved(true);
        assert_eq!(all_on.raw(), 0xFFFe_FFFF_FFFF_FFFFu64); // Just reserved bit false.
    }

    #[test]
    fn non_configurable_have_priority() {
        let mut all_sc = Name(0xFFFF_FFFF_FFFF_FFFFu64);
        let mut none_sc = Name(0x0000_0000_0000_0000u64);
        let mut all_nsc = Name(0xFFFF_FFFF_FFFF_FFFFu64);
        let mut none_nsc = Name(0x0000_0000_0000_0000u64);
        all_sc.set_self_configurable(true);
        none_sc.set_self_configurable(true);
        all_nsc.set_self_configurable(false);
        none_nsc.set_self_configurable(false);
        // Lower ID has priority
        assert_eq!(true, all_nsc.raw() < all_sc.raw());
        assert_eq!(true, all_nsc < all_sc);
        assert_eq!(true, none_nsc < none_sc);
    }
}

/*
mod events {
    use crate::can::FrameData;



    #[derive(Copy, Clone)]
    pub enum Event {
        None(),
        NameReady(u8)
    }

    pub trait EventSender<E> {
        fn send(&mut self, event: &E) -> bool;
    }

    struct EventSenderList<'a, E, const N: usize> {
        event_receivers: heapless::Vec<& 'a mut dyn EventSender<E>, N>
        //event_receivers: T
    }

    impl<'a, E, const N: usize> EventSenderList<'a, E, N> {

        fn new() -> EventSenderList<'a, E, N> {//heapless::Vec<&'static dyn EventSender<E>, N>> {
            let mut vec: heapless::Vec<&'a mut dyn EventSender<E>, N> = heapless::Vec::new();
            EventSenderList {
                event_receivers: vec
            }
        }

        fn add_sender(&mut self, sender: &'a mut dyn EventSender<E> ) {
            self.event_receivers.push(sender);
        }

        fn send(&mut self, event: &E) {
            for sender in self.event_receivers.iter_mut() {
                sender.send(event);
            }
        }
        //fn add_sender(&mut self, )

    }

    #[test]
    fn event_list() {


        struct TestEventReceiver {
            bah: std::sync::Arc<Mutex<Event>>
        }

        impl TestEventReceiver {
            fn new(v : std::sync::Arc<Mutex<Event>>) ->TestEventReceiver {
                TestEventReceiver { bah: v }
            }
        }

        impl EventSender<Event> for TestEventReceiver {
            fn send(&mut self, event: &Event) -> bool {
                //print!("EVENT FIRE\n");
                let mut unlocked = self.bah.lock().unwrap();
                *unlocked = *event;
                true
            }

        }
        let mut value = 5;
        use std::sync::{Arc, Mutex};
        let sender_int = Arc::new(Mutex::new(
            Event::None()
        ));

        assert!(matches!(*sender_int.lock().unwrap(), Event::None()));

        let mut sender = TestEventReceiver::new(
            Arc::clone(&sender_int)
        );
        //let mut sender = TestEventReceiver::new(value);
        let event = Event::NameReady((0));
        sender.send(&event);

        assert!(matches!(*sender_int.lock().unwrap(), Event::NameReady(0)));

        let mut senders = EventSenderList::<Event, 8>::new();
        senders.add_sender(& mut sender);


        senders.send(&Event::NameReady((1)));
        assert!(matches!(*sender_int.lock().unwrap(), Event::NameReady(1)));



    }

}
*/
