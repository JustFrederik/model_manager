use std::any::Any;
use std::convert::Infallible;
use indicatif::style::TemplateError;
use tokio::task::JoinError;
use zip_extract::ZipExtractError;

#[derive(Debug)]
#[allow(dead_code)]
pub enum Error {
    Fetch(String),
    ConsoleTemplateError(TemplateError),
    ConsoleClearError(std::io::Error),
    ThreadSendError(String),
    ThreadJoin,
    AsyncThreadJoin(JoinError),
    OpenFileError(std::io::Error),
    WriteFileError(String),
    Custom{message:String, error: String},
    CustomEmpty {message: String},
    ZipExtractError(ZipExtractError),
    PathBufError(Infallible),
    PathBufCustomError(String)
}

impl Error {
    pub fn new(message: impl ToString, error: impl ToString) -> Self {
        Error::Custom{
            message: message.to_string(),
            error: error.to_string(),
        }
    }

    pub fn new_option(message: impl ToString) -> Self {
        Error::CustomEmpty {
            message: message.to_string(),
        }
    }

    pub fn pathbuf_open(error: Infallible) -> Self {
        Error::PathBufError(error)
    }
    pub fn pathbuf_custom(error: impl ToString) -> Self {
        Error::PathBufCustomError(error.to_string())
    }

    pub fn fetch(error: reqwest::Error) -> Self {
        Error::Fetch(error.to_string())
    }
    pub fn fetch_custom(message: impl ToString) -> Self {
        Error::Fetch(message.to_string())
    }

    pub fn console_template(error: TemplateError) -> Self {
        Error::ConsoleTemplateError(error)
    }

    pub fn console_clear(error: std::io::Error) -> Self{
        Error::ConsoleClearError(error)
    }

    pub fn thread_send(error: impl ToString) -> Self {
        Error::ThreadSendError(error.to_string())
    }

    pub fn thread_join(_: Box<dyn Any + Send>) -> Self {
        Error::ThreadJoin
    }

    pub fn async_thread_join(error: JoinError) -> Self {
        Error::AsyncThreadJoin(error)
    }

    pub fn write_file(error: std::io::Error) -> Self {
        Error::WriteFileError(error.to_string())
    }

    pub fn write_file_extra(error: fs_extra::error::Error) -> Self {
        Error::WriteFileError(error.to_string())

    }

    pub fn open_file(error: std::io::Error) -> Self {
        Error::OpenFileError(error)
    }

    pub fn zip_extract(error: ZipExtractError) -> Self {
        Error::ZipExtractError(error)
    }
}
