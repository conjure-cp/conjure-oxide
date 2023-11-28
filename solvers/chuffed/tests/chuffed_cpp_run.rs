use chuffed_rs::bindings::{
    new_xyz_problem, solve_xyz
};


#[test]
fn run_cpp_problem() {
    let n: i32 = 1; 

    unsafe {

        let p = new_xyz_problem(n);
        solve_xyz(p);
        
        // Pass test if no crash occurs
        assert!(true); 
    }
}
