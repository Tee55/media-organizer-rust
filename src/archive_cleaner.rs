use image::{
    codecs::webp::WebPEncoder, imageops::FilterType, DynamicImage, ExtendedColorType, GenericImage,
    GenericImageView,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use zip::write::SimpleFileOptions;
use zip::{read::ZipArchive, write::ZipWriter};

use crate::file_handler::extract_file_info;
use crate::file_handler::IMAGE_SIZE;

fn process_images_threaded(
    cleaner: Arc<ArchiveCleaner>, 
    images: Vec<DynamicImage>, 
    shared_zip: Arc<Mutex<ZipWriter<File>>>,
    pb: ProgressBar,
) {
    let multi_progress = MultiProgress::new();
    let total_images = images.len();
    let mut handles = vec![];

    for (index, image) in images.into_iter().enumerate() {
        let image = image.clone();
        let pb = pb.clone();
        let spinner = multi_progress.add(ProgressBar::new_spinner());
        spinner.set_style(
            ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
                .unwrap()
                .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
        );
        spinner.enable_steady_tick(Duration::from_millis(100));
        spinner.set_prefix(format!("[{}/{}]", index + 1, total_images));

        // Clone necessary variables into the thread
        let cleaner = Arc::clone(&cleaner);
        let shared_zip = Arc::clone(&shared_zip);

        // Spawn the thread
        let handle = thread::spawn(move || {
            if let Err(e) = cleaner.process_image(index, &image, shared_zip, &pb, &spinner) {
                spinner.finish_with_message(format!("Error: {e}"));
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }
    pb.finish_with_message("Processing complete!");
}

enum ArchiveType {
    Manhwa,
    Manga,
}

struct ImageInfo {
    width: u32,
    height: u32,
}

pub struct ArchiveCleaner {
    archive_type: ArchiveType,
    min_image_size: ImageInfo,
    archive_path: PathBuf,
}

impl ArchiveCleaner {
    pub fn new(archive_path: &Path) -> Self {
        Self {
            archive_type: ArchiveType::Manga,
            min_image_size: ImageInfo {
                width: IMAGE_SIZE.0,
                height: IMAGE_SIZE.1,
            },
            archive_path: archive_path.to_path_buf(),
        }
    }

    pub fn clean_archive_file(mut self, max_images_to_check: usize) -> Result<(), io::Error> {
        let images = match self.read_images_from_archive() {
            Ok(imgs) => imgs,
            Err(e) => return Err(io::Error::new(io::ErrorKind::Other, format!("Failed to read images from archive: {}", e))),
        };
    
        if self.should_write_archive(&images, max_images_to_check) {
            match self.write_archive(&images) {
                Ok(_) => (),
                Err(e) => return Err(io::Error::new(io::ErrorKind::Other, format!("Failed to write archive: {}", e))),
            }
        }
        
        Ok(())
    }

    fn read_images_from_archive(&self) -> Result<Vec<DynamicImage>, io::Error> {
        let archive_file = File::open(self.archive_path.clone())?;
        let mut archive = ZipArchive::new(archive_file)?;
        let mut images = Vec::new();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if self.is_image(file.name()) {
                let mut file_data = Vec::new();
                file.read_to_end(&mut file_data)?;
                if let Ok(img) = image::load_from_memory(&file_data) {
                    images.push(img);
                }
            }
        }
        Ok(images)
    }

    fn should_write_archive(
        &mut self,
        images: &[DynamicImage],
        max_images_to_check: usize,
    ) -> bool {
        for (_i, img) in images.iter().take(max_images_to_check).enumerate() {
            let (w, h) = img.dimensions();
            self.archive_type = if h >= 3 * w {
                ArchiveType::Manhwa
            } else {
                ArchiveType::Manga
            };
            if self.image_meets_criteria(w, h) {
                return true;
            }
        }
        false
    }

    fn image_meets_criteria(&self, w: u32, h: u32) -> bool {
        (h >= 3 * w && h > self.min_image_size.height)
            || (h < 3 * w && w > self.min_image_size.width && h > self.min_image_size.height)
    }

    fn combine_images(&self, images: &[DynamicImage]) -> DynamicImage {
        let total_height: u32 = images.iter().map(|img| img.height()).sum();
        let max_width: u32 = images.iter().map(|img| img.width()).max().unwrap_or(0);
        let mut combined_image = DynamicImage::new_rgba8(max_width, total_height);

        let mut y_offset = 0;
        for img in images {
            let resized_img = img.resize_exact(max_width, img.height(), FilterType::Lanczos3);
            combined_image
                .copy_from(
                    &DynamicImage::ImageRgba8(resized_img.to_rgba8()),
                    0,
                    y_offset,
                )
                .expect("Failed to copy image segment");
            y_offset += img.height();
        }

        combined_image
    }

    fn write_archive(self, images: &[DynamicImage]) -> io::Result<()> {
        match self.archive_type {
            ArchiveType::Manhwa => self.process_manhwa_images(&images),
            ArchiveType::Manga => self.process_non_manhwa_images(&images),
        }
    }

    pub fn encode_webp(&self, image: &DynamicImage) -> Result<Vec<u8>, String> {
        // Convert the image to RGBA format
        let rgba_image = image.to_rgba8();
        let mut webp_data = Vec::new();
        // Create a WebP encoder (support only lossless feature for now)
        let encoder = WebPEncoder::new_lossless(&mut webp_data);
        encoder
            .encode(
                rgba_image.as_raw(),
                rgba_image.width(),
                rgba_image.height(),
                ExtendedColorType::Rgba8, // Image crate support only Rgb8 or Rgba8 data
            )
            .map_err(|e| format!("{e}"))?;

        Ok(webp_data)
    }

    fn crop_image(&self, image: &DynamicImage, y_offset: u32, slice_bottom: u32) -> DynamicImage {
        image.crop_imm(0, y_offset, image.width(), slice_bottom - y_offset)
    }

    fn process_manhwa_images(&self, images: &[DynamicImage]) -> io::Result<()> {
        // Extract file info
        let (name, dir_path) = extract_file_info(&self.archive_path)?;

        // Create a temporary zip file
        let temp_archive_path = dir_path.join(format!("{}.temp.cbz", name));
        let mut new_zip = ZipWriter::new(File::create(&temp_archive_path)?);

        // Combine images
        let combined_image = self.combine_images(&images);

        let slice_height = self.min_image_size.height;
        let total_height = combined_image.height();
        let num_slices = (total_height + slice_height - 1) / slice_height;

        for slice_index in 0..num_slices {
            let slice_bottom = ((slice_index + 1) * slice_height).min(total_height);
            let cropped_image =
                self.crop_image(&combined_image, slice_index * slice_height, slice_bottom);
            let image_data = self
                .encode_webp(&cropped_image)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            new_zip.start_file(
                format!("{}.webp", slice_index + 1),
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )?;
            new_zip.write_all(&image_data)?;
        }

        // TODO: Remove commented code later
        // fs::remove_file(file_path)?;
        fs::rename(temp_archive_path, dir_path.join(format!("{}.cbz", name)))?;

        Ok(())
    }

    fn process_non_manhwa_images(self, images: &[DynamicImage]) -> io::Result<()> {
        let (name, dir_path) = extract_file_info(&self.archive_path)?;
        let temp_archive_path = dir_path.join(format!("{}.temp.cbz", name));
        let new_zip = ZipWriter::new(File::create(&temp_archive_path)?);
        let shared_zip = Arc::new(Mutex::new(new_zip));
        let total_images = images.len();
        let pb = ProgressBar::new(total_images as u64);
        pb.set_style(
            ProgressStyle::with_template("{msg} {wide_bar} {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );
    
        let cleaner_arc = Arc::new(self);
        let images_cloned: Vec<_> = images.to_vec();
        process_images_threaded(cleaner_arc, images_cloned, shared_zip.clone(), pb);
        fs::rename(temp_archive_path, dir_path.join(format!("{}.cbz", name)))?;
        Ok(())
    }

    /// Process a single image, handle progress updates, write to the zip file and handle spinners.
    fn process_image(
        &self,
        index: usize,
        image: &DynamicImage,
        zip_writer: Arc<Mutex<ZipWriter<std::fs::File>>>,
        pb: &ProgressBar,
        spinner: &ProgressBar,
    ) -> Result<(), io::Error> {
        // Lock the Mutex to get access to the ZipWriter
        let mut zip_writer = zip_writer.lock().map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to lock ZipWriter: {}", e),
            )
        })?;

        // Check if image meets criteria
        if !self.image_meets_criteria(image.width(), image.height()) {
            pb.inc(1); // Update overall progress even if skipped
            spinner.finish_with_message("Skipped"); // Mark spinner as skipped
            return Err(io::Error::new(io::ErrorKind::Other, "Processing error"));
        }

        // Resize image
        let resized_image = image.thumbnail(self.min_image_size.width, self.min_image_size.height);

        // Encode image in webp
        let image_data = match self.encode_webp(&resized_image) {
            Ok(data) => data,
            Err(e) => {
                pb.inc(1); // Update overall progress on failure
                spinner.finish_with_message("Error");
                return Err(io::Error::new(io::ErrorKind::Other, e));
            }
        };

        // Create the file name
        let file_name = format!("{}.webp", index + 1);

        // Start new file in the zip archive
        zip_writer.start_file(
            file_name,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )?;

        // Write image data to zip file
        zip_writer.write_all(&image_data)?;

        pb.inc(1); // Update the main progress bar after each image
        spinner.finish_with_message("Done!"); // Mark the spinner as finished
        Ok(())
    }

    fn is_image(&self, file_name: &str) -> bool {
        file_name.ends_with(".webp")
            || file_name.ends_with(".jpg")
            || file_name.ends_with(".jpeg")
            || file_name.ends_with(".png")
    }
}
