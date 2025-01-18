use crate::block::{Block, MB};
use crate::delta::{compute_deltas, slice_dest, BlockDescriptor, Delta, FileMetadata};
use anyhow::{anyhow, Context};
use filetime::{set_file_mtime, FileTime};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::Permissions;
use std::net::{IpAddr, SocketAddr};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinError;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize)]
pub enum Frame {
    Init {
        src_base_dir: Option<PathBuf>,
        dest_path: PathBuf,
    },
    InitAck,
    Fin,
    FinAck,

    FileMetadata(FileMetadata),

    BlockDescriptors {
        block_size: usize,
        src_path: PathBuf,
        descriptors: HashMap<u16, Vec<BlockDescriptor>>,
    },

    Deltas(PathBuf, Vec<Delta>),

    NeedEntireFile(PathBuf),
    EntireFile(PathBuf, Vec<u8>),

    ChunkedStart {
        kind: ChunkKind,
        path: PathBuf,
        data: Vec<u8>,
    },
    ChunkedData {
        is_last: bool,
        data: Vec<u8>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ChunkKind {
    File,
    Diff,
}

const MAX_FRAME_SIZE: usize = 2 * MB;

pub struct WriteConnection(pub FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>);

impl WriteConnection {
    pub async fn send_frame(&mut self, frame: Frame) -> anyhow::Result<()> {
        let serialized = bincode::serialize(&frame)?;
        self.0.send(serialized.into()).await?;
        Ok(())
    }
}

pub struct ReadConnection(pub FramedRead<OwnedReadHalf, LengthDelimitedCodec>);

impl ReadConnection {
    pub async fn receive_frame(&mut self) -> anyhow::Result<Option<Frame>> {
        let frame = if let Some(Ok(bytes)) = self.0.next().await {
            let frame = bincode::deserialize::<Frame>(&bytes)?;

            match frame {
                Frame::ChunkedStart { kind, path, data } => {
                    let mut buffer = data;

                    while let Some(Ok(chunk)) = self.0.next().await {
                        let chunked_frame = bincode::deserialize::<Frame>(&chunk)?;

                        match chunked_frame {
                            Frame::ChunkedData { is_last, data } => {
                                buffer.extend(data);

                                if is_last {
                                    break;
                                }
                            }
                            _ => anyhow::bail!("Unexpected frame type while receiving chunks"),
                        }
                    }

                    match kind {
                        ChunkKind::File => Some(Frame::EntireFile(path, buffer)),
                        ChunkKind::Diff => {
                            let deltas = bincode::deserialize::<Vec<Delta>>(&buffer)?;
                            Some(Frame::Deltas(path, deltas))
                        }
                    }
                }

                f => Some(f),
            }
        } else {
            None
        };

        Ok(frame)
    }
}

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Server is unable to establish the socket connection: {}", ._0)]
    Connection(#[from] std::io::Error),

    #[error("Server is unable to handle this type of frame")]
    UnsupportedFrame,

    #[error("Unable to initialize the communication with the client")]
    MissingInit,

    #[error("Server is unable to continue its execution")]
    Completion(#[from] JoinError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

const PORT: u16 = 8080;

pub async fn run_server(ip_addr: IpAddr) -> Result<(), ServerError> {
    let addr = SocketAddr::new(ip_addr, PORT);
    let listener = TcpListener::bind(addr).await?;

    let shutdown = Arc::new(AtomicBool::new(false));

    while !shutdown.load(Ordering::Relaxed) {
        let (stream, _) = listener.accept().await?;

        tokio::spawn(handle_client(stream, shutdown.clone())).await??;
    }

    Ok(())
}

/// receive and acknowledge the src_base_dir and dest_path or return a MissingInit error
async fn init_connection(
    reader: &mut ReadConnection,
    writer: &mut WriteConnection,
) -> Result<(Option<PathBuf>, PathBuf), ServerError> {
    let frame = reader.receive_frame().await?;

    if let Some(Frame::Init {
        src_base_dir,
        dest_path,
    }) = frame
    {
        writer.send_frame(Frame::InitAck).await?;
        log::debug!("Connection successfully initialized");
        Ok((src_base_dir, dest_path))
    } else {
        Err(ServerError::MissingInit)
    }
}

async fn update_metadata(
    src: &FileMetadata,
    dest: &FileMetadata,
) -> anyhow::Result<(), anyhow::Error> {
    if src.permissions != dest.permissions {
        // UNIX only
        let permissions = Permissions::from_mode(src.permissions);
        tokio::fs::set_permissions(&dest.path, permissions).await?;
    }

    if src.modified != dest.modified {
        let timestamp = src.modified.unwrap_or(0) as i64;
        let modified_time = FileTime::from_unix_time(timestamp, 0);
        set_file_mtime(&dest.path, modified_time)?;
    }
    Ok(())
}

async fn handle_client(stream: TcpStream, shutdown: Arc<AtomicBool>) -> Result<(), ServerError> {
    let (read_stream, write_stream) = stream.into_split();
    let mut read_connection = ReadConnection(
        LengthDelimitedCodec::builder()
            .max_frame_length(MAX_FRAME_SIZE)
            .new_read(read_stream),
    );
    let mut write_connection = WriteConnection(
        LengthDelimitedCodec::builder()
            .max_frame_length(MAX_FRAME_SIZE)
            .new_write(write_stream),
    );

    let (src_base_dir, dest_path) =
        init_connection(&mut read_connection, &mut write_connection).await?;

    let (blocks_tx, mut blocks_rx) = mpsc::unbounded_channel();

    while let Ok(Some(frame)) = read_connection.receive_frame().await {
        log::debug!("received frame {:?}", frame);
        match frame {
            Frame::FileMetadata(src_metadata) => {
                let dest_path =
                    resolve_dest_path(&src_metadata.path, src_base_dir.as_deref(), &dest_path)
                        .await?;

                if !dest_path.exists() {
                    write_connection
                        .send_frame(Frame::NeedEntireFile(src_metadata.path))
                        .await?;
                    continue;
                }

                let metadata = tokio::fs::metadata(&dest_path).await?;
                let dest_metadata = FileMetadata::new(dest_path, &metadata);

                if !src_metadata.needs_update(&dest_metadata) {
                    log::debug!(
                        "{:?} is up-to-date with the corresponding {:?}",
                        src_metadata.path,
                        dest_metadata.path
                    );

                    continue;
                }

                update_metadata(&src_metadata, &dest_metadata).await?;

                if src_metadata.len != dest_metadata.len {
                    let block_size =
                        Block::calculate_block_size(src_metadata.len, dest_metadata.len);

                    let mut dest_reader = BufReader::new(File::open(dest_metadata.path).await?);
                    let (dest_blocks, dest_descriptors) =
                        slice_dest(&mut dest_reader, block_size).await;

                    blocks_tx.send(dest_blocks).map_err(|e| anyhow!(e))?;

                    write_connection
                        .send_frame(Frame::BlockDescriptors {
                            block_size,
                            src_path: src_metadata.path,
                            descriptors: dest_descriptors,
                        })
                        .await?;
                }
            }

            Frame::Deltas(path, deltas) => {
                let dest_path =
                    resolve_dest_path(&path, src_base_dir.as_deref(), &dest_path).await?;

                let mut writer = BufWriter::new(
                    tokio::fs::OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(dest_path)
                        .await?,
                );

                if let Some(dest_blocks) = blocks_rx.recv().await {
                    for delta in deltas {
                        match delta {
                            Delta::Literal(bytes) => writer.write_all(&bytes).await?,

                            Delta::Reference(idx) => writer.write_all(&dest_blocks[idx].0).await?,
                        }
                    }

                    writer.flush().await?;
                }
            }

            Frame::EntireFile(path, bytes) => {
                let dest_path =
                    resolve_dest_path(&path, src_base_dir.as_deref(), &dest_path).await?;

                if let Some(parent) = dest_path.parent() {
                    // handle recursive folders
                    tokio::fs::create_dir_all(parent).await?;
                }

                tokio::fs::write(dest_path, bytes).await?;
            }

            Frame::Fin => {
                log::debug!("Received Fin, acknowledge fin and quit");
                write_connection.send_frame(Frame::FinAck).await?;
            }

            _ => return Err(ServerError::UnsupportedFrame),
        }
    }

    shutdown.store(true, Ordering::Relaxed);
    Ok(())
}

async fn resolve_dest_path(
    path: &Path,
    src_base_dir: Option<&Path>,
    dest_path: &Path,
) -> anyhow::Result<PathBuf> {
    if !dest_path.exists() {
        if dest_path.extension().is_some() {
            if src_base_dir.is_some() {
                anyhow::bail!("Source cannot be a folder when the destination is a file");
            }

            let parent = dest_path
                .parent()
                .context("Destination path does not have a valid parent directory")?;

            // create the parent directories if they don't exist
            tokio::fs::create_dir_all(parent).await?;

            File::create(&dest_path).await?;
        } else {
            tokio::fs::create_dir_all(&dest_path).await?;
        }
    }

    let resolved_path = if let Some(src_base_dir) = src_base_dir {
        Some(
            path.strip_prefix(src_base_dir)
                .with_context(|| format!("{:?} is not a prefix of {:?}", src_base_dir, path))?,
        )
    } else if dest_path.extension().is_none() {
        Some(
            path.file_name()
                .map(Path::new)
                .context("Invalid file name extracted from path")?,
        )
    } else {
        None
    };

    if let Some(resolved_path) = resolved_path {
        Ok(dest_path.join(resolved_path))
    } else {
        Ok(dest_path.to_path_buf())
    }
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Client failed to connect to the server: {}", ._0)]
    Connection(#[from] std::io::Error),

    #[error("Client is unable to handle this type of frame")]
    UnsupportedFrame,

    #[error("Unable to initialize the communication with the server")]
    MissingAck,

    #[error("Client is unable to continue its execution")]
    Completion(#[from] JoinError),

    #[error("{}", ._0)]
    Serde(#[from] bincode::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub async fn run_client(
    ip_addr: IpAddr,
    src_path: &Path,
    dest_path: &Path,
) -> Result<(), ClientError> {
    let addr = SocketAddr::new(ip_addr, PORT);
    let stream = TcpStream::connect(addr).await?;
    let (read_stream, write_stream) = stream.into_split();
    let mut read_connection = ReadConnection(
        LengthDelimitedCodec::builder()
            .max_frame_length(MAX_FRAME_SIZE)
            .new_read(read_stream),
    );
    let mut write_connection = WriteConnection(
        LengthDelimitedCodec::builder()
            .max_frame_length(MAX_FRAME_SIZE)
            .new_write(write_stream),
    );

    let src_path = src_path.to_path_buf();
    let dest_path = dest_path.to_path_buf();

    if src_path.is_dir() {
        write_connection
            .send_frame(Frame::Init {
                src_base_dir: Some(src_path.clone()),
                dest_path,
            })
            .await?;
    } else if src_path.is_file() {
        write_connection
            .send_frame(Frame::Init {
                src_base_dir: None,
                dest_path,
            })
            .await?;
    } else if !src_path.exists() {
        return Err(anyhow!("Source path {:?} doesn't exist", src_path).into());
    } else {
        unimplemented!("Symlinks are not supported");
    }

    if !matches!(read_connection.receive_frame().await?, Some(Frame::InitAck)) {
        return Err(ClientError::MissingAck);
    }

    let (walker_tx, mut rx) = mpsc::unbounded_channel();
    let listener_tx = walker_tx.clone();

    let listener = tokio::spawn(async move {
        while let Ok(Some(frame)) = read_connection.receive_frame().await {
            match frame {
                Frame::NeedEntireFile(path) => {
                    let all_bytes = tokio::fs::read(&path).await?;

                    if all_bytes.len() <= MAX_FRAME_SIZE {
                        listener_tx
                            .send(Frame::EntireFile(path, all_bytes))
                            .map_err(|e| anyhow!(e))?;
                    } else {
                        let chunks = all_bytes.chunks(MAX_FRAME_SIZE - (1 * MB));
                        let len = chunks.len();
                        let mut chunks_iter = chunks.enumerate();

                        if let Some((_, chunk)) = chunks_iter.next() {
                            listener_tx
                                .send(Frame::ChunkedStart {
                                    kind: ChunkKind::File,
                                    path,
                                    data: chunk.to_vec(),
                                })
                                .map_err(|e| anyhow!(e))?;
                        }

                        for (i, chunk) in chunks_iter {
                            listener_tx
                                .send(Frame::ChunkedData {
                                    is_last: i == len - 1,
                                    data: chunk.to_vec(),
                                })
                                .map_err(|e| anyhow!(e))?;
                        }
                    }
                }

                Frame::BlockDescriptors {
                    block_size,
                    src_path,
                    descriptors,
                } => {
                    let buffer = tokio::fs::read(&src_path).await?;
                    let deltas = compute_deltas(block_size, buffer, descriptors);
                    let all_bytes = bincode::serialize(&deltas)?;

                    if all_bytes.len() <= MAX_FRAME_SIZE {
                        listener_tx
                            .send(Frame::Deltas(src_path, deltas))
                            .map_err(|e| anyhow!(e))?;
                    } else {
                        let chunks = all_bytes.chunks(MAX_FRAME_SIZE - (1 * MB));
                        let len = chunks.len();
                        let mut chunks_iter = chunks.enumerate();

                        if let Some((_, chunk)) = chunks_iter.next() {
                            listener_tx
                                .send(Frame::ChunkedStart {
                                    kind: ChunkKind::Diff,
                                    path: src_path,
                                    data: chunk.to_vec(),
                                })
                                .map_err(|e| anyhow!(e))?;
                        }

                        for (i, chunk) in chunks_iter {
                            listener_tx
                                .send(Frame::ChunkedData {
                                    is_last: i == len - 1,
                                    data: chunk.to_vec(),
                                })
                                .map_err(|e| anyhow!(e))?;
                        }
                    }
                }

                Frame::FinAck => return Ok(()),

                _ => return Err(ClientError::UnsupportedFrame),
            };
        }

        Ok(())
    });

    let walker = tokio::spawn(async move {
        for entry in WalkDir::new(src_path).into_iter().filter_map(|entry| {
            entry
                .map_err(|e| {
                    log::error!("Error accessing file: {e}");
                })
                .ok()
        }) {
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                let fm = FileMetadata::new(entry.into_path(), &metadata);

                walker_tx.send(Frame::FileMetadata(fm))?;
            }
        }

        walker_tx.send(Frame::Fin)?;

        Ok::<_, anyhow::Error>(())
    });

    let sender = tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            write_connection.send_frame(frame).await?;
        }

        Ok::<_, anyhow::Error>(())
    });

    let (walker_res, listener_res, sender_res) = tokio::try_join!(walker, listener, sender)?;

    if let Err(err) = walker_res {
        log::error!("Error while traversing the files. {err}");
    }
    if let Err(err) = listener_res {
        log::error!("Error while receiving data from the server. {err}");
    }
    if let Err(err) = sender_res {
        log::error!("Error while sending data to the server. {err}");
    }

    Ok(())
}
