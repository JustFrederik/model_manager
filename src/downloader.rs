use std::cmp::min;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::model_manager::{HuggingfaceModel, ModelSource};
use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;

use crate::error::Error;

pub async fn download_file(
    url: &ModelSource,
    model: String,
    version: String,
    path: PathBuf,
    m: &MultiProgress,
) -> Result<(), Error> {
    match url {
        ModelSource::Huggingface(v) => download_huggingface(v, model, version, path, m).await,
        ModelSource::Zip(url) => download_zip_file(url, model, version, path, m).await,
    }
}

async fn download_huggingface(
    links: &HuggingfaceModel,
    model: String,
    version: String,
    path: PathBuf,
    m: &MultiProgress,
) -> Result<(), Error> {
    for v in links.url() {
        let v = download_single_file(v.0, &v.1, &model, path.clone(), m, 40).await?;
        m.remove(&v);
    }
    create_version(&path, version)?;
    Ok(())
}

fn get_progress_style() -> Result<ProgressStyle, Error> {
    let spinner_color = "33";
    let proccessed_color = "magenta"; //brighter magenta
    let coming_color = "white"; //grey
    let total_bytes_color = "green";
    let bytes_per_sec_color = "red";
    let eta_exact_color = "cyan";
    Ok(ProgressStyle::with_template(&format!(" {{spinner:.{spinner_color}}} {{msg}} {{wide_bar:.{proccessed_color}/{coming_color}}} {{bytes:.{total_bytes_color}}}/{{total_bytes:.{total_bytes_color}}} {{bytes_per_sec:.{bytes_per_sec_color}}} eta {{eta:.{eta_exact_color}}}"))
        .map_err(Error::console_template)?.progress_chars("━╸━"))
}

async fn download_single_file(
    filename: String,
    url: &str,
    model: &str,
    path: PathBuf,
    m: &MultiProgress,
    reload_speed: u64,
) -> Result<ProgressBar, Error> {
    let res = Client::new().get(url).send().await.map_err(Error::fetch)?;

    let total_size = res
        .content_length()
        .ok_or_else(|| Error::fetch_custom("Failed to get size of request"))?;

    // Indicatif setup downloader
    let pb = m.add(ProgressBar::new(total_size));
    let template = get_progress_style()?;
    pb.set_style(template);
    pb.set_message(format!("Downloading {}", model));

    // end spinner when download is complete
    let (sender, receiver) = channel();

    // shared data between threads
    let progress = Arc::new(Mutex::new(0));
    let task1_progress: Arc<Mutex<u64>> = progress.clone();

    let task1 = tokio::spawn(async move {
        // download chunks
        let p = &path.join(filename);
        std::fs::create_dir_all(remove_last(p.clone())).map_err(Error::write_file)?;
        let mut file = File::create(p).map_err(Error::write_file)?;
        let mut stream = res.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk =
                item.map_err(|_| Error::fetch_custom("Error while downloading file stream"))?;
            file.write_all(&chunk).map_err(Error::write_file)?;
            //TODO: wait for instead of unwrap
            let mut shared_data = task1_progress.lock().unwrap();
            let new = min(*shared_data + (chunk.len() as u64), total_size);

            *shared_data = new;
            drop(shared_data);
        }
        sender.send(()).map_err(Error::thread_send)
    });

    let task2_spinner = pb.clone();

    let task2 = thread::spawn(move || {
        while receiver.try_recv().is_err() {
            let shared_data_t = progress.lock().unwrap();
            task2_spinner.set_position(*shared_data_t);
            drop(shared_data_t);
            thread::sleep(Duration::from_millis(reload_speed));
        }
    });

    task1.await.map_err(Error::async_thread_join)??;
    task2.join().map_err(Error::thread_join)?;
    Ok(pb)
}

fn create_version(path: &Path, version: String) -> Result<(), Error> {
    let mut file = File::create(path.join("version")).map_err(Error::write_file)?;
    file.write_all(version.as_bytes())
        .map_err(Error::write_file)?;
    Ok(())
}

async fn download_zip_file(
    url: &str,
    model: String,
    version: String,
    path: PathBuf,
    m: &MultiProgress,
) -> Result<(), Error> {
    let spinner_color = "33";
    let filename = "archive";
    let reload_speed = 40;
    let pb = download_single_file(
        filename.to_string(),
        url,
        &model,
        path.clone(),
        m,
        reload_speed,
    )
    .await?;

    // setup styling for unzip
    let spinner2 = ProgressStyle::with_template(&format!(" {{spinner:.{spinner_color}}} {{msg}}"))
        .map_err(Error::console_template)?;
    pb.set_style(spinner2);
    pb.set_message(format!("Unpacking {}", model));

    // end spinner when unzip is complete
    let (sender, receiver): (Sender<()>, Receiver<()>) = channel();

    let task1_path = path.clone();
    let task1 = thread::spawn(move || {
        zip_extract::extract(
            File::open(task1_path.join(filename)).map_err(Error::open_file)?,
            &task1_path,
            true,
        )
        .map_err(Error::zip_extract)?;
        std::fs::remove_file(task1_path.join(filename)).map_err(Error::write_file)?;
        create_version(&task1_path, version)?;
        sender.send(()).map_err(Error::thread_send)
    });

    let pb_task2 = pb.clone();
    let task2 = thread::spawn(move || {
        while receiver.try_recv().is_err() {
            pb_task2.inc(1);
            thread::sleep(Duration::from_millis(reload_speed))
        }
    });
    task1.join().map_err(Error::thread_join)??;
    task2.join().map_err(Error::thread_join)?;
    pb.finish_and_clear();
    Ok(())
}

fn remove_last(v: PathBuf) -> PathBuf {
    let mut v = v.iter().collect::<Vec<&OsStr>>();
    v.pop();
    PathBuf::from_iter(v)
}
