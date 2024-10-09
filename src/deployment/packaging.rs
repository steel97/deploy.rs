use core::fmt::Write;
use flate2::{write::GzEncoder, Compression};
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    fs::{self, metadata, File},
    io,
    path::{Path, PathBuf},
};

pub struct PackageCreator<'a> {
    server_hash_map: &'a HashMap<String, String>,
}

impl PackageCreator<'_> {
    pub fn new<'a>(server_hashes: &'a HashMap<String, String>) -> PackageCreator<'a> {
        PackageCreator {
            server_hash_map: server_hashes,
        }
    }

    pub fn prepare_package_for_target(self, local_temp_file: &File, local_dir: String) -> bool {
        let mut target_files: Vec<String> = Vec::new();
        for (key, val) in self.server_hash_map {
            // get hash
            let path = local_dir.clone() + key;
            let mut file = fs::File::open(&path).unwrap();

            let mut hasher = Sha1::new();
            io::copy(&mut file, &mut hasher).unwrap();
            let hash_bytes = hasher.finalize();
            let n = hash_bytes.len();
            let mut s = String::with_capacity(2 * n);
            for byte in hash_bytes {
                write!(s, "{:02X}", byte).unwrap();
            }
            s = s.to_lowercase();

            if &s == val {
                continue;
            }

            target_files.push(key.to_string());
        }

        if target_files.len() == 0 {
            return false;
        }

        //let tar_gz: File = tempfile::NamedTempFile::new().unwrap(); // tempfile::tempfile().unwrap();
        //let tar_gz: File = File::create("D:/test.tar.gz").unwrap();
        //let file_handle = tar_gz.try_clone().unwrap();

        let enc = GzEncoder::new(local_temp_file, Compression::default());
        let mut tar = tar::Builder::new(enc);

        for key in target_files {
            let _ = tar.append_path_with_name(local_dir.clone() + &key, key);
            //tar.append_file(key, &mut file).unwrap();
        }

        tar.finish().unwrap();

        true
    }

    // static block
    pub fn collect_files_ext(local_dir: String, files: &mut Vec<String>) {
        let ldir = PathBuf::from(local_dir);
        PackageCreator::collect_files(
            ldir.canonicalize().unwrap().to_str().unwrap().to_string(),
            "".to_string(),
            files,
        );
    }

    pub fn collect_files(local_dir: String, cur_dir: String, files: &mut Vec<String>) {
        let path_to_current_directory = Path::new(&local_dir).join(cur_dir);
        let paths: Vec<_> = fs::read_dir(path_to_current_directory)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        for path in &paths {
            let file_path = path.path();
            let md = metadata(file_path).unwrap();
            if !md.is_file() {
                continue;
            }

            let file_path = path.path();
            let mut name = file_path
                .into_os_string()
                .into_string()
                .unwrap()
                .replace(&local_dir, "");
            if name.starts_with('\\') {
                name = name[1..].to_string();
            }

            name = name.replace("\\", "/");

            files.push(name);
        }

        for path in paths {
            let file_path = path.path();
            let md = metadata(file_path).unwrap();
            if !md.is_dir() {
                continue;
            }

            let file_path = path.path();
            PackageCreator::collect_files(
                local_dir.to_string(),
                file_path.to_str().unwrap().to_string(),
                files,
            );
        }
    }
}
