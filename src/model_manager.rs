use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Instant;

use chrono::Utc;
use console::{Emoji, style};
use fs_extra::dir::CopyOptions;
use futures::{stream, StreamExt};
use indicatif::{HumanDuration, MultiProgress};

use crate::downloader::download_file;
use crate::error::Error;

static LOOKING_GLASS: Emoji<'_, '_> = Emoji("🔍  ", "");
static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", ":-)");


pub struct ModelManager {
    model_path: PathBuf,
    models: HashMap<String, Model>,
}

impl ModelManager {
    pub fn new() -> Result<ModelManager, Error> {
        let models = HashMap::new();

        Ok(Self {
            model_path: PathBuf::from_str("models")
                .map_err(Error::pathbuf_open)?,
            models,
        })
    }

    pub fn new_custom(path: PathBuf) -> ModelManager {
        Self {
            model_path: path,
            models: HashMap::new(),
        }
    }

    pub fn register_models(&mut self, map: HashMap<String, Model>) {
        self.models.extend(map)
    }

    pub fn remove_zips(&self) -> Result<(), Error> {
        Self::delete_files_named_archive(&self.model_path).map_err(Error::write_file)
    }

    fn delete_files_named_archive(dir: &Path) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_file() && path.file_name().unwrap_or_default() == "archive" {
                std::fs::remove_file(&path)?;
            } else if path.is_dir() {
                Self::delete_files_named_archive(&path)?;
            }
        }
        Ok(())
    }

    pub fn clean_directory(&self) -> Result<(), Error> {
        use fs_extra::dir::move_dir;
        let timestamp = Utc::now().timestamp();

        let mut options = CopyOptions::new(); //Initialize default values for CopyOptions
        options.content_only = true;
        let mut to = self.model_path.iter().map(|v| v.to_str()).collect::<Option<Vec<&str>>>().ok_or_else(|| Error::pathbuf_custom("Path has empty element"))?.iter().map(|v| v.to_string()).collect::<Vec<_>>();
        match to.last_mut() {
            None => return Err(Error::pathbuf_custom("path is empty")),
            Some(v) => {
                v.push('-');
                v.push_str(timestamp.to_string().as_ref())
            }
        }
        let to = PathBuf::from_str(&to.join("/")).map_err(Error::pathbuf_open)?;
        std::fs::create_dir_all(&to).map_err(Error::write_file)?;
        move_dir(&self.model_path, &to, &options).map_err(Error::write_file_extra)?;

        for model in &self.models {
            let from = &to.join(&model.1.directory);
            let to = &self.model_path.join(&model.1.directory);
            std::fs::create_dir_all(to).map_err(Error::write_file)?;
            move_dir(from, to, &options).map_err(Error::write_file_extra)?;
        }
        std::fs::remove_dir_all(to).map_err(Error::write_file)?;
        Ok(())
    }

    fn check_download_needed(&self, path: PathBuf, version: f32) -> bool {
        let ver = std::fs::read_to_string(path.join("version"));
        if let Ok(v) = ver {
            return match v.parse::<f32>() {
                Ok(v) => v != version,
                Err(_) => true,
            };
        }
        true
    }

    fn create_paths(&self, down: &Vec<(&String, &Model)>) -> Result<(), Error> {
        for model in down {
            let path = self.model_path.join(&model.1.directory);
            let _ = std::fs::remove_dir_all(&path).map_err(Error::write_file);
            std::fs::create_dir_all(path).map_err(Error::write_file)?;
        }
        Ok(())
    }

    pub async fn proccess(&self, processes: usize) -> Result<(), Error> {
        let started = Instant::now();
        println!(
            "{} {}Resolving {} models...",
            style("[1/3]").bold().dim(),
            LOOKING_GLASS, self.models.len()
        );
        let download = self.models.iter().filter(|m| self.check_download_needed(self.model_path.join(&m.1.directory), m.1.version)).collect::<Vec<_>>();
        self.create_paths(&download)?;
        println!(
            "{} {}Processing {} models...",
            style("[2/3]").bold().dim(),
            LOOKING_GLASS,
            download.len()
        );

        println!(
            "{} {}Downloading models...",
            style("[3/3]").bold().dim(),
            LOOKING_GLASS
        );

        let m = MultiProgress::new();
        let handles = stream::iter(download)
            .map(|v| async {
                download_file(v.1.url.to_string(), v.0.to_string(), v.1.version, self.model_path.join(&v.1.directory), &m).await
            }).buffer_unordered(processes);
        let v = handles.collect::<Vec<Result<(), Error>>>().await;
        v.into_iter().collect::<Result<Vec<_>, Error>>()?;
        m.clear().map_err(Error::console_clear)?;

        println!("{} Done in {}", SPARKLE, HumanDuration(started.elapsed()));

        Ok(())
    }
}

pub struct Model {
    pub url: String,
    pub directory: PathBuf,
    pub version: f32,
}