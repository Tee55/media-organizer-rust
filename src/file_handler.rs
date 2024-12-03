use image::{GenericImageView, ImageFormat, ImageReader};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};
use std::vec;
use tar::Archive as TarArchive;
use zip::write::SimpleFileOptions;

use crate::archive_cleaner;

pub const IMAGE_SIZE: (u32, u32) = (1024, 1024);

pub struct FileHandler {
    dir_path: PathBuf,
    file_path: PathBuf,
    file_name: String,
}

pub fn extract_file_info<'a>(file_path: &'a Path) -> io::Result<(String, PathBuf)> {
    // Attempt to get the file name, returning an error if it doesn't exist
    let filename = match file_path.file_name() {
        Some(name) => name.to_string_lossy().to_string(),
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid file path: no file name",
            ))
        }
    };

    // Attempt to split the name and extension, handling cases with no extension
    let (name, _) = filename.rsplit_once('.').unwrap_or((&filename, ""));

    // Attempt to get the parent directory, returning an error if it's not present
    let parent_path = match file_path.parent() {
        Some(parent) => parent.to_path_buf(),
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid file path: no parent directory",
            ))
        }
    };

    Ok((name.to_string(), parent_path))
}

impl FileHandler {
    pub fn new(archive_path: &Path) -> Self {
        let (file_name, dir_path) = extract_file_info(archive_path).unwrap();
        Self {
            dir_path,
            file_path: archive_path.to_path_buf(),
            file_name,
        }
    }

    pub fn clean(&self) -> Result<(), String> {
        match self.file_path.extension().and_then(|ext| ext.to_str()) {
            Some("zip") => self.handle_zip_file(),
            Some("rar") => self.handle_rar_file(),
            Some("tar") | Some("gz") => self.handle_tar_file(),
            Some("jpg") | Some("jpeg") | Some("png") | Some("bmp") => self.handle_image_file(),
            Some("gif") => self.handle_gif_file(),
            Some("srt") | Some("ass") => self.handle_subtitle_file(),
            Some("mp4") | Some("mkv") => self.handle_video_file(),
            _ => Err(format!(
                "Unsupported file format: {}",
                self.file_path.display()
            )),
        }
    }

