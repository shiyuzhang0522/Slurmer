use std::{
    fmt,
    fs::File,
    io::{self, Read, Seek},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use crossbeam::{
    channel::{unbounded, Receiver, Sender},
    select,
};
use notify::{event::ModifyKind, RecursiveMode, Watcher};

type JobOutput = Result<String, FileWatcherError>;

struct FileReader {
    content_sender: Sender<io::Result<String>>,
    receiver: Receiver<()>,
    file_path: PathBuf,
    interval: Duration,
    content: String,
    pos: u64,
}

struct FileWatcher {
    app: Sender<JobOutput>,
    receiver: Receiver<FileWatcherMessage>,
    file_path: Option<PathBuf>,
    interval: Duration,
}

pub enum FileWatcherMessage {
    FilePath(Option<PathBuf>),
}

pub struct FileWatcherHandle {
    sender: Sender<FileWatcherMessage>,
    file_path: Option<PathBuf>,
}

#[derive(Debug)]
pub enum FileWatcherError {
    Watcher(notify::Error),
    File(io::Error),
}

impl fmt::Display for FileWatcherError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FileWatcherError::Watcher(error) => write!(f, "Watcher error: {error}"),
            FileWatcherError::File(error) => write!(f, "Read error: {error}"),
        }
    }
}

impl FileWatcher {
    fn new(
        app: Sender<JobOutput>,
        receiver: Receiver<FileWatcherMessage>,
        interval: Duration,
    ) -> Self {
        Self {
            app,
            receiver,
            file_path: None,
            interval,
        }
    }

    fn run(&mut self) {
        let (watch_sender, watch_receiver) = unbounded();
        let watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
            if let Ok(event) = result {
                if matches!(
                    event.kind,
                    notify::EventKind::Modify(ModifyKind::Data(_))
                        | notify::EventKind::Modify(ModifyKind::Name(_))
                ) {
                    let _ = watch_sender.send(());
                }
            }
        });
        let mut watcher = match watcher {
            Ok(watcher) => watcher,
            Err(error) => {
                let _ = self.app.send(Err(FileWatcherError::Watcher(error)));
                return;
            }
        };

        let (_, mut content_receiver) = unbounded::<io::Result<String>>();
        let (mut refresh_sender, mut _refresh_receiver) = unbounded::<()>();

        loop {
            select! {
                recv(self.receiver) -> message => {
                    let Ok(FileWatcherMessage::FilePath(file_path)) = message else {
                        return;
                    };
                    let (next_content_sender, next_content_receiver) = unbounded();
                    let (next_refresh_sender, next_refresh_receiver) = unbounded();
                    content_receiver = next_content_receiver;
                    refresh_sender = next_refresh_sender;
                    _refresh_receiver = next_refresh_receiver;

                    if let Some(path) = self.file_path.take() {
                        if let Err(error) = watcher.unwatch(&path) {
                            let _ = self.app.send(Err(FileWatcherError::Watcher(error)));
                        }
                    }

                    if let Some(path) = file_path {
                        match watcher.watch(Path::new(&path), RecursiveMode::NonRecursive) {
                            Ok(()) => {
                                self.file_path = Some(path.clone());
                                let interval = self.interval;
                                let sender = next_content_sender.clone();
                                let receiver = _refresh_receiver.clone();
                                thread::spawn(move || {
                                    FileReader::new(sender, receiver, path, interval).run()
                                });
                            }
                            Err(error) => {
                                let _ = self.app.send(Err(FileWatcherError::Watcher(error)));
                            }
                        }
                    } else {
                        let _ = self.app.send(Ok(String::new()));
                    }
                }
                recv(watch_receiver) -> signal => {
                    if signal.is_err() || refresh_sender.send(()).is_err() {
                        return;
                    }
                }
                recv(content_receiver) -> message => {
                    match message {
                        Ok(result) => {
                            if self.app.send(result.map_err(FileWatcherError::File)).is_err() {
                                return;
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
        }
    }
}

impl FileReader {
    fn new(
        content_sender: Sender<io::Result<String>>,
        receiver: Receiver<()>,
        file_path: PathBuf,
        interval: Duration,
    ) -> Self {
        Self {
            content_sender,
            receiver,
            file_path,
            interval,
            content: String::new(),
            pos: 0,
        }
    }

    fn run(&mut self) {
        loop {
            if self.update().is_err() {
                return;
            }
            select! {
                recv(self.receiver) -> signal => {
                    if signal.is_err() {
                        return;
                    }
                }
                default(self.interval) => {}
            }
        }
    }

    fn update(&mut self) -> Result<(), ()> {
        let result = File::open(&self.file_path).and_then(|mut file| {
            let length = file.metadata()?.len();
            if length < self.pos {
                self.pos = 0;
                self.content.clear();
            }
            file.seek(io::SeekFrom::Start(self.pos))?;
            self.pos += file.read_to_string(&mut self.content)? as u64;
            Ok(self.content.clone())
        });
        self.content_sender.send(result).map_err(|_| ())
    }
}

impl FileWatcherHandle {
    pub fn new(app: Sender<JobOutput>, interval: Duration) -> Self {
        let (sender, receiver) = unbounded();
        let mut actor = FileWatcher::new(app, receiver, interval);
        thread::spawn(move || actor.run());
        Self {
            sender,
            file_path: None,
        }
    }

    pub fn set_file_path(&mut self, file_path: Option<PathBuf>) {
        if self.file_path != file_path {
            self.file_path = file_path.clone();
            let _ = self.sender.send(FileWatcherMessage::FilePath(file_path));
        }
    }
}
