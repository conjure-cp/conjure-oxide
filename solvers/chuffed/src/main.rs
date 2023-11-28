use chuffed_rs::bindings::{
    new_xyz_problem, solve_xyz
};

// Entry point for running this problem
fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        println!("Invalid number of arguments");
        return;
    }

    let n: i32 = args[1].parse().expect("Invalid input");

    unsafe {
        let p = new_xyz_problem(n);
        solve_xyz(p);
        
    }
}