    fn handle_zip_file(&self) -> Result<(), String> {
        match self.clean_archive_file() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to clean archive: {}", e)),
        }
    }

    fn handle_rar_file(&self) -> Result<(), String> {
        let zip_path = self.dir_path.join(format!("{}.zip", self.file_name));
        match self.rar_to_zip(&zip_path) {
            Ok(_) => self.handle_zip_file(),
            Err(e) => Err(format!("Failed to extract RAR file: {}", e)),
        }
    }

    fn handle_tar_file(&self) -> Result<(), String> {
        let zip_path = self.dir_path.join(format!("{}.zip", self.file_name));
        match self.tar_to_zip(&zip_path) {
            Ok(_) => self.handle_zip_file(),
            Err(e) => Err(format!("Failed to handle TAR file: {}", e)),
        }
    }

    fn clean_archive_file(&self) -> Result<(), String> {
        let archive_cleaner = archive_cleaner::ArchiveCleaner::new(&self.file_path);
        match archive_cleaner.clean_archive_file(5) {
            Ok(_) => println!("Archive cleaned successfully."),
            Err(e) => println!("Failed to clean archive: {}", e),
        }
        Ok(())
    }

    pub fn tar_to_zip(&self, zip_path: &Path) -> Result<(), String> {
        // Open the TAR file
        let tar_file =
            File::open(&self.file_path).map_err(|e| format!("Failed to open tar file: {}", e))?;
        let mut archive = TarArchive::new(tar_file);

        // Create the ZIP file
        let zip_file =
            File::create(zip_path).map_err(|e| format!("Failed to create zip file: {}", e))?;
        let mut zip_writer = zip::ZipWriter::new(zip_file);

        // Iterate over each entry in the TAR file
        for entry in archive
            .entries()
            .map_err(|e| format!("Failed to read tar entries: {}", e))?
        {
            let mut entry = entry.map_err(|e| format!("Failed to read tar entry: {}", e))?;

            // Get the path of the TAR entry
            let path = entry
                .path()
                .map_err(|e| format!("Failed to get entry path: {}", e))?;

            // Start a new file in the ZIP archive
            zip_writer
                .start_file(path.to_string_lossy(), SimpleFileOptions::default())
                .map_err(|e| format!("Failed to start zip file entry: {}", e))?;

            // Read the content of the tar entry and write it to the zip file
            let mut buffer = Vec::new();
            entry
                .read_to_end(&mut buffer)
                .map_err(|e| format!("Failed to read tar entry data: {}", e))?;

            // Write the data to the zip file
            zip_writer
                .write_all(&buffer)
                .map_err(|e| format!("Failed to write data to zip file: {}", e))?;
        }

        // Finish the ZIP archive
        zip_writer
            .finish()
            .map_err(|e| format!("Failed to finish zip archive: {}", e))?;

        Ok(())
    }

    pub fn rar_to_zip(&self, zip_path: &Path) -> Result<(), String> {
        // Open the RAR file
        let mut archive = unrar::Archive::new(&self.file_path)
            .open_for_processing()
            .map_err(|e| format!("Failed to open RAR file: {}", e))?;

        // Create the ZIP file
        let zip_file =
            File::create(zip_path).map_err(|e| format!("Failed to create zip file: {}", e))?;
        let mut zip_writer = zip::ZipWriter::new(zip_file);

        // Iterate over each entry in the RAR archive
        while let Some(header) = archive.read_header().expect("read header") {
            
            zip_writer
                .start_file(
                    header.entry().filename.to_string_lossy().to_string(),
                    SimpleFileOptions::default(),
                )
                .map_err(|e| format!("Failed to start zip file entry: {}", e))?;

            let (data, cursor) = header.read().expect("read data");
            zip_writer.write(&data).expect("write data");
            archive = cursor;
        }
        zip_writer
            .finish()
            .map_err(|e| format!("Failed to finish zip archive: {}", e))?;

        Ok(())
    }

    fn handle_image_file(&self) -> Result<(), String> {
        let image = ImageReader::open(&self.file_path)
            .map_err(|e| format!("Failed to open image: {}", e))?
            .decode()
            .map_err(|e| format!("Failed to decode image: {}", e))?;

        if self
            .file_path
            .extension()
            .map_or(false, |ext| ext == "webp")
        {
            return Ok(());
        }

        let image = image.thumbnail(IMAGE_SIZE.0, IMAGE_SIZE.1);
        let webp_file_path = self.dir_path.join(format!("{}.webp", self.file_name));

        if !webp_file_path.exists() {
            image
                .save_with_format(webp_file_path, ImageFormat::WebP)
                .map_err(|e| format!("Failed to save image as webp: {}", e))?;
        } else {
            let timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or(Duration::new(0, 0))
                .as_secs();
            let new_name = format!("{}_{timestamp}.webp", self.file_name);
            image
                .save_with_format(self.dir_path.join(new_name), ImageFormat::WebP)
                .map_err(|e| format!("Failed to save image as webp: {}", e))?;
        }
        fs::remove_file(&self.file_path)
            .map_err(|e| format!("Failed to remove old file: {}", e))?;
        Ok(())
    }

    fn handle_gif_file(&self) -> Result<(), String> {
        let gif_image = ImageReader::open(&self.file_path)
            .map_err(|e| format!("Failed to open gif file: {}", e))?
            .decode()
            .map_err(|e| format!("Failed to decode gif file: {}", e))?;

        let (width, height) = gif_image.dimensions();
        if width > IMAGE_SIZE.0 && height > IMAGE_SIZE.1 {
            let resized_image = gif_image.thumbnail(IMAGE_SIZE.0, IMAGE_SIZE.1);
            resized_image
                .save(self.dir_path.join(format!("{}.gif", self.file_name)))
                .map_err(|e| format!("Failed to save gif file: {}", e))?;
        }
        Ok(())
    }

    fn handle_video_file(&self) -> Result<(), String> {
        let command = Command::new("ffmpeg")
            .arg("-i")
            .arg(&self.file_path)
            .arg("-c:v")
            .arg("libx264")
            .arg("-c:a")
            .arg("aac")
            .arg("-c:s")
            .arg("mov_text")
            .arg("-metadata:s:a:0")
            .arg("language=jpn")
            .arg("-metadata:s:s:0")
            .arg("language=eng")
            .arg(self.dir_path.join(format!("{}.mp4", self.file_name)))
            .output()
            .map_err(|e| format!("Failed to execute ffmpeg command: {}", e))?;

        if !command.status.success() {
            return Err(format!(
                "FFmpeg command failed: {}",
                String::from_utf8_lossy(&command.stderr)
            ));
        }
        fs::remove_file(&self.file_path)
            .map_err(|e| format!("Failed to remove old video file: {}", e))?;
        Ok(())
    }

    fn handle_subtitle_file(&self) -> Result<(), String> {
        // Subtitle files handling logic if necessary.
        Ok(())
    }

    pub fn get_supported_extensions() -> Vec<&'static str> {
        vec![
            "zip", "rar", "tar", "gz", "jpg", "jpeg", "png", "bmp", "gif", "webp", "mp4", "mkv",
            "srt", "ass",
        ]
    }
}
