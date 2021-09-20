use std::{
    fs,
    io::prelude::*,
    path::{Path, PathBuf},
};
use std::time;
use std::thread;

pub fn get_file_name(dir: &Path, id: usize) -> PathBuf {
    let file_name = format!("id:{:06}", id);
    dir.join(file_name)
}

pub fn read_from_file(path: &Path) -> Option<Vec<u8>> {
    let mut file;
    let mut i = 0;
    loop {
        match fs::File::open(path) {
            Ok(f) => {
                file = f;
                break;
            }
            Err(e) => {
                error!("fail to read from file : ({:?}) -- {:?}", path, e);
            }
        };
        i += 1;
        if i == 10 {
            thread::sleep(time::Duration::from_millis(10));
            return None
            // panic!();
        }
    }

    let mut buf = Vec::new();

    match file.read_to_end(&mut buf) {
        Ok(_) => (),
        Err(error) =>  { warn!("Failed to read to end on file {:?} error {:?}", path, error); return None; },
    };

    Some(buf)
}
