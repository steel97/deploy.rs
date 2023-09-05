use std::{
    fs::{self, metadata},
    path::{Path, PathBuf},
};

pub struct PackageCreator {}

impl PackageCreator {
    pub fn new() -> PackageCreator {
        PackageCreator {}
    }

    pub fn collect_files_ext(&mut self, local_dir: String, files: &mut Vec<String>) {
        let ldir = PathBuf::from(local_dir);
        self.collect_files(
            ldir.canonicalize().unwrap().to_str().unwrap().to_string(),
            "".to_string(),
            files,
        );
    }

    pub fn collect_files(&mut self, local_dir: String, cur_dir: String, files: &mut Vec<String>) {
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
            self.collect_files(
                local_dir.to_string(),
                file_path.to_str().unwrap().to_string(),
                files,
            );
        }
    }
}
