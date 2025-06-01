use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
pub type StackInMutex = Mutex<CriticalSectionRawMutex, crate::stack::Stack>;
