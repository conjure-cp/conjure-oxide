use std::{io::Write, fs::File, path::Path, fs::{read_dir, DirEntry}, env::var};

fn main() {
    let out_dir = var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("gen_tests.rs");
    let mut f = File::create(&dest).unwrap();

    for dir in read_dir("./tests/integration/").unwrap() {
        write_test(&mut f, &dir.unwrap());
    }
}

fn write_test(file: &mut File, dir: &DirEntry) {
    write!(
        file,
        include_str!("./tests/gen_test_template"),
        name = dir.file_name().to_str().unwrap(),
        path=dir.path().to_str().unwrap()
    ).unwrap();
}
