use log_analyzer_transient_types::{Document, Field};
use notify::{watcher, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use std::fs::File;
use std::io::BufRead;
use std::sync::mpsc::Receiver;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

pub struct FileWatcher {
    watcher: RecommendedWatcher, // Needs to be protected from drop
    watch_rx: Receiver<DebouncedEvent>,
    document_sink: UnboundedSender<Document>,
    reader: std::io::BufReader<File>,
    regex: Regex,
}

impl FileWatcher {
    pub fn new(log_file: &str, document_sink: UnboundedSender<Document>, regex: &str) -> Self {
        let (watch_tx, watch_rx) = std::sync::mpsc::channel();
        let mut watcher = watcher(watch_tx, Duration::from_millis(10)).unwrap();
        watcher.watch(log_file, RecursiveMode::NonRecursive).unwrap();
        let file = std::fs::File::open(log_file).unwrap();
        let reader = std::io::BufReader::new(file);
        let regex = Regex::new(regex).unwrap();
        Self {
            watcher,
            watch_rx,
            document_sink,
            reader,
            regex,
        }
    }

    pub fn start_watch(mut self) {
        tokio::task::spawn_blocking(move || loop {
            match self.watch_rx.recv() {
                Ok(event) => self.handle_file_event(event),
                Err(e) => tracing::error!("watch error: {}", e),
            }
        });
    }

    fn handle_file_event(&mut self, event: DebouncedEvent) {
        tracing::trace!("Detected file changes: {:?}", &event);
        if let DebouncedEvent::Write(path) = event {
            tracing::trace!("File write on {}", path.to_string_lossy());
            let mut line = String::new();
            while self.reader.read_line(&mut line).unwrap() != 0 {
                let mut doc = Document { fields: vec![] };
                let names: Vec<String> = self.regex.capture_names().flatten().map(|e| e.to_owned()).collect();
                for cap in self.regex.captures_iter(&line) {
                    for name in names.iter() {
                        if let Some(content) = cap.name(&name) {
                            if !content.as_str().is_empty() {
                                doc.fields.push(Field {
                                    name: name.to_owned(),
                                    content: content.as_str().to_owned(),
                                });
                            }
                        }
                    }
                }
                self.document_sink.send(doc).unwrap();
            }
        }
    }
}
