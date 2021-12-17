use core::panic::PanicInfo;

use crate::console::kprintln;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kprintln!("======================= PANIC ====================");
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        kprintln!("Message: {:?}", s);
    }
    if let Some(location) = info.location() {
        kprintln!("File: {}", location.file());
        kprintln!("Line: {}", location.line());
    } else {
        kprintln!("<no location information>!");
    }

    if let Some(m) = info.message() {
        kprintln!("");
        kprintln!("Message: {:?}", m);
    }
    loop {}
}
