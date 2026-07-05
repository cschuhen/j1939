//#![deny(unsafe_code)]
#![deny(warnings)]

#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Error {
    pub ebytes: [u8; 8],
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum ErrorCode {
    CheckPoint = 0, // Not an error
    NotEnoughBytes = 1,
    NameTableFull = 2,
    SearchFail = 3,
    NoExist = 4,
    NotResolved = 5,
    NotLocal = 6,
    NoSpace = 7,
    NotEnabled = 8,
    InvalidId = 9,
}

pub fn mkerr_base(file_code: u8, code: u8, line: u32, detail: u32) -> crate::error::Error {
    let mut ebytes = [0u8; 8];
    ebytes[0] = file_code;
    ebytes[1] = code as u8;
    ebytes[2] = (line & 0xFF) as u8;
    ebytes[3] = ((line >> 8) & 0xFF) as u8;
    ebytes[4] = (detail & 0xFF) as u8;
    ebytes[5] = ((detail >> 8) & 0xFF) as u8;
    ebytes[6] = ((detail >> 16) & 0xFF) as u8;
    ebytes[7] = ((detail >> 24) & 0xFF) as u8;
    crate::error::Error { ebytes }
}

pub fn mkerr_ptr(
    file_code: u8,
    code: u8,
    line: u32,
    detail_ptr: *const u8,
    detail_size: usize,
) -> crate::error::Error {
    let mut ebytes = [0u8; 8];
    ebytes[0] = file_code;
    ebytes[1] = code as u8;
    ebytes[2] = (line & 0xFF) as u8;
    ebytes[3] = ((line >> 8) & 0xFF) as u8;
    unsafe {
        for i in 0..4 {
            ebytes[4 + i] = if i < detail_size {
                *(detail_ptr.add(i))
            } else {
                0
            };
        }
    }

    crate::error::Error { ebytes }
}

pub fn mkerr_generic<D>(file_code: u8, code: u8, line: u32, detail: D) -> Error {
    mkerr_ptr(
        file_code,
        code,
        line,
        &detail as *const D as *const u8,
        core::mem::size_of::<D>(),
    )
}

pub fn convert_if_err<R, E>(
    result: Result<R, E>,
    file_code: u8,
    code: u8,
    line: u32,
) -> Result<R, crate::error::Error> {
    match result {
        Ok(res) => Ok(res),

        Err(_) => Err(mkerr_generic(file_code, code, line, 0)),
    }
}

pub fn mkerr(file_code: u8, code: crate::error::ErrorCode, line: u32) -> crate::error::Error {
    mkerr_base(file_code, code as u8, line, 0)
}

pub fn mkerr_u32(
    file_code: u8,
    code: crate::error::ErrorCode,
    line: u32,
    detail: u32,
) -> crate::error::Error {
    mkerr_base(file_code, code as u8, line, detail)
}

pub trait SendError {
    fn send(&mut self, error: &Error);

    fn report(&mut self, file_code: u8, code: u8, line: u32) {
        self.send(&mkerr_base(file_code, code as u8, line, 0));
    }
    fn reportd(&mut self, file_code: u8, code: u8, line: u32, detail: u32) {
        self.send(&mkerr_base(file_code, code as u8, line, detail));
    }
}
