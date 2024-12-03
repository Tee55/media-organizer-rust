use rfd::FileDialog;
use reader_rust::archive_cleaner::ArchiveCleaner;

fn main() {
    
    // Open a file dialog to select a ZIP file
    let zip_path = FileDialog::new()
        .add_filter("ZIP Files", &["zip"])  // Filter for .zip files
        .pick_file();  // Open the dialog to pick a file

    match zip_path {
        Some(path) => {
            // If the user selected a file, print the selected file path
            println!("Selected ZIP file: {:?}", path);

            // Create an instance of the ArchiveCleaner struct
            let archive_cleaner = ArchiveCleaner::new(path.as_path());

            // Call the clean_archive_file method to clean the archive file
            archive_cleaner
                .clean_archive_file( 5)
                .expect("Failed to clean archive file");
        }
        None => {
            // If no file was selected, print a message
            println!("No file selected.");
        }
    }
}
