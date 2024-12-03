use rfd::FileDialog;
use reader_rust::file_handler::FileHandler;

fn main() {

    let extensions = FileHandler::get_supported_extensions();
    
    // Open a file dialog to select a ZIP file
    let file_path = FileDialog::new()
        .add_filter("Files", &extensions)  // Filter for .zip files
        .pick_file();  // Open the dialog to pick a file

    match file_path {
        Some(path) => {
            // If the user selected a file, print the selected file path
            println!("Selected file: {:?}", path);

            // Create an instance of the ArchiveCleaner struct
            let file_handler = FileHandler::new(path.as_path());

            // Call the clean_archive_file method to clean the archive file
            match file_handler.clean() {
                Ok(_) => {
                    println!("File cleaned successfully.");
                }
                Err(e) => {
                    println!("Failed to clean file: {}", e);
                }
            }
        }
        None => {
            // If no file was selected, print a message
            println!("No file selected.");
        }
    }
}
