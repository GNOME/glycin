#[cfg(feature = "external")]
use std::os::fd::OwnedFd;

#[cfg(feature = "builtin")]
use futures_util::SinkExt;
use gio::prelude::*;

use crate::{Error, ErrorKind, Source};

const BUF_SIZE: usize = u16::MAX as usize;

#[derive(Debug)]
pub struct SourceTransmission {
    file: Option<gio::File>,
    input_stream: gio::InputStream,
    first_bytes: Vec<u8>,
}

impl SourceTransmission {
    pub async fn init(source: Source) -> Result<SourceTransmission, Error> {
        tracing::trace!("Opening source");

        let input_stream = source.to_stream().await?;
        let buf = vec![0; BUF_SIZE];

        tracing::trace!("Read first {BUF_SIZE} bytes");

        let (buf, n) = input_stream
            .read_future(buf, glib::Priority::DEFAULT)
            .await
            .map_err(|(_, err)| ErrorKind::ImageSource(err).err())?;

        let first_bytes = buf
            .get(..n)
            .ok_or_else(|| ErrorKind::unreachable().err())?
            .to_vec();

        Ok(Self {
            file: source.file(),
            input_stream,
            first_bytes,
        })
    }

    #[cfg(feature = "external")]
    async fn spawn_with_stream(self, stream: gio_unix::OutputStream) -> Result<(), Error> {
        let res = stream
            .write_all_future(self.first_bytes, glib::Priority::DEFAULT)
            .await;

        match res {
            Err((_, err)) => return Err(err.into()),
            Ok((_, _, Some(err))) => return Err(err.into()),
            Ok(_) => {}
        }

        loop {
            use crate::ErrorKind;

            let buf = vec![0; BUF_SIZE];

            let (buf, n) = self
                .input_stream
                .read_future(buf, glib::Priority::DEFAULT)
                .await
                .map_err(|(_, err)| ErrorKind::ImageSource(err).err())?;
            if n == 0 {
                return Ok(());
            }

            // TODO: Avoiding to_vec()
            let res = stream
                .write_all_future(
                    buf.get(..n)
                        .ok_or_else(|| ErrorKind::unreachable().err())?
                        .to_vec(),
                    glib::Priority::DEFAULT,
                )
                .await;

            match res {
                Err((_, err)) => return Err(err.into()),
                Ok((_, _, Some(err))) => return Err(err.into()),
                Ok(_) => {}
            }
        }
    }

    #[cfg(feature = "external")]
    pub fn spawn_external(
        self,
    ) -> Result<(OwnedFd, impl Future<Output = Result<(), Error>>), Error> {
        let (external_reader, writer) = std::os::unix::net::UnixStream::pair()?;

        let writer = gio_unix::OutputStream::take_fd(writer.into());

        Ok((external_reader.into(), self.spawn_with_stream(writer)))
    }

    #[cfg(feature = "builtin")]
    async fn spawn_with_channel(
        self,
        mut channel: futures_channel::mpsc::Sender<Vec<u8>>,
    ) -> Result<(), Error> {
        channel.send(self.first_bytes.to_vec()).await.unwrap();

        if self.first_bytes.len() < BUF_SIZE {
            // TODO: Potentially unsound, but gives 10 micro seconds
            return Ok(());
        }

        loop {
            use crate::ErrorKind;

            let buf = vec![0; BUF_SIZE];

            let (buf, n) = self
                .input_stream
                .read_future(buf, glib::Priority::DEFAULT)
                .await
                .map_err(|(_, err)| ErrorKind::ImageSource(err).err())?;
            if n == 0 {
                return Ok(());
            }

            channel.send(buf[..n].to_vec()).await.unwrap();
        }
    }

    #[cfg(feature = "builtin")]
    pub fn spawn_builtin(self) -> (BuiltinSourceReader, impl Future<Output = Result<(), Error>>) {
        let (writer, builtin_reader) = futures_channel::mpsc::channel(100);

        let builtin_reader = BuiltinSourceReader::new(builtin_reader);

        (builtin_reader, self.spawn_with_channel(writer))
    }

    pub fn file(&self) -> Option<&gio::File> {
        self.file.as_ref()
    }

    pub fn first_bytes(&self) -> &[u8] {
        &self.first_bytes
    }
}

#[cfg(feature = "builtin")]
pub struct BuiltinSourceReader {
    stream: futures_channel::mpsc::Receiver<Vec<u8>>,
    cache: Vec<u8>,
}

#[cfg(feature = "builtin")]
impl BuiltinSourceReader {
    fn new(stream: futures_channel::mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            stream,
            cache: Vec::new(),
        }
    }
}

#[cfg(feature = "builtin")]
impl std::io::Read for BuiltinSourceReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if !self.cache.is_empty() {
                let (len, cache) = write_data(buf, &self.cache);
                self.cache = cache;
                return Ok(len);
            } else {
                match self.stream.try_recv() {
                    Err(futures_channel::mpsc::TryRecvError::Closed) => return Ok(0),
                    Err(futures_channel::mpsc::TryRecvError::Empty) => continue,
                    Ok(data) => {
                        let (len, cache) = write_data(buf, &data);
                        self.cache = cache;
                        return Ok(len);
                    }
                }
            }
        }
    }
}

#[cfg(feature = "builtin")]
fn write_data(target: &mut [u8], src: &[u8]) -> (usize, Vec<u8>) {
    if let Some((send, cache)) = src.split_at_checked(target.len()) {
        target.copy_from_slice(send);
        (target.len(), cache.to_vec())
    } else {
        target[..src.len()].copy_from_slice(&src);
        (src.len(), Vec::new())
    }
}
