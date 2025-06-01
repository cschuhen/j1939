pub type RawCanId = u32;
pub type FrameData = heapless::Vec<u8, 8>;
pub type Address = u8;
pub type Priority = u8; // 3 bits

pub(crate) type ApiId = embedded_can::ExtendedId;

pub fn new_id(
    pgn: u32,
    source: Address,
    destination: Address,
    priority: Priority,
) -> Option<embedded_can::ExtendedId> {
    let mut id = 0u32;

    if priority > 0x7 || source == 255 || destination == 254 || pgn > 0x3FFFF {
        return None;
    }

    id |= source as u32;
    id |= ((priority & 0x7) as u32) << 26;
    let pgn = pgn & 0x3FFFF;
    match pgn >= 0xF000 {
        true => id |= pgn << 8,
        false => {
            if pgn & 0xFF != 0 {
                return None;
            }
            id |= ((pgn & 0x3FF00) | (destination as u32)) << 8
        }
    }
    embedded_can::ExtendedId::new(id)
}

pub fn new_id_unchecked(
    _pgn: u32,
    source: Address,
    destination: Address,
    priority: u8,
) -> embedded_can::ExtendedId {
    let mut id = 0u32;
    id |= source as u32;
    id |= ((priority & 0x7) as u32) << 26;
    let pgn = _pgn & 0x3FFFF;
    match pgn >= 0xF000 {
        true => id |= pgn << 8,
        false => id |= ((pgn & 0x3FF00) | (destination as u32)) << 8,
    }
    embedded_can::ExtendedId::new(id).unwrap()
}

pub trait Id {
    fn as_raw(&self) -> RawCanId;

    fn pdu_format(&self) -> RawCanId {
        (self.as_raw() >> 16) & 0xFFu32
    }

    fn pdu_speciffic(&self) -> RawCanId {
        (self.as_raw() >> 8) & 0xFFu32
    }

    fn pdu2(&self) -> bool {
        self.pdu_format() >= 0xF0u32
    }

    fn unicast_pgn(&self) -> bool {
        !self.pdu2()
    }

    fn broadcast_pgn(&self) -> bool {
        self.pdu2()
    }

    fn source(&self) -> u8 {
        (self.as_raw() & 0xFF) as u8
    }

    fn destination(&self) -> u8 {
        match self.pdu2() {
            true => 255,
            false => self.pdu_speciffic() as u8,
        }
    }

    fn pgn(&self) -> RawCanId {
        match self.pdu2() {
            true => (self.as_raw() >> 8) & 0x3FFFF,
            false => (self.as_raw() >> 8) & 0x3FF00,
        }
    }

    fn priority(&self) -> u8 {
        ((self.as_raw() >> 26) & 0x07) as u8
    }
}

impl Id for embedded_can::ExtendedId {
    fn as_raw(&self) -> RawCanId {
        self.as_raw()
    }
}

#[cfg(test)]
mod id_tests {
    use crate::can::Id;
    #[test]
    fn pgns() {
        //let id = embedded_hal::can::ExtendedId(1234);
        let id = embedded_can::ExtendedId::new(0x00eeffcc).unwrap();
        assert_eq!(0xff, id.pdu_speciffic());
        assert_eq!(0xee00, id.pgn());

        let id1 = embedded_can::ExtendedId::new(0x18eefff8).unwrap();
        assert_eq!(id1.pgn(), 0xee00);
        assert_eq!(id1.source(), 248);
        assert_eq!(id1.destination(), 255);
        assert_eq!(id1.priority(), 6);
        assert_eq!(id1.pdu2(), false);
        assert_eq!(id1.unicast_pgn(), true);
        assert_eq!(id1.broadcast_pgn(), false);

        let id2 = crate::can::new_id(0xEE00, 154, 0x55, 6).unwrap();
        assert_eq!(id2.unicast_pgn(), true);
        assert_eq!(id2.pgn(), 0xee00);
        assert_eq!(id2.source(), 154);
        assert_eq!(id2.destination(), 0x55);
        assert_eq!(id2.priority(), 6);
    }
}

#[derive(Debug)]
pub struct Frame {
    m_id: ApiId,
    m_data: FrameData,
}

impl Frame {
    pub fn from_slice(id_: RawCanId, data_: &[u8]) -> Frame {
        Frame {
            m_id: ApiId::new(id_).unwrap(),
            m_data: FrameData::from_slice(data_).unwrap(),
        }
    }
    pub fn from_iter<'a, I>(id_: RawCanId, data_: I) -> Frame
    where
        I: Iterator<Item = u8>,
    {
        Frame {
            m_id: ApiId::new(id_).unwrap(),
            m_data: FrameData::from_iter(data_),
        }
    }
    pub fn id(self: &Self) -> ApiId {
        self.m_id
    }
    pub fn data(self: &Self) -> &FrameData {
        &self.m_data
    }
}

pub struct DummyCanInerface {
    pub tx_queue: heapless::spsc::Queue<Frame, 10>,
}

pub trait CanInterface {
    fn transmit(&mut self, frame: Frame) -> Result<(), Frame>;
}

impl DummyCanInerface {
    pub fn new() -> DummyCanInerface {
        DummyCanInerface {
            tx_queue: heapless::spsc::Queue::new(),
        }
    }
}

impl CanInterface for DummyCanInerface {
    fn transmit(&mut self, frame: Frame) -> Result<(), Frame> {
        self.tx_queue.enqueue(frame)?;
        Ok(())
    }
}
