use std::{
    error::Error,
    fs::{self, File},
    io::Write,
    path::Path,
};

const SOURCE_DIR: &str = "../assets/images/";

fn main() -> Result<(), Box<dyn Error>> {
    let dest_path = Path::new(&"../assets/").join("images.rs");
    let mut all_the_files = File::create(&dest_path)?;

    writeln!(&mut all_the_files, r##"["##,)?;

    for f in fs::read_dir(SOURCE_DIR)? {
        let f = f?;

        if !f.file_type()?.is_file() {
            continue;
        }

        writeln!(
            &mut all_the_files,
            r##"("{name}", include_bytes!(r#"{name}"#)),"##,
            name = f.path().display(),
        )?;
    }

    writeln!(&mut all_the_files, r##"]"##,)?;

    Ok(())
}
