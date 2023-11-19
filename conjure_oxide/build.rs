use std::{
    env::var,
    fs::File,
    fs::{read_dir, DirEntry},
    io::Write,
    path::Path,
};

fn main() {
    let out_dir = var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("gen_tests.rs");
    let mut f = File::create(&dest).unwrap();

    let test_dir = "tests/integration";
    for dir in read_dir(test_dir).unwrap() {
        write_test(&mut f, &dir.unwrap());
    }
}

fn write_test(file: &mut File, dir: &DirEntry) {
    let binding = dir.path();
    let path = binding.to_str().unwrap();
    write!(
        file,
        include_str!("./tests/gen_test_template"),
        name = path.replace("./", "").replace("/", "_"),
        path = path
    )
    .unwrap();
}
