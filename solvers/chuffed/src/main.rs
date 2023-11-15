use chuffed_rs::bindings::{
    new_problem, solve
};


// Entry point for running this problem
fn main() {

    unsafe {
        let p = new_problem();
        solve(p);
        
    }
}
