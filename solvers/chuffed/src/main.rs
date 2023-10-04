use chuffed_rs::bindings::createVar;
use chuffed_rs::bindings::IntVar;
pub fn main() {
    let mut var = std::mem::MaybeUninit::<IntVar>::uninit();
    unsafe {
        createVar(&mut var.as_mut_ptr(), 0, 5, false);
    }
}
